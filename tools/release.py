#!/usr/bin/env python3
"""Versions and releases of publet-cli, recorded by the package publet.

A release of the `pub` command is a version of the package publet
`pkg.publet-cli` in this repository's corpus: its `identity` claim states
the version, tag, and commit, and its `release` claim lists what changed,
by category. The publet's lineage is the release history; CHANGELOG.md is
generated from it. Changes not yet released accumulate in
`corpus/publets/pkg.publet-cli/unreleased.json` -- workshop data, never
published, like a draft.

The version follows from the categories of what changed since the last
release, never from judgement at release time:

    changed, removed   a published interface changed   major
    added              a feature was added             minor
    fixed, security    the same functionality          patch

Usage:
    release.py selftest          check the arithmetic and parsing
    release.py check BASE        CI: a code change records what it changes;
                                 a version change is the bump it must be
    release.py next              the version the unreleased changes imply
    release.py prepare [--no-pr] cut a release: publish the package publet
                                 locally, commit, and open a release PR
    release.py changelog         regenerate CHANGELOG.md from the publet
    release.py tag               CI on main: tag and release what was published
"""

from __future__ import annotations

import datetime
import json
import os
import pathlib
import re
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parents[1]
CORPUS = ROOT / "corpus"
PKG = "pkg.publet-cli"
PKG_TAG = "PKG-PUBCLI-10-2026"
PKG_DIR = CORPUS / "publets" / PKG
UNRELEASED = PKG_DIR / "unreleased.json"
REPO = "https://github.com/schryer/publet"

BUMPS = {"changed": 0, "removed": 0, "added": 1, "fixed": 2, "security": 2}
# Paths whose change is a change to what users run.
CODE = ("crates/", "bin/", "porcelain/", "Cargo.toml", "Cargo.lock")
# Before any release, the first one is the baseline.
UNPUBLISHED = "0.0.1"
BASELINE = "0.1.0"


def fail(message: str) -> None:
    print(message, file=sys.stderr)
    sys.exit(1)


# --- versions -----------------------------------------------------------------


def parse(version: str) -> tuple[int, int, int]:
    parts = version.split(".")
    if len(parts) != 3 or not all(p.isdigit() for p in parts):
        raise ValueError(f"{version!r} is not MAJOR.MINOR.PATCH")
    return (int(parts[0]), int(parts[1]), int(parts[2]))


def bump(previous: str, categories: list[str]) -> str:
    """The version after `previous`, given what changed."""
    unknown = [c for c in categories if c not in BUMPS]
    if unknown:
        raise ValueError(f"unknown change categories: {', '.join(unknown)} "
                         f"(known: {', '.join(BUMPS)})")
    if not categories:
        raise ValueError("nothing changed, so there is nothing to release")
    if previous == UNPUBLISHED:
        return BASELINE
    major, minor, patch = parse(previous)
    level = min(BUMPS[c] for c in categories)
    if level == 0:
        return f"{major + 1}.0.0"
    if level == 1:
        return f"{major}.{minor + 1}.0"
    return f"{major}.{minor}.{patch + 1}"


def workspace_version(cargo_toml: str) -> str:
    section = cargo_toml.split("[workspace.package]", 1)[-1]
    match = re.search(r'^version\s*=\s*"([^"]+)"', section, re.M)
    if not match:
        raise ValueError("no [workspace.package] version")
    return match.group(1)


def set_workspace_version(cargo_toml: str, version: str) -> str:
    head, _, rest = cargo_toml.partition("[workspace.package]")
    rest = re.sub(r'^version\s*=\s*"[^"]+"', f'version = "{version}"', rest, count=1, flags=re.M)
    return head + "[workspace.package]" + rest


# --- the package publet's sources -------------------------------------------


def changes(text: str | None) -> list[dict]:
    """The rows of an unreleased.json, validated."""
    if not text:
        return []
    rows = json.loads(text).get("changes", [])
    for row in rows:
        if row.get("category") not in BUMPS or not row.get("change"):
            raise ValueError(f"each change needs a known category and a description: {row}")
    return rows


def write_unreleased(rows: list[dict]) -> None:
    UNRELEASED.parent.mkdir(parents=True, exist_ok=True)
    UNRELEASED.write_text(json.dumps({
        "comment": "Changes to publet-cli not yet released. Add one row per change a PR "
                   "makes: category is changed or removed (a published interface "
                   "changed: major), added (a feature: minor), or fixed or security "
                   "(patch). `make release-pr` moves them into the package publet.",
        "changes": rows,
    }, indent=2) + "\n")


def package_source(version: str, date: str, commit: str, rows: list[dict]) -> dict:
    """The package publet's source for a release."""
    created = f"{date}T00:00:00Z"
    tag = f"v{version}"
    source = {
        "slug": PKG, "tag": PKG_TAG, "created": created,
        "title": "publet-cli, the pub command",
        "claims": {
            "identity": {
                "class": "archival",
                "scope": f"the published release {tag} of publet-cli",
                "content": f"publet-cli {version} is the pub command released as "
                           f"{tag} from commit {commit}.",
                "data": {"columns": [{"name": n} for n in
                                     ["name", "version", "check", "expect", "tag", "commit"]],
                         "rows": [["pub", version, "pub --version", f"pub {version}", tag, commit]]},
                "sources": [{"ref": REPO, "revision": commit}],
            },
            "release": {
                "class": "archival",
                "scope": f"the published release {tag} of publet-cli",
                "content": f"publet-cli {version} was released on {date} from commit "
                           f"{commit} with these changes.",
                "depends": ["#identity"],
                "data": {"columns": [{"name": "category"}, {"name": "change"}],
                         "rows": [[r["category"], r["change"]] for r in rows]},
                "sources": [{"ref": REPO, "revision": commit}],
            },
        },
        "sections": [
            {"heading": "Identity", "items": [{"ref": "#identity"}]},
            {"heading": "Release", "items": [{"ref": "#release"}]},
            {"heading": "Help", "items": [{"ref": "base:xdoc.publet-readme", "role": "background"}]},
        ],
    }
    return source


# --- git and pub ------------------------------------------------------------


def git(*args: str, check: bool = True) -> str:
    proc = subprocess.run(["git", *args], cwd=ROOT, capture_output=True, text=True)
    if check and proc.returncode != 0:
        fail(f"git {' '.join(args)}: {proc.stderr.strip()}")
    return proc.stdout.strip()


def at(ref: str, path: str) -> str | None:
    proc = subprocess.run(["git", "show", f"{ref}:{path}"], cwd=ROOT,
                          capture_output=True, text=True)
    return proc.stdout if proc.returncode == 0 else None


def pub_bin() -> pathlib.Path:
    base = os.environ.get("PUBLET_BIN_DIR")
    path = pathlib.Path(base) / "pub" if base else ROOT / "target" / "debug" / "pub"
    if not path.exists():
        fail(f"no pub at {path}; build it first (cargo build -p publet-cli)")
    return path


def read_object(cid: str) -> dict:
    """A published object of the corpus, decoded by `pub read --json`."""
    path = CORPUS / "objects" / (cid.replace(":", "_") + ".cbor")
    proc = subprocess.run([str(pub_bin()), "read", "--json", str(path)],
                          capture_output=True, text=True)
    if proc.returncode != 0:
        fail(f"cannot read {cid}: {proc.stderr.strip()}")
    return json.loads(proc.stdout)["object"]


def published(slug: str) -> str | None:
    lock = CORPUS / "corpus.lock"
    if not lock.exists():
        return None
    rows = json.loads(lock.read_text()).get("publets", [])
    return next((r["cid"] for r in rows if r["slug"] == slug), None)


def published_version() -> str | None:
    """The version the published package identity states, if any."""
    cid = published(f"{PKG}#identity")
    if cid is None:
        return None
    body = read_object(cid)["body"]
    return body["data"]["rows"][0][1]


# --- commands ---------------------------------------------------------------


def cmd_check(base: str) -> None:
    changed = [p for p in git("diff", "--name-only", f"{base}...HEAD").splitlines() if p]
    code = [p for p in changed if p.startswith(CODE)]
    rel = str(UNRELEASED.relative_to(ROOT))
    before = changes(at(base, rel))
    after = changes(at("HEAD", rel))
    added = [r for r in after if r not in before]

    old = workspace_version(at(base, "Cargo.toml") or "")
    new = workspace_version((ROOT / "Cargo.toml").read_text())
    if new != old:
        source = json.loads((PKG_DIR / "publet.json").read_text())
        rows = source["claims"]["release"]["data"]["rows"]
        expected = bump(old, [r[0] for r in rows])
        if new != expected:
            fail(f"the version moved {old} -> {new}, but the release's changes "
                 f"({', '.join(sorted({r[0] for r in rows}))}) make it {expected}")
        stated = source["claims"]["identity"]["data"]["rows"][0][1]
        if stated != new:
            fail(f"the package publet's identity states {stated}, not {new}")
        if published_version() != new:
            fail(f"{new} is not published: corpus.lock's {PKG}#identity does not state it; "
                 f"`make release-pr` publishes it")
        if after:
            fail("a release moves every unreleased change into the package publet; "
                 f"{rel} still lists some")
        print(f"release {old} -> {new}: the bump its changes require, and published")
        return
    if code and not added:
        fail("this change touches " + ", ".join(sorted({c.split('/')[0] for c in code})) +
             f" but records no change in {rel}. Add a row: category changed or removed "
             "(a published interface changed), added (a feature), or fixed / security.")
    print(f"ok: {len(added)} change(s) recorded" if code else "ok: no code changed")


def cmd_next() -> None:
    version = workspace_version((ROOT / "Cargo.toml").read_text())
    print(bump(version, [r["category"] for r in changes(
        UNRELEASED.read_text() if UNRELEASED.exists() else None)]))


def cmd_prepare(open_pr: bool) -> None:
    if git("status", "--porcelain"):
        fail("the tree is not clean; a release is cut from a clean main")
    git("fetch", "origin", "main")
    if git("rev-parse", "HEAD") != git("rev-parse", "origin/main"):
        fail("HEAD is not origin/main; a release is cut from the current main")
    rows = changes(UNRELEASED.read_text() if UNRELEASED.exists() else None)
    cargo = ROOT / "Cargo.toml"
    old = workspace_version(cargo.read_text())
    new = bump(old, [r["category"] for r in rows])
    commit = git("rev-parse", "HEAD")
    date = datetime.date.today().isoformat()
    print(f"releasing publet-cli {old} -> {new} from {commit[:12]}")

    git("checkout", "-b", f"release/v{new}")
    source_path = PKG_DIR / "publet.json"
    source_path.write_text(json.dumps(package_source(new, date, commit, rows), indent=2) + "\n")
    write_unreleased([])
    cargo.write_text(set_workspace_version(cargo.read_text(), new))
    subprocess.run(["cargo", "update", "--workspace", "--offline"], cwd=ROOT, check=True)
    subprocess.run(["cargo", "build", "-p", "publet-cli"], cwd=ROOT, check=True)
    pub = str(pub_bin())
    subprocess.run([pub, "build"], cwd=CORPUS, check=True)
    subprocess.run([pub, "publish", PKG], cwd=CORPUS, check=True)
    cmd_changelog()

    git("add", "-A")
    git("commit", "-m", f"Release v{new}\n\nPublishes {PKG} with publet-cli {new}.\n\n"
                        "Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>")
    if not open_pr:
        print(f"committed on release/v{new}; push it and open a PR to release")
        return
    git("push", "-u", "origin", f"release/v{new}")
    notes = release_notes(new)
    subprocess.run(["gh", "pr", "create", "--base", "main", "--title", f"Release v{new}",
                    "--body", notes + "\n\n🤖 Generated with [Claude Code](https://claude.com/claude-code)"],
                   cwd=ROOT, check=True)


def releases() -> list[dict]:
    """Every published release of the package, newest first, read back
    from the corpus's objects -- the package publet is the record."""
    out = []
    for path in sorted((CORPUS / "objects").glob("*.cbor")):
        proc = subprocess.run([str(pub_bin()), "read", "--json", str(path)],
                              capture_output=True, text=True)
        if proc.returncode != 0:
            continue
        doc = json.loads(proc.stdout)
        body = doc["object"].get("body", {})
        match = re.match(r"publet-cli (\S+) was released on (\S+) from commit (\w+) ",
                         body.get("content", ""))
        if doc["object"].get("type") != "claim.prose" or not match:
            continue
        out.append({"version": match.group(1), "date": match.group(2),
                    "commit": match.group(3), "cid": doc["cid"],
                    "rows": body.get("data", {}).get("rows", [])})
    return sorted(out, key=lambda r: parse(r["version"]), reverse=True)


def section(release: dict) -> str:
    lines = [f"## {release['version']} - {release['date']}", "",
             f"Commit `{release['commit'][:12]}`; release record `{release['cid']}`.", ""]
    for category in BUMPS:
        entries = [r[1] for r in release["rows"] if r[0] == category]
        if entries:
            lines.append(f"### {category.capitalize()}")
            lines.extend(f"- {e}" for e in entries)
            lines.append("")
    return "\n".join(lines)


def release_notes(version: str) -> str:
    match = next((r for r in releases() if r["version"] == version), None)
    if match is None:
        fail(f"no published release record states {version}")
    return section(match)


def cmd_changelog() -> None:
    body = "\n".join(section(r) for r in releases())
    (ROOT / "CHANGELOG.md").write_text(
        "# Changelog\n\n"
        "Generated by `tools/release.py changelog` from the package publet "
        f"`{PKG}` [{PKG_TAG}] in `corpus/`. Do not edit: each release is a "
        "published version of that publet, and this file is a view of it.\n\n" + body)
    print("CHANGELOG.md written")


def cmd_tag() -> None:
    version = workspace_version((ROOT / "Cargo.toml").read_text())
    tag = f"v{version}"
    if git("tag", "-l", tag):
        print(f"{tag} already exists; nothing to do")
        return
    if version == UNPUBLISHED:
        print("no release has been cut yet; nothing to tag")
        return
    if published_version() != version:
        fail(f"{version} is in Cargo.toml but corpus.lock's {PKG}#identity does not state it")
    identity = published(f"{PKG}#identity")
    record = published(f"{PKG}#release")
    git("tag", "-a", tag, "-m",
        f"publet-cli {version}\n\npackage {PKG} [{PKG_TAG}]\n"
        f"identity {identity}\nrelease  {record}")
    git("push", "origin", tag)
    notes = release_notes(version) + f"\n\nInstall:\n\n```\ncargo install --locked --git {REPO} --tag {tag} publet-cli\n```\n"
    subprocess.run(["gh", "release", "create", tag, "--title", f"publet-cli {version}",
                    "--notes", notes], cwd=ROOT, check=True)
    print(f"tagged and released {tag}")


def cmd_selftest() -> None:
    assert bump("0.0.1", ["added"]) == "0.1.0"
    assert bump("0.1.0", ["added"]) == "0.2.0"
    assert bump("0.1.0", ["fixed", "added"]) == "0.2.0"
    assert bump("0.1.0", ["changed"]) == "1.0.0"
    assert bump("0.1.0", ["fixed", "removed"]) == "1.0.0"
    assert bump("0.1.0", ["fixed"]) == "0.1.1"
    assert bump("1.4.2", ["security"]) == "1.4.3"
    for bad in (["oops"], []):
        try:
            bump("0.1.0", bad)
        except ValueError:
            pass
        else:
            raise AssertionError(f"bump accepted {bad}")
    toml = '[workspace]\nmembers = []\n\n[workspace.package]\nversion = "0.0.1"\nedition = "2024"\n'
    assert workspace_version(toml) == "0.0.1"
    assert workspace_version(set_workspace_version(toml, "0.1.0")) == "0.1.0"
    assert changes('{"changes": [{"category": "added", "change": "x"}]}')[0]["category"] == "added"
    try:
        changes('{"changes": [{"category": "nope", "change": "x"}]}')
    except ValueError:
        pass
    else:
        raise AssertionError("an unknown category was accepted")
    print("release.py selftest: ok")


def main() -> None:
    args = sys.argv[1:]
    if not args:
        fail(__doc__)
    command, rest = args[0], args[1:]
    if command == "selftest":
        cmd_selftest()
    elif command == "check" and len(rest) == 1:
        cmd_check(rest[0])
    elif command == "next":
        cmd_next()
    elif command == "prepare":
        cmd_prepare(open_pr="--no-pr" not in rest)
    elif command == "changelog":
        cmd_changelog()
    elif command == "tag":
        cmd_tag()
    else:
        fail(__doc__)


if __name__ == "__main__":
    main()
