"""Steps for data payloads, sources, blobs, views, `pub doc`, and `pub revise`.

Gathering and rendering are separate acts. What is gathered -- values,
where they were read from, why they were assembled that way -- is signed
and stored; how they are shown is a view in the document that shows them,
and the showing itself is never stored. These scenarios drive the porcelain
a gather step would drive, and read results back the way a reader would.
"""

from __future__ import annotations

import json

from pytest_bdd import given, parsers, scenarios, then, when

from support.objects import author_key, cbor_map, cid_of, head, obj, text, write_store

scenarios("../features/conformance/data.feature")
scenarios("../features/conformance/sources.feature")
scenarios("../features/conformance/blobs.feature")
scenarios("../features/porcelain/doc.feature")
scenarios("../features/porcelain/revise.feature")

CONTENT = "Planned P0 spend by item, Oct 2026 to Mar 2027."
SCOPE = "Sustenaut P0 plan as approved for the first phase"
BUDGET = {
    "columns": [{"name": "item"}, {"name": "spend", "unit": "EUR"}],
    "rows": [["seed trays", "120.00"], ["grow lights", "480.00"]],
}
REPO = "https://github.com/schryer/business-plan"


def _held(runner) -> set[str]:
    out = runner.run("pub-store", "--store=.publet/objects.redb", "list")
    assert out.code == 0, out.stderr
    return set(out.stdout.split())


def _write(runner, name: str, content) -> str:
    path = runner.cwd / name
    if isinstance(content, (dict, list)):
        content = json.dumps(content)
    path.write_text(content)
    return str(path)


def _compose_budget(runner, ctx, data, *extra: str):
    ctx.setdefault("before", _held(runner))
    args = ["compose", "--class=normative", f"--content={CONTENT}", f"--scope={SCOPE}"]
    if data is not None:
        args.append(f"--data={_write(runner, 'data.json', data)}")
    result = runner.run("pub", *args, *extra)
    if result.code == 0:
        ctx["claim"] = result.stdout.split()[0]
    return result


def _read(runner, cid: str) -> str:
    out = runner.run("pub", "read", cid)
    assert out.code == 0, out.stderr
    return out.stdout


# --- givens ------------------------------------------------------------------


@given("a workspace", target_fixture="ctx")
def workspace(runner):
    assert runner.run("pub", "init").code == 0
    return {}


@given("a workspace with a budget claim", target_fixture="ctx")
def workspace_with_budget(runner):
    assert runner.run("pub", "init").code == 0
    ctx: dict = {}
    out = _compose_budget(runner, ctx, BUDGET, f"--source={REPO}",
                          "--source-revision=abc123",
                          "--source-locator=bizplan/plan/p0.py")
    assert out.code == 0, out.stderr
    ctx["budget"] = ctx.pop("claim")
    ctx.pop("before")
    return ctx


@given("a workspace with a normative claim about grouping", target_fixture="ctx")
def workspace_with_reason(runner):
    assert runner.run("pub", "init").code == 0
    out = runner.run(
        "pub", "compose", "--class=normative", "--scope=unconditional",
        "--content=Budgets are grouped by phase because each phase is approved separately.",
    )
    assert out.code == 0, out.stderr
    return {"reason": out.stdout.strip()}


@given(parsers.parse('a file "{name}" containing "{content}"'))
def a_file(runner, name: str, content: str):
    _write(runner, name, content)


@given(parsers.parse('a claim whose data is the file "{name}" as "{media}"'))
def claim_with_file(runner, ctx, name: str, media: str):
    out = _compose_budget(runner, ctx, {"media": media, "file": name})
    assert out.code == 0, out.stderr
    ctx.pop("before")


def _prose(author: str, depends: list[str], evidence: list[bytes]) -> bytes:
    return obj("claim.prose", author, "2026-09-12T10:00:00Z", [
        ("class", text("normative")),
        ("lang", text("en")),
        ("content", text(CONTENT)),
        ("scope", cbor_map([("domain", text("unconditional")),
                            ("conditions", head(4, 0))])),
        ("depends", head(4, len(depends)) + b"".join(text(d) for d in depends)),
        ("evidence", head(4, len(evidence)) + b"".join(evidence)),
    ])


def _store_with_source(tmp_path, listed: bool):
    ka = author_key("K_a")
    reason = obj("claim.prose", ka, "2026-09-12T09:00:00Z", [
        ("class", text("normative")),
        ("lang", text("en")),
        ("content", text("Budgets are grouped by phase.")),
        ("scope", cbor_map([("domain", text("unconditional")),
                            ("conditions", head(4, 0))])),
        ("depends", head(4, 0)),
    ])
    source = cbor_map([
        ("kind", text("claim")),
        ("role", text("source")),
        ("ref", text(cid_of(reason))),
    ])
    budget = _prose(ka, [cid_of(reason)] if listed else [], [source])
    write_store(tmp_path, [reason, budget])
    return {"dir": tmp_path}


@given("a store holding a claim whose source claim is not in depends",
       target_fixture="store")
def store_source_unlisted(tmp_path):
    return _store_with_source(tmp_path, listed=False)


@given("a store holding a claim whose source claim is in depends",
       target_fixture="store")
def store_source_listed(tmp_path):
    return _store_with_source(tmp_path, listed=True)


def _manifest(budget: str, view=None, item_extra=None) -> dict:
    item = {"ref": budget, "role": "assert",
            "gloss": "Planned spend for the first phase."}
    if view is not None:
        item["view"] = view
    if item_extra:
        item.update(item_extra)
    return {"title": "Sustenaut Business Plan",
            "sections": [{"heading": "Budget", "items": [item]}]}


def _build_doc(runner, ctx, manifest: dict, name: str = "doc.json"):
    ctx.setdefault("before", _held(runner))
    result = runner.run("pub", "doc", _write(runner, name, manifest))
    if result.code == 0:
        ctx.setdefault("docs", []).append(result.stdout.strip())
    return result


@given(parsers.parse('a document viewing the budget as "{renderer}"'))
def a_document(runner, ctx, renderer: str):
    out = _build_doc(runner, ctx, _manifest(ctx["budget"], {"renderer": renderer}))
    assert out.code == 0, out.stderr
    ctx["doc"] = out.stdout.strip()
    ctx.pop("before")


# --- whens -------------------------------------------------------------------


@when("I compose a budget claim with the data", target_fixture="result")
def compose_with_data(runner, ctx, docstring: str):
    return _compose_budget(runner, ctx, docstring)


@when(parsers.parse('I compose a budget claim read from "{ref}" at revision '
                    '"{revision}" in "{locator}"'), target_fixture="result")
def compose_from_file(runner, ctx, ref: str, revision: str, locator: str):
    return _compose_budget(runner, ctx, BUDGET, f"--source={ref}",
                           f"--source-revision={revision}",
                           f"--source-locator={locator}")


@when(parsers.parse('I compose a budget claim read from table "{table}" by '
                    'query "{query}"'), target_fixture="result")
def compose_from_table(runner, ctx, table: str, query: str):
    return _compose_budget(runner, ctx, BUDGET, f"--source={table}",
                           f"--source-query={query}")


@when("I compose a budget claim read from that claim", target_fixture="result")
def compose_from_claim(runner, ctx):
    return _compose_budget(runner, ctx, BUDGET, f"--source={ctx['reason']}")


@when("I compose a budget claim read from an identifier nothing here holds",
      target_fixture="result")
def compose_from_nothing(runner, ctx):
    return _compose_budget(runner, ctx, BUDGET, f"--source={cid_of(b'elsewhere')}")


@when(parsers.parse('I compose a claim whose data is the file "{name}" as "{media}"'),
      target_fixture="result")
def compose_with_file(runner, ctx, name: str, media: str):
    ctx["blob"] = cid_of((runner.cwd / name).read_bytes())
    return _compose_budget(runner, ctx, {"media": media, "file": name})


@when("I compose a claim whose data cites a blob nothing here holds",
      target_fixture="result")
def compose_with_missing_blob(runner, ctx):
    return _compose_budget(runner, ctx, {"media": "application/pdf",
                                         "ref": cid_of(b"elsewhere"), "size": 9})


@when(parsers.parse("I compose a claim citing that blob with size {size:d}"),
      target_fixture="result")
def compose_with_wrong_size(runner, ctx, size: int):
    blob = cid_of((runner.cwd / "plan.pdf").read_bytes())
    return _compose_budget(runner, ctx, {"media": "application/pdf",
                                         "ref": blob, "size": size})


@when(parsers.parse('I build a document from a manifest viewing the budget as '
                    '"{renderer}"'), target_fixture="result")
def build_doc(runner, ctx, renderer: str):
    return _build_doc(runner, ctx, _manifest(ctx["budget"], {"renderer": renderer}),
                      name=f"{renderer}.json")


@when("I build a document from a manifest whose view has no renderer",
      target_fixture="result")
def build_doc_no_renderer(runner, ctx):
    return _build_doc(runner, ctx, _manifest(ctx["budget"],
                                             {"options": {"totals": True}}))


@when(parsers.parse('I build a document from a manifest with an item key "{key}"'),
      target_fixture="result")
def build_doc_bad_key(runner, ctx, key: str):
    return _build_doc(runner, ctx, _manifest(ctx["budget"], item_extra={key: "x"}))


@when("I revise the budget claim with the data", target_fixture="result")
def revise_with_data(runner, ctx, docstring: str):
    ctx["before"] = _held(runner)
    return runner.run("pub", "revise", ctx["budget"],
                      f"--data={_write(runner, 'next.json', docstring)}")


@when("I revise the budget claim with its own data again", target_fixture="result")
def revise_same(runner, ctx):
    ctx["before"] = _held(runner)
    return runner.run("pub", "revise", ctx["budget"],
                      f"--data={_write(runner, 'same.json', BUDGET)}")


@when(parsers.parse('I revise the budget claim read from revision "{revision}"'),
      target_fixture="result")
def revise_sources(runner, ctx, revision: str):
    return runner.run("pub", "revise", ctx["budget"], f"--source={REPO}",
                      f"--source-revision={revision}",
                      "--source-locator=bizplan/plan/p0.py")


@when(parsers.parse('I revise the document from a manifest viewing the budget as '
                    '"{renderer}"'), target_fixture="result")
def revise_doc(runner, ctx, renderer: str):
    path = _write(runner, "next-doc.json",
                  _manifest(ctx["budget"], {"renderer": renderer}))
    return runner.run("pub", "revise", ctx["doc"], f"--manifest={path}")


@when("I revise the document with no manifest", target_fixture="result")
def revise_doc_bare(runner, ctx):
    return runner.run("pub", "revise", ctx["doc"])


# --- thens -------------------------------------------------------------------


@then(parsers.parse('reading it shows "{fragment}"'))
def reading_shows(runner, ctx, fragment: str):
    shown = _read(runner, ctx["claim"])
    assert fragment in shown, shown


@then("nothing new is stored")
def nothing_stored(runner, ctx):
    assert _held(runner) == ctx["before"], "a refused object was stored anyway"


@then("the claim presupposes that claim")
def presupposes(runner, ctx):
    shown = _read(runner, ctx["claim"])
    after = shown.split("presupposes:", 1)
    assert len(after) == 2 and ctx["reason"] in after[1], shown


@then("the store holds that blob and no object for it")
def blob_held(runner, ctx):
    assert ctx["blob"] not in _held(runner)
    # The blob is fetched by the porcelain that cited it; reading the claim
    # names it, and the store's own object listing must not.
    assert ctx["blob"] in _read(runner, ctx["claim"])


@then(parsers.parse('reading the document shows "{fragment}"'))
def reading_doc_shows(runner, ctx, fragment: str):
    shown = _read(runner, ctx["docs"][-1])
    assert fragment in shown, shown


@then("the two documents are different objects citing the same claim")
def two_docs(runner, ctx):
    first, second = ctx["docs"]
    assert first != second
    assert ctx["budget"] in _read(runner, first)
    assert ctx["budget"] in _read(runner, second)


@then("it prints the new version and the supersedes relation")
def prints_two(result, ctx):
    lines = result.stdout.split()
    assert len(lines) == 2, result.stdout
    ctx["new"], ctx["relation"] = lines


@then("it prints only the budget claim")
def prints_old(result, ctx):
    assert result.stdout.split() == [ctx["budget"]], result.stdout


@then("the relation says the new version supersedes the budget claim")
def relation_supersedes(runner, ctx):
    shown = _read(runner, ctx["relation"])
    assert "kind      supersedes" in shown, shown
    assert f"from      {ctx['new']}" in shown, shown
    assert f"to        {ctx['budget']}" in shown, shown


@then("the new version keeps the budget claim's content and scope")
def keeps_content(runner, ctx):
    shown = _read(runner, ctx["new"])
    assert CONTENT in shown and SCOPE in shown, shown
    assert "revision abc123" in shown, shown


def _new_version(result, ctx) -> str:
    return ctx.get("new") or result.stdout.split()[0]


@then(parsers.parse('reading the new version shows "{fragment}"'))
def new_version_shows(runner, result, ctx, fragment: str):
    shown = _read(runner, _new_version(result, ctx))
    assert fragment in shown, shown


@then(parsers.parse('reading the new version does not show "{fragment}"'))
def new_version_hides(runner, result, ctx, fragment: str):
    shown = _read(runner, _new_version(result, ctx))
    assert fragment not in shown, shown
