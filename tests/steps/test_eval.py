"""Steps for the evaluation features.

These drive the committed vectors rather than fixtures built here, because
the vectors are also what the cross-architecture matrix compares. One set of
inputs, checked for behaviour by these scenarios and for determinism by
tools/matrix.sh.
"""

from __future__ import annotations

import json
import re
from pathlib import Path

import pytest
from pytest_bdd import given, parsers, scenarios, then, when

scenarios("../features/conformance/evidence_dominance.feature")
scenarios("../features/conformance/restricted_access.feature")
scenarios("../features/conformance/determinism.feature")
scenarios("../features/plumbing/eval.feature")
scenarios("../features/conformance/reproducible_evaluation.feature")
scenarios("../features/conformance/equivalence.feature")
scenarios("../features/conformance/viewpoint.feature")

VECTORS = Path(__file__).resolve().parents[2] / "vectors" / "eval"


@given(parsers.parse('the "{name}" vector'), target_fixture="vector")
def named_vector(name: str) -> Path:
    path = VECTORS / f"{name}.cbor"
    if not path.exists():
        pytest.fail(f"missing vector: {path}")
    return path


@given("every committed evaluation vector", target_fixture="vectors")
def all_vectors() -> list[Path]:
    found = sorted(VECTORS.glob("*.cbor"))
    assert found, "no vectors are committed"
    return found


@given("the same vector evaluated under a policy with a replication floor of zero",
       target_fixture="relaxed")
def relaxed_vector() -> Path:
    return VECTORS / "11-floor-zero.cbor"


@given("an evaluation vector where one key affirms three times",
       target_fixture="vector")
def repeated_signer() -> Path:
    return VECTORS / "12-repeated-signer.cbor"


@given("an evaluation vector whose policy declares no roots",
       target_fixture="vector")
def rootless_policy() -> Path:
    path = VECTORS.parent / "invalid" / "policy-without-roots.cbor"
    if not path.exists():
        pytest.fail(f"missing vector: {path}")
    return path


@when("I evaluate it", target_fixture="result")
def evaluate_one(runner, vector: Path):
    return runner.run("pub-eval", f"--vector={vector}")


@when("I evaluate each one twice", target_fixture="pairs")
def evaluate_all_twice(runner, vectors: list[Path]):
    return [
        (runner.run("pub-eval", f"--vector={v}"),
         runner.run("pub-eval", f"--vector={v}"))
        for v in vectors
    ]


def _parsed(result):
    assert result.code == 0, f"pub-eval failed: {result.stderr!r}"
    return json.loads(result.stdout)


@then(parsers.parse('the result is "{outcome}"'))
def result_is(result, outcome: str):
    assert _parsed(result)["result"] == outcome


@then(parsers.parse('the result is not "{outcome}"'))
def result_is_not(result, outcome: str):
    assert _parsed(result)["result"] != outcome


@then(parsers.parse('the delta is "{value}"'))
def delta_is(result, value: str):
    assert _parsed(result)["delta"] == value


@then(parsers.parse('the delta is greater than "{value}"'))
def delta_greater(result, value: str):
    assert float(_parsed(result)["delta"]) > float(value)


@then(parsers.parse('the reported reproducibility class is "{value}"'))
def reproducibility_is(result, value: str):
    assert _parsed(result)["reproducibility"] == value


@then(parsers.parse("the reported independent consistent reproductions are {n:d}"))
def independent_consistent_is(result, n: int):
    assert _parsed(result)["reproductions"]["independent_consistent"] == n


@then("both runs produce byte-identical output")
def runs_identical_bytes(pairs):
    for first, second in pairs:
        assert first.out == second.out, f"nondeterministic: {first.argv}"
        assert first.code == 0


@then("the output names affirm, deny, abstain, active and delta")
def output_has_components(result):
    parsed = _parsed(result)
    for field in ("affirm", "deny", "abstain", "active", "delta"):
        assert field in parsed, f"{field} missing; a badge is not a standing"


@then("every weight is written to exactly six decimal places")
def weights_are_six_places(result):
    parsed = _parsed(result)
    for field in ("affirm", "deny", "abstain", "active", "delta"):
        assert re.fullmatch(r"-?\d+\.\d{6}", parsed[field]), parsed[field]


@then("the two evaluations disagree about the outcome")
def evaluations_disagree(runner, vector: Path, relaxed: Path):
    strict = json.loads(
        runner.run("pub-eval", f"--vector={vector}").stdout
    )["result"]
    lenient = json.loads(
        runner.run("pub-eval", f"--vector={relaxed}").stdout
    )["result"]
    assert strict != lenient, (
        "identical evidence must be able to yield different outcomes under "
        f"different policies, got {strict} both times"
    )


@then("the affirm weight equals that of a single affirmation")
def one_signer_counts_once(runner, result):
    single = runner.run("pub-eval", f"--vector={VECTORS / '13-single-signer.cbor'}")
    assert single.code == 0, single.stderr
    assert json.loads(result.stdout)["affirm"] == json.loads(single.stdout)["affirm"]


@then("it fails")
def evaluation_fails(result):
    assert result.code != 0, result.stdout


@then("it says a policy must declare at least one root")
def says_roots_required(result):
    assert "at least one root" in result.stderr, result.stderr
