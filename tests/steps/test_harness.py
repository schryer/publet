"""Step definitions for the harness self-check."""

from __future__ import annotations

import os
from pathlib import Path

from pytest_bdd import given, scenarios, then

scenarios("../features/harness.feature")


@given("the binary directory is configured", target_fixture="configured_dir")
def configured_dir(bin_dir: Path) -> Path:
    return bin_dir


@then("it is the directory named by PUBLET_BIN_DIR")
def matches_environment(configured_dir: Path) -> None:
    assert configured_dir == Path(os.environ["PUBLET_BIN_DIR"])
