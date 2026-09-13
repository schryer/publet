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

## Cross-architecture determinism

The protocol requires that the same policy over the same snapshot yields
bit-identical results in every conformant implementation. A test that
passes on one machine says nothing about that property, so it is checked
across architectures that differ in the ways which actually break it.

```sh
make matrix            # every target
make matrix ARGS=--quick   # native and big-endian only
./tools/matrix.sh --list
```

| Target | Mode | Catches |
|---|---|---|
| `x86_64` | native | baseline |
| `s390x` | cross-build, emulated run | **byte order** — big-endian |
| `i686` | emulated image | 32-bit pointer and `usize` width |
| `aarch64` | emulated image | a second 64-bit little-endian architecture |

Big-endian coverage is the reason this exists rather than relying on CI
alone: every GitHub-hosted runner is little-endian, so an integer or length
prefix serialized with native endianness into a hash input produces
different identifiers on different machines while every hosted job passes.
The same four bytes read as `[01, 00, 00, 00]` on x86_64 and
`[00, 00, 00, 01]` on s390x.

Conversely the matrix cannot cover Windows or macOS, which no Linux
container can host. Those remain CI's job, and the line-ending behaviour of
the CID-lines protocol is the specific risk there.

Requires `podman` and `qemu-user-binfmt`:

```sh
sudo apt install podman qemu-user-binfmt
```

Targets without an official Rust image are cross-compiled natively in an
amd64 builder (`Containerfile.cross`) and only their artifacts are run under
emulation, which is far faster than emulating the compiler.

## The store

`pub-store` manages a node's local objects: what it holds, which domains it
has declared, and whether the bytes still match their identifiers.

```sh
pub-store --store=s.redb add < manifest.cbor  # store the domain manifest
pub-store --store=s.redb declare DOMAIN < cids
pub-store --store=s.redb gc                   # discard undeclared objects
pub-store --store=s.redb scan                 # re-hash everything
pub-cat   --store=s.redb CID                  # read one back
```

Three rules are enforced rather than documented:

- **Stored bytes always match their identifier**, verified on write, again
  on read, and re-hashed by `scan`.
- **A domain's membership is fixed by its manifest, not by the node's
  inventory.** `declare` requires the manifest to be held and checks that
  the supplied members hash to the snapshot root it declares. Declaring
  whatever happens to be in the store would invert the dependency, and
  nothing downstream could detect it.
- **Garbage collection never touches a declared set** -- neither its
  members nor the manifests that define them. Service within a declared set
  is unconditional, and a node that collected its own manifest would lose
  the ability to describe what it serves.

**Store-backed commands cannot be piped into one another.** The database
takes an exclusive file lock, so `pub-store list | pub-store declare`
deadlocks and says so. Redirect through a file, or produce identifiers with
a directory-backed command such as `pub-ls`.

## The `pub` command

```sh
pub init                    # create a workspace here
pub sync --domain=CID       # fetch a domain from a peer by delta
pub read CID                # the assertion, with the scope it was made under
pub why CID                 # its standing, and every component of it
pub compose --class=... --content=... --scope=...
pub propose CID             # submit, recording what it was composed against
pub witness DOMAIN          # the log root you observed
```

Local-first by default, and **every command says which mode it used**. The
privacy properties of reading a replica are real, and a reader who does not
know which mode they are in cannot know whether they have them.

`pub why` never reduces a standing to a badge. It shows the weights, the
divergence factor, the threshold, the reproduction counts against the
replication floor, and a sentence naming which rule produced the outcome --
because "accepted" alone does not say whether that rests on replication or
on agreement, and those are not the same claim.

`pub compose` requires `--scope`. An assertion that states its own validity
conditions does not drift; one that does not is a different assertion every
time it is read.

`pub propose` records the generation it was composed against and reports the
three ways that can be stale: the target already has another successor, a
dependency has been superseded, or a dispute answers a resolved question.
None of them is a refusal. Nothing here is overwritten, so a stale basis
cannot clobber anything -- it is disclosed, not enforced.

## Serving and syncing

```sh
pub-serve --store=s.redb --bind=127.0.0.1:8787
pub-sync  --peer=http://host:8787 --domain=CID --store=replica.redb --from=0 --to=20
```

**A sync request carries a domain identifier and two integers.** It carries
nothing about what the client holds, and there is no endpoint that accepts
such a list. `have`/`want` negotiation would let a peer compute a smaller
transfer at the cost of learning what the reader has been working with,
which forfeits most of what local-first operation provides. The route table
is the conformance surface, and a test asserts over it.

Every object received is verified against its own identifier before it is
stored, so a peer serving altered bytes is caught on arrival. `--proxy=URL`
routes through an anonymizing transport: which domains a reader follows is
visible to whoever serves them, and a proxy is what separates that from who
they are.

## Optional layers

Three areas of the specification are optional, and each is optional in the
strong sense: the workspace builds, and the whole suite passes, with the
code removed.

```sh
pub-settle --prohibitions                  # what settlement may never reach
pub-settle --bounty=b.cbor                 # read one, or refuse to
pub-person --scheme=ID... --schemes        # what this build carries
pub-ann --dir=D --assumptions-by=CID       # who has staked standing on whom
pub-ann --dir=D --triage-of=CID            # what a model said, and nothing more
```

**Settlement holds money and no authority.** This implementation ships the
null ledger only, so `pub-settle --settle=` always refuses and says why.
`conformance/verify-optional.sh` deletes the crate and its binary, rebuilds,
and runs the suite -- because "settlement is optional" is worth exactly as
much as the test that removes it.

**Personhood is never required to publish.** It bounds keys per person
within a scope and decides nothing else. This implementation ships no real
scheme: `pub-person` carries two demonstration schemes that perform no
cryptography, named `demo-` so nothing reaches for one by accident. What
they exist to demonstrate is that fewer than two schemes is not conformant,
and that a nullifier is unique within its scope and unlinkable across
scopes.

**Assumption and triage bear on the graph without authority in it.** An
assumption records two keys and a basis -- never who holds the assumed key,
because writing that down manufactures a compellable artifact where none
existed. A triage annotation is read by `pub-ann` and by nothing that
computes: adding one leaves every standing byte-identical.

`pub-lint` reports the Section 5.7 authoring tests: compound assertions,
several quantities, anaphora pointing outside the publet, contested terms
left out of `depends`. Findings are warnings; a flagged publet is valid. It
will never merge two publets for you. Whether two phrasings assert the same
thing is a claim, and a claim is made by signing it -- so the lint crate
takes one publet's text and has no second one to compare against.

## Conformance

The Gherkin suite is the conformance definition, not a description of this
implementation. Another implementation points one environment variable at
its own binaries and runs it unmodified:

```sh
PUBLET_BIN_DIR=/path/to/binaries pytest tests/ -m conformance
```

`conformance/COVERAGE.md` maps every section of the specification containing
normative language to the scenarios exercising it, and lists the sections
with none. The gaps are published rather than hidden.

`conformance/verify-suite.sh` breaks one specification rule at a time,
rebuilds, and asserts the suite notices. A suite never shown to fail is not
evidence of anything -- and this is how three generation scenarios were
found to be placeholders rather than tests.

`conformance/verify-optional.sh` removes the settlement crate and its
binary and runs everything else, which is what makes the optionality in
Section 12.1 a fact about the code rather than a sentence about it.

## Guards

Four project rules are checked mechanically rather than trusted to review,
because each fails silently in production:

- **`make guard-floats`** — floating point is forbidden in the crates that
  compute hashes and weights. Bit-identical evaluation across
  implementations is a protocol requirement and floats cannot provide it.
  The clippy lint catches arithmetic; this catches the types, including in
  signatures where no arithmetic appears yet.
- **`make guard-deps`** — the evaluation crate may not reach a store, a
  socket, a clock, or a random source, at any depth. Evaluation must be a
  pure function of its inputs.
- **`make guard-eval`** — the evaluation vectors' output is recorded in
  `vectors/eval/DIGEST`. Two implementations must return the same standing
  for the same inputs, so a change to what this one returns is a change to
  the protocol, not a refactor. A deliberate change re-records the digest
  in the same commit as the rule that caused it. The cross-architecture
  matrix compares against this value too: four architectures agreeing with
  each other would not notice all four drifting together, and they share
  the source.
- **`make lint`** — clippy with warnings as errors, including denied
  `unwrap`, `expect`, and `panic` outside tests.

## Status

All phases of the implementation plan are implemented, including the
optional layers. Every section of the specification containing normative
language has at least one scenario exercising it; `conformance/COVERAGE.md`
is the current map and is regenerated by `make coverage`.

## Licence

Apache-2.0.
