"""Steps for the network features.

These start a real server and drive a real client, because the properties
under test -- what crosses the wire, and whether bytes verify on arrival --
are not observable from inside the process.
"""

from __future__ import annotations

import json
import socket
import subprocess
import time
from pathlib import Path

import pytest
from pytest_bdd import given, parsers, scenarios, then, when

from support.objects import author_key, cbor_map, cid_of, head, obj, publet, text, uint

scenarios("../features/conformance/no_have_want.feature")
scenarios("../features/scenarios/two_node_sync.feature")

KA = author_key("K_a")


def arr(items):
    return head(4, len(items)) + b"".join(items)


def free_port() -> int:
    with socket.socket() as s:
        s.bind(("127.0.0.1", 0))
        return s.getsockname()[1]


@given("a node serving a domain of five objects", target_fixture="node")
def serving_node(runner, bin_dir: Path, tmp_path: Path):
    store = tmp_path / "source.redb"

    members = []
    objects = []
    for i in range(5):
        data = publet(KA, f"2026-09-12T10:0{i}:00Z", "empirical", f"claim {i}")
        objects.append(data)
        members.append(cid_of(data))
    members.sort()

    root = runner.run("pub-merkle", stdin=("\n".join(members) + "\n").encode())
    assert root.code == 0, root.stderr
    manifest = obj("domain", KA, "2026-09-12T10:00:00Z", [
        ("label", text("physics")),
        ("snapshot", text(root.stdout.strip())),
        ("bound", uint(1_000_000)),
        ("size", uint(500)),
    ])

    for data in [*objects, manifest]:
        assert runner.run("pub-store", f"--store={store}", "add", stdin=data).code == 0
    domain = cid_of(manifest)
    assert runner.run(
        "pub-store", f"--store={store}", "declare", domain,
        stdin=("\n".join(members) + "\n").encode(),
    ).code == 0

    running = []
    for index, member in enumerate(members, start=1):
        running.append(member)
        gen_root = runner.run(
            "pub-merkle", stdin=("\n".join(sorted(running)) + "\n").encode()
        )
        record = obj("generation", KA, "2026-09-12T10:00:00Z", [
            ("domain", text(domain)),
            ("index", uint(index)),
            ("parent", text(cid_of(b"parent"))),
            ("snapshot", text(gen_root.stdout.strip())),
            ("added", arr([text(member)])),
            ("removed", arr([])),
        ])
        assert runner.run(
            "pub-store", f"--store={store}", "generation", stdin=record
        ).code == 0

    port = free_port()
    proc = subprocess.Popen(
        [str(bin_dir / "pub-serve"), f"--store={store}", f"--bind=127.0.0.1:{port}"],
        stdout=subprocess.PIPE, stderr=subprocess.PIPE,
    )
    base = f"http://127.0.0.1:{port}"
    for _ in range(100):
        try:
            with socket.create_connection(("127.0.0.1", port), timeout=0.1):
                break
        except OSError:
            time.sleep(0.05)
    else:
        proc.kill()
        pytest.fail("server did not start")

    yield {"base": base, "domain": domain, "members": members,
           "replica": tmp_path / "replica.redb", "proc": proc}
    proc.kill()
    proc.wait(timeout=5)


@when("a client synchronizes from generation 0", target_fixture="result")
def sync_from_zero(runner, node):
    return runner.run(
        "pub-sync", f"--peer={node['base']}", f"--domain={node['domain']}",
        f"--store={node['replica']}", "--from=0", "--to=5",
    )


@when("the declared set is requested", target_fixture="result")
def request_set(runner, node):
    return runner.pipeline([f"curl -sS {node['base']}/pub/v1/set"])


@when("the first member is fetched by identifier", target_fixture="result")
def fetch_first(runner, node):
    return runner.pipeline(
        [f"curl -sS {node['base']}/pub/v1/object/{node['members'][0]}"]
    )


@when("an object that was never published is fetched", target_fixture="result")
def fetch_absent(runner, node):
    missing = cid_of(b"never published")
    return runner.pipeline(
        [f"curl -sS -o /dev/null -w '%{{http_code}}' {node['base']}/pub/v1/object/{missing}"]
    )


@when("the checkpoints are requested", target_fixture="result")
def request_checkpoints(runner, node):
    return runner.pipeline(
        [f"curl -sS {node['base']}/pub/v1/domain/{node['domain']}/checkpoints"]
    )


@given("the served route table", target_fixture="routes")
def route_table():
    source = Path(__file__).resolve().parents[2] / "crates/publet-net/src/server.rs"
    return [
        line.strip() for line in source.read_text().splitlines()
        if line.strip().startswith(".route(")
    ]


@then("the requests carry the domain identifier")
def requests_carry_domain(result, node):
    assert result.code == 0, result.stderr
    assert node["domain"] in result.stdout


@then("no request carries any other identifier")
def no_other_identifiers(result, node):
    # The client is the only thing that could leak holdings, and the sync
    # path sends a domain and two integers.
    payload = json.loads(result.stdout)
    assert payload["domain"] == node["domain"]
    assert set(payload) == {"domain", "from", "to", "stored"}


@then("only one route accepts a body")
def one_accepting_route(routes):
    assert sum(1 for r in routes if "post(" in r) == 1, routes


@then("that route takes a single object, not a list")
def accepting_route_is_singular(routes):
    accepting = [r for r in routes if "post(" in r]
    assert accepting[0].count("/pub/v1/object") == 1, accepting


@then(parsers.parse("the replica holds {n:d} objects"))
def replica_holds(runner, node, n: int):
    listed = runner.run("pub-store", f"--store={node['replica']}", "list")
    assert listed.code == 0, listed.stderr
    assert len(listed.lines) == n, listed.lines


@then("every stored object verifies against its identifier")
def replica_scan_clean(runner, node):
    scanned = runner.run("pub-store", f"--store={node['replica']}", "scan")
    assert scanned.code == 0, scanned.stdout


@then("it names the served domain")
def set_names_domain(result, node):
    assert node["domain"] in result.stdout


@then("it verifies against the identifier requested")
def fetched_verifies(runner, node, result):
    verified = runner.run(
        "pub-verify", f"--cid={node['members'][0]}", stdin=result.out
    )
    assert verified.code == 0, verified.stderr


@then("the request fails with a not-found status")
def fetch_is_not_found(result):
    assert result.stdout.strip() == "404", result.stdout


@then("they are exponentially spaced below the head")
def checkpoints_spaced(result):
    points = [int(line) for line in result.lines if line.strip()]
    assert points, result.stdout
    gaps = {5 - p for p in points}
    assert gaps <= {1, 2, 4, 8, 16}, gaps
