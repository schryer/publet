#!/usr/bin/env bash
# Prove the conformance suite detects a non-conformant implementation.
#
# A suite that has never been shown to fail is not evidence of anything.
# Each case below breaks one rule the specification states, rebuilds, and
# asserts the suite notices. The working tree is restored from git after
# every case, so a break cannot leak into the next.
set -uo pipefail
cd "$(dirname "$0")/.."
source tools/env.sh

if ! git diff --quiet -- crates bin; then
  echo "working tree has uncommitted changes under crates/ or bin/;" >&2
  echo "this script restores from git and would discard them" >&2
  exit 2
fi
trap 'git checkout -- crates bin 2>/dev/null || true' EXIT

break_and_check() { # description, file, sed-expression, expected-pattern
  local what="$1" file="$2" expr="$3" expect="$4"
  printf '\n\033[1m==> %s\033[0m\n' "$what"

  sed -i "$expr" "$file"
  if git diff --quiet -- "$file"; then
    echo "  the edit changed nothing; the break was not applied"
    git checkout -- "$file"
    return 1
  fi

  if ! cargo build --workspace -q 2>/dev/null; then
    echo "  the broken build does not compile; nothing was tested"
    git checkout -- "$file"
    return 1
  fi

  local out
  out=$(pytest tests/ -q -m conformance 2>&1)
  git checkout -- "$file"

  if echo "$out" | grep -Eq "$expect"; then
    echo "  detected by: $(echo "$out" | grep -E "^FAILED" | head -3 | sed 's/^FAILED //' | tr '\n' ' ')"
    return 0
  fi
  echo "  NOT DETECTED -- the suite passed a non-conformant build"
  echo "$out" | tail -3
  return 1
}

status=0

# 4.1: accept non-canonical bytes instead of rejecting them.
break_and_check "accept unsorted map keys" \
  crates/publet-core/src/cbor/decode.rs \
  's|std::cmp::Ordering::Less => {|std::cmp::Ordering::Less => if false {|' \
  "canonical_form|FAILED" || status=1

# 11.4.1: let endorsement reach `accepted` without replication.
break_and_check "ignore the replication floor" \
  crates/publet-eval/src/standing.rs \
  's|evidence.reproductions.independent_consistent >= policy.replication_floor|true|' \
  "evidence_dominance|restricted_access|FAILED" || status=1

# 14.1.1: admit a removal carrying no accounting reference.
break_and_check "allow unaccounted removals" \
  crates/publet-domain/src/generation.rs \
  's@ok_or_else(|| GenerationError::UnaccountedRemoval@or(Some(cid.clone())).ok_or_else(|| GenerationError::UnaccountedRemoval@' \
  "generations|FAILED" || status=1

cargo build --workspace -q 2>/dev/null

printf '\n'
if [ "$status" -eq 0 ]; then
  echo "the suite detected every break"
else
  echo "at least one break went undetected; that is a gap in the suite,"
  echo "not a gap in the implementation"
fi
exit "$status"
