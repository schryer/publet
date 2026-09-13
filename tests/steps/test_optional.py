"""Steps for the optional layers.

These exercise rules rather than mechanisms. The cryptography of a
personhood scheme and the consensus of a ledger are properties of whatever
is plugged in; what the protocol requires, and what is asserted here, is how
those things may be treated once present.

Everything runs the shipped binaries, like the rest of the suite. A scenario
that asserted "the Rust test passes" would be unrunnable against a second
implementation, which is the whole point of writing these in Gherkin.
"""

from __future__ import annotations

import json
from pathlib import Path

import pytest
from pytest_bdd import given, parsers, scenarios, then, when

from support.objects import (
    assumption, attestation, author_key, bounty, cid_of, publet, triage,
    write_store,
)

scenarios("../features/conformance/settlement.feature")
scenarios("../features/conformance/personhood.feature")
scenarios("../features/conformance/assumption.feature")

KA = author_key("K_a")
KB = author_key("K_b")
POLICY = cid_of(b"a settlement policy")
TARGET = cid_of(b"the object under review")


def write_one(tmp_path: Path, data: bytes, name: str) -> Path:
    path = tmp_path / name
    path.write_bytes(data)
    return path


# --- settlement ------------------------------------------------------------

@given("no ledger is configured", target_fixture="bounty_file")
def no_ledger(tmp_path):
    return write_one(
        tmp_path,
        bounty(KA, "2026-09-12T10:00:00Z", TARGET, POLICY, 6000),
        "bounty.cbor",
    )


@given("a bounty naming no policy", target_fixture="bounty_file")
def bounty_no_policy(tmp_path):
    return write_one(
        tmp_path,
        bounty(KA, "2026-09-12T10:00:00Z", TARGET, None, 6000),
        "bounty.cbor",
    )


@given(parsers.parse("a bounty with a review share of {share:d} basis points"),
       target_fixture="bounty_file")
def bounty_share(tmp_path, share: int):
    return write_one(
        tmp_path,
        bounty(KA, "2026-09-12T10:00:00Z", TARGET, POLICY, share),
        "bounty.cbor",
    )


@when("a bounty is settled", target_fixture="result")
def settle(runner, bounty_file):
    return runner.run("pub-settle", f"--settle={bounty_file}")


@then("reading it fails", target_fixture="result")
def reading_fails(runner, bounty_file):
    out = runner.run("pub-settle", f"--bounty={bounty_file}")
    assert out.code != 0, f"expected a refusal, got {out.stdout!r}"
    return out


@then("reading it succeeds", target_fixture="result")
def reading_succeeds(runner, bounty_file):
    out = runner.run("pub-settle", f"--bounty={bounty_file}")
    assert out.code == 0, out.stderr
    return out


@then("the reason says settlement is optional")
def reason_optional(result):
    assert "settlement is optional" in result.stderr, result.stderr


@then("the reason says a bounty must name the policy it settles against")
def reason_no_policy(result):
    assert "policy" in result.stderr.lower(), result.stderr


@given("the settlement prohibitions", target_fixture="result")
def prohibitions(runner):
    out = runner.run("pub-settle", "--prohibitions")
    assert out.code == 0, out.stderr
    return out


@then("each names what it forbids and why")
def each_prohibition_explained(result):
    rows = [json.loads(line) for line in result.stdout.splitlines() if line.strip()]
    assert len(rows) >= 4, rows
    for row in rows:
        assert row["forbids"], row
        assert len(row["reason"].split()) >= 5, row


@given("the ledger trait", target_fixture="ledger_surface")
def ledger_trait():
    """The trait's whole surface, read from the source that defines it.

    A rule about what an interface *cannot express* is a rule about the
    interface, so the interface is what gets read. There is no run-time
    observation that could establish the absence of a method.
    """
    path = (Path(__file__).resolve().parents[2]
            / "crates/publet-settle/src/ledger.rs")
    if not path.exists():
        # A deployment may omit settlement entirely (Section 12.1). There is
        # then no trait to read, and no rule about it to break.
        pytest.skip("settlement is not present in this build")
    source = path.read_text()
    body = source.split("pub trait Ledger {", 1)[1].split("\n}", 1)[0]
    return [line.strip() for line in body.splitlines()
            if line.strip().startswith("fn ")]


@then("it offers no way to report or influence a standing")
def no_standing_method(ledger_surface):
    for signature in ledger_surface:
        assert "standing" not in signature, signature
        assert "weight" not in signature, signature


@then("it offers no way to gate a retrieval")
def no_retrieval_method(ledger_surface):
    for signature in ledger_surface:
        for word in ("read", "fetch", "serve", "retriev", "access"):
            assert word not in signature, signature


# --- personhood ------------------------------------------------------------

@given("an implementation carrying one personhood scheme",
       target_fixture="schemes")
def one_scheme():
    return ["--scheme=demo-unlinkable"]


@given("an implementation carrying two personhood schemes",
       target_fixture="schemes")
def two_schemes():
    return ["--scheme=demo-unlinkable", "--scheme=demo-issuer-linkable"]


@then("it does not meet the scheme minimum")
def below_minimum(runner, schemes):
    out = runner.run("pub-person", *schemes, "--schemes")
    assert out.code != 0, out.stdout
    assert "at least" in out.stderr, out.stderr


@then("it meets the scheme minimum", target_fixture="result")
def meets_minimum(runner, schemes):
    out = runner.run("pub-person", *schemes, "--schemes")
    assert out.code == 0, out.stderr
    return out


@then("their declared anonymity differs")
def anonymity_differs(result):
    rows = [json.loads(line) for line in result.stdout.splitlines() if line.strip()]
    assert len(rows) == 2, rows
    claims = [tuple(sorted((k, v) for k, v in row.items() if k != "scheme"))
              for row in rows]
    assert claims[0] != claims[1], rows


@given("a verified personhood attestation in a scope", target_fixture="person")
def first_attestation(tmp_path):
    nullifier = b"\x11\x22\x33"
    path = write_one(
        tmp_path,
        attestation(KA, "2026-09-12T10:00:00Z", cid_of(b"key one"),
                    "demo-unlinkable", "domain-a", nullifier),
        "first.cbor",
    )
    return {"dir": tmp_path, "first": path, "nullifier": nullifier}


@when("a second key presents the same nullifier in that scope",
      target_fixture="result")
def same_nullifier_same_scope(runner, person):
    second = write_one(
        person["dir"],
        attestation(KB, "2026-09-12T10:01:00Z", cid_of(b"key two"),
                    "demo-unlinkable", "domain-a", person["nullifier"]),
        "second.cbor",
    )
    return runner.run("pub-person", "--scheme=demo-unlinkable",
                      f"--attest={person['first']}", f"--attest={second}")


@when("the same person presents in a different scope", target_fixture="result")
def same_nullifier_other_scope(runner, person):
    # A different scope means a different nullifier for the same person:
    # that is what unlinkability across scopes *is*. Presenting the same
    # bytes in another scope would be the scheme failing, not the person.
    second = write_one(
        person["dir"],
        attestation(KB, "2026-09-12T10:01:00Z", cid_of(b"key two"),
                    "demo-unlinkable", "domain-b", b"\x44\x55\x66"),
        "second.cbor",
    )
    return runner.run("pub-person", "--scheme=demo-unlinkable",
                      f"--attest={person['first']}", f"--attest={second}")


@then("it is refused as the same person")
def refused_same_person(result):
    assert result.code != 0, result.stdout
    assert "already holds a key" in result.stderr, result.stderr


@then("it is accepted")
def accepted(result):
    assert result.code == 0, result.stderr
    assert len(result.stdout.splitlines()) == 2, result.stdout


@given("a workspace", target_fixture="workspace_dir")
def bare_workspace(runner, tmp_path):
    out = runner.run("pub", "init")
    assert out.code == 0, out.stderr
    return tmp_path


@when("I compose a claim", target_fixture="result")
def compose_without_personhood(runner, workspace_dir):
    return runner.run("pub", "compose", "--class=definitional",
                      "--content=a term: a meaning", "--scope=unconditional")


@then("no personhood attestation was required")
def no_attestation_required(result):
    body = result.stdout + result.stderr
    assert "personhood" not in body.lower(), body
    assert body.strip().startswith("pub:sha2-256:"), body


# --- assumption and triage -------------------------------------------------

@given("a key assuming accountability for another", target_fixture="store")
def one_assumption(tmp_path):
    pseudonym = author_key("K_p")
    own = publet(pseudonym, "2026-09-12T09:00:00Z", "definitional",
                 "a term: a meaning")
    write_store(tmp_path, [
        own,
        assumption(KA, "2026-09-12T10:00:00Z", pseudonym),
    ])
    return {"dir": tmp_path, "assumer": KA, "assumed": pseudonym,
            "publet": cid_of(own)}


@given("a key assuming accountability for three others", target_fixture="store")
def three_assumptions(tmp_path):
    objects = []
    for n in range(3):
        pseudonym = author_key(f"K_p{n}")
        objects.append(
            assumption(KA, f"2026-09-12T10:0{n}:00Z", pseudonym))
    write_store(tmp_path, objects)
    return {"dir": tmp_path, "assumer": KA}


@then("the assumed key remains the author of its own publets")
def still_the_author(runner, store):
    # Vouching moves no authorship. The publet is still listed under the
    # key that signed it, and listing by the assumer's key returns nothing:
    # the assumer has staked their standing, not taken the byline.
    by_assumed = runner.run("pub-ls", f"--dir={store['dir']}", "--type=publet",
                            f"--author={store['assumed']}")
    assert by_assumed.code == 0, by_assumed.stderr
    assert by_assumed.stdout.split() == [store["publet"]], by_assumed.stdout

    by_assumer = runner.run("pub-ls", f"--dir={store['dir']}", "--type=publet",
                            f"--author={store['assumer']}")
    assert by_assumer.code == 0, by_assumer.stderr
    assert by_assumer.stdout.strip() == "", by_assumer.stdout


@then(parsers.parse("the count of assumptions it holds is {n:d}"))
def assumption_count(runner, store, n: int):
    out = runner.run("pub-ann", f"--dir={store['dir']}",
                     f"--assumptions-by={store['assumer']}")
    assert out.code == 0, out.stderr
    rows = [json.loads(line) for line in out.stdout.splitlines() if line.strip()]
    assert len(rows) == n, rows
    assert len({row["assumed"] for row in rows}) == n, rows


@then("the assumption records no identity for the assumed key")
def no_identity_recorded(runner, store):
    out = runner.run("pub-ann", f"--dir={store['dir']}",
                     f"--assumptions-by={store['assumer']}")
    assert out.code == 0, out.stderr
    rows = [json.loads(line) for line in out.stdout.splitlines() if line.strip()]
    assert len(rows) == 1, rows
    # Two keys and a basis. A field for who holds the assumed key is the one
    # thing the record must never carry.
    assert set(rows[0]) == {"assumer", "assumed", "basis"}, rows[0]


@given("a claim with a standing", target_fixture="workspace")
def claim_with_standing(runner, tmp_path):
    assert runner.run("pub", "init").code == 0
    out = runner.run("pub", "compose", "--class=definitional",
                     "--content=a term: a meaning", "--scope=unconditional")
    assert out.code == 0, out.stderr
    claim = out.stdout.strip()
    why = runner.run("pub", "why", claim)
    assert why.code == 0, why.stderr
    return {"dir": tmp_path, "target": claim, "before": why.stdout}


@given("a triage annotation calling it redundant")
def add_triage(runner, workspace):
    data = triage(KB, "2026-09-12T10:00:00Z", workspace["target"],
                  "redundant-with")
    added = runner.run("pub-store", "--store=.publet/objects.redb", "add",
                       stdin=data)
    assert added.code == 0, added.stderr
    workspace["triage"] = added.stdout.strip()


@then("the standing is unchanged")
def standing_unchanged(runner, workspace):
    # The comparison proves nothing unless the annotation really landed.
    held = runner.run("pub-store", "--store=.publet/objects.redb", "list")
    assert held.code == 0, held.stderr
    assert workspace["triage"] in held.stdout, held.stdout

    after = runner.run("pub", "why", workspace["target"])
    assert after.code == 0, after.stderr
    assert after.stdout == workspace["before"], (
        workspace["before"], after.stdout)


@given("a triage annotation with no engine declared", target_fixture="store")
def triage_without_engine(tmp_path):
    claim = publet(KA, "2026-09-12T09:00:00Z", "definitional",
                   "a term: a meaning")
    write_store(tmp_path, [
        claim,
        triage(KB, "2026-09-12T10:00:00Z", cid_of(claim), "redundant-with",
               engine=None),
    ])
    return {"dir": tmp_path, "target": cid_of(claim)}


@then("it is not treated as a triage finding")
def not_read_as_triage(runner, store):
    out = runner.run("pub-ann", f"--dir={store['dir']}",
                     f"--triage-of={store['target']}")
    assert out.code == 0, out.stderr
    assert out.stdout.strip() == "", out.stdout
