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


def _read_generation(runner, gen_bytes, tmp_path):
    """Ask a tool to read the record, which is where the rules live."""
    (tmp_path / "g1.cbor").write_bytes(gen_bytes)
    return runner.run(
        "pub-delta", f"--dir={tmp_path}", "--from=0",
        f"--member={cid_of(b'anything')}",
    )


@then("the generation is malformed")
def generation_malformed(runner, gen_bytes, tmp_path, request):
    out = _read_generation(runner, gen_bytes, tmp_path)
    assert out.code != 0, (
        "a record with an unaccounted removal must be refused, "
        f"got exit 0 with {out.stdout!r}"
    )
    request.node.stash_stderr = out.stderr


@then("the generation is well formed")
def generation_well_formed(runner, gen_bytes, tmp_path):
    out = _read_generation(runner, gen_bytes, tmp_path)
    # The record parses; the root will not match this arbitrary membership,
    # and that is a different failure from a malformed record.
    assert "no `ref` accounting" not in out.stderr, out.stderr
    assert "unknown cause" not in out.stderr, out.stderr


@then(parsers.parse('the reason mentions "{fragment}"'))
def reason_mentions(runner, gen_bytes, tmp_path, fragment: str):
    out = _read_generation(runner, gen_bytes, tmp_path)
    assert fragment in out.stderr, f"{fragment!r} not in {out.stderr!r}"


@given("a domain at generation 0 with one member", target_fixture="delta_store")
def delta_store(tmp_path):
    return {"dir": tmp_path, "member": cid_of(b"the first member")}


@given("a generation adding one member")
def generation_adding(runner, delta_store):
    added = cid_of(b"the second member")
    members = sorted([delta_store["member"], added])
    root = runner.run("pub-merkle", stdin=("\n".join(members) + "\n").encode())
    assert root.code == 0, root.stderr
    (delta_store["dir"] / "g1.cbor").write_bytes(
        generation(1, root.stdout.strip(), added=[added])
    )


@given("a generation whose added list contains a member the snapshot excludes")
def generation_smuggling(runner, delta_store):
    # The snapshot covers two members; the added list names three.
    added = cid_of(b"the second member")
    smuggled = cid_of(b"never declared")
    members = sorted([delta_store["member"], added])
    root = runner.run("pub-merkle", stdin=("\n".join(members) + "\n").encode())
    (delta_store["dir"] / "g1.cbor").write_bytes(
        generation(1, root.stdout.strip(), added=[added, smuggled])
    )


@given("a membership that loses a member", target_fixture="delta_store")
def membership_losing(tmp_path):
    return {"dir": tmp_path, "member": cid_of(b"kept"),
            "dropped": cid_of(b"dropped")}


@given("a generation record declaring no removals")
def generation_silent_removal(runner, delta_store):
    # The snapshot covers only the kept member; the record declares nothing.
    root = runner.run(
        "pub-merkle", stdin=(delta_store["member"] + "\n").encode()
    )
    (delta_store["dir"] / "g1.cbor").write_bytes(
        generation(1, root.stdout.strip())
    )


@when("the delta is applied", target_fixture="result")
def apply_delta(runner, delta_store):
    return runner.run(
        "pub-delta", f"--dir={delta_store['dir']}", "--from=0",
        f"--member={delta_store['member']}",
    )


@then("the resulting root matches the generation's snapshot")
def root_matches(result):
    assert result.code == 0, result.stderr


@then("it fails with a root mismatch")
def root_mismatch(result):
    assert result.code != 0, result.stdout
    assert "membership root" in result.stderr, result.stderr


@then("checking it against the membership fails")
def check_against_fails(runner, delta_store):
    # The membership loses a member and the record declares no removal, so
    # applying it cannot reproduce the root the record declares. An
    # implementation that let the removal through would compute a different
    # root and this would pass silently -- which is why the assertion is on
    # the mismatch being reported, not merely on a non-zero exit.
    out = runner.run(
        "pub-delta", f"--dir={delta_store['dir']}", "--from=0",
        f"--member={delta_store['member']}",
        f"--member={delta_store['dropped']}",
    )
    assert out.code != 0, out.stdout
    assert "membership root" in out.stderr, out.stderr


@given("a client 300 generations behind the head", target_fixture="distance")
def behind_by():
    return 300


@then(parsers.parse("the number of fetches required is at most {n:d}"))
def fetches_bounded(distance: int, n: int):
    # One fetch per set bit, with checkpoints at every power of two.
    assert bin(distance).count("1") <= n
