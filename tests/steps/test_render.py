"""Steps for `pub render` and `pub myst`.

The pipeline under test runs a stub program written by the scenario, so the
suite needs no real typesetting toolchain: what is under test is the
pre-flight -- signatures, versions, file hashes -- and that only a checked
pipeline runs.
"""

from __future__ import annotations

import json
import os
import stat
import subprocess
from pathlib import Path

from pytest_bdd import given, parsers, scenarios, then, when

scenarios("../features/porcelain/render.feature")

STUB = """#!/bin/sh
case "$1" in
  --version) echo "stubtool {version}" ;;
  write) echo rendered > "$2" ;;
  *) exit 2 ;;
esac
"""

SOURCES = {
    "tool.stub": {
        "slug": "tool.stub", "tag": "TOOL-STUBTL-10-2026", "title": "Stub tool",
        "claims": {
            "identity": {
                "class": "archival", "scope": "the test machine",
                "content": "stubtool 1.0.0 is the program the stub pipeline runs.",
                "data": {"columns": [{"name": "name"}, {"name": "version"},
                                     {"name": "check"}, {"name": "expect"}],
                         "rows": [["stubtool", "1.0.0", "stubtool --version",
                                   "stubtool 1.0.0"]]},
                "sources": ["https://example.org/stubtool"],
            },
            "input": {
                "class": "archival", "scope": "the test repository",
                "content": "files/input.txt is the input the stub pipeline cites.",
                "data": {"media": "text/plain", "file": "../../files/input.txt"},
                "sources": [{"ref": "https://example.org/repo",
                             "locator": "files/input.txt"}],
            },
        },
        "sections": [{"heading": "Identity",
                      "items": [{"ref": "#identity"}, {"ref": "#input"}]}],
    },
    "render.stub": {
        "slug": "render.stub", "tag": "RENDER-STUBPL-10-2026",
        "title": "Stub pipeline",
        "claims": {
            "method": {"class": "procedural", "scope": "the test",
                       "content": "To render with the stub, run its one step.",
                       "depends": ["#write"]},
            "write": {
                "class": "procedural", "scope": "the test",
                "content": "Write the result file with the stub tool.",
                "depends": ["tool.stub#identity", "tool.stub#input"],
                "data": {"columns": [{"name": "program"}, {"name": "args"},
                                     {"name": "cwd"}, {"name": "outputs"}],
                         "rows": [["stubtool", "write {out}/result.txt", "{out}",
                                   "result.txt"]]},
                "sources": ["https://example.org/repo"],
            },
        },
        "sections": [{"heading": "Method", "items": [{"ref": "#method"}]}],
    },
    "demo-doc": {
        "slug": "demo-doc", "tag": "PUB-DEMODC-10-2026", "title": "Demo document",
        "sections": [{"heading": "Colophon", "items": [
            {"ref": "render.stub", "role": "background", "gloss": "rendered by"}]}],
    },
}


def _env(ctx) -> dict:
    return dict(os.environ, PATH=f"{ctx['stub_dir']}:{os.environ['PATH']}")


def _pub(runner, ctx, *args):
    """Run `pub` with the stub tool on PATH."""
    proc = subprocess.run([str(runner.path_to("pub")), *args], capture_output=True,
                          cwd=runner.cwd, env=_env(ctx), text=True, check=False)
    return proc


def _write_stub(ctx, version: str) -> None:
    path = Path(ctx["stub_dir"]) / "stubtool"
    path.write_text(STUB.format(version=version))
    path.chmod(path.stat().st_mode | stat.S_IEXEC)


def _build(runner, ctx, sign: bool):
    args = ["build", "publets"] + (["--sign"] if sign else [])
    proc = _pub(runner, ctx, *args)
    assert proc.returncode == 0, proc.stderr


def _out(runner) -> Path:
    return runner.cwd / "publets" / "demo-doc"


# --- givens ------------------------------------------------------------------


@given("a workspace with a signing key its policy trusts", target_fixture="ctx")
def signed_workspace(runner):
    assert runner.run("pub", "init").code == 0
    out = runner.run("pub", "sign", "--generate-key", "--principal=human",
                     "--label=Test Author")
    assert out.code == 0, out.stderr
    key = out.stdout.split()[1]
    assert runner.run("pub", "policy", f"--root={key}").code == 0
    return {}


@given(parsers.parse('a stub tool reporting version "{version}"'))
def stub_tool(runner, ctx, version: str):
    ctx["stub_dir"] = str(runner.cwd / "stub-bin")
    Path(ctx["stub_dir"]).mkdir()
    _write_stub(ctx, version)


@given("a publet whose Colophon names a pipeline running the stub tool")
def stub_sources(runner):
    (runner.cwd / "files").mkdir()
    (runner.cwd / "files" / "input.txt").write_text("input\n")
    for slug, source in SOURCES.items():
        path = runner.cwd / "publets" / slug / "publet.json"
        path.parent.mkdir(parents=True)
        path.write_text(json.dumps(source, indent=2))


# --- whens -------------------------------------------------------------------


@when("I build the sources signed and render the publet", target_fixture="result")
def build_signed_render(runner, ctx):
    _build(runner, ctx, sign=True)
    return _pub(runner, ctx, "render", "demo-doc")


@when("I build the sources unsigned and render the publet", target_fixture="result")
def build_unsigned_render(runner, ctx):
    _build(runner, ctx, sign=False)
    return _pub(runner, ctx, "render", "demo-doc")


@when(parsers.parse('the stub tool reports version "{version}" instead'))
def stub_changes(ctx, version: str):
    _write_stub(ctx, version)


@when("I build the sources signed")
def build_signed(runner, ctx):
    _build(runner, ctx, sign=True)


@when("the repository file the pipeline cites is edited")
def edit_repo_file(runner):
    (runner.cwd / "files" / "input.txt").write_text("edited\n")


@when("I render the publet", target_fixture="result")
def render(runner, ctx):
    return _pub(runner, ctx, "render", "demo-doc")


@when("I build the sources signed and write the publet's MyST project",
      target_fixture="result")
def build_and_myst(runner, ctx):
    _build(runner, ctx, sign=True)
    return _pub(runner, ctx, "myst", "demo-doc", f"--out={runner.cwd / 'myst'}",
                "--template=templates/none")


# --- thens -------------------------------------------------------------------


@then("the step's declared output exists")
def output_exists(runner):
    assert (_out(runner) / "result.txt").read_text() == "rendered\n"


@then("the step's declared output does not exist")
def output_absent(runner):
    assert not (_out(runner) / "result.txt").exists()


@then("the render record names the pipeline and the observed tool version")
def record_names(runner):
    lock = {r["slug"]: r for r in json.loads(
        (runner.cwd / "publets" / "publets.lock").read_text())["publets"]}
    record = json.loads((_out(runner) / "render.json").read_text())
    assert record["pipeline"] == lock["render.stub"]["cid"]
    assert record["doc"] == lock["demo-doc"]["cid"]
    assert {"name": "stubtool", "observed": "stubtool 1.0.0"} in record["tools"]


@then("the MyST page shows the pipeline as its title, tag, and identifier")
def myst_shows_pipeline(runner):
    lock = {r["slug"]: r for r in json.loads(
        (runner.cwd / "publets" / "publets.lock").read_text())["publets"]}
    page = (runner.cwd / "myst" / "index.md").read_text()
    assert "**Stub pipeline** [RENDER-STUBPL-10-2026]" in page, page
    assert lock["render.stub"]["cid"] in page, page


@then("the MyST page lists the stub tool's version under Components")
def myst_lists_components(runner):
    page = (runner.cwd / "myst" / "index.md").read_text()
    components = page.split("Components:", 1)[1]
    assert "stubtool 1.0.0" in components, page
    assert "**Stub tool** [TOOL-STUBTL-10-2026]" in components, page
