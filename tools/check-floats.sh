#!/usr/bin/env bash
# R8 requires bit-identical evaluation across implementations and platforms.
# Floating point cannot provide that, so the specification bans it from the
# wire format (4.1) and this project bans it from the crates that compute
# hashes and weights.
#
# The clippy lint catches float *arithmetic*. This catches the *types*,
# including in struct fields and signatures where no arithmetic appears yet
# -- a `fn f(x: f64) -> f64 { x }` passes clippy and fails here.
#
# Comment lines are excluded. The documentation explaining why floats are
# forbidden necessarily names them, and a guard that fires on its own
# rationale is a guard people learn to bypass.
set -euo pipefail
cd "$(dirname "$0")/.."

GUARDED=(crates/publet-core crates/publet-eval crates/publet-merkle)
status=0

for dir in "${GUARDED[@]}"; do
  [ -d "$dir" ] || continue
  hits=$(
    grep -rnE '\b(f32|f64)\b' "$dir" --include='*.rs' 2>/dev/null \
      | grep -vE '^[^:]+:[0-9]+:[[:space:]]*(//|/\*|\*)' \
      || true
  )
  if [ -n "$hits" ]; then
    echo "error: floating point is forbidden in $dir" >&2
    echo "$hits" >&2
    status=1
  fi
done

if [ "$status" -eq 0 ]; then
  echo "guard-floats: clean (${#GUARDED[@]} crates checked)"
fi
exit "$status"
