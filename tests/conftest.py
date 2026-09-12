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
from dataclasses import dataclass
from pathlib import Path

import pytest


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
