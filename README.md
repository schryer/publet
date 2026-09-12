# publet

A reference implementation of the Publet Protocol
(`draft-schryer-publet-protocol-01`) in Rust.

The protocol defines a decentralized format and evaluation model for atomic
units of human assertion. This implementation is a set of small composable
utilities over narrow library crates, in the manner of Unix filters and
git's plumbing commands, rather than a single application.

## Getting started

```sh
./bootstrap.sh     # prepare a bare machine; idempotent
make check         # everything CI runs
make help          # available targets
```

`bootstrap.sh` needs [rustup](https://rustup.rs) and Python 3.11 or later.
Nothing else is assumed about the machine.

## Reproducibility

Builds must not depend on how any particular machine is configured.

| Concern | Mechanism |
|---|---|
| Compiler version | `rust-toolchain.toml` pins the channel and components; rustup honours it on any cargo invocation in this tree |
| Rust dependencies | `Cargo.lock` is committed |
| Python dependencies | `tests/requirements.lock` pins exact versions with sha256 hashes, installed with `--require-hashes` |
| Tool discovery | `tools/env.sh` and the `Makefile` resolve the toolchain themselves; no target depends on the caller's `PATH` |
| CI parity | `Containerfile` builds the same environment hermetically |

Regenerate the Python lock with `make lock` after editing
`tests/requirements.txt`.

## Layout

```
crates/     library crates; dependency direction is enforced, not conventional
bin/        one thin binary per plumbing command
porcelain/  the `pub` multi-call binary
tests/      Python and Gherkin functional suite
vectors/    committed byte-level test vectors
tools/      environment resolution and the CI guards
```

## Testing

Testing is split by what it tests, and the split is strict.

**Rust** covers unit, property, fuzz, and snapshot testing — invariants with
no user-visible surface, such as canonical decoding rules and fixed-point
arithmetic.

**Python and Gherkin** cover every functional test, invoking the binaries as
a user would. No functional test links or imports the Rust. Binaries are
resolved from `PUBLET_BIN_DIR`, so the same suite is the cross-implementation
conformance definition: another implementation sets one environment variable
and runs it unmodified.

```sh
make test          # Rust
make functional    # Python and Gherkin
```

## Guards

Three project rules are checked mechanically rather than trusted to review,
because each fails silently in production:

- **`make guard-floats`** — floating point is forbidden in the crates that
  compute hashes and weights. Bit-identical evaluation across
  implementations is a protocol requirement and floats cannot provide it.
  The clippy lint catches arithmetic; this catches the types, including in
  signatures where no arithmetic appears yet.
- **`make guard-deps`** — the evaluation crate may not reach a store, a
  socket, a clock, or a random source, at any depth. Evaluation must be a
  pure function of its inputs.
- **`make lint`** — clippy with warnings as errors, including denied
  `unwrap`, `expect`, and `panic` outside tests.

## Status

Phase 0 of the implementation plan: foundations only. No protocol code yet.

## Licence

Apache-2.0.
