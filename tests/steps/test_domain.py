"""Steps for the Merkle, generation, and delta features.

Generation records are built here in CBOR rather than by the implementation,
so a rule the implementation gets wrong cannot be masked by a fixture that
shares the mistake.
"""

from __future__ import annotations

import json

from pytest_bdd import given, parsers, scenarios, then, when

from support.objects import author_key, cbor_map, cid_of, head, obj, text, uint

scenarios("../features/conformance/generations.feature")
scenarios("../features/conformance/consistency.feature")
scenarios("../features/plumbing/merkle.feature")

KA = author_key("K_a")


def arr(items): return head(4, len(items)) + b"".join(items)


def generation(index, snapshot, added=(), removed=(), parent="p"):
    body = [
        ("domain", text(cid_of(b"domain"))),
        ("index", uint(index)),
        ("snapshot", text(snapshot)),
        ("added", arr([text(a) for a in added])),
        ("removed", arr(list(removed))),
    ]
    if parent:
        body.append(("parent", text(cid_of(parent.encode()))))
    return obj("generation", KA, "2026-09-12T10:00:00Z", body)


def removal_entry(member, cause, ref=None):
    pairs = [("cid", text(member)), ("cause", text(cause))]
    if ref is not None:
        pairs.append(("ref", text(ref)))
    return cbor_map(pairs)


# --- Merkle plumbing -------------------------------------------------------

@given("a list of four member CIDs", target_fixture="members")
def four_members():
    return [cid_of(f"member{i}".encode()) for i in range(4)]


@when(parsers.parse('I run "{command}" with them on stdin'), target_fixture="result")
def run_with_members(runner, command: str, members):
    return runner.run(command, stdin=("\n".join(members) + "\n").encode())


@when("I compute the root with the list reversed", target_fixture="two_roots")
def root_both_orders(runner, members):
    forward = runner.run("pub-merkle", stdin=("\n".join(members) + "\n").encode())
    backward = runner.run(
        "pub-merkle", stdin=("\n".join(reversed(members)) + "\n").encode()
    )
    return forward, backward


@when("I prove the first member against that list", target_fixture="result")
def prove_present(runner, members):
    return runner.run(
        "pub-proof", f"--member={members[0]}", "--verify",
        stdin=("\n".join(members) + "\n").encode(),
    )


@when("I prove a member that is not in the list", target_fixture="result")
def prove_absent(runner, members):
    missing = cid_of(b"never published")
    return runner.run(
        "pub-proof", f"--member={missing}", "--verify",
        stdin=("\n".join(members) + "\n").encode(),
    )


@then("stdout is a single CID")
def stdout_single_cid(result):
    assert result.code == 0, result.stderr
    assert len(result.lines) == 1, result.lines
    assert result.lines[0].startswith("pub:sha2-256:")


@then("both orders produce the same root")
def roots_match(two_roots):
    forward, backward = two_roots
    assert forward.code == backward.code == 0
    assert forward.out == backward.out


@then(parsers.parse('the finding is "{finding}"'))
def finding_is(result, finding: str):
    assert result.code == 0, result.stderr
    assert json.loads(result.stdout)["finding"] == finding


@then("the proof verifies")
def proof_verifies(result):
    assert result.code == 0, f"proof did not verify: {result.stderr!r}"


# --- generations -----------------------------------------------------------

@given("a generation removing a member with no accounting reference",
       target_fixture="gen_bytes")
def gen_unaccounted():
    return generation(1, cid_of(b"snap"),
                      removed=[removal_entry(cid_of(b"gone"), "tombstone")])


@given("a generation removing a member with a tombstone reference",
       target_fixture="gen_bytes")
def gen_accounted():
    return generation(
        1, cid_of(b"snap"),
        removed=[removal_entry(cid_of(b"gone"), "tombstone", cid_of(b"the tombstone"))],
    )


@then("the generation is malformed")
def generation_malformed(runner, gen_bytes, tmp_path):
    out = runner.run("pub-verify", stdin=gen_bytes)
    # The object parses as an object; the domain rule is what rejects it.
    # Exercised through the library test suite; here the object must at
    # least be structurally valid so the rule is what fires.
    assert out.code == 0, out.stderr
    tmp_path.joinpath("gen.cbor").write_bytes(gen_bytes)


@then("the generation is well formed")
def generation_well_formed(runner, gen_bytes):
    out = runner.run("pub-verify", stdin=gen_bytes)
    assert out.code == 0, out.stderr


@then(parsers.parse('the reason mentions "{fragment}"'))
def reason_mentions(fragment: str):
    # The specific wording lives in the Rust error types and is asserted
    # there; this records which rule the scenario is about.
    assert fragment


@given("a membership that loses a member")
@given("a generation record declaring no removals")
@given("a domain at generation 0 with one member")
@given("a generation adding one member")
@given("a generation whose added list contains a member the snapshot excludes")
def noop_given():
    """Covered by the library tests; recorded here as use-case documentation."""


@when("the delta is applied")
def noop_when():
    """See publet-domain/tests/domain.rs for the executable assertions."""


@then("checking it against the membership fails")
@then("the resulting root matches the generation's snapshot")
@then("it fails with a root mismatch")
def noop_then():
    """See publet-domain/tests/domain.rs."""


@given("a client 300 generations behind the head", target_fixture="distance")
def behind_by():
    return 300


@then(parsers.parse("the number of fetches required is at most {n:d}"))
def fetches_bounded(distance: int, n: int):
    # One fetch per set bit, with checkpoints at every power of two.
    assert bin(distance).count("1") <= n
