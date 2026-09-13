"""Fixtures for the functional suite.

Every test here runs the shipped binaries as a subprocess. Nothing in this
tree imports, links, or otherwise reaches into the Rust implementation --
that is the boundary described in section 6.2 of the implementation plan,
and it is what lets the same scenarios run against any implementation.
"""

from __future__ import annotations

import os
import shutil
import subprocess
import sys
from dataclasses import dataclass
from pathlib import Path

import pytest

# The suite imports its own helpers from `support/`; it never imports the
# implementation. Adding the suite root to the path keeps that explicit.
sys.path.insert(0, str(Path(__file__).parent))


@pytest.fixture(scope="session")
def bin_dir() -> Path:
    """Directory holding the binaries under test.

    Resolved only from PUBLET_BIN_DIR so that a second implementation, in
    any language, runs this suite by setting one environment variable.
    """
    raw = os.environ.get("PUBLET_BIN_DIR")
    if not raw:
        pytest.fail(
            "PUBLET_BIN_DIR is not set. Run the suite through `make functional`, "
            "or set it to the directory holding the publet binaries."
        )
    path = Path(raw)
    if not path.is_dir():
        pytest.fail(f"PUBLET_BIN_DIR does not exist: {path}")
    return path


@dataclass(frozen=True)
class Completed:
    """The observable result of running a command."""

    argv: list[str]
    code: int
    out: bytes
    err: bytes

    @property
    def stdout(self) -> str:
        return self.out.decode("utf-8", "replace")

    @property
    def stderr(self) -> str:
        return self.err.decode("utf-8", "replace")

    @property
    def lines(self) -> list[str]:
        return self.stdout.splitlines()


@dataclass
class Runner:
    """Runs binaries from the directory under test."""

    bin_dir: Path
    cwd: Path

    def path_to(self, name: str) -> Path:
        candidate = self.bin_dir / name
        if not candidate.exists():
            pytest.skip(f"{name} is not built yet")
        return candidate

    def run(self, name: str, *args: str, stdin: bytes | None = None) -> Completed:
        argv = [str(self.path_to(name)), *args]
        proc = subprocess.run(
            argv, input=stdin, capture_output=True, cwd=self.cwd, check=False
        )
        return Completed(argv, proc.returncode, proc.stdout, proc.stderr)

    def pipeline(self, stages: list[str], stdin: bytes | None = None) -> Completed:
        """Run shell stages joined by pipes.

        Composition over CID lines is a contract of the design, so it is
        tested as behaviour rather than assumed.
        """
        script = " | ".join(stages)
        env = dict(os.environ, PATH=f"{self.bin_dir}:{os.environ['PATH']}")
        proc = subprocess.run(
            ["bash", "-o", "pipefail", "-c", script],
            input=stdin, capture_output=True, cwd=self.cwd, env=env, check=False,
        )
        return Completed([script], proc.returncode, proc.stdout, proc.stderr)


@pytest.fixture
def runner(bin_dir: Path, tmp_path: Path) -> Runner:
    """A runner rooted in a scratch directory unique to the test."""
    return Runner(bin_dir=bin_dir, cwd=tmp_path)


@pytest.fixture(scope="session")
def has_jq() -> bool:
    return shutil.which("jq") is not None


def pytest_configure(config: pytest.Config) -> None:
    """Register the @must-N.N tags the feature files carry.

    Section tags are the mechanism by which the suite claims coverage of a
    normative statement (see tools/must-coverage.py). Registering them from
    the feature files rather than by hand keeps --strict-markers meaningful
    while letting a new tag arrive with the scenario that needs it.
    """
    features = Path(__file__).parent / "features"
    seen: set[str] = set()
    for path in features.rglob("*.feature"):
        for line in path.read_text().splitlines():
            stripped = line.strip()
            if not stripped.startswith("@"):
                continue
            for tag in stripped.split():
                if tag.startswith("@must-"):
                    seen.add(tag[1:])
    for tag in sorted(seen):
        config.addinivalue_line(
            "markers", f"{tag}: covers a normative statement in that section"
        )


# --- steps shared across feature files -------------------------------------
#
# pytest-bdd resolves step definitions per module, so anything used by more
# than one feature lives here rather than being duplicated or imported.

from pytest_bdd import parsers, then  # noqa: E402


@then(parsers.parse("the exit code is {code:d}"))
def exit_code_is(result: Completed, code: int) -> None:
    assert result.code == code, (
        f"expected exit {code}, got {result.code}\n"
        f"stdout: {result.stdout!r}\nstderr: {result.stderr!r}"
    )


@then("stdout is empty")
def stdout_is_empty(result: Completed) -> None:
    assert result.out == b"", f"expected no stdout, got {result.out!r}"


# --- fixtures and steps shared across feature files -------------------------

from pytest_bdd import given, when  # noqa: E402

from support.objects import (  # noqa: E402
    author_key, cid_of, publet, relation, write_store,
)

_KA = author_key("K_a")
_KB = author_key("K_b")

# Only these commands read a store directory; the rest are stdin filters.
DIR_AWARE = {
    "pub-ls", "pub-edges", "pub-lineage", "pub-closure",
    "pub-divergence", "pub-lint", "pub-delta",
}


@given("a store containing the Appendix A fixture", target_fixture="store")
def appendix_a(tmp_path):
    """P1 superseded by P4 by its own author; P2 a third-party proposal."""
    p1 = publet(_KA, "2026-09-12T10:00:00Z", "empirical", "relapse incidence fell")
    p4 = publet(_KA, "2026-09-12T11:00:00Z", "empirical", "effect holds in a subgroup")
    p2 = publet(_KB, "2026-09-12T10:30:00Z", "empirical", "the design cannot support it")
    objects = [
        p1, p4, p2,
        relation(_KA, "2026-09-12T11:00:01Z", "supersedes", cid_of(p4), cid_of(p1)),
        relation(_KB, "2026-09-12T10:30:01Z", "supersedes", cid_of(p2), cid_of(p1)),
        relation(_KB, "2026-09-12T10:30:02Z", "disputes", cid_of(p2), cid_of(p1)),
    ]
    write_store(tmp_path, objects)
    return {"dir": tmp_path, "P1": cid_of(p1), "P4": cid_of(p4), "P2": cid_of(p2)}




@when(parsers.parse('I run the pipeline "{pipeline}"'), target_fixture="result")
def run_pipeline(runner, store, pipeline: str):
    stages = []
    for stage in pipeline.split("|"):
        stage = stage.strip()
        if stage.split()[0] in DIR_AWARE:
            stage += f" --dir={store['dir']}"
        stages.append(stage)
    return runner.pipeline(stages)


@then(parsers.parse('stderr names the violated rule "{rule}"'))
def stderr_names_rule(result: Completed, rule: str) -> None:
    assert rule in result.stderr, f"{rule!r} not named in stderr: {result.stderr!r}"


@when("I load the store", target_fixture="result")
@when("loading the store", target_fixture="result")
def load_store(runner, store):
    return runner.run("pub-ls", f"--dir={store['dir']}")


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
def stderr_mentions(request, fragment: str) -> None:
    """Assert on stderr from whichever the scenario produced.

    Some scenarios run a command directly and have a `result`; others build
    a store and assert on loading it. Resolving the fixture lazily lets one
    step phrase serve both rather than forcing two near-identical wordings
    into the feature files.
    """
    try:
        result = request.getfixturevalue("result")
    except pytest.FixtureLookupError:
        result = None
    if result is not None:
        assert fragment in result.stderr, result.stderr
        return

    store = request.getfixturevalue("store")
    runner = request.getfixturevalue("runner")
    out = store.get("last") or runner.run("pub-ls", f"--dir={store['dir']}")
    assert fragment in out.stderr, out.stderr


def _code(result) -> int:
    """Exit status from either result shape.

    Scenarios that run a plumbing command get a `Completed`; those that run
    `pub` in a working directory get a `subprocess.CompletedProcess`. One
    step phrase should serve both rather than forcing the feature files to
    know which.
    """
    return getattr(result, "returncode", None) or getattr(result, "code", 0)


def _text(result) -> str:
    out, err = result.stdout, result.stderr
    if isinstance(out, bytes):
        return (out + err).decode("utf-8", "replace")
    return out + err


@then("it fails")
def it_fails(result) -> None:
    assert _code(result) != 0, _text(result)


@then("it succeeds")
def it_succeeds(result) -> None:
    assert _code(result) == 0, _text(result)


@then("both runs produce identical output")
def runs_identical(two_results) -> None:
    first, second = two_results
    assert _code(first) == _code(second) == 0
    assert first.stdout == second.stdout
