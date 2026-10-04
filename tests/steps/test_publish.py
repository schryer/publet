"""Steps for building, rendering, and publishing in a corpus.

One corpus per scenario, holding a stub tool whose output names the
document it rendered, a pipeline that runs it under a version constraint,
and a document whose Colophon cites the pipeline's lineage.
"""

from __future__ import annotations

import json
import os
import stat
import subprocess
from pathlib import Path

from pytest_bdd import given, parsers, scenarios, then, when

scenarios("../features/porcelain/publish.feature")

STUB = """#!/bin/sh
case "$1" in
  --version) echo "stubtool {version}" ;;
  write) echo "rendered $3" > "$2" ;;
  *) exit 2 ;;
esac
"""


def identity(version: str) -> dict:
    return {
        "class": "archival", "scope": "the test machine",
        "content": f"stubtool {version} is the program the stub pipeline runs.",
        "data": {"columns": [{"name": "name"}, {"name": "version"},
                             {"name": "check"}, {"name": "expect"}],
                 "rows": [["stubtool", version, "stubtool --version",
                           f"stubtool {version}"]]},
        "sources": ["https://example.org/stubtool"],
    }


def sources(version: str, title: str = "Demo document", other: str = "Other",
            method: str = "To render with the stub, run its one step.") -> dict:
    return {
        "tool.stub": {
            "tag": "TOOL-STUBTL-10-2026", "title": "Stub tool",
            "claims": {"identity": identity(version)},
            "sections": [{"heading": "Identity", "items": [{"ref": "#identity"}]}],
        },
        "render.stub": {
            "tag": "RENDER-STUBPL-10-2026", "title": "Stub pipeline",
            "claims": {
                "method": {"class": "procedural", "scope": "the test",
                           "content": method,
                           "depends": ["#write"]},
                "write": {
                    "class": "procedural", "scope": "the test",
                    "content": "Write the result file with the stub tool.",
                    "depends": ["~tool.stub#identity"],
                    "data": {"columns": [{"name": "program"}, {"name": "requires"},
                                         {"name": "args"}, {"name": "cwd"},
                                         {"name": "outputs"}],
                             "rows": [["stubtool", "1.0",
                                       "write {out}/result.txt {doc}", "{out}",
                                       "result.txt"]]},
                    "sources": ["https://example.org/repo"],
                },
            },
            "sections": [{"heading": "Method", "items": [{"ref": "#method"}]}],
        },
        "demo-doc": {
            "tag": "PUB-DEMODC-10-2026", "title": title,
            "sections": [{"heading": "Colophon", "items": [
                {"ref": "render.stub", "role": "background", "bind": "lineage",
                 "gloss": "rendered by"}]}],
        },
        "other": {
            "tag": "PUB-OTHERS-10-2026", "title": other,
            "sections": [{"heading": "See", "items": [
                {"ref": "tool.stub", "role": "background"}]}],
        },
    }


class Corpus:
    def __init__(self, runner):
        self.runner = runner
        self.root = runner.cwd / "corpus"
        self.bin = runner.cwd / "stub-bin"
        self.version = "1.0.0"
        self.claimed = "1.0.0"
        self.title = "Demo document"
        self.other = "Other"
        self.method = "To render with the stub, run its one step."

    def pub(self, *args: str):
        env = dict(os.environ, PATH=f"{self.bin}:{os.environ['PATH']}")
        return subprocess.run([str(self.runner.path_to("pub")), *args], capture_output=True,
                              text=True, cwd=self.root, env=env, check=False)

    def ok(self, *args: str):
        proc = self.pub(*args)
        assert proc.returncode == 0, proc.stdout + proc.stderr
        return proc

    def stub(self, version: str):
        self.version = version
        path = self.bin / "stubtool"
        path.write_text(STUB.format(version=version))
        path.chmod(path.stat().st_mode | stat.S_IEXEC)

    def write(self):
        for slug, body in sources(self.claimed, self.title, self.other, self.method).items():
            path = self.root / "publets" / slug / "publet.json"
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(json.dumps({"slug": slug, **body}, indent=2))

    def lock(self) -> dict:
        return json.loads((self.root / "corpus.lock").read_text())

    def cid(self, slug: str) -> str:
        return next(r["cid"] for r in self.lock()["publets"] if r["slug"] == slug)

    def objects(self) -> set[str]:
        return {p.name for p in (self.root / "objects").glob("*.cbor")}

    def generation(self) -> int:
        return self.lock()["corpus"]["generation"]["index"]

    def drafts(self) -> dict:
        path = self.root / ".publet" / "drafts.json"
        rows = json.loads(path.read_text())["publets"] if path.exists() else []
        return {r["slug"]: r for r in rows}

    def read(self, cid: str) -> dict:
        out = self.ok("read", "--json", cid)
        return json.loads(out.stdout)["object"]

    def renderings(self) -> str:
        return self.ok("corpus", "renderings").stdout


# --- givens ------------------------------------------------------------------


@given(parsers.parse('a corpus with a stub tool at "{version}", a pipeline requiring '
                     '"{requires}", and a document rendered by it'),
       target_fixture="corpus")
def corpus_with_pipeline(runner, version: str, requires: str):
    c = Corpus(runner)
    c.root.mkdir()
    c.bin.mkdir()
    c.stub(version)
    c.ok("corpus", "init", "--name=test")
    key = c.ok("sign", "--generate-key", "--principal=human",
               "--label=Test Author").stdout.split()[1]
    c.ok("policy", f"--root={key}")
    c.write()
    return c


@given("everything in it is published")
def publish_everything(corpus):
    corpus.ok("build")
    corpus.ok("publish", "--all", "--no-rendering")
    corpus.before = (corpus.objects(), (corpus.root / "corpus.lock").read_text(),
                     corpus.generation())


# --- whens -------------------------------------------------------------------


@when("I edit the document and build twice", target_fixture="result")
def edit_build_twice(corpus):
    corpus.title = "Demo document, edited"
    corpus.write()
    corpus.ok("build")
    return corpus.ok("build")


@when("I edit the document, build, edit it again, and build")
def edit_twice(corpus):
    corpus.title = "First edit"
    corpus.write()
    corpus.ok("build")
    corpus.first_draft = corpus.drafts()["demo-doc"]["cid"]
    corpus.title = "Second edit"
    corpus.write()
    corpus.ok("build")


@when("I edit the unrelated publet and build")
def edit_other(corpus):
    corpus.other = "Other, edited"
    corpus.write()
    corpus.ok("build")


@when("I publish the document", target_fixture="result")
def publish_document(corpus):
    corpus.previous = corpus.cid("demo-doc")
    return corpus.ok("publish", "demo-doc", "--no-rendering")


@when("I render the document and publish it", target_fixture="result")
def render_and_publish(corpus):
    corpus.ok("render", "demo-doc")
    return corpus.ok("publish", "demo-doc")


@when(parsers.parse('the stub tool becomes "{version}" and its identity claim says so'))
def upgrade_both(corpus, version: str):
    corpus.stub(version)
    corpus.claimed = version
    corpus.write()


@when(parsers.parse('the stub tool becomes "{version}" but its identity claim does not'))
def upgrade_machine(corpus, version: str):
    corpus.stub(version)


@when("I build", target_fixture="result")
def build(corpus):
    return corpus.ok("build")


@when("I publish everything, render the document, and publish it", target_fixture="result")
def publish_render_publish(corpus):
    corpus.ok("publish", "--all", "--no-rendering")
    corpus.ok("render", "demo-doc")
    return corpus.ok("publish", "demo-doc")


@when("I render the document", target_fixture="result")
def render(corpus):
    return corpus.pub("render", "demo-doc")


@when("I build and render the document", target_fixture="result")
def build_render(corpus):
    corpus.ok("build")
    return corpus.pub("render", "demo-doc")


@when("I edit the document, build, render it, and publish it", target_fixture="result")
def edit_render_publish(corpus):
    corpus.first_rendered = corpus.cid("demo-doc")
    corpus.title = "Demo document, second edition"
    corpus.write()
    corpus.ok("build")
    corpus.ok("render", "demo-doc")
    return corpus.ok("publish", "demo-doc")


@when("I reword the pipeline's method and build", target_fixture="result")
def reword_method(corpus):
    corpus.method = "To render with the stub, run its single step."
    corpus.write()
    return corpus.ok("build")


@when("I list the corpus's renderings", target_fixture="result")
def list_renderings(corpus):
    return corpus.ok("corpus", "renderings")


@when("I prune the corpus's renderings", target_fixture="result")
def prune(corpus):
    corpus.objects_before_prune = corpus.objects()
    return corpus.ok("corpus", "renderings", "--prune")


# --- thens -------------------------------------------------------------------


@then(parsers.parse('the draft is reported "{state}"'))
def draft_reported(result, state: str):
    assert any(line.split()[:2] == [state, "demo-doc"] for line in result.stdout.splitlines()), \
        result.stdout


@then("objects/, corpus.lock, and the generation are unchanged")
def unchanged(corpus):
    objects, lock, generation = corpus.before
    assert corpus.objects() == objects
    assert (corpus.root / "corpus.lock").read_text() == lock
    assert corpus.generation() == generation


@then("the document's new version supersedes its previously published version")
def supersedes_published(corpus):
    new = corpus.cid("demo-doc")
    assert new != corpus.previous
    edges = []
    for name in corpus.objects() - corpus.before[0]:
        obj = corpus.read(str(corpus.root / "objects" / name))
        if obj["type"] == "claim.relation" and obj["body"]["kind"] == "supersedes":
            edges.append((obj["body"]["from"], obj["body"]["to"]))
    assert (new, corpus.previous) in edges, edges


@then("no intermediate draft was exported")
def no_intermediate(corpus):
    assert corpus.first_draft.replace(":", "_") + ".cbor" not in corpus.objects()


@then("the unrelated publet is still a draft")
def other_still_draft(corpus):
    assert "other" in corpus.drafts()
    assert corpus.drafts()["other"]["cid"] != corpus.cid("other")


@then("the generation advanced once")
def advanced_once(corpus):
    assert corpus.generation() == corpus.before[2] + 1


@then("one rendering is pinned for the document")
def one_pinned(corpus):
    listing = corpus.renderings()
    assert listing.count("demo-doc") == 1, listing


@then("its output is kept in blobs/ under its identifier")
def kept(corpus):
    out = corpus.root / "publets" / "demo-doc" / "result.txt"
    data = out.read_bytes()
    from support.objects import cid_of
    assert (corpus.root / "blobs" / cid_of(data).replace(":", "_")).read_bytes() == data


@then(parsers.parse('it reports "{text}"'))
def reports(result, text: str):
    assert text in result.stdout, result.stdout + result.stderr


@then(parsers.parse('the publet "{slug}" is reported "{state}"'))
def publet_reported(result, slug: str, state: str):
    assert any(line.split()[:2] == [state, slug] for line in result.stdout.splitlines()), \
        result.stdout


@then("the first rendering's output is gone from blobs/ but its records remain")
def first_pruned(corpus, result):
    assert corpus.objects() == corpus.objects_before_prune
    kept = {p.name for p in (corpus.root / "blobs").iterdir()}
    first = [line for line in result.stdout.splitlines() if "unavailable: pruned" in line]
    assert len(first) == 1, result.stdout
    blob = first[0].split()[1]
    assert blob.replace(":", "_") not in kept
