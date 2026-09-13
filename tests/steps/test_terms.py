"""Steps for term lookup and N-way partitioning.

Both exist because the corpus asks questions the pairwise tools do not
answer: which publets define this word, and how do several of them divide.
"""

from __future__ import annotations

import json

from pytest_bdd import given, parsers, scenarios, then, when

from support.objects import author_key, cid_of, publet, relation, write_store

scenarios("../features/plumbing/terms.feature")
scenarios("../features/plumbing/partition.feature")

KA = author_key("K_a")
KB = author_key("K_b")


@given("a store with competing definitions", target_fixture="store")
def competing_definitions(tmp_path):
    first = publet(KA, "2026-09-12T10:00:00Z", "definitional",
                   "merkle tree: a tree whose non-leaf nodes hash their children")
    rival = publet(KB, "2026-09-12T10:01:00Z", "definitional",
                   "merkle tree: a hash tree committing to many leaves")
    other = publet(KA, "2026-09-12T10:02:00Z", "definitional",
                   "content address: an identifier derived from an object's bytes")
    dispute = relation(KB, "2026-09-12T11:00:00Z", "disputes",
                       cid_of(rival), cid_of(first))
    write_store(tmp_path, [first, rival, other, dispute])
    return {"dir": tmp_path}


def _cohort_store(tmp_path, readings: int):
    """Publets presupposing one or two definitions of `cohort`."""
    enrolled = publet(KA, "2026-09-12T09:00:00Z", "definitional",
                      "cohort: the group enrolled in a single year")
    observed = publet(KA, "2026-09-12T09:01:00Z", "definitional",
                      "cohort: the group observed in a single year")
    objects = [enrolled, observed]
    cids = []
    for i in range(4):
        # The last one reads the term differently when two readings exist.
        dep = observed if (readings == 2 and i == 3) else enrolled
        made = publet(KA, f"2026-09-12T10:0{i}:00Z", "definitional",
                      f"rate {i}: measured per cohort", [cid_of(dep)])
        objects.append(made)
        cids.append(cid_of(made))
    write_store(tmp_path, objects)
    return {"dir": tmp_path, "cids": cids}


@given("four publets presupposing two definitions of one term",
       target_fixture="store")
def two_readings(tmp_path):
    return _cohort_store(tmp_path, readings=2)


@given("four publets presupposing one definition of a term",
       target_fixture="store")
def one_reading(tmp_path):
    return _cohort_store(tmp_path, readings=1)


@when(parsers.parse('I look up "{term}"'), target_fixture="result")
def look_up(runner, store, term: str):
    return runner.run("pub-term", f"--dir={store['dir']}", term)


@when("I list every term", target_fixture="result")
def list_terms(runner, store):
    return runner.run("pub-term", f"--dir={store['dir']}", "--list")


@when("I ask how they partition", target_fixture="result")
def ask_partition(runner, store):
    return runner.run("pub-divergence", f"--dir={store['dir']}", *store["cids"])


def _rows(result):
    return [json.loads(line) for line in result.stdout.splitlines() if line.strip()]


@then(parsers.parse("{count:d} definitions are reported"))
@then(parsers.parse("{count:d} definition is reported"))
def definitions_reported(result, count: int):
    assert result.code == 0, result.stderr
    assert len(_rows(result)) == count, result.stdout


@then("one of them is marked contested")
def one_contested(result):
    flags = [row["contested"] for row in _rows(result)]
    assert flags.count(True) == 1, flags


@then(parsers.parse('{count:d} groups are reported for "{term}"'))
def groups_reported(result, count: int, term: str):
    assert result.code == 0, result.stderr
    rows = [r for r in _rows(result) if r["term"] == term]
    assert len(rows) == 1, rows
    assert len(rows[0]["groups"]) == count, rows[0]


@then(parsers.parse("the majority group holds {count:d} publets"))
def majority_group(result, count: int):
    # Largest first, so the reading most publets share leads and the one
    # worth looking at sorts to the end.
    groups = _rows(result)[0]["groups"]
    assert len(groups[0]["publets"]) == count, groups


@then(parsers.parse("the outlier group holds {count:d}"))
def outlier_group(result, count: int):
    groups = _rows(result)[0]["groups"]
    assert len(groups[-1]["publets"]) == count, groups


@then("nothing is reported")
def nothing_reported(result):
    assert result.code == 0, result.stderr
    assert result.stdout.strip() == "", result.stdout
