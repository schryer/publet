"""Steps for the store features.

These drive `pub-store` and `pub-cat` as a node operator would, rather than
reaching into the database, so the declared-set rules are exercised at the
boundary where a real deployment would meet them.
"""

from __future__ import annotations

import json

from pytest_bdd import given, parsers, scenarios, then, when

from support.objects import author_key, cid_of, minimal_object, publet

scenarios("../features/scenarios/store.feature")
scenarios("../features/plumbing/cat.feature")


@given("an empty store", target_fixture="store_path")
def empty_store(tmp_path):
    return tmp_path / "objects.redb"


@given("two objects in the store", target_fixture="stored")
def two_objects(runner, store_path):
    first = minimal_object("publet")
    second = publet(
        author_key("K_a"), "2026-09-12T11:00:00Z", "empirical", "a second claim"
    )
    for data in (first, second):
        out = runner.run("pub-store", f"--store={store_path}", "add", stdin=data)
        assert out.code == 0, out.stderr
    return {"first": first, "second": second,
            "first_cid": cid_of(first), "second_cid": cid_of(second)}


@given("the first is declared as a domain member")
def declare_first(runner, store_path, stored):
    domain = cid_of(b"a declared domain")
    out = runner.run(
        "pub-store", f"--store={store_path}", "declare", domain,
        stdin=(stored["first_cid"] + "\n").encode(),
    )
    assert out.code == 0, out.stderr


@when("garbage is collected", target_fixture="result")
def collect(runner, store_path):
    return runner.run("pub-store", f"--store={store_path}", "gc")


@when("garbage is collected twice", target_fixture="result")
def collect_twice(runner, store_path):
    runner.run("pub-store", f"--store={store_path}", "gc")
    return runner.run("pub-store", f"--store={store_path}", "gc")


@when("the store is scanned", target_fixture="result")
def scan(runner, store_path):
    return runner.run("pub-store", f"--store={store_path}", "scan")


@when("the first object is read back", target_fixture="result")
def read_first(runner, store_path, stored):
    return runner.run("pub-cat", f"--store={store_path}", stored["first_cid"])


@when("the first identifier is piped in", target_fixture="result")
def pipe_identifier(runner, store_path, stored):
    return runner.run(
        "pub-cat", f"--store={store_path}",
        stdin=(stored["first_cid"] + "\n").encode(),
    )


@when("an object that was never stored is requested", target_fixture="result")
def read_absent(runner, store_path):
    return runner.run("pub-cat", f"--store={store_path}", cid_of(b"never stored"))


@when("two store commands are run against the same store at once",
      target_fixture="result")
def concurrent_commands(runner, store_path):
    # The database takes an exclusive file lock, so a pipeline whose two ends
    # both open it deadlocks. The message must say so.
    return runner.pipeline([
        f"pub-store --store={store_path} list",
        f"pub-store --store={store_path} list",
    ])


def _held(runner, store_path):
    out = runner.run("pub-store", f"--store={store_path}", "list")
    assert out.code == 0, out.stderr
    return out.lines


@then("the first object is still held")
def first_held(runner, store_path, stored):
    assert stored["first_cid"] in _held(runner, store_path)


@then("the second object is gone")
def second_gone(runner, store_path, stored):
    assert stored["second_cid"] not in _held(runner, store_path)


@then("the store is empty")
def store_empty(runner, store_path):
    assert _held(runner, store_path) == []


@then("the scan reports no findings")
def scan_clean(result):
    assert result.code == 0, result.stderr
    assert result.lines == []


@then("it verifies against the identifier it was stored under")
def verifies_against_identifier(runner, store_path, stored):
    out = runner.run(
        "pub-cat", f"--store={store_path}", stored["first_cid"]
    )
    assert out.code == 0
    verified = runner.run(
        "pub-verify", f"--cid={stored['first_cid']}", stdin=out.out
    )
    assert verified.code == 0, verified.stderr


@then("stdout is byte-identical to what was stored")
def stdout_matches_stored(result, stored):
    assert result.code == 0, result.stderr
    assert result.out == stored["first"]


@then("the failure explains that the store is already open")
def failure_explains_lock(result):
    assert result.code != 0
    assert "already open" in result.stderr, result.stderr
