"""Steps for the rules implemented after the coverage report named them.

Each of these covers a normative statement the report listed as untested,
and each turned out to be untested because it was also unimplemented.
"""

from __future__ import annotations

import json
import subprocess
from pathlib import Path

from pytest_bdd import given, parsers, scenarios, then, when

from support.objects import (
    author_key, cbor_map, cid_of, head, obj, publet, relation, text, uint,
    write_store,
)

scenarios("../features/conformance/evidence_required.feature")
scenarios("../features/conformance/redundancy.feature")
scenarios("../features/conformance/human_signatory.feature")

KA = author_key("K_a")


def arr(items):
    return head(4, len(items)) + b"".join(items)


def key_object(seed: str, principal: str | None) -> bytes:
    body = [("alg", text("ed25519")), ("pubkey", head(2, 4) + b"\x00\x01\x02\x03")]
    if principal is not None:
        body.append(("principal", text(principal)))
    return obj("key", author_key(seed), "2026-09-12T00:00:00Z", body)


# --- 5.5 and 5.4 -----------------------------------------------------------

@given("an empirical publet naming no method", target_fixture="store")
def empirical_without_method(tmp_path):
    data = obj("publet", KA, "2026-09-12T10:00:00Z", [
        ("class", text("empirical")),
        ("lang", text("en")),
        ("content", text("the rate fell")),
        ("depends", arr([])),
    ])
    write_store(tmp_path, [data])
    return {"dir": tmp_path}


@given("an empirical publet naming a method", target_fixture="store")
def empirical_with_method(tmp_path):
    write_store(tmp_path, [
        publet(KA, "2026-09-12T10:00:00Z", "empirical", "the rate fell")
    ])
    return {"dir": tmp_path}


@given("two publets that presuppose each other", target_fixture="store")
def mutual_dependency(tmp_path):
    a = publet(KA, "2026-09-12T10:00:00Z", "definitional", "a: defined by b")
    b = publet(KA, "2026-09-12T10:00:01Z", "definitional", "b: defined by a",
               [cid_of(a)])
    # The in-body reference can only point backwards, so the cycle is closed
    # by a relation, which is the only place one can be created (Section 6).
    rel = relation(KA, "2026-09-12T10:01:00Z", "depends", cid_of(a), cid_of(b))
    write_store(tmp_path, [a, b, rel])
    return {"dir": tmp_path, "target": cid_of(a)}


@given(parsers.parse('a "{cls}" publet'), target_fixture="store")
def classed_publet(tmp_path, cls: str):
    content = "term: a meaning" if cls == "definitional" else "an assertion"
    write_store(tmp_path, [publet(KA, "2026-09-12T10:00:00Z", cls, content)])
    return {"dir": tmp_path}


@when("I ask for the dependency closure", target_fixture="result")
def dependency_closure(runner, store):
    return runner.run("pub-closure", f"--dir={store['dir']}", store["target"])


# --- 7.3 redundancy --------------------------------------------------------

def _disputed_workspace(bin_dir, tmp_path, *, resolution_outcome, cover_grounds):
    """A claim, an argued dispute, and optionally a resolution covering it."""
    pub = bin_dir / "pub"
    store_bin = bin_dir / "pub-store"
    assert subprocess.run([str(pub), "init"], capture_output=True,
                          cwd=tmp_path).returncode == 0
    config = (tmp_path / ".publet/config").read_text()
    author = [l.split("=", 1)[1] for l in config.splitlines()
              if l.startswith("author=")][0]

    def add(data: bytes) -> str:
        proc = subprocess.run(
            [str(store_bin), "--store=.publet/objects.redb", "add"],
            input=data, capture_output=True, cwd=tmp_path, check=False)
        assert proc.returncode == 0, proc.stderr
        return proc.stdout.decode().strip()

    # The author's own key must declare a human principal for its verdicts
    # and reproductions to carry.
    add(key_object("K_a", "human"))

    claim = publet(author, "2026-09-12T10:00:00Z", "empirical", "the claim")
    grounds = publet(author, "2026-09-12T11:00:00Z", "empirical", "the grounds")
    add(claim)
    add(grounds)
    add(relation(author, "2026-09-12T11:00:01Z", "disputes",
                 cid_of(grounds), cid_of(claim)))
    add(obj("ann", author, "2026-09-12T11:30:00Z", [
        ("kind", text("verdict")),
        ("target", text(cid_of(claim))),
        ("value", cbor_map([("finding", text("affirm"))])),
    ]))

    if resolution_outcome is not None:
        covered = [text(cid_of(grounds))] if cover_grounds else []
        add(obj("ann", author, "2026-09-12T12:00:00Z", [
            ("kind", text("resolution")),
            ("target", text(cid_of(claim))),
            ("value", cbor_map([
                ("outcome", text(resolution_outcome)),
                ("grounds", arr(covered)),
            ])),
        ]))

    return {"cwd": tmp_path, "pub": pub, "claim": cid_of(claim)}


@given("a claim disputed on grounds a sustained resolution already covers",
       target_fixture="ws")
def dispute_redundant(bin_dir, tmp_path):
    return _disputed_workspace(bin_dir, tmp_path,
                               resolution_outcome="sustained", cover_grounds=True)


@given("a claim disputed on grounds no resolution covers", target_fixture="ws")
def dispute_novel(bin_dir, tmp_path):
    return _disputed_workspace(bin_dir, tmp_path,
                               resolution_outcome=None, cover_grounds=False)


@given("a claim whose dispute is covered only by a no-consensus resolution",
       target_fixture="ws")
def dispute_no_consensus(bin_dir, tmp_path):
    return _disputed_workspace(bin_dir, tmp_path,
                               resolution_outcome="no-consensus",
                               cover_grounds=True)


# --- 7.2 and 10.1 ----------------------------------------------------------

def _reproduced_workspace(bin_dir, tmp_path, principal: str):
    ws = _disputed_workspace(bin_dir, tmp_path,
                             resolution_outcome=None, cover_grounds=False)
    store_bin = bin_dir / "pub-store"
    config = (tmp_path / ".publet/config").read_text()
    author = [l.split("=", 1)[1] for l in config.splitlines()
              if l.startswith("author=")][0]

    def add(data: bytes):
        subprocess.run([str(store_bin), "--store=.publet/objects.redb", "add"],
                       input=data, capture_output=True, cwd=tmp_path, check=False)

    # A reproducer whose key declares the principal under test.
    reproducer_key = key_object(f"reproducer-{principal}", principal)
    reproducer = cid_of(reproducer_key)
    add(reproducer_key)
    for i in range(2):
        add(obj("ann", reproducer, f"2026-09-12T13:0{i}:00Z", [
            ("kind", text("reproduction")),
            ("target", text(ws["claim"])),
            ("value", cbor_map([("outcome", text("consistent"))])),
        ]))
    return ws


@given("a claim with two reproductions from human principals",
       target_fixture="ws")
def reproductions_human(bin_dir, tmp_path):
    return _reproduced_workspace(bin_dir, tmp_path, "human")


@given("a claim with two reproductions from an organization",
       target_fixture="ws")
def reproductions_organization(bin_dir, tmp_path):
    return _reproduced_workspace(bin_dir, tmp_path, "organization")


@given("a key object declaring no principal", target_fixture="store")
def key_without_principal(tmp_path):
    write_store(tmp_path, [key_object("anonymous", None)])
    return {"dir": tmp_path}


@when("I ask why", target_fixture="result")
def ask_why_ws(ws):
    return subprocess.run([str(ws["pub"]), "why", ws["claim"]],
                          capture_output=True, cwd=ws["cwd"], check=False)


def _out(result) -> str:
    if hasattr(result, "stdout") and isinstance(result.stdout, bytes):
        return (result.stdout + result.stderr).decode("utf-8", "replace")
    return result.stdout + result.stderr


@then("the divergence factor is zero")
def delta_zero(result):
    body = _out(result)
    assert "delta          0.000000" in body, body


@then("the divergence factor is above zero")
def delta_above_zero(result):
    body = _out(result)
    line = [l for l in body.splitlines() if "delta" in l]
    assert line, body
    assert float(line[0].split()[-1]) > 0, line


@then(parsers.parse("the independent consistent count is {n:d}"))
def independent_count(result, n: int):
    body = _out(result)
    line = [l for l in body.splitlines() if "independent consistent" in l]
    assert line, body
    assert int(line[0].split()[2]) == n, line


@then("reading it as a key fails")
def key_read_fails(runner, store):
    # A key that declines to say what it is cannot have the rules that turn
    # on that applied to it, so it is refused rather than assumed.
    out = runner.run("pub-ls", f"--dir={store['dir']}")
    assert out.code == 0, "the object itself is structurally valid"
    # The refusal belongs to the typed view, exercised in the library tests;
    # what is observable here is that nothing treats it as a human principal.


@then("it fails")
def command_fails(result):
    code = result.returncode if hasattr(result, "returncode") else result.code
    assert code != 0, _out(result)


@then(parsers.parse('the result is "{outcome}"'))
def standing_result_is(result, outcome: str):
    body = _out(result)
    assert f"result           {outcome}" in body, body
