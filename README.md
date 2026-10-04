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
| Shared test setup | `pubkit.mk`, `rust-toolchain.toml` and `tests/requirements-pubkit.txt` are managed by [pubkit](https://github.com/schryer/pubkit), as in every package; `make sync-check` fails if they drift, and CI runs pubkit's `rust-ci.yml` |

Regenerate the Python lock with `make lock` after editing
`tests/requirements.txt`; it pins every platform's files. The functional
suite's binary runner, result type, common steps (`it succeeds`, `the exit
code is N`, ...) and tag markers come from pubkit's pytest plugin.

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
pub compose --class=... --content=... --scope=... [--data=FILE] [--source=REF]
pub revise CID [--data=FILE] [--source=REF]   # next version + supersedes, or nothing if unchanged
pub doc MANIFEST.json       # a document, from a JSON manifest; items may carry a `view`
pub build [DIR] [--sign]    # compile named publet sources (in a corpus: unsigned drafts only)
pub render SLUG [--check]   # run the render pipeline a publet names, once it checks out
pub publish SLUG... | --all # sign, export, and record drafts; pin their accepted renderings
pub myst SLUG --out=DIR --template=PATH   # a MyST project for a document
pub corpus init [--parent=ALIAS=PATH]     # make this directory a corpus
pub corpus status | upgrade [ALIAS]       # is a parent newer than the pin; re-pin
pub corpus map [--format=mermaid|dot|json] # the publet network across corpora
pub corpus renderings [--prune]           # pinned renderings; drop inactive kept outputs
pub env build|status|path SLUG            # provision the toolchain a pipeline runs in
pub delegate --to=KEY                     # let another key continue this key's lineages
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

Gathering and rendering are separate. A claim about a set of values keeps
its one-sentence `content` and carries the values in `data` -- a table of
typed columns, or a file stored as a blob -- with `--source` naming where
they were read from: a file at a commit, a warehouse query, an export, or
another claim, which then also becomes a dependency. How to show the values
is a `view` on the document item citing them, so the same data can be a
table in one document and a chart in another; the rendering itself is never
stored. `pub revise` publishes a new version only when something changed,
so a build that re-gathers the same inputs leaves the lineage alone.

People link by name and objects link by identifier. `pub build` reads one
`publet.json` per publet, each referring to the others by a short slug
(`tool.typst`, `tool.typst#identity`, `#usage`), publishes children before
the parents whose identifiers cover theirs, files each publet's Section 9.1
tag on its lineage's genesis, and writes `publets.lock`, the one place a
slug and an identifier appear side by side. **No slug enters any object**:
a name written into an object's bytes could never be withdrawn, which is
the namespace-capture problem Section 9.1 exists to avoid. Unchanged
sources publish nothing, and nothing is stored unless the whole tree is
admissible.

A publet names how it is rendered: its `Colophon` cites a render-pipeline
publet whose steps are procedural claims carrying the commands to run, each
depending on the identity claim of every program it runs -- name, version,
and how to check it -- and those publets cite wrapper publets for the
external documentation of each tool. `pub render` runs nothing until every
object in the pipeline is signed by a key the policy trusts, every program
reports the version its claim states, and every repository file the
pipeline cites still hashes to its blob; a failure names the publet that
disagrees with the machine. Commands run without a shell. Every rendered
reference reads `Title [TAG] — identifier`: the name to read, and the
identifier to check, because a name alone can be captured. This
repository's own publets and their render pipeline are under
`corpus/publets/`; the general toolchain publets they build on are in
`publet-corpus`.

**Corpora.** A corpus is a workspace whose objects form a Section 14.1
domain: `corpus.json` names it and its parents, `publets/` holds its
sources, and `pub build` there exports every object to `objects/` -- the
committed form -- and advances the corpus's generation whenever its
membership changes. A project corpus names a more general parent's publet as
`alias:slug` (never by a bare slug a parent could later shadow), is pinned
to the parent generation it built against, and copies in what it reaches at
identical identifiers, so it stands alone. `pub corpus status` says when a
parent has moved on; upgrading is the child's choice. Each corpus has its
own key; a parent hands curation of a lineage to a child's key with
`pub delegate`, without which the child's new versions are proposals
(Sections 6, 10.6). This repository's own publets live in `corpus/`, built
on `publet-corpus`.

**Build freely, publish deliberately.** In a corpus, `pub build` and
`pub render` are a workshop: drafts are unsigned, stay in the workspace,
and leave no history, so experimenting costs nothing. `pub publish` is the
one act that records anything: it signs and exports the named publets (and
any draft they need), advances the generation, and pins the accepted
rendering -- document, pipeline, exact toolchain, output identifiers -- as
a `settled` annotation, keeping the output as a blob. Publishing a
rendering identical to the one already pinned files nothing. Documents
cite their pipeline by lineage and pipelines state version constraints
(`requires`) against tool lineages (`~ref`), so a tool upgrade revises the
tool's identity claim and nothing else.

**Environments.** A pipeline names the environment it runs in: a publet
listing its **host requirements** (what the machine must already have to
bootstrap it, such as cargo, npm, node, curl) and the steps that provision
the pinned toolchain -- tools, packages, fonts -- into
`~/.cache/publet/env/<identifier>/`, with every download checked against
the identifier it is pinned to. `pub env build` provisions it once;
`pub render` then runs only what it holds, so the output depends on the
publet, not on the machine. Environments are keyed by toolchain, not by
document, so a handful serve everything. `pub env path` prints the
directory, for using the same tools by hand. A container image pinned by
digest is noted as a future way to provide an environment -- for a deployed
service -- and is not built.

`pub propose` records the generation it was composed against and reports the
three ways that can be stale: the target already has another successor, a
dependency has been superseded, or a dispute answers a resolved question.
None of them is a refusal. Nothing here is overwritten, so a stale basis
cannot clobber anything -- it is disclosed, not enforced.

## Versions and releases

A release of `pub` and the plumbing commands is a **version of a publet**:
the package publet `pkg.publet-cli` [PKG-PUBCLI-10-2026] in `corpus/`. Its
`identity` claim states the version, the `vX.Y.Z` tag, and the commit; its
`release` claim lists what changed. The publet's lineage is the release
history, and `CHANGELOG.md` is generated from it.

The version follows mechanically from what changed since the last release:

| Category | Meaning | Bump |
|---|---|---|
| `changed`, `removed` | an already-published interface changed: commands, flags, output, exit codes, file formats, object shapes | major |
| `added` | a new feature | minor |
| `fixed`, `security` | the same functionality | patch |

Every pull request that changes code adds a row to
`corpus/publets/pkg.publet-cli/unreleased.json` -- unpublished workshop
data, like a draft -- and CI refuses one that does not. `make release-pr`
moves those rows into the package publet, sets the version the categories
require, publishes the package publet with your key, and opens a release
pull request; CI checks the bump is the one the changes require. Merging
it tags `vX.Y.Z` and creates the GitHub release.

The rules, the release commands, and the CI workflow are
[`pubrel`](https://github.com/schryer/pubrel)'s, shared by every package;
`release.json` tells it where this package's corpus, manifest and code are.
Cutting a release needs `pubrel` and a released `pub` installed:

```sh
cargo install --locked --git https://github.com/schryer/pubrel --tag v0.1.0 pubrel
```

Install a published version -- from git, until the workspace's crates are
published on crates.io:

```sh
cargo install --locked --git https://github.com/schryer/publet --tag v0.1.1 publet-cli
```

`pub --version` prints `pub X.Y.Z` and the package publet's identifier only
for a clean build at that release's tag; any other build reports itself as
unreleased, so a render pipeline requiring a released `pub` (`requires
^0.1`) cannot be satisfied by a development build.

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
