#!/usr/bin/env bash
# Prove that the optional layers are optional (Section 12.1).
#
# Section 12.1 claims every other layer functions with no ledger present,
# and that an implementation may omit settlement entirely and remain
# conformant. A claim like that is worth exactly as much as the test that
# demonstrates it, so this removes the crate, rebuilds the workspace, and
# runs the whole conformance suite without it.
set -uo pipefail
cd "$(dirname "$0")/.."
source tools/env.sh

if ! git diff --quiet -- crates Cargo.toml; then
  echo "working tree has uncommitted changes; this script moves a crate" >&2
  echo "aside and restores from git, and would discard them" >&2
  exit 2
fi

if ! git diff --quiet -- bin; then
  echo "working tree has uncommitted changes under bin/" >&2
  exit 2
fi

STASH="$(mktemp -d)"
restore() {
  for crate in crates/publet-settle bin/pub-settle; do
    name="$(basename "$crate")"
    if [ -d "$STASH/$name" ]; then
      rm -rf "$crate"
      mv "$STASH/$name" "$crate"
    fi
  done
  git checkout -- Cargo.toml 2>/dev/null || true
  rm -rf "$STASH"
  cargo build --workspace -q 2>/dev/null || true
}
trap restore EXIT

printf '\n\033[1m==> removing crates/publet-settle and bin/pub-settle\033[0m\n'
mv crates/publet-settle "$STASH/publet-settle"
# Its command-line surface goes with it. A deployment that omits settlement
# ships no settlement binary, which is what the suite must cope with.
mv bin/pub-settle "$STASH/pub-settle"
# Removing the source leaves the previously built binary sitting in the
# target directory, where the suite would happily keep running it and
# report a pass that means nothing.
rm -f target/debug/pub-settle target/release/pub-settle
# The workspace dependency table still names it; drop that line too.
sed -i '/^publet-settle = /d' Cargo.toml

printf '\n\033[1m==> building without it\033[0m\n'
if ! cargo build --workspace 2>&1 | tail -3; then
  echo "FAILED: the workspace does not build without settlement"
  exit 1
fi

printf '\n\033[1m==> running the conformance suite without it\033[0m\n'
if ! pytest tests/ -q -m conformance 2>&1 | tail -3; then
  echo "FAILED: the conformance suite depends on settlement"
  exit 1
fi

printf '\n\033[1m==> running the Rust tests without it\033[0m\n'
if ! cargo test --workspace -q 2>&1 | tail -3; then
  echo "FAILED: the test suite depends on settlement"
  exit 1
fi

printf '\nsettlement is genuinely optional: the workspace builds and the\n'
printf 'whole suite passes with the crate absent\n'
