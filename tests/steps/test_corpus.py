"""Steps for corpora: `pub corpus`, `pub build` in a corpus, `pub delegate`.

Two corpora are made per scenario in sibling directories, each with its own
workspace and key, as a parent corpus and a project corpus would have.
"""

from __future__ import annotations

import json
import subprocess
from pathlib import Path

from pytest_bdd import given, parsers, scenarios, then, when

scenarios("../features/porcelain/corpus.feature")


class Corpora:
    def __init__(self, runner):
        self.runner = runner
        self.parent = runner.cwd / "base"
        self.child = runner.cwd / "proj"

    def pub(self, where: Path, *args: str):
        return subprocess.run([str(self.runner.path_to("pub")), *args],
                              capture_output=True, text=True, cwd=where, check=False)

    def ok(self, where: Path, *args: str):
        proc = self.pub(where, *args)
        assert proc.returncode == 0, proc.stdout + proc.stderr
        return proc

    def publish(self, where: Path, *names: str):
        built = self.pub(where, "build")
        if built.returncode != 0:
            return built
        return self.pub(where, "publish", *(names or ("--all",)))

    def published(self, where: Path, *names: str):
        proc = self.publish(where, *names)
        assert proc.returncode == 0, proc.stdout + proc.stderr
        return proc

    def keyed(self, where: Path, label: str):
        out = self.ok(where, "sign", "--generate-key", "--principal=human", f"--label={label}")
        key = out.stdout.split()[1]
        self.ok(where, "policy", f"--root={key}")
        return key

    def source(self, where: Path, slug: str, body: dict):
        path = where / "publets" / slug / "publet.json"
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(json.dumps({"slug": slug, **body}, indent=2))

    def lock(self, where: Path) -> dict:
        return json.loads((where / "corpus.lock").read_text())

    def cid(self, where: Path, slug: str) -> str:
        return next(r["cid"] for r in self.lock(where)["publets"] if r["slug"] == slug)


def tool(version: str, slug: str = "tool.x", tag: str = "TOOL-TOOLXX-10-2026") -> dict:
    return {
        "tag": tag, "title": f"Tool {slug}",
        "claims": {"identity": {
            "class": "archival", "scope": "the test",
            "content": f"{slug} {version} is the tool.",
            "data": {"columns": [{"name": "name"}, {"name": "version"}],
                     "rows": [[slug, version]]},
            "sources": ["https://example.org/x"]}},
        "sections": [{"heading": "Identity", "items": [{"ref": "#identity"}]}],
    }


def doc(*refs: str) -> dict:
    return {"tag": "PUB-DOCDOC-10-2026", "title": "A project doc",
            "sections": [{"heading": "Uses", "items": [
                {"ref": r, "role": "background"} for r in refs]}]}


# --- givens ------------------------------------------------------------------


@given(parsers.parse('a parent corpus "{name}" declaring the publet "{slug}" at version "{version}"'),
       target_fixture="corpora")
def parent_corpus(runner, name: str, slug: str, version: str):
    c = Corpora(runner)
    c.parent.mkdir()
    c.ok(c.parent, "corpus", "init", f"--name={name}")
    c.parent_key = c.keyed(c.parent, "Parent key")
    c.source(c.parent, slug, tool(version, slug))
    c.published(c.parent)
    return c


@given(parsers.parse('a child corpus "{name}" whose publet cites "{ref}"'))
def child_corpus(corpora, name: str, ref: str):
    c = corpora
    c.child.mkdir()
    c.ok(c.child, "corpus", "init", f"--name={name}", "--parent=base=../base")
    c.child_key = c.keyed(c.child, "Child key")
    c.refs = [ref]
    c.source(c.child, "doc", doc(*c.refs))


# --- whens -------------------------------------------------------------------


@when("I publish the child", target_fixture="result")
@when("I publish the child again", target_fixture="result")
def publish_child(corpora):
    return corpora.publish(corpora.child)


@when(parsers.parse('the child\'s publet cites "{ref}" and I build the child'),
      target_fixture="result")
def cite_bare(corpora, ref: str):
    corpora.source(corpora.child, "doc", doc(ref))
    return corpora.pub(corpora.child, "build")


@when(parsers.parse('the parent\'s "{slug}" becomes version "{version}" and the parent is rebuilt'))
def parent_advances(corpora, slug: str, version: str):
    corpora.source(corpora.parent, slug, tool(version, slug))
    corpora.published(corpora.parent)


@when("I ask the child's corpus status", target_fixture="result")
def status(corpora):
    return corpora.ok(corpora.child, "corpus", "status")


@when("I upgrade the child and publish it again", target_fixture="result")
def upgrade_publish(corpora):
    corpora.ok(corpora.child, "corpus", "upgrade")
    return corpora.publish(corpora.child)


@when(parsers.parse('the parent declares a new publet "{slug}" and is rebuilt'))
def parent_adds(corpora, slug: str):
    corpora.source(corpora.parent, slug, tool("1.0", slug, "TOOL-TOOLYY-10-2026"))
    corpora.published(corpora.parent)


@when(parsers.parse('the child\'s publet also cites "{ref}" and I build the child'),
      target_fixture="result")
def cite_newer(corpora, ref: str):
    corpora.source(corpora.child, "doc", doc(*corpora.refs, ref))
    return corpora.pub(corpora.child, "build")


@when("the child supersedes the parent's \"tool.x\" and I dry-run the child",
      target_fixture="result")
def supersede(corpora):
    corpora.source(corpora.child, "takeover", {
        "tag": "TOOL-TOOLXX-10-2026", "title": "Tool X, project edition",
        "supersedes": corpora.cid(corpora.parent, "tool.x"),
        "sections": [{"heading": "Identity",
                      "items": [{"ref": "base:tool.x#identity"}]}]})
    return corpora.ok(corpora.child, "build", "--dry-run")


@when("the parent delegates its lineage to the child's key and is rebuilt")
def delegate(corpora):
    corpora.ok(corpora.parent, "delegate", f"--to={corpora.child_key}")
    corpora.published(corpora.parent)


@when("I upgrade the child and dry-run it", target_fixture="result")
def upgrade_dry(corpora):
    corpora.ok(corpora.child, "corpus", "upgrade")
    return corpora.ok(corpora.child, "build", "--dry-run")


@when("I map the child's corpus as JSON", target_fixture="result")
def map_json(corpora):
    return corpora.ok(corpora.child, "corpus", "map", "--format=json")


# --- thens -------------------------------------------------------------------


@then(parsers.parse('the child\'s objects include the parent\'s "{slug}" claim'))
def copied(corpora, slug: str):
    name = corpora.cid(corpora.parent, slug).replace(":", "_") + ".cbor"
    assert (corpora.child / "objects" / name).exists()


@then("the child reads its publet with the parent directory moved away")
def reads_alone(corpora):
    corpora.parent.rename(corpora.parent.with_name("away"))
    try:
        out = corpora.pub(corpora.child, "read", corpora.cid(corpora.child, "doc"))
        assert out.returncode == 0, out.stderr
        assert "[local]" in out.stdout
    finally:
        corpora.parent.with_name("away").rename(corpora.parent)


@then(parsers.parse('it reports "{text}"'))
def reports(result, text: str):
    assert text in result.stdout, result.stdout + result.stderr


@then("the child's publet is published again")
def child_republished(result):
    assert "published  doc" in result.stdout, result.stdout


@then(parsers.parse('the superseding publet is reported "{status}"'))
def takeover_status(result, status: str):
    assert any(line.startswith(status) and line.split()[1] == "takeover"
               for line in result.stdout.splitlines()), result.stdout


@then(parsers.parse("the child's generation is {index:d}"))
def generation(corpora, index: int):
    assert corpora.lock(corpora.child)["corpus"]["generation"]["index"] == index


@then(parsers.parse('the map holds corpora "{a}" and "{b}"'))
def map_corpora(result, a: str, b: str):
    assert json.loads(result.stdout)["corpora"] == [a, b]


@then(parsers.parse('the map shows "{a}" citing "{b}"'))
def map_cites(result, a: str, b: str):
    cites = json.loads(result.stdout)["cites"]
    assert any(c["from"]["corpus"] == a and c["to"]["corpus"] == b for c in cites), cites


@then(parsers.parse('the map shows an external link to "{url}"'))
def map_external(result, url: str):
    assert url in [e["url"] for e in json.loads(result.stdout)["external"]]


@when("I check the child's publet for rendering", target_fixture="result")
def render_check(corpora):
    return corpora.pub(corpora.child, "render", "doc", "--check")
