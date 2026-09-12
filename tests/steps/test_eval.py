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
