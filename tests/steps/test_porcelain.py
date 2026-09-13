"""Steps for the porcelain features.

These drive `pub` from a real working directory, because a workspace is a
directory and the commands resolve it from where they are run. Nothing here
reaches into the store; what a user would see is what is asserted.
"""

from __future__ import annotations

import subprocess
from pathlib import Path

import pytest
from pytest_bdd import given, parsers, scenarios, then, when

from support.objects import author_key, cbor_map, cid_of, head, obj, text, uint

scenarios("../features/porcelain/init.feature")
scenarios("../features/porcelain/read.feature")
scenarios("../features/porcelain/why.feature")
scenarios("../features/porcelain/compose.feature")
scenarios("../features/scenarios/stale_basis.feature")
scenarios("../features/scenarios/privacy_modes.feature")

COHORT = (
    "In the 2031 cohort (n=4,182), daily supplementation with compound X "
    "reduced 12-month relapse incidence from 18.4% to 11.9%."
)


class Pub:
    """Run `pub` in a working directory, as a user would."""

    def __init__(self, bin_dir: Path, cwd: Path):
        self.bin = bin_dir / "pub"
        self.store_bin = bin_dir / "pub-store"
        self.cwd = cwd

    def run(self, *args: str):
        proc = subprocess.run(
            [str(self.bin), *args], capture_output=True, cwd=self.cwd, check=False
        )
        return proc

    def add(self, data: bytes) -> str:
        proc = subprocess.run(
            [str(self.store_bin), "--store=.publet/objects.redb", "add"],
            input=data, capture_output=True, cwd=self.cwd, check=False,
        )
        assert proc.returncode == 0, proc.stderr
        return proc.stdout.decode().strip()


def text_of(proc) -> str:
    return (proc.stdout + proc.stderr).decode("utf-8", "replace")


@given("an empty directory", target_fixture="pub")
def empty_dir(bin_dir: Path, tmp_path: Path):
    return Pub(bin_dir, tmp_path)


@given("a workspace", target_fixture="pub")
def workspace(bin_dir: Path, tmp_path: Path):
    p = Pub(bin_dir, tmp_path)
    assert p.run("init").returncode == 0
    return p


@given("a workspace containing the cohort claim", target_fixture="pub")
def workspace_with_claim(bin_dir: Path, tmp_path: Path):
    p = Pub(bin_dir, tmp_path)
    assert p.run("init").returncode == 0
    out = p.run(
        "compose", "--class=empirical", f"--content={COHORT}",
        "--scope=adults 40-65, single-centre, unblinded",
        f"--method={cid_of(b'the trial protocol')}",
    )
    assert out.returncode == 0, text_of(out)
    p.claim = out.stdout.decode().strip()
    return p


@given("fifty trusted keys affirming the claim")
def fifty_affirmations(pub):
    # The author's own key is the only trust root a fresh workspace has, so
    # the affirmations come from it: the point is that weight alone, however
    # large, does not reach `accepted` without replication.
    author = (Path(pub.cwd) / ".publet/config").read_text()
    author_cid = [l.split("=", 1)[1] for l in author.splitlines()
                  if l.startswith("author=")][0]
    value = cbor_map([("finding", text("affirm"))])
    for i in range(50):
        ann = obj("ann", author_cid, f"2026-09-12T12:{i:02d}:00Z", [
            ("kind", text("verdict")),
            ("target", text(pub.claim)),
            ("value", value),
        ])
        pub.add(ann)


def _author_of(pub) -> str:
    config = (Path(pub.cwd) / ".publet/config").read_text()
    return [l.split("=", 1)[1] for l in config.splitlines()
            if l.startswith("author=")][0]


def _arr(items):
    return head(4, len(items)) + b"".join(items)


def _publet(author, created, cls, content, depends=()):
    body = [
        ("class", text(cls)),
        ("lang", text("en")),
        ("content", text(content)),
        ("scope", cbor_map([("domain", text("unconditional"))])),
        ("depends", _arr([text(d) for d in depends])),
    ]
    # Section 5.5: an empirical claim must name a method others can execute.
    if cls == "empirical":
        body.append(("evidence", _arr([cbor_map([
            ("kind", text("publet")),
            ("role", text("method")),
            ("ref", text(cid_of(b"a protocol"))),
        ])])))
    return obj("publet", author, created, body)


def _relation(author, created, kind, frm, to):
    return obj("rel", author, created, [
        ("kind", text(kind)), ("from", text(frm)), ("to", text(to)),
    ])


@given("a workspace where a claim has been superseded twice",
       target_fixture="pub")
def superseded_twice(bin_dir: Path, tmp_path: Path):
    # Two authors each supersede the same claim. The lineage branches, which
    # is permitted; the proposal should say so rather than refuse.
    p = Pub(bin_dir, tmp_path)
    assert p.run("init").returncode == 0
    a = _author_of(p)

    original = _publet(a, "2026-09-12T10:00:00Z", "empirical", "the original claim")
    first = _publet(a, "2026-09-12T11:00:00Z", "empirical", "the first successor")
    second = _publet(a, "2026-09-12T12:00:00Z", "empirical", "the second successor")
    for data in (original, first, second):
        p.add(data)
    p.add(_relation(a, "2026-09-12T11:00:01Z", "supersedes",
                    cid_of(first), cid_of(original)))
    p.add(_relation(a, "2026-09-12T12:00:01Z", "supersedes",
                    cid_of(second), cid_of(original)))
    p.target = cid_of(second)
    return p


@given("a workspace where a claim depends on an outdated definition",
       target_fixture="pub")
def depends_on_outdated(bin_dir: Path, tmp_path: Path):
    p = Pub(bin_dir, tmp_path)
    assert p.run("init").returncode == 0
    a = _author_of(p)

    old = _publet(a, "2026-09-12T08:00:00Z", "definitional",
                  "relapse: a return of symptoms")
    new = _publet(a, "2026-09-12T09:00:00Z", "definitional",
                  "relapse: a return of symptoms meeting the stated threshold")
    claim = _publet(a, "2026-09-12T10:00:00Z", "empirical",
                    "relapse fell", [cid_of(old)])
    for data in (old, new, claim):
        p.add(data)
    p.add(_relation(a, "2026-09-12T09:00:01Z", "supersedes",
                    cid_of(new), cid_of(old)))
    p.target = cid_of(claim)
    return p


@given("a workspace where a dispute targets a resolved question",
       target_fixture="pub")
def dispute_already_resolved(bin_dir: Path, tmp_path: Path):
    p = Pub(bin_dir, tmp_path)
    assert p.run("init").returncode == 0
    a = _author_of(p)

    claim = _publet(a, "2026-09-12T10:00:00Z", "empirical", "the claim")
    grounds = _publet(a, "2026-09-12T11:00:00Z", "empirical", "the grounds")
    for data in (claim, grounds):
        p.add(data)
    p.add(_relation(a, "2026-09-12T11:00:01Z", "disputes",
                    cid_of(grounds), cid_of(claim)))
    p.add(obj("ann", a, "2026-09-12T11:30:00Z", [
        ("kind", text("resolution")),
        ("target", text(cid_of(claim))),
        ("value", cbor_map([("outcome", text("sustained"))])),
    ]))
    p.target = cid_of(grounds)
    return p


@when("I propose the second successor", target_fixture="result")
@when("I propose that claim", target_fixture="result")
@when("I propose the dispute", target_fixture="result")
def propose_target(pub):
    return pub.run("propose", pub.target)


@then("it reports that the lineage will branch")
def reports_branch(result):
    assert "lineage will branch" in text_of(result), text_of(result)


@then("it reports the definition has been superseded")
def reports_superseded_definition(result):
    body = text_of(result)
    assert "has been superseded by" in body, body


@then("it reports that a resolution already exists")
def reports_resolution(result):
    assert "resolutions already exist" in text_of(result), text_of(result)


@when(parsers.parse('I run "{command}"'), target_fixture="result")
def run_command(pub, command: str):
    return pub.run(*command.split()[1:])


@when(parsers.parse('I run "{command}" on any identifier'), target_fixture="result")
def run_on_identifier(pub, command: str):
    return pub.run(*command.split()[1:], cid_of(b"anything"))


@when("I read the claim", target_fixture="result")
def read_claim(pub):
    return pub.run("read", pub.claim)


@when("I ask why", target_fixture="result")
def ask_why(pub):
    return pub.run("why", pub.claim)


@when("I compose a claim with no scope", target_fixture="result")
def compose_no_scope(pub):
    return pub.run("compose", "--class=empirical", "--content=a claim",
                   f"--method={cid_of(b'a protocol')}")


@when("I compose a claim with no class", target_fixture="result")
def compose_no_class(pub):
    return pub.run("compose", "--content=a claim", "--scope=unconditional")


@when("I compose an empirical claim", target_fixture="result")
def compose_empirical(pub):
    return pub.run(
        "compose", "--class=empirical", "--content=the rate fell",
        "--scope=unconditional", f"--method={cid_of(b'a protocol')}",
    )


@when("I compose a claim joining two assertions", target_fixture="result")
def compose_compound(pub):
    return pub.run(
        "compose", "--class=empirical",
        "--content=the rate fell and the cohort was unblinded",
        "--scope=unconditional", f"--method={cid_of(b'a protocol')}",
    )


@then("a workspace exists")
def workspace_exists(pub):
    assert (Path(pub.cwd) / ".publet" / "objects.redb").exists()


@then("it reports the policy it created")
def reports_policy(result):
    assert "policy" in text_of(result)


@then("it says the policy trusts only this workspace")
def says_policy_is_narrow(result):
    assert "trusts only this workspace" in text_of(result)


@then(parsers.parse('it says to run "{command}"'))
def says_to_run(result, command: str):
    assert command in text_of(result)


@then("the content is shown")
def content_shown(result):
    assert "2031 cohort" in text_of(result)


@then("the scope is shown")
def scope_shown(result):
    body = text_of(result)
    assert "asserted under" in body
    assert "single-centre, unblinded" in body


@then(parsers.parse('the mode is reported as "{mode}"'))
def mode_reported(result, mode: str):
    assert f"[{mode}]" in text_of(result)


@then("the output says nothing was disclosed")
def nothing_disclosed(result):
    assert "nothing was disclosed" in text_of(result)


@then("the affirm, deny, abstain and active weights are shown")
def weights_shown(result):
    body = text_of(result)
    for field in ("affirm", "deny", "abstain", "active"):
        assert field in body, field


@then("the divergence factor and threshold are shown")
def delta_shown(result):
    body = text_of(result)
    assert "delta" in body and "threshold" in body


@then("the reproduction counts and the replication floor are shown")
def reproductions_shown(result):
    body = text_of(result)
    assert "independent consistent" in body
    assert "floor" in body


@then(parsers.parse('the result is "{outcome}"'))
def result_is(result, outcome: str):
    assert f"result           {outcome}" in text_of(result), text_of(result)


@then("the explanation says it is a fact about the viewpoint")
def explains_viewpoint(result):
    assert "fact about your viewpoint" in text_of(result)


@then("the explanation says no amount of agreement substitutes for replication")
def explains_dominance(result):
    assert "substitutes for one replication" in text_of(result)


@then("the output says the computation was local")
def computation_local(result):
    assert "locally" in text_of(result)


@then("it names the policy it used")
def names_policy(result):
    assert "policy pub:sha2-256:" in text_of(result)


@then("reading never reports a query mode for a held object")
def held_objects_are_local(pub):
    out = pub.run("read", pub.claim)
    assert "[local]" in text_of(out)
    assert "[query]" not in text_of(out)


@then("it fails")
def it_fails(result):
    assert result.returncode != 0, text_of(result)


@then("it succeeds")
def it_succeeds(result):
    assert result.returncode == 0, text_of(result)


@then("it says a scope is required")
def says_scope_required(result):
    assert "--scope is required" in text_of(result)


@then("it says a class is required")
def says_class_required(result):
    assert "--class is required" in text_of(result)


@then("it warns that endorsement alone cannot accept it")
def warns_about_replication(result):
    assert "endorsement alone" in text_of(result)


@then("it warns about joining two assertions")
def warns_compound(result):
    assert "two assertions" in text_of(result)
