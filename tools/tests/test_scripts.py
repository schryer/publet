"""Tests for the maintainer scripts in tools/ and spec/.

These scripts are not protocol code and the functional suite in tests/
never runs them, so they are tested here, in-process, by loading each one
as a module. `make script-tests` runs them under coverage for SonarQube
Cloud.
"""

from __future__ import annotations

import bz2
import importlib.util
import json
import runpy
import sys
import urllib.request
from pathlib import Path

import pytest

ROOT = Path(__file__).resolve().parent.parent.parent


def load(relative: str):
    """The script at `relative` (from the repository root), as a module."""
    path = ROOT / relative
    name = path.stem.replace("-", "_")
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


audit = load("spec/audit.py")
must_coverage = load("tools/must-coverage.py")
wiki_seed = load("tools/wiki-seed.py")
vectors = load("tools/vectors/generate.py")


@pytest.fixture(autouse=True)
def at_root(monkeypatch):
    """Run each script from the repository root, as `make` does."""


def run(module, monkeypatch, *argv):
    monkeypatch.setattr(sys, "argv", [module.__name__, *map(str, argv)])
    return module.main()


# confined: the same rule in each script.

@pytest.mark.parametrize("module", [audit, must_coverage, wiki_seed])
def test_confined_admits_the_working_directory(module, tmp_path, monkeypatch):
    monkeypatch.chdir(tmp_path)
    assert module.confined("sub/file", "--x") == (tmp_path / "sub" / "file").resolve()


@pytest.mark.parametrize("module", [audit, must_coverage, wiki_seed])
def test_confined_admits_a_temporary_directory(module, tmp_path, monkeypatch):
    target = tmp_path / "file"
    assert module.confined(target, "--x") == target.resolve()


@pytest.mark.parametrize("module", [audit, must_coverage, wiki_seed])
def test_confined_refuses_anywhere_else(module, monkeypatch):
    with pytest.raises(SystemExit) as stop:
        module.confined("/etc/passwd", "--x")
    assert "--x (/etc/passwd) must be in the working directory" in str(stop.value)


@pytest.mark.parametrize("module", [audit, must_coverage, wiki_seed])
def test_confined_refuses_a_climb_out(module, monkeypatch):
    with pytest.raises(SystemExit):
        module.confined("../../../../../../etc/passwd", "--x")


# spec/audit.py

def test_audit_finds_the_specification_consistent(monkeypatch, capsys):
    assert run(audit, monkeypatch) == 0
    assert "index.md: consistent" in capsys.readouterr().out


def test_audit_reads_a_path_given(monkeypatch, tmp_path, capsys):
    spec = tmp_path / "index.md"
    spec.write_text((ROOT / "spec" / "index.md").read_text())
    assert run(audit, monkeypatch, spec) == 0
    assert "consistent" in capsys.readouterr().out


def test_audit_reports_its_findings(monkeypatch, tmp_path, capsys):
    spec = tmp_path / "index.md"
    spec.write_text("## 1. Only\n\nSee Section 9.\n")
    assert run(audit, monkeypatch, spec) == 1
    assert "finding(s)" in capsys.readouterr().out


def test_audit_refuses_a_path_outside(monkeypatch):
    with pytest.raises(SystemExit) as stop:
        run(audit, monkeypatch, "/etc/hostname")
    assert "the specification" in str(stop.value)


# tools/must-coverage.py

def test_must_coverage_writes_the_report(monkeypatch, tmp_path, capsys):
    report = tmp_path / "COVERAGE.md"
    assert run(must_coverage, monkeypatch, "--report", report) == 0
    assert report.read_text().startswith("#")
    assert f"report written to {report.resolve()}" in capsys.readouterr().out


def test_must_coverage_without_a_specification(monkeypatch, tmp_path, capsys):
    assert run(must_coverage, monkeypatch, "--spec", tmp_path / "absent.md") == 0
    assert "specification not found" in capsys.readouterr().out


def test_must_coverage_fails_under_the_floor(monkeypatch, tmp_path):
    spec = tmp_path / "index.md"
    spec.write_text("## 99. Untested\n\nAn implementation MUST do this.\n")
    assert run(must_coverage, monkeypatch, "--spec", spec, "--fail-under", "50") == 1


def test_must_coverage_refuses_a_report_outside(monkeypatch):
    with pytest.raises(SystemExit) as stop:
        run(must_coverage, monkeypatch, "--report", "/etc/COVERAGE.md")
    assert "--report" in str(stop.value)


# tools/wiki-seed.py

def write_index(path: Path, *lines: str) -> Path:
    with bz2.open(path, "wt", encoding="utf-8") as handle:
        handle.write("".join(line + "\n" for line in lines))
    return path


def test_wiki_seed_fetches_the_index(monkeypatch, tmp_path):
    fetched = []
    monkeypatch.setattr(wiki_seed.urllib.request, "urlretrieve",
                        lambda url, target: fetched.append((url, target)))
    target = tmp_path / "index.bz2"
    assert run(wiki_seed, monkeypatch, "--fetch-index", target) is None
    assert fetched == [(wiki_seed.urls("enwiki")[1], target.resolve())]


def test_wiki_seed_runs_as_a_script(monkeypatch, tmp_path):
    fetched = []
    monkeypatch.setattr(urllib.request, "urlretrieve",
                        lambda url, target: fetched.append(target))
    target = tmp_path / "index.bz2"
    monkeypatch.setattr(sys, "argv", ["wiki-seed.py", "--fetch-index", str(target)])
    runpy.run_path(str(ROOT / "tools" / "wiki-seed.py"), run_name="__main__")
    assert fetched == [target.resolve()]


def test_wiki_seed_writes_each_lead_sentence(monkeypatch, tmp_path):
    index = write_index(tmp_path / "index.bz2",
                        "100:7:Alpha", "100:8:Redirect", "900:9:Beta")
    titles = tmp_path / "titles.txt"
    titles.write_text("Alpha\nRedirect\nAbsent\n")
    pages = {"Alpha": ("41", "'''Alpha''' is the first letter of the Greek alphabet. More follows."),
             "Redirect": ("42", "#REDIRECT [[Alpha]]")}
    requests = []

    def articles(dump, start, end):
        requests.append((start, end))
        return pages

    monkeypatch.setattr(wiki_seed, "articles_in_stream", articles)
    out = tmp_path / "seed.jsonl"
    run(wiki_seed, monkeypatch, "--index", index, "--titles", titles, "--out", out)

    assert requests == [(100, 900)]
    records = [json.loads(line) for line in out.read_text().splitlines()]
    assert [(r["title"], r["page_id"], r["revision"]) for r in records] == [("Alpha", 7, "41")]
    assert records[0]["lead"] == "Alpha is the first letter of the Greek alphabet."


def test_wiki_seed_requires_index_and_titles(monkeypatch):
    with pytest.raises(SystemExit):
        run(wiki_seed, monkeypatch, "--titles", "titles.txt")


@pytest.mark.parametrize("flag", ["--titles", "--index", "--out"])
def test_wiki_seed_refuses_a_path_outside(flag, monkeypatch, tmp_path):
    index = write_index(tmp_path / "index.bz2", "100:7:Alpha")
    titles = tmp_path / "titles.txt"
    titles.write_text("Alpha\n")
    paths = {"--index": index, "--titles": titles, "--out": tmp_path / "out.jsonl"}
    paths[flag] = Path("/etc") / "seed-test"
    monkeypatch.setattr(wiki_seed, "articles_in_stream", lambda *_: {})
    with pytest.raises(SystemExit) as stop:
        run(wiki_seed, monkeypatch, *(item for pair in paths.items() for item in pair))
    assert flag in str(stop.value)


# vectors: the H3 cells come from the H3 library, the same on every run.


def test_drawn_is_below_its_bound_and_the_same_each_time():
    assert vectors.drawn("lat 0", 10) == vectors.drawn("lat 0", 10)
    assert all(0 <= vectors.drawn(f"x {i}", 7) < 7 for i in range(100))


def test_h3_vectors_are_the_committed_ones(monkeypatch):
    written = {}
    monkeypatch.setattr(vectors, "write", lambda path, data: written.update({path: data}))
    vectors.h3_cells()
    path = "crates/publet-algorithms/testdata/h3.json"
    assert written[path] == json.loads((ROOT / path).read_text())
