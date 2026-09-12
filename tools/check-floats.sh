#!/usr/bin/env bash
# R8 requires bit-identical evaluation across implementations and platforms.
# Floating point cannot provide that, so the specification bans it from the
# wire format (4.1) and this project bans it from the crates that compute
# hashes and weights. The clippy lint catches arithmetic; this catches the
# types themselves, including in struct fields and signatures where no
# arithmetic appears yet.
set -euo pipefail
cd "$(dirname "$0")/.."

GUARDED=(crates/publet-core crates/publet-eval crates/publet-merkle)
status=0

for dir in "${GUARDED[@]}"; do
  [ -d "$dir" ] || continue
  if hits=$(grep -rnE '\b(f32|f64)\b' "$dir" --include='*.rs' 2>/dev/null); then
    echo "error: floating point is forbidden in $dir" >&2
    echo "$hits" >&2
    status=1
  fi
done

if [ "$status" -eq 0 ]; then
  echo "guard-floats: clean (${#GUARDED[@]} crates checked)"
fi
exit "$status"
