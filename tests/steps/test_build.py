"""Steps for `pub build`: named publet sources compiled into objects.

The scenarios write source trees the way an author would, run the built
`pub` against them, and inspect only what any implementation exposes: the
lock it writes, what it prints, and the objects its store hands back.
"""

from __future__ import annotations

import json
from pathlib import Path

from pytest_bdd import given, parsers, scenarios, then, when

scenarios("../features/porcelain/build.feature")

TOOL = {
    "slug": "tool.demo",
    "tag": "TOOL-DEMOTL-10-2026",
    "title": "Demo tool",
    "claims": {
        "identity": {
            "class": "archival",
            "content": "demo-tool 1.0.0 is distributed at https://example.org/demo-tool.",
            "scope": "the demo-tool 1.0.0 release",
            "data": {
                "columns": [{"name": "name"}, {"name": "version"}],
                "rows": [["demo-tool", "1.0.0"]],
            },
        }
    },
    "sections": [{"heading": "Identity", "items": [{"ref": "#identity"}]}],
}

DOC = {
    "slug": "demo-doc",
    "tag": "PUB-DEMODC-10-2026",
    "title": "Demo document",
    "sections": [
        {"heading": "Tooling", "items": [
            {"ref": "tool.demo#identity", "role": "background"},
            {"ref": "tool.demo", "role": "background", "bind": "lineage"},
        ]},
    ],
}


def _write_tree(root: Path, tool: dict, doc: dict) -> None:
    for source in (tool, doc):
        path = root / "publets" / source["slug"] / "publet.json"
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(json.dumps(source, indent=2))


def _held(runner) -> set[str]:
    out = runner.run("pub-store", "--store=.publet/objects.redb", "list")
    assert out.code == 0, out.stderr
    return set(out.stdout.split())


def _bytes(runner, cid: str) -> bytes:
    out = runner.run("pub-cat", "--store=.publet/objects.redb", cid)
    assert out.code == 0, out.stderr
    return out.out


def _lock(runner) -> dict[str, dict]:
    data = json.loads((runner.cwd / "publets" / "publets.lock").read_text())
    return {row["slug"]: row for row in data["publets"]}


def _build(runner, ctx):
    ctx.setdefault("before", _held(runner))
    return runner.run("pub", "build", "publets")


def _tags_of(runner, cid: str) -> list[str]:
    """Tags on `cid`, read with `pub-ann` over an export of the store."""
    export = runner.cwd / "export"
    export.mkdir(exist_ok=True)
    for held in _held(runner):
        (export / (held.replace(":", "_") + ".cbor")).write_bytes(_bytes(runner, held))
    out = runner.run("pub-ann", f"--dir={export}", f"--tagged-of={cid}")
    assert out.code == 0, out.stderr
    return [line.strip() for line in out.stdout.splitlines() if line.strip()]


# --- givens ------------------------------------------------------------------


@given("a workspace", target_fixture="ctx")
def workspace(runner):
    assert runner.run("pub", "init").code == 0
    return {}


@given("a source tree with a tool publet and a document citing it by name")
def source_tree(runner, ctx):
    ctx["tool"] = json.loads(json.dumps(TOOL))
    ctx["doc"] = json.loads(json.dumps(DOC))
    _write_tree(runner.cwd, ctx["tool"], ctx["doc"])


# --- whens -------------------------------------------------------------------


@when("I build the source tree", target_fixture="result")
def build(runner, ctx):
    result = _build(runner, ctx)
    assert result.code == 0, result.stderr
    ctx["first_lock"] = (runner.cwd / "publets" / "publets.lock").read_bytes()
    ctx["after_first"] = _held(runner)
    return result


@when("I build the source tree again", target_fixture="result")
def build_again(runner):
    return runner.run("pub", "build", "publets")


@when("I change the tool claim's content and build again", target_fixture="result")
def change_tool(runner, ctx):
    ctx["tool"]["claims"]["identity"]["content"] = (
        "demo-tool 1.0.1 is distributed at https://example.org/demo-tool."
    )
    _write_tree(runner.cwd, ctx["tool"], ctx["doc"])
    return runner.run("pub", "build", "publets")


@when(parsers.parse('I change the document\'s tag to "{tag}" and build again'),
      target_fixture="result")
@when(parsers.parse('I change the document\'s tag to "{tag}" and build'),
      target_fixture="result")
def change_tag(runner, ctx, tag: str):
    ctx["doc"]["tag"] = tag
    _write_tree(runner.cwd, ctx["tool"], ctx["doc"])
    return _build(runner, ctx)


@when(parsers.parse('I add a reference to "{slug}" and build'), target_fixture="result")
def add_dangling(runner, ctx, slug: str):
    ctx["doc"]["sections"][0]["items"].append({"ref": slug})
    _write_tree(runner.cwd, ctx["tool"], ctx["doc"])
    return _build(runner, ctx)


@when("I make the tool cite the document and build", target_fixture="result")
def make_cycle(runner, ctx):
    ctx["tool"]["sections"][0]["items"].append({"ref": "demo-doc"})
    _write_tree(runner.cwd, ctx["tool"], ctx["doc"])
    return _build(runner, ctx)


# --- thens -------------------------------------------------------------------


@then("the lock names every slug in the tree")
def lock_names_all(runner):
    assert set(_lock(runner)) == {"tool.demo", "tool.demo#identity", "demo-doc"}


@then("the document cites the tool's claim by the identifier the lock records")
def cites_by_identifier(runner):
    lock = _lock(runner)
    doc = _bytes(runner, lock["demo-doc"]["cid"])
    assert lock["tool.demo#identity"]["cid"].encode() in doc
    # The lineage-bound item cites the genesis, `at` the version read.
    assert lock["tool.demo"]["genesis"].encode() in doc


@then("no object holds a slug")
def no_slug_in_objects(runner):
    lock = _lock(runner)
    for row in lock.values():
        data = _bytes(runner, row["cid"])
        for slug in ("tool.demo", "demo-doc", "#identity"):
            assert slug.encode() not in data, f"{row['slug']} holds {slug!r}"


@then("the tool's lineage carries its tag")
def tool_tagged(runner):
    lock = _lock(runner)
    assert "TOOL-DEMOTL-10-2026" in _tags_of(runner, lock["tool.demo"]["genesis"])


@then(parsers.parse('it reports "{text}"'))
def reports(result, text: str):
    assert text in result.stdout, result.stdout


@then("the lock is byte-identical to the first build's")
def lock_identical(runner, ctx):
    assert (runner.cwd / "publets" / "publets.lock").read_bytes() == ctx["first_lock"]


@then(parsers.parse('the tool claim is reported "{status}"'))
def tool_claim_status(result, status: str):
    assert any(line.startswith(status) and "tool.demo#identity" in line
               for line in result.stdout.splitlines()), result.stdout


@then(parsers.parse('the document is reported "{status}"'))
def doc_status(result, status: str):
    assert any(line.startswith(status) and line.split()[1] == "demo-doc"
               for line in result.stdout.splitlines()), result.stdout


@then("the document's tag is not filed again")
def tag_not_refiled(result):
    assert not any(line.startswith("tagged") and "demo-doc" in line
                   for line in result.stdout.splitlines()), result.stdout


@then(parsers.parse('the document\'s lineage carries only the tag "{tag}"'))
def only_tag(runner, tag: str):
    lock = _lock(runner)
    assert _tags_of(runner, lock["demo-doc"]["genesis"]) == [tag]


@then("nothing new is stored")
def nothing_stored(runner, ctx):
    assert _held(runner) == ctx["before"], "a refused build stored objects anyway"
