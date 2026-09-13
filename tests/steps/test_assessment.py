"""Steps for assessments (Section 5.2).

An assessment carries the partial correctness a weight cannot: a claim
sound where its scope says and misleading outside it. The protocol has no
number for that, and naming the conditions says more than one would.
"""

from __future__ import annotations

from pytest_bdd import given, parsers, scenarios, then, when

scenarios("../features/conformance/assessment.feature")

CONTENT = "hash function: a function mapping data of arbitrary size to fixed-size values"
BASIS = "omits collision resistance, so it misleads in a security context"


@given("a workspace with a scoped definition", target_fixture="defn")
def scoped_definition(runner, tmp_path):
    assert runner.run("pub", "init").code == 0
    out = runner.run("pub", "compose", "--class=definitional",
                     "--scope=non-cryptographic contexts", f"--content={CONTENT}")
    assert out.code == 0, out.stderr
    cid = out.stdout.strip()
    before = runner.run("pub", "why", cid)
    assert before.code == 0, before.stderr
    return {"cid": cid, "weights": _weights(before.stdout)}


def _weights(body: str) -> list[str]:
    block = body.split("weight", 1)[1].split("evidence", 1)[0]
    return [line.strip() for line in block.splitlines() if line.strip()]


@when(parsers.parse('I assess it as "{verdict}"'), target_fixture="result")
def assess(runner, defn, verdict: str):
    return runner.run("pub", "annotate", "--kind=assessment",
                      f"--target={defn['cid']}", f"--verdict={verdict}",
                      f"--basis={BASIS}")


@when("I assess it with no basis", target_fixture="result")
def assess_no_basis(runner, defn):
    return runner.run("pub", "annotate", "--kind=assessment",
                      f"--target={defn['cid']}", "--verdict=sound-in-scope")


@then("asking why shows the judgement")
def shows_judgement(runner, defn):
    body = runner.run("pub", "why", defn["cid"]).stdout
    assert "sound-in-scope" in body, body
    assert BASIS in body, body


@then("asking why still reports the class not truth-apt")
def still_not_truth_apt(runner, defn):
    # The judgement is displayed and decides nothing. If filing one could
    # move the result, it would be a verdict wearing another name.
    body = runner.run("pub", "why", defn["cid"]).stdout
    assert "not-truth-apt" in body, body


@then("the weights are unchanged")
def weights_unchanged(runner, defn):
    body = runner.run("pub", "why", defn["cid"]).stdout
    assert _weights(body) == defn["weights"], body
