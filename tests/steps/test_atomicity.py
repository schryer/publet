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
