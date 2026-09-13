#!/usr/bin/env bash
# Pin the evaluation vectors' output (R8).
#
# tools/matrix.sh proves that four architectures agree with each other. It
# cannot notice that all four now agree on a different answer than they did
# last week, because it compares them only to one another. Evaluation
# semantics are a protocol commitment: two implementations must return the
# same standing for the same inputs, and a change to what this one returns
# is a change to the protocol, not a refactor.
#
# So the answer is recorded. A deliberate change re-records it with
# --update, in the same commit as the rule that caused it, where a reviewer
# can see the two together.
set -uo pipefail
cd "$(dirname "$0")/.."
source tools/env.sh

VECTOR_DIR="vectors/eval"
RECORD="vectors/eval/DIGEST"
BIN="target/release/pub-eval"

if [ ! -d "$VECTOR_DIR" ] || [ -z "$(ls -A "$VECTOR_DIR"/*.cbor 2>/dev/null)" ]; then
  echo "guard-eval-digest: no vectors, nothing to pin"
  exit 0
fi

if [ ! -x "$BIN" ]; then
  cargo build --release -p pub-eval -q || exit 1
fi

actual="$(for v in "$VECTOR_DIR"/*.cbor; do
  "$BIN" --vector="$v" || exit 1
done | sha256sum | cut -c1-16)"

if [ "${1:-}" = "--update" ]; then
  printf '%s\n' "$actual" > "$RECORD"
  echo "guard-eval-digest: recorded $actual"
  exit 0
fi

if [ ! -f "$RECORD" ]; then
  echo "guard-eval-digest: no recorded digest." >&2
  echo "  run: ./tools/check-eval-digest.sh --update" >&2
  exit 1
fi

expected="$(tr -d '[:space:]' < "$RECORD")"
if [ "$actual" != "$expected" ]; then
  cat >&2 <<EOF
guard-eval-digest: evaluation output changed.

  recorded: $expected
  actual:   $actual

Evaluation is a protocol commitment (R8): the same inputs must yield the
same standing in every conformant implementation, so a change here is a
change to the protocol. If it is deliberate, re-record it in the same
commit as the rule that caused it:

  ./tools/check-eval-digest.sh --update
EOF
  exit 1
fi

echo "guard-eval-digest: clean ($actual over $(ls "$VECTOR_DIR"/*.cbor | wc -l) vectors)"
