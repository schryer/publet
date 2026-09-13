"""Steps for `pub relate`.

Relations are the edges. Until this command existed the porcelain could
compose two object kinds and the graph could read nine, so a workspace
could hold publets and never connect them.
"""

from __future__ import annotations

from pytest_bdd import given, parsers, scenarios, then, when

scenarios("../features/porcelain/relate.feature")

FIRST = "content address: an identifier computed from the bytes of an object"
SECOND = "content address: an identifier naming an object by its digest"


@given("a workspace with two composed definitions", target_fixture="pair")
def two_definitions(runner, tmp_path):
    assert runner.run("pub", "init").code == 0
    made = []
    for content in (FIRST, SECOND):
        out = runner.run("pub", "compose", "--class=definitional",
                         "--scope=unconditional", f"--content={content}")
        assert out.code == 0, out.stderr
        made.append(out.stdout.strip())
    return {"first": made[0], "second": made[1], "dir": tmp_path}


def _held(runner) -> set[str]:
    out = runner.run("pub-store", "--store=.publet/objects.redb", "list")
    assert out.code == 0, out.stderr
    return set(out.stdout.split())


@given(parsers.parse('they are already related as "{kind}"'))
def already_related(runner, pair, kind: str):
    out = runner.run("pub", "relate", f"--kind={kind}",
                     f"--from={pair['first']}", f"--to={pair['second']}")
    assert out.code == 0, out.stderr
    pair["before"] = _held(runner)


@when(parsers.parse('I relate them as "{kind}"'), target_fixture="result")
def relate(runner, pair, kind: str):
    pair.setdefault("before", _held(runner))
    return runner.run("pub", "relate", f"--kind={kind}",
                      f"--from={pair['first']}", f"--to={pair['second']}")


@when(parsers.parse('I relate them the other way as "{kind}"'),
      target_fixture="result")
def relate_reverse(runner, pair, kind: str):
    pair.setdefault("before", _held(runner))
    return runner.run("pub", "relate", f"--kind={kind}",
                      f"--from={pair['second']}", f"--to={pair['first']}")


@when(parsers.parse('I relate the first to itself as "{kind}"'),
      target_fixture="result")
def relate_self(runner, pair, kind: str):
    pair.setdefault("before", _held(runner))
    return runner.run("pub", "relate", f"--kind={kind}",
                      f"--from={pair['first']}", f"--to={pair['first']}")


@then("the edge is readable from the first")
def edge_readable(runner, pair):
    # Read it back through the plumbing rather than trusting the writer:
    # a command that reports success and stores nothing would pass any
    # assertion made against its own output.
    held = _held(runner)
    assert len(held) > len(pair["before"]), held


@then("the store is unchanged")
def store_unchanged(runner, pair):
    assert _held(runner) == pair["before"], "a refused edge was stored anyway"
