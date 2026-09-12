"""Steps for the canonical-form, CID, and verification features."""

from __future__ import annotations

import pytest
from pytest_bdd import given, parsers, scenarios, then, when

from support.objects import DEFECTS, cid_of, cbor_map, head, minimal_object, text, uint

scenarios("../features/conformance/canonical_form.feature")
scenarios("../features/plumbing/cid.feature")
scenarios("../features/plumbing/verify.feature")


# --- Given -----------------------------------------------------------------

@given(parsers.parse("an object encoded with {defect}"), target_fixture="payload")
def payload_with_defect(defect: str) -> bytes:
    if defect not in DEFECTS:
        pytest.fail(f"no fixture for defect {defect!r}")
    return DEFECTS[defect][0]


@given("a well-formed object", target_fixture="payload")
def well_formed() -> bytes:
    return minimal_object()


@given("a second object differing only in its created field",
       target_fixture="other_payload")
def other_object() -> bytes:
    return minimal_object(created="2026-09-12T10:00:01Z")


@given(parsers.parse("a well-formed object with the {field} field removed"),
       target_fixture="payload")
def object_missing_field(field: str) -> bytes:
    fields = {
        "pub": text("1"),
        "type": text("publet"),
        "created": text("2026-09-12T10:00:00Z"),
        "author": text(cid_of(b"example key object")),
        "body": cbor_map([]),
    }
    del fields[field]
    return cbor_map(list(fields.items()))


@given(parsers.parse('a well-formed object with an extra top-level field "{name}"'),
       target_fixture="payload")
def object_with_extra_field(name: str) -> bytes:
    return cbor_map([
        ("pub", text("1")),
        ("type", text("publet")),
        ("created", text("2026-09-12T10:00:00Z")),
        ("author", text(cid_of(b"example key object"))),
        ("body", cbor_map([])),
        (name, uint(1)),
    ])


# --- When ------------------------------------------------------------------

@when(parsers.parse('I run "{command}" with that object on stdin'),
      target_fixture="result")
def run_command(runner, command: str, payload: bytes):
    return runner.run(command, stdin=payload)


@when(parsers.parse('I run "{command}" with that object on stdin twice'),
      target_fixture="two_results")
def run_twice(runner, command: str, payload: bytes):
    return [runner.run(command, stdin=payload) for _ in range(2)]


@when(parsers.parse('I run "{command}" on each'), target_fixture="two_results")
def run_on_each(runner, command: str, payload: bytes, other_payload: bytes):
    return [runner.run(command, stdin=p) for p in (payload, other_payload)]


@when("I verify it against the CID this suite computed", target_fixture="result")
def verify_against_own_cid(runner, payload: bytes):
    return runner.run("pub-verify", f"--cid={cid_of(payload)}", stdin=payload)


@when("I verify the first against the second's CID", target_fixture="result")
def verify_against_other_cid(runner, payload: bytes, other_payload: bytes):
    return runner.run("pub-verify", f"--cid={cid_of(other_payload)}", stdin=payload)


# --- Then ------------------------------------------------------------------

@then(parsers.parse('stderr mentions "{fragment}"'))
def stderr_mentions(result, fragment: str):
    assert fragment in result.stderr, f"{fragment!r} not in stderr: {result.stderr!r}"


@then("stdout is byte-identical to the input")
def stdout_matches_input(result, payload: bytes):
    assert result.out == payload


@then("stdout is the CID this suite computed for the input")
def stdout_is_expected_cid(result, payload: bytes):
    assert result.stdout.strip() == cid_of(payload)


@then("both runs produce identical output")
def runs_identical(two_results):
    first, second = two_results
    assert first.out == second.out
    assert first.code == second.code == 0


@then("the two identifiers differ")
def identifiers_differ(two_results):
    first, second = two_results
    assert first.code == second.code == 0
    assert first.out != second.out
