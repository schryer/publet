"""Steps for usage evidence (Section 5.2).

A definitional publet accepts no verdict, so before `usage` existed it had
no evidence channel at all and `pub why` showed a column of zeroes.
"""

from __future__ import annotations

from pytest_bdd import given, scenarios, then, when

scenarios("../features/conformance/usage.feature")

CONTENT = "merkle tree: a tree whose every non-leaf node is the hash of its children"
FIRST = "en.wiktionary.org/wiki/Merkle_tree"
SECOND = "doc.rust-lang.org/reference"


@given("a workspace with a composed definition", target_fixture="defn")
def composed_definition(runner, tmp_path):
    assert runner.run("pub", "init").code == 0
    out = runner.run("pub", "compose", "--class=definitional",
                     "--scope=unconditional", f"--content={CONTENT}")
    assert out.code == 0, out.stderr
    return {"cid": out.stdout.strip(), "dir": tmp_path}


@when("I record a verdict on it", target_fixture="result")
def record_verdict(runner, defn):
    return runner.run("pub", "annotate", "--kind=verdict",
                      f"--target={defn['cid']}", "--finding=affirm")


@when("I cite two sources using it", target_fixture="result")
def cite_two(runner, defn):
    last = None
    for source in (FIRST, SECOND):
        last = runner.run("pub", "annotate", "--kind=usage",
                          f"--target={defn['cid']}", f"--source={source}",
                          "--locator=sense 1")
        assert last.code == 0, last.stderr
    return last


@when("I cite the same source twice", target_fixture="result")
def cite_twice(runner, defn):
    last = None
    for locator in ("sense 1", "sense 2"):
        last = runner.run("pub", "annotate", "--kind=usage",
                          f"--target={defn['cid']}", f"--source={FIRST}",
                          f"--locator={locator}")
        assert last.code == 0, last.stderr
    return last


def _why(runner, defn) -> str:
    out = runner.run("pub", "why", defn["cid"])
    assert out.code == 0, out.stderr
    return out.stdout


@then("asking why lists both sources")
def lists_both(runner, defn):
    body = _why(runner, defn)
    assert FIRST in body, body
    assert SECOND in body, body


@then("asking why reports one distinct source")
def one_source(runner, defn):
    body = _why(runner, defn)
    assert "1 distinct source" in body, body


@then("asking why reports no corpus citations")
def no_citations(runner, defn):
    assert "no corpus citations" in _why(runner, defn)


@then("no citation carries the defined text")
def no_text_reproduced(runner, defn):
    # The point of a citation is that it names a location. If the defined
    # text travelled with it, four sources agreeing would mean four copies.
    out = runner.run("pub-ls", f"--dir={defn['dir']}", "--type=ann")
    body = _why(runner, defn)
    usage_block = body.split("usage", 1)[1]
    assert "merkle tree:" not in usage_block.lower(), usage_block
    assert out.code in (0, 1)
