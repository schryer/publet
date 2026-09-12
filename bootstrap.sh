#!/usr/bin/env bash
# Bring a bare machine to a working state. Idempotent; safe to re-run.
#
# Installs nothing system-wide and writes nothing outside this tree except
# the rustup toolchain, which rustup itself manages under ~/.rustup.
set -euo pipefail
cd "$(dirname "$0")"
# shellcheck source=tools/env.sh
source tools/env.sh

step() { printf '\n\033[1m==> %s\033[0m\n' "$1"; }

step "Rust toolchain"
if ! have rustup; then
  echo "rustup not found." >&2
  echo "Install it, then re-run: https://rustup.rs" >&2
  exit 1
fi
# rust-toolchain.toml drives the version and components; this materializes it.
rustup show active-toolchain
cargo --version
cargo clippy --version
cargo fmt --version

step "Cargo tools"
# Pinned so a version bump is a reviewable commit, never a surprise.
install_cargo_tool() {
  local name="$1" version="$2"
  if cargo "${name#cargo-}" --version 2>/dev/null | grep -q "$version"; then
    echo "$name $version already present"
  else
    cargo install --locked "$name" --version "$version"
  fi
}
install_cargo_tool cargo-deny 0.18.6

step "Python environment"
python3 -m venv --upgrade-deps "$VENV" >/dev/null
"$VENV/bin/python" -m pip install --quiet --upgrade pip
if [ -f tests/requirements.lock ]; then
  "$VENV/bin/python" -m pip install --quiet --require-hashes -r tests/requirements.lock
else
  "$VENV/bin/python" -m pip install --quiet -r tests/requirements.txt
fi
"$VENV/bin/python" -m pytest --version

step "Done"
echo "Toolchain and test environment ready. Run: make check"
