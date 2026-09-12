#!/usr/bin/env bash
# Resolve the project toolchain without depending on the caller's PATH.
#
# Every entry point sources this. It never mutates the user's shell profile
# and never installs anything; bootstrap.sh does installation.
set -euo pipefail

PUBLET_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
export PUBLET_ROOT

# rustup places its shims here regardless of whether the user's PATH has them.
if [ -d "${CARGO_HOME:-$HOME/.cargo}/bin" ]; then
  export PATH="${CARGO_HOME:-$HOME/.cargo}/bin:$PATH"
fi

export VENV="$PUBLET_ROOT/.venv"
if [ -d "$VENV/bin" ]; then
  export PATH="$VENV/bin:$PATH"
fi

# Functional tests locate binaries here and nowhere else (plan 6.2.3).
export PUBLET_BIN_DIR="${PUBLET_BIN_DIR:-$PUBLET_ROOT/target/debug}"

have() { command -v "$1" >/dev/null 2>&1; }
