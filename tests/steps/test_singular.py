"""Steps for singulars, facts about them, and `pub get` (Section 5.9.6).

The worked example is the Tallinn basement: a Sustenaut site in Tallinn,
a basement that is part of it, and the basement's floor area of 50 m², an
exact value its owner confirmed. Every object is composed through `pub`, as
an author would, and read back through `pub get`, as a reader would.
"""

from __future__ import annotations

import json

from pytest_bdd import given, parsers, scenarios, then, when

scenarios("../features/conformance/singulars.feature")
scenarios("../features/porcelain/get.feature")

UCUM = {"sort": "system", "name": "UCUM", "version": "2.2"}
AREA_KIND = {"sort": "kind", "name": "area", "dimension": {"m": 2}, "units": ["m2"]}
LENGTH_KIND = {"sort": "kind", "name": "length", "dimension": {"m": 1}, "units": ["m", "cm"]}
EXACT = {
    "sort": "measure", "name": "exact value", "describes": "exact",
    "parameters": [{"name": "value", "type": "quantity", "meaning": "the value, stated as exact"}],
}
THREE_POINT = {
    "sort": "measure", "name": "three-point estimate", "describes": "uncertainty",
    "parameters": [
        {"name": "low", "type": "quantity", "meaning": "the least it plausibly is"},
        {"name": "likely", "type": "quantity", "meaning": "the most likely value"},
        {"name": "high", "type": "quantity", "meaning": "the most it plausibly is"},
    ],
}
# The usage pattern of spec Section 5.9.6: places under the concept "Place"
# at UDC's place class (1/9), and narrower PDC codes written with `~`.
PLACE = "(1/9)"
SITE = "(1/9)~1"
FLOOR_AREA = "69~1"
CEILING_HEIGHT = "69~2"
# A resolution-9 cell in central Tallinn.
TALLINN_CELL = "89089b1a217ffff"


def _write(runner, name: str, content) -> str:
    path = runner.cwd / name
    if isinstance(content, (dict, list)):
        content = json.dumps(content)
    path.write_text(content)
    return str(path)


def _define(runner, name: str, content: str, defines, *depends: str):
    args = ["compose", "--class=definitional", f"--content={content}",
            "--scope=unconditional", f"--defines={_write(runner, name + '.json', defines)}"]
    args += [f"--depends={d}" for d in depends]
    return runner.run("pub", *args)


def _defined(runner, name: str, content: str, defines, *depends: str) -> str:
    out = _define(runner, name, content, defines, *depends)
    assert out.code == 0, out.stderr
    return out.stdout.split()[0]


def _compose(runner, *args: str) -> str:
    out = runner.run("pub", "compose", *args)
    assert out.code == 0, out.stderr
    return out.stdout.split()[0]


def _depends(runner, cid: str) -> str:
    out = runner.run("pub", "read", cid)
    assert out.code == 0, out.stderr
    return out.stdout.split("presupposes:", 1)[1] if "presupposes:" in out.stdout else ""


def _fact_table(about, value, *, concept=FLOOR_AREA, about_extra=None, kind, measure):
    column = {"name": "about", **(about_extra or {})}
    return {"columns": [column,
                        {"name": "floor area", "unit": "m2", "kind": kind,
                         "measure": measure, "concept": concept}],
            "rows": [[about, value]]}


def _compose_fact(runner, ctx, data, *extra: str):
    s = ctx["s"]
    return runner.run(
        "pub", "compose", "--class=empirical", "--scope=unconditional",
        "--content=The basement at the Tallinn site has a floor area of 50 square metres.",
        f"--method={s['method']}", f"--statement={s['statement']}",
        f"--depends={s['ucum']}", f"--depends={s['floor_area']}",
        f"--data={_write(runner, 'fact.json', data)}", *extra)


# --- givens ------------------------------------------------------------------


@given("a workspace with the Tallinn basement", target_fixture="ctx")
def tallinn_basement(runner):
    assert runner.run("pub", "init").code == 0
    s = {}
    s["ucum"] = _defined(runner, "ucum", "UCUM: the Unified Code for Units of Measure, version 2.2.", UCUM)
    s["area_kind"] = _defined(runner, "area_kind", "area: the size of a surface.", AREA_KIND, s["ucum"])
    s["exact"] = _defined(runner, "exact", "exact value: a value stated as exact, such as a count or a figure measured or confirmed.", EXACT)
    s["place"] = _defined(runner, "place", "Place: a place, as UDC's place class (1/9) has it.",
                          {"sort": "concept", "pdc": PLACE, "name": "Place"})
    s["site"] = _defined(runner, "site", "site: a place where Sustenaut grows, such as a basement or a shop.",
                         {"sort": "concept", "pdc": SITE, "name": "site"}, s["place"])
    s["floor_area"] = _defined(
        runner, "floor_area", "floor area: the area of a room's floor, measured inside its walls.",
        {"sort": "concept", "pdc": FLOOR_AREA, "name": "floor area", "kind": s["area_kind"],
         "boundary": {"includes": ["Floor inside the walls, at floor level"],
                      "excludes": ["Area under fixed fittings deeper than 60 cm"]}})
    s["tallinn"] = _defined(
        runner, "tallinn", "Tallinn: Sustenaut's growing site in Tallinn.",
        {"sort": "singular", "name": "Tallinn", "class": SITE, "location": {"h3": TALLINN_CELL}},
        s["site"], s["place"])
    s["basement"] = _defined(
        runner, "basement", "basement: the basement of Sustenaut's Tallinn site.",
        {"sort": "singular", "name": "basement", "class": PLACE, "part_of": s["tallinn"]},
        s["place"])
    s["method"] = _compose(runner, "--class=procedural", "--scope=unconditional",
                           "--content=Measure the floor's length and width inside the walls with a tape, and multiply.")
    s["statement"] = _compose(runner, "--class=attributive", "--scope=unconditional",
                              "--content=The owner of the Tallinn site confirmed on 20 October 2026 that its basement's floor area is 50 square metres.")
    ctx = {"s": s}
    out = _compose_fact(runner, ctx, _fact_table(s["basement"], {"value": "50"},
                                                 kind=s["area_kind"], measure=s["exact"]))
    assert out.code == 0, out.stderr
    s["fact"] = out.stdout.split()[0]
    return ctx


@given(parsers.parse("the floor area fact is revised to {value}"))
def revise_fact(runner, ctx, value: str):
    s = ctx["s"]
    cell = None if value == "unknown" else {"value": value}
    data = _fact_table(s["basement"], cell, kind=s["area_kind"], measure=s["exact"])
    out = runner.run("pub", "revise", s["fact"], f"--data={_write(runner, 'next.json', data)}")
    assert out.code == 0, out.stderr


@given("an estimate of the basement's ceiling height")
def ceiling_estimate(runner, ctx):
    s = ctx["s"]
    length = _defined(runner, "length", "length: the extent of something along one direction.",
                      LENGTH_KIND, s["ucum"])
    three = _defined(runner, "three", "three-point estimate: a value as its least, likeliest and greatest plausible size.",
                     THREE_POINT)
    height = _defined(
        runner, "ceiling", "ceiling height: from the finished floor to the ceiling's lowest point.",
        {"sort": "concept", "pdc": CEILING_HEIGHT, "name": "ceiling height", "kind": length,
         "boundary": {"includes": ["Floor to ceiling"], "excludes": ["Beams and ducts"]}})
    data = {"columns": [{"name": "about"},
                        {"name": "ceiling height", "unit": "m", "kind": length, "measure": three,
                         "concept": CEILING_HEIGHT}],
            "rows": [[s["basement"], {"low": "2.1", "likely": "2.3", "high": "2.4"}]]}
    out = runner.run("pub", "compose", "--class=normative", "--scope=planning the Tallinn site",
                     "--content=The Tallinn basement is planned as having a ceiling height of about 2.3 m.",
                     f"--depends={s['ucum']}", f"--depends={height}",
                     f"--data={_write(runner, 'ceiling.json', data)}")
    assert out.code == 0, out.stderr


@given("a dispute of the floor area fact")
def dispute(runner, ctx):
    s = ctx["s"]
    counter = _compose(runner, "--class=normative", "--scope=unconditional",
                       "--content=The basement's floor area should be measured again before the lease is signed.")
    out = runner.run("pub", "relate", "--scope=unconditional", "--kind=disputes",
                     f"--from={counter}", f"--to={s['fact']}")
    assert out.code == 0, out.stderr


@given(parsers.parse('a shed whose class has concepts named "{first}" and "{second}"'))
def contested_shed(runner, ctx, first: str, second: str):
    s = ctx["s"]
    rival = _defined(runner, "rival", f"{second}: a place, under another name.",
                     {"sort": "concept", "pdc": PLACE, "name": second})
    shed = _defined(runner, "shed", "shed: the shed behind the Tallinn site.",
                    {"sort": "singular", "name": "shed", "class": PLACE}, s["place"], rival)
    data = _fact_table(shed, {"value": "6"}, kind=s["area_kind"], measure=s["exact"])
    out = _compose_fact(runner, ctx, data)
    assert out.code == 0, out.stderr


# --- whens -------------------------------------------------------------------


@when("I compose a singular defining", target_fixture="result")
def compose_singular(runner, ctx, docstring: str):
    return _define(runner, "bad", "shed: a shed.", json.loads(docstring), ctx["s"]["place"])


@when(parsers.parse('I compose a singular of class "{code}" with no concept for it'),
      target_fixture="result")
def compose_unclassed(runner, ctx, code: str):
    return _define(runner, "unclassed", "shed: the shed at the Tallinn site.",
                   {"sort": "singular", "name": "shed", "class": code}, ctx["s"]["place"])


@when("I compose a singular part of the floor area concept", target_fixture="result")
def compose_part_of_concept(runner, ctx):
    s = ctx["s"]
    return _define(runner, "wrong_whole", "shed: a shed.",
                   {"sort": "singular", "name": "shed", "class": PLACE, "part_of": s["floor_area"]},
                   s["place"])


@when(parsers.parse('I compose a fact about "{about}"'), target_fixture="result")
def compose_fact_about_text(runner, ctx, about: str):
    s = ctx["s"]
    return _compose_fact(runner, ctx, _fact_table(about, {"value": "50"},
                                                  kind=s["area_kind"], measure=s["exact"]))


@when("I compose a fact about the floor area concept", target_fixture="result")
def compose_fact_about_concept(runner, ctx):
    s = ctx["s"]
    return _compose_fact(runner, ctx, _fact_table(s["floor_area"], {"value": "50"},
                                                  kind=s["area_kind"], measure=s["exact"]))


@when(parsers.parse('I compose a fact whose about column has the unit "{unit}"'),
      target_fixture="result")
def compose_fact_about_unit(runner, ctx, unit: str):
    s = ctx["s"]
    return _compose_fact(runner, ctx, _fact_table(s["basement"], {"value": "50"},
                                                  about_extra={"unit": unit},
                                                  kind=s["area_kind"], measure=s["exact"]))


@when(parsers.parse('I compose a fact whose column\'s concept is "{code}"'),
      target_fixture="result")
def compose_fact_unknown_concept(runner, ctx, code: str):
    s = ctx["s"]
    return _compose_fact(runner, ctx, _fact_table(s["basement"], {"value": "50"}, concept=code,
                                                  kind=s["area_kind"], measure=s["exact"]))


@when(parsers.parse("I run pub get with {arguments}"), target_fixture="result")
def get_with(runner, arguments: str):
    args = [] if arguments == "no arguments" else [a.strip('" ') for a in arguments.split(" and ")]
    return runner.run("pub", "get", *args)


@when(parsers.parse('I get "{path}"'), target_fixture="result")
def get(runner, path: str):
    return runner.run("pub", "get", path)


# --- thens -------------------------------------------------------------------


@then(parsers.parse('reading the basement shows "{fragment}"'))
def reading_basement(runner, ctx, fragment: str):
    out = runner.run("pub", "read", ctx["s"]["basement"])
    assert fragment in out.stdout, out.stdout


@then("the basement presupposes the Tallinn site and the Place concept")
def basement_presupposes(runner, ctx):
    s = ctx["s"]
    listed = _depends(runner, s["basement"])
    assert s["tallinn"] in listed and s["place"] in listed, listed


@then("the floor area fact presupposes the basement")
def fact_presupposes(runner, ctx):
    s = ctx["s"]
    listed = _depends(runner, s["fact"])
    assert s["basement"] in listed, listed


@then(parsers.parse('it does not print "{text}"'))
def does_not_print(result, text: str):
    assert text not in result.stdout, result.stdout
