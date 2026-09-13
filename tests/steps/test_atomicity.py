"""Steps for Section 5.7: structural findings, and the refusal to merge.

The findings are advisory, so every scenario here checks two things at
once: that the linter says something, and that the store still holds a
valid publet after it has said it.
"""

from __future__ import annotations

import json

from pytest_bdd import given, parsers, scenarios, then, when

from support.objects import (
    author_key, cid_of, publet, relation, write_store,
)

scenarios("../features/conformance/atomicity.feature")

KA = author_key("K_a")
KB = author_key("K_b")


@given("a publet joining two assertions with a conjunction",
       target_fixture="store")
def compound(tmp_path):
    write_store(tmp_path, [
        publet(KA, "2026-09-12T10:00:00Z", "empirical",
               "the rate fell and the cohort grew")
    ])
    return {"dir": tmp_path}


@given(parsers.parse('a "{cls}" publet joining two clauses'),
       target_fixture="store")
def compound_of_class(tmp_path, cls: str):
    # One sentence, two clauses joined by "and". Read as an assertion it is
    # two claims; read as a method it is two steps, and Section 5.5 obliges
    # the whole method to be one publet.
    content = ("the buffer is prepared and the sample is added"
               if cls != "definitional"
               else "buffer: a solution prepared and then added to")
    write_store(tmp_path, [
        publet(KA, "2026-09-12T10:00:00Z", cls, content)
    ])
    return {"dir": tmp_path}


@given(parsers.parse('a "{cls}" publet opening with a pronoun'),
       target_fixture="store")
def pronoun_of_class(tmp_path, cls: str):
    write_store(tmp_path, [
        publet(KA, "2026-09-12T10:00:00Z", cls,
               "it is then added to the buffer at this point")
    ])
    return {"dir": tmp_path}


@given("a publet opening with a pronoun", target_fixture="store")
def pronoun(tmp_path):
    write_store(tmp_path, [
        publet(KA, "2026-09-12T10:00:00Z", "empirical",
               "it fell in the second year")
    ])
    return {"dir": tmp_path}


def _disputed_term_store(tmp_path, declare: bool):
    """A definitional publet under dispute, plus a publet using its term.

    A term nobody disputes needs no declaration. What makes the declaration
    worth asking for is that someone has said the definition is wrong, so
    the dispute relation is the thing that turns the term contested.
    """
    term = publet(KA, "2026-09-12T09:00:00Z", "definitional",
                  "cohort: the group enrolled in a single year")
    rival = publet(KB, "2026-09-12T09:30:00Z", "definitional",
                   "cohort: the group observed in a single year")
    dispute = relation(KB, "2026-09-12T09:40:00Z", "disputes",
                       cid_of(rival), cid_of(term))
    depends = [cid_of(term)] if declare else None
    user = publet(KA, "2026-09-12T10:00:00Z", "empirical",
                  "the cohort was measured twice", depends)
    write_store(tmp_path, [term, rival, dispute, user])
    return {"dir": tmp_path, "target": cid_of(user)}


@given("a publet using a disputed term it does not declare",
       target_fixture="store")
def undeclared_term(tmp_path):
    return _disputed_term_store(tmp_path, declare=False)


@given("a publet using a disputed term it declares", target_fixture="store")
def declared_term(tmp_path):
    return _disputed_term_store(tmp_path, declare=True)


@given("two publets whose wording differs by one word", target_fixture="store")
def near_identical(tmp_path):
    write_store(tmp_path, [
        publet(KA, "2026-09-12T10:00:00Z", "empirical",
               "the rate fell in the second year"),
        publet(KB, "2026-09-12T10:00:01Z", "empirical",
               "the rate rose in the second year"),
    ])
    return {"dir": tmp_path}


@given("two publets with identical wording from different authors",
       target_fixture="store")
def identical_wording(tmp_path):
    # Same sentence, two signatures. They are two claims, because a claim is
    # a person standing behind a sentence and two people are standing here.
    content = "the rate fell in the second year"
    write_store(tmp_path, [
        publet(KA, "2026-09-12T10:00:00Z", "empirical", content),
        publet(KB, "2026-09-12T10:00:00Z", "empirical", content),
    ])
    return {"dir": tmp_path}


@given("a method naming sub-procedures that name further ones",
       target_fixture="store")
def decomposed_method(tmp_path):
    # `depends` is acyclic, so a decomposition necessarily bottoms out at
    # base steps. Section 5.5 asks for exactly one identifier naming the
    # method; that is a requirement on the citation, not on the method
    # being a leaf.
    base = publet(KA, "2026-09-12T09:00:00Z", "procedural",
                  "prepare the buffer and bring it to pH 7.4")
    warm = publet(KA, "2026-09-12T09:01:00Z", "procedural",
                  "warm the sample to 37C and hold it there")
    mid = publet(KA, "2026-09-12T09:02:00Z", "procedural",
                 "condition the sample", [cid_of(base), cid_of(warm)])
    top = publet(KA, "2026-09-12T09:03:00Z", "procedural",
                 "run the assay: condition the sample and read absorbance",
                 [cid_of(mid)])
    write_store(tmp_path, [base, warm, mid, top])
    return {"dir": tmp_path, "method": cid_of(top),
            "steps": {cid_of(base), cid_of(warm), cid_of(mid)}}


@when("I ask for the dependency closure of the method", target_fixture="result")
def method_closure(runner, store):
    return runner.run("pub-closure", f"--dir={store['dir']}", store["method"])


@then("every sub-procedure is reached")
def every_step_reached(result, store):
    assert result.code == 0, result.stderr
    assert set(result.stdout.split()) == store["steps"], result.stdout


@then("no finding is reported for any of them")
def no_finding_for_steps(runner, store):
    listed = runner.run("pub-ls", f"--dir={store['dir']}", "--type=publet")
    assert listed.code == 0, listed.stderr
    out = runner.run("pub-lint", f"--dir={store['dir']}", *listed.stdout.split())
    assert out.stdout.strip() == "", out.stdout
    assert out.code == 0, out.stderr


@when("I lint the store", target_fixture="result")
def lint(runner, store):
    listed = runner.run("pub-ls", f"--dir={store['dir']}", "--type=publet")
    assert listed.code == 0, listed.stderr
    cids = [c for c in listed.stdout.split() if c]
    return runner.run("pub-lint", f"--dir={store['dir']}", *cids)


@when("I list the store", target_fixture="result")
def list_store(runner, store):
    return runner.run("pub-ls", f"--dir={store['dir']}", "--type=publet")


@then(parsers.parse('a "{test}" finding is reported'))
@then(parsers.parse('an "{test}" finding is reported'))
def finding_reported(result, test: str):
    fired = [json.loads(line)["test"]
             for line in result.stdout.splitlines() if line.strip()]
    assert test in fired, f"{test!r} not among {fired!r}"
    assert result.code == 1, "a finding must set the findings exit code"


@then("no finding is reported")
def no_finding(result):
    assert result.stdout.strip() == "", result.stdout
    assert result.code == 0, result.stderr


@then(parsers.parse("{count:d} publets are listed"))
def publets_listed(result, count: int):
    assert result.code == 0, result.stderr
    lines = [c for c in result.stdout.split() if c]
    assert len(lines) == count, f"expected {count}, got {lines!r}"
    assert len(set(lines)) == count, f"identifiers repeat: {lines!r}"
