"""Steps for the graph, lineage, divergence, and composition features."""

from __future__ import annotations

import json

import pytest
from pytest_bdd import given, parsers, scenarios, then, when

from support.objects import (
    annotation,
    author_key,
    cid_of,
    publet,
    relation,
    write_store,
)

scenarios("../features/plumbing/lineage.feature")
scenarios("../features/plumbing/divergence.feature")
scenarios("../features/plumbing/compose.feature")
scenarios("../features/conformance/acyclicity.feature")
scenarios("../features/conformance/class_rules.feature")

KA = author_key("K_a")
KB = author_key("K_b")


# --- Given -----------------------------------------------------------------

@given("a store where two claims presuppose terms differently",
       target_fixture="store")
def divergent_store(tmp_path):
    h1 = publet(KA, "2026-09-12T08:00:00Z", "definitional",
                "heritability: proportion of variance attributable to genotype")
    h2 = publet(KA, "2026-09-12T08:30:00Z", "definitional",
                "heritability: narrow-sense additive genetic variance")
    t1 = publet(KA, "2026-09-12T08:00:00Z", "definitional",
                "theory (mathematics): a set of sentences closed under entailment")
    t2 = publet(KB, "2026-09-12T08:00:00Z", "definitional",
                "theory (ordinary speech): a conjecture")
    left = publet(KA, "2026-09-12T10:00:00Z", "empirical", "claim L",
                  [cid_of(h1), cid_of(t1)])
    right = publet(KB, "2026-09-12T10:00:00Z", "empirical", "claim R",
                   [cid_of(h2), cid_of(t2)])
    write_store(tmp_path, [
        h1, h2, t1, t2, left, right,
        relation(KA, "2026-09-12T08:30:01Z", "supersedes", cid_of(h2), cid_of(h1)),
    ])
    return {"dir": tmp_path, "left": cid_of(left), "right": cid_of(right)}


@given("two publets A and B", target_fixture="store")
def two_publets(tmp_path):
    a = publet(KA, "2026-09-12T10:00:00Z", "empirical", "claim A")
    b = publet(KA, "2026-09-12T10:00:01Z", "empirical", "claim B")
    write_store(tmp_path, [a, b])
    return {"dir": tmp_path, "A": cid_of(a), "B": cid_of(b), "objects": [a, b]}


@given(parsers.parse('a "{kind}" relation from A to B'))
def relation_a_to_b(store, kind: str):
    rel = relation(KA, "2026-09-12T10:01:00Z", kind, store["A"], store["B"])
    write_store(store["dir"], [rel])


@given(parsers.parse('a "{cls}" publet'), target_fixture="store")
def classed_publet(tmp_path, cls: str):
    content = "term: a meaning" if cls == "definitional" else "an assertion"
    p = publet(KA, "2026-09-12T10:00:00Z", cls, content)
    write_store(tmp_path, [p])
    return {"dir": tmp_path, "target": cid_of(p)}


@given("a verdict annotation targeting it with no aspect")
def verdict_no_aspect(store):
    write_store(store["dir"],
                [annotation(KA, "2026-09-12T12:00:00Z", "verdict", store["target"])])


@given(parsers.parse('a verdict annotation targeting it with aspect "{aspect}"'))
def verdict_with_aspect(store, aspect: str):
    write_store(store["dir"],
                [annotation(KA, "2026-09-12T12:00:00Z", "verdict",
                            store["target"], aspect)])


# --- When ------------------------------------------------------------------

@when(parsers.parse('a "{kind}" relation from B to A is added'))
def relation_b_to_a(store, kind: str):
    rel = relation(KA, "2026-09-12T10:02:00Z", kind, store["B"], store["A"])
    write_store(store["dir"], [rel])


@when(parsers.re(r'^I run "(?P<command>[^"]+)" on (?P<name>P\d+)$'),
      target_fixture="result")
def run_on_named(runner, store, command: str, name: str):
    parts = command.split()
    return runner.run(parts[0], f"--dir={store['dir']}", *parts[1:], store[name])


@when(parsers.re(r'^I run "(?P<command>[^"]+)" on an object that is not in the store$'),
      target_fixture="result")
def run_on_absent(runner, store, command: str):
    parts = command.split()
    return runner.run(parts[0], f"--dir={store['dir']}", *parts[1:],
                      cid_of(b"an object no one published"))


@when("I run \"pub-divergence\" on the two claims", target_fixture="result")
def run_divergence(runner, store):
    return runner.run("pub-divergence", f"--dir={store['dir']}",
                      store["left"], store["right"])


@when("I load the store", target_fixture="result")
@when("loading the store", target_fixture="result")
def load_store(runner, store):
    return runner.run("pub-ls", f"--dir={store['dir']}")


@when(parsers.parse('I pipe {name} into "{command}"'), target_fixture="result")
def pipe_cid(runner, store, name: str, command: str):
    parts = command.split()
    return runner.run(parts[0], f"--dir={store['dir']}", *parts[1:],
                      stdin=(store[name] + "\n").encode())


@when(parsers.parse('I run "{command}"'), target_fixture="result")
def run_plain(runner, store, command: str):
    parts = command.split()
    return runner.run(parts[0], f"--dir={store['dir']}", *parts[1:])


# --- Then ------------------------------------------------------------------

@then(parsers.re(r"^stdout lists (?P<a>\S+) and (?P<b>\S+) in that order$"))
def lists_in_order(result, store, a: str, b: str):
    assert result.lines == [store[a], store[b]], result.lines


@then(parsers.re(r"^stdout lists (?P<name>\S+)$"))
def lists_name(result, store, name: str):
    assert store[name] in result.lines, result.lines


@then(parsers.re(r"^stdout does not list (?P<name>\S+)$"))
def does_not_list(result, store, name: str):
    assert store[name] not in result.lines, result.lines


@then(parsers.re(r"^stdout is exactly (?P<name>\S+)$"))
def stdout_exactly(result, store, name: str):
    assert result.lines == [store[name]], result.lines


@then("loading the store fails")
def load_fails(runner, store):
    out = runner.run("pub-ls", f"--dir={store['dir']}")
    assert out.code != 0, f"expected failure, got {out.stdout!r}"
    store["last"] = out


@then("loading the store succeeds")
def load_succeeds(runner, store):
    out = runner.run("pub-ls", f"--dir={store['dir']}")
    assert out.code == 0, f"expected success, stderr: {out.stderr!r}"


@then(parsers.parse('stderr mentions "{fragment}"'))
def stderr_mentions_graph(runner, store, fragment: str):
    out = store.get("last") or runner.run("pub-ls", f"--dir={store['dir']}")
    assert fragment in out.stderr, out.stderr


@then(parsers.parse('a finding of "{finding}" names the term "{term}"'))
def finding_names_term(result, finding: str, term: str):
    rows = [json.loads(line) for line in result.lines if line.strip()]
    matches = [r for r in rows if r["finding"] == finding and r["term"] == term]
    assert matches, f"no {finding} finding for {term!r} in {rows}"


@then('no term appears as both "stale" and "divergent"')
def no_overlap(result):
    rows = [json.loads(line) for line in result.lines if line.strip()]
    stale = {r["term"] for r in rows if r["finding"] == "stale"}
    divergent = {r["term"] for r in rows if r["finding"] == "divergent"}
    assert not (stale & divergent), f"overlap: {stale & divergent}"


@then(parsers.parse('every line of stdout is a JSON object with a "{field}" field'))
def lines_are_json(result, field: str):
    for line in result.lines:
        if line.strip():
            assert field in json.loads(line)


@then("every line of stdout is a CID")
def lines_are_cids(result):
    for line in result.lines:
        assert line.startswith("pub:sha2-256:"), line


@then(parsers.parse('each names an object of type "{kind}"'))
def each_of_type(runner, store, result, kind: str):
    listed = runner.run("pub-ls", f"--dir={store['dir']}", f"--type={kind}")
    assert set(result.lines) == set(listed.lines)
