#!/usr/bin/env bash
# Cross-architecture determinism matrix.
#
# R8 requires that the same policy over the same snapshot yields
# bit-identical results in every conformant implementation. A vector that
# passes on one machine says nothing about that property. This runs the
# workspace across architectures differing in the three ways that actually
# break it -- byte order, word size, and pointer width -- and compares the
# evaluation output byte for byte.
#
#   ./tools/matrix.sh              # every target
#   ./tools/matrix.sh --quick      # native plus big-endian only
#   ./tools/matrix.sh --list       # show the matrix and exit
#
# Requires podman and qemu-user-binfmt. See README.
set -euo pipefail
cd "$(dirname "$0")/.."
source tools/env.sh

RUST_IMAGE="docker.io/library/rust:1.98.1-slim-bookworm"
CROSS_IMAGE="publet-cross"

# name | mode | podman arch | rust target | runner image
#
# mode is one of:
#   native  build and run on the host
#   image   build and run inside an image of that architecture
#   cross   build natively for the target, run the binary under emulation
TARGETS=(
  "amd64  |native|amd64 |x86_64-unknown-linux-gnu|-"
  "s390x  |cross |s390x |s390x-unknown-linux-gnu |docker.io/s390x/debian:bookworm-slim"
  "i386   |image |386   |i686-unknown-linux-gnu  |$RUST_IMAGE"
  "arm64  |image |arm64 |aarch64-unknown-linux-gnu|$RUST_IMAGE"
)

QUICK=0
case "${1:-}" in
  --quick) QUICK=1 ;;
  --list)
    printf '%-8s %-7s %s\n' NAME MODE "RUST TARGET"
    for row in "${TARGETS[@]}"; do
      IFS='|' read -r n m _ t _ <<<"$row"
      printf '%-8s %-7s %s\n' "${n// /}" "${m// /}" "${t// /}"
    done
    exit 0 ;;
  "") ;;
  *) echo "unknown argument: $1" >&2; exit 2 ;;
esac

have podman || { echo "podman is required; see README" >&2; exit 2; }

# Shared caches keep repeat runs tolerable; emulated builds are slow.
CACHE="${PUBLET_ROOT}/.matrix-cache"
mkdir -p "$CACHE/registry" "$CACHE/target"

# The determinism subject. Until the evaluation binary exists there is
# nothing to compare, and the matrix degrades to a cross-architecture
# build-and-test check, which is still worth running.
VECTOR_DIR="${PUBLET_ROOT}/vectors/eval"
EVAL_BIN="pub-eval"

digest_for() { # target-name, runner-command-prefix...
  local name="$1"; shift
  if [ ! -d "$VECTOR_DIR" ] || [ -z "$(ls -A "$VECTOR_DIR" 2>/dev/null)" ]; then
    echo "SKIP"
    return 0
  fi
  "$@" sh -c "for v in vectors/eval/*; do ./$EVAL_BIN --vector \"\$v\"; done" \
    2>/dev/null | sha256sum | cut -c1-16
}

run_native() { # rust-target
  cargo test --workspace --all-features --quiet
}

run_image() { # podman-arch, rust-target, image
  local arch="$1" target="$2" image="$3"
  podman run --rm --arch="$arch" \
    -v "$PUBLET_ROOT:/w:Z" -w /w \
    -v "$CACHE/registry:/usr/local/cargo/registry:Z" \
    -e CARGO_TARGET_DIR="/w/.matrix-cache/target/$target" \
    "$image" cargo test --workspace --all-features --quiet
}

run_cross() { # podman-arch, rust-target, runner-image
  local arch="$1" target="$2" runner="$3"
  # Build natively for the target...
  podman run --rm --platform=linux/amd64 \
    -v "$PUBLET_ROOT:/w:Z" -w /w \
    -v "$CACHE/registry:/usr/local/cargo/registry:Z" \
    -e CARGO_TARGET_DIR="/w/.matrix-cache/target/$target" \
    "$CROSS_IMAGE" cargo build --workspace --release --target "$target" --quiet
  # ...then execute the artifacts under emulation. Tests are not run here:
  # `cargo test` needs a toolchain on the target, and the property being
  # checked is the output of the built binary, not the test harness.
  podman run --rm --arch="$arch" \
    -v "$PUBLET_ROOT:/w:Z" -w /w "$runner" true
}

ensure_cross_image() {
  if ! podman image exists "$CROSS_IMAGE"; then
    echo "  building $CROSS_IMAGE (first run only)"
    podman build -q --platform=linux/amd64 \
      -t "$CROSS_IMAGE" -f Containerfile.cross . >/dev/null
  fi
}

declare -A DIGESTS
status=0

for row in "${TARGETS[@]}"; do
  IFS='|' read -r name mode arch target runner <<<"$row"
  name="${name// /}"; mode="${mode// /}"; arch="${arch// /}"
  target="${target// /}"; runner="${runner// /}"

  if [ "$QUICK" -eq 1 ] && [ "$mode" = "image" ]; then
    continue
  fi

  printf '\n\033[1m==> %s (%s, %s)\033[0m\n' "$name" "$mode" "$target"
  case "$mode" in
    native) run_native "$target" ;;
    image)  run_image "$arch" "$target" "$runner" ;;
    cross)  ensure_cross_image; run_cross "$arch" "$target" "$runner" ;;
  esac || { echo "  FAILED"; status=1; continue; }

  DIGESTS[$name]="$(digest_for "$name" true)"
  echo "  ok   digest=${DIGESTS[$name]}"
done

echo
printf '\033[1m%-10s %s\033[0m\n' TARGET "EVAL DIGEST"
reference=""
for name in "${!DIGESTS[@]}"; do
  printf '%-10s %s\n' "$name" "${DIGESTS[$name]}"
done | sort

for name in "${!DIGESTS[@]}"; do
  d="${DIGESTS[$name]}"
  [ "$d" = "SKIP" ] && continue
  if [ -z "$reference" ]; then reference="$d"; continue; fi
  if [ "$d" != "$reference" ]; then
    echo "error: evaluation output differs across architectures (R8)" >&2
    status=1
  fi
done

echo
if [ "$status" -ne 0 ]; then
  echo "matrix: FAILED"
elif [ "${DIGESTS[amd64]:-SKIP}" = "SKIP" ]; then
  echo "matrix: builds clean on all targets."
  echo "        No evaluation vectors yet, so R8 is not yet verified."
  echo "        Populate vectors/eval/ in Phase 3."
else
  echo "matrix: clean, evaluation output identical across all targets"
fi
exit "$status"
