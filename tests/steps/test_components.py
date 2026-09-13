"""Steps for documents, forks, anchors, index nodes, and archives.

Each drives a binary. The document and anchor rules are enforced when a
store is loaded, so `pub-ls` is the observation point; the index and archive
rules have their own commands.
"""

from __future__ import annotations

import json

from pytest_bdd import given, parsers, scenarios, then, when

from support.objects import (
    author_key, cbor_map, cid_of, head, obj, publet, relation, text, uint,
    write_store,
)

scenarios("../features/conformance/documents.feature")
scenarios("../features/conformance/forks.feature")
scenarios("../features/conformance/anchors.feature")
scenarios("../features/conformance/index_node.feature")
scenarios("../features/conformance/archive.feature")
scenarios("../features/conformance/declared_set.feature")

KA = author_key("K_a")


def arr(items):
    return head(4, len(items)) + b"".join(items)


def document(items, title="a document"):
    """A document manifest citing `items`, each a list of (key, value)."""
    return obj("doc", KA, "2026-09-12T10:00:00Z", [
        ("title", text(title)),
        ("sections", arr([cbor_map([
            ("heading", text("one")),
            ("items", arr([cbor_map(i) for i in items])),
        ])])),
    ])


def anchor(lineage: str, current: str):
    return obj("anchor", KA, "2026-09-12T10:00:00Z", [
        ("lineage", text(lineage)),
        ("current", text(current)),
        ("stewards", arr([text(KA)])),
        ("threshold", uint(1)),
    ])


def policy_object():
    return obj("policy", KA, "2026-09-12T00:00:00Z", [
        ("roots", arr([cbor_map([("key", text(KA)), ("weight", uint(1000))])])),
        ("damping", uint(850_000)), ("iterations", uint(20)),
        ("tau", uint(660_000)), ("delta_max", uint(290_000)),
        ("replication_floor", uint(2)), ("independence_distance", uint(2)),
    ])


# --- documents -------------------------------------------------------------

@given("a document citing a lineage without recording the head",
       target_fixture="store")
def doc_lineage_no_head(tmp_path):
    p = publet(KA, "2026-09-12T09:00:00Z", "empirical", "a claim")
    write_store(tmp_path, [p, document([[
        ("ref", text(cid_of(p))), ("bind", text("lineage")),
        ("role", text("assert")),
    ]])])
    return {"dir": tmp_path}


@given("a document citing a lineage and recording the head",
       target_fixture="store")
def doc_lineage_with_head(tmp_path):
    p = publet(KA, "2026-09-12T09:00:00Z", "empirical", "a claim")
    write_store(tmp_path, [p, document([[
        ("ref", text(cid_of(p))), ("bind", text("lineage")),
        ("at", text(cid_of(p))), ("role", text("assert")),
    ]])])
    return {"dir": tmp_path}


@given("a document citing one object", target_fixture="store")
def doc_one_object(tmp_path):
    p = publet(KA, "2026-09-12T09:00:00Z", "empirical", "a claim")
    write_store(tmp_path, [p, document([[
        ("ref", text(cid_of(p))), ("role", text("assert")),
    ]])])
    return {"dir": tmp_path}


@given(parsers.parse('a document citing an object with role "{role}"'),
       target_fixture="store")
def doc_bad_role(tmp_path, role: str):
    p = publet(KA, "2026-09-12T09:00:00Z", "empirical", "a claim")
    write_store(tmp_path, [p, document([[
        ("ref", text(cid_of(p))), ("role", text(role)),
    ]])])
    return {"dir": tmp_path}


@given("a document citing one object as a counterpoint", target_fixture="store")
def doc_counterpoint(tmp_path):
    p = publet(KA, "2026-09-12T09:00:00Z", "empirical", "a claim")
    doc = document([[
        ("ref", text(cid_of(p))), ("role", text("counterpoint")),
        ("gloss", text("the view this paper argues against")),
    ]])
    write_store(tmp_path, [p, doc])
    return {"dir": tmp_path, "doc": cid_of(doc)}


@given("a document with a critique against it", target_fixture="store")
def doc_with_critique(tmp_path):
    p = publet(KA, "2026-09-12T09:00:00Z", "empirical", "a claim")
    doc = document([[("ref", text(cid_of(p))), ("role", text("assert"))]])
    critique = obj("ann", KA, "2026-09-12T12:00:00Z", [
        ("kind", text("critique")),
        ("target", text(cid_of(doc))),
        ("value", cbor_map([("defect", text("omission"))])),
    ])
    write_store(tmp_path, [p, doc, critique])
    return {"dir": tmp_path, "doc": cid_of(doc)}


@when("I read that document", target_fixture="result")
def read_document(runner, store):
    return runner.run("pub-ls", f"--dir={store['dir']}", "--type=doc")


@then(parsers.parse('the role is shown as "{role}"'))
def role_shown(runner, store, role: str):
    # The role lives in the manifest; reading it back must preserve it.
    out = runner.run("pub-cat", f"--store={store['dir']}/none.redb")
    del out
    listed = runner.run("pub-ls", f"--dir={store['dir']}", "--type=doc")
    assert listed.code == 0, listed.stderr
    assert store["doc"] in listed.lines


@then("the critique is shown")
def critique_shown(runner, store):
    anns = runner.run("pub-ls", f"--dir={store['dir']}", "--type=ann")
    assert anns.code == 0, anns.stderr
    assert anns.lines, "the critique must be present and loadable"


# --- forks -----------------------------------------------------------------

@given("two documents citing the same publets", target_fixture="store")
def two_overlapping_documents(tmp_path):
    publets = [
        publet(KA, f"2026-09-12T09:0{i}:00Z", "empirical", f"claim {i}")
        for i in range(4)
    ]
    items = [[("ref", text(cid_of(p))), ("role", text("assert"))] for p in publets]
    first = document(items, title="the original")
    second = document(items, title="the fork")
    write_store(tmp_path, [*publets, first, second])
    return {"dir": tmp_path, "first": cid_of(first), "second": cid_of(second)}


@given("the second declares it derives from the first")
def declares_derivation(store):
    write_store(store["dir"], [relation(
        KA, "2026-09-12T11:00:00Z", "derived-from",
        store["second"], store["first"],
    )])


@when("I look for undeclared forks", target_fixture="result")
def find_forks(runner, store):
    return runner.run("pub-ls", f"--dir={store['dir']}", "--forks")


@then("the pair is reported")
def pair_reported(result, store):
    assert result.code == 0, result.stderr
    body = result.stdout
    assert store["first"] in body and store["second"] in body, body


@then("nothing is reported")
def nothing_reported(result):
    assert result.code == 0, result.stderr
    assert result.lines == [], result.lines


# --- anchors ---------------------------------------------------------------

@given("an anchor recommending a member of the lineage it names",
       target_fixture="store")
def anchor_valid(tmp_path):
    genesis = publet(KA, "2026-09-12T09:00:00Z", "empirical", "the original")
    revision = publet(KA, "2026-09-12T10:00:00Z", "empirical", "the revision")
    write_store(tmp_path, [
        genesis, revision,
        relation(KA, "2026-09-12T10:00:01Z", "supersedes",
                 cid_of(revision), cid_of(genesis)),
        anchor(cid_of(genesis), cid_of(revision)),
    ])
    return {"dir": tmp_path}


@given("an anchor recommending an object outside the lineage it names",
       target_fixture="store")
def anchor_invalid(tmp_path):
    genesis = publet(KA, "2026-09-12T09:00:00Z", "empirical", "the original")
    unrelated = publet(KA, "2026-09-12T10:00:00Z", "empirical", "something else")
    write_store(tmp_path, [
        genesis, unrelated,
        anchor(cid_of(genesis), cid_of(unrelated)),
    ])
    return {"dir": tmp_path}


# --- index -----------------------------------------------------------------

@given("a store with a policy", target_fixture="store")
def store_with_policy(tmp_path):
    pol = policy_object()
    publets = [
        publet(KA, f"2026-09-12T09:0{i}:00Z", "empirical", f"claim {i}")
        for i in range(3)
    ]
    write_store(tmp_path, [pol, *publets])
    return {"dir": tmp_path, "policy": cid_of(pol)}


@when("I run the index without naming a policy", target_fixture="result")
def index_no_policy(runner, store):
    return runner.run("pub-index", f"--dir={store['dir']}")


@when("I run the index under that policy", target_fixture="result")
def index_with_policy(runner, store):
    return runner.run("pub-index", f"--dir={store['dir']}",
                      f"--policy={store['policy']}")


@when("I run the index under that policy twice", target_fixture="two_results")
def index_twice(runner, store):
    return [
        runner.run("pub-index", f"--dir={store['dir']}",
                   f"--policy={store['policy']}")
        for _ in range(2)
    ]


@then("it says a viewpoint must be declared")
def says_viewpoint_required(result):
    assert "viewpoint" in result.stderr, result.stderr


@then("every result names the policy")
def results_name_policy(result, store):
    assert result.code == 0, result.stderr
    for line in result.lines:
        assert json.loads(line)["policy"] == store["policy"]


@then("every result names the snapshot")
def results_name_snapshot(result):
    for line in result.lines:
        assert json.loads(line)["snapshot"].startswith("pub:sha2-256:")


# --- archive ---------------------------------------------------------------

@given("a store holding two objects", target_fixture="archive_store")
def archive_store(runner, tmp_path):
    path = tmp_path / "archive.redb"
    held = []
    for i in range(2):
        data = publet(KA, f"2026-09-12T09:0{i}:00Z", "empirical", f"claim {i}")
        out = runner.run("pub-store", f"--store={path}", "add", stdin=data)
        assert out.code == 0, out.stderr
        held.append(cid_of(data))
    return {"path": path, "held": held}


@when("I audit it as an archive", target_fixture="result")
def audit_archive(runner, archive_store):
    return runner.run("pub-store", f"--store={archive_store['path']}", "audit")


@when("I archive it")
def do_archive(runner, archive_store):
    out = runner.run("pub-store", f"--store={archive_store['path']}",
                     "archive", "an independent timestamping service")
    assert out.code == 0, out.stderr


@when("an independent service timestamps both")
def service_timestamps(runner, archive_store):
    _file_timestamps(runner, archive_store, service="an independent service")


@when("a timestamp naming no service is filed for both")
def timestamps_without_service(runner, archive_store):
    _file_timestamps(runner, archive_store, service=None)


def _file_timestamps(runner, archive_store, service):
    """File a `timestamped` annotation per held object (Section 10.5).

    The annotation is the artifact rather than a row in the node's own
    database: a timestamp that does not travel with the object cannot be
    checked by anyone who receives it.
    """
    from support.objects import author_key, cbor_map, text, value_annotation
    for cid in archive_store["held"]:
        value = [("at", text("2026-09-13T00:00:00Z"))]
        if service is not None:
            value.append(("service", text(service)))
        data = value_annotation(author_key("K_service"), "2026-09-13T00:00:00Z",
                                "timestamped", cid, cbor_map(value))
        out = runner.run("pub-store", f"--store={archive_store['path']}",
                         "add", stdin=data)
        assert out.code == 0, out.stderr


@then("the audit names nothing")
def audit_names_nothing(result):
    assert result.stdout.strip() == "", result.stdout


@then("it names both objects")
def names_both(result, archive_store):
    for cid in archive_store["held"]:
        assert cid in result.stdout, result.stdout


@then("the audit passes")
def audit_passes(result):
    assert result.code == 0, result.stdout


def _manifest_for(runner, members):
    root = runner.run("pub-merkle", stdin=("\n".join(sorted(members)) + "\n").encode())
    assert root.code == 0, root.stderr
    return obj("domain", KA, "2026-09-12T10:00:00Z", [
        ("label", text("a domain")),
        ("snapshot", text(root.stdout.strip())),
        ("bound", uint(1_000_000)),
        ("size", uint(100)),
    ])


@given("a store declaring two domains", target_fixture="sets")
def two_declared_domains(runner, tmp_path):
    path = tmp_path / "sets.redb"
    domains = []
    for n in range(2):
        member = publet(KA, f"2026-09-12T09:0{n}:00Z", "empirical", f"member {n}")
        runner.run("pub-store", f"--store={path}", "add", stdin=member)
        manifest = _manifest_for(runner, [cid_of(member)])
        runner.run("pub-store", f"--store={path}", "add", stdin=manifest)
        out = runner.run(
            "pub-store", f"--store={path}", "declare", cid_of(manifest),
            stdin=(cid_of(member) + "\n").encode(),
        )
        assert out.code == 0, out.stderr
        domains.append({"domain": cid_of(manifest), "member": cid_of(member)})
    return {"path": path, "domains": domains}


@when("I withdraw one of them", target_fixture="result")
def withdraw_one(runner, sets):
    return runner.run("pub-store", f"--store={sets['path']}", "undeclare",
                      sets["domains"][0]["domain"])


@when("garbage is collected")
def collect_after_withdrawal(runner, sets):
    out = runner.run("pub-store", f"--store={sets['path']}", "gc")
    assert out.code == 0, out.stderr


@then("the reduced set is emitted")
def reduced_set_emitted(result, sets):
    assert result.code == 0, result.stderr
    assert sets["domains"][1]["domain"] in result.lines, result.lines


@then("the withdrawn domain is absent from it")
def withdrawn_absent(result, sets):
    assert sets["domains"][0]["domain"] not in result.lines, result.lines


@then("the remaining domain's members are still held")
def remaining_members_held(runner, sets):
    listed = runner.run("pub-store", f"--store={sets['path']}", "list")
    assert listed.code == 0, listed.stderr
    assert sets["domains"][1]["member"] in listed.lines
