# Security

## Reporting a vulnerability

Report it privately through GitHub:
**[report a vulnerability](https://github.com/schryer/publet/security/advisories/new)**
(the repository's Security tab, "Report a vulnerability"). Please don't open a
public issue for it.

Say what is affected (which package and version, which command or function),
how to reproduce it, and what an attacker could do. You'll get a reply in the
advisory, and the fix is credited to you unless you'd rather not be named.

## Supported versions

Fixes go into the next release of each package:

| Package | Releases | Tags |
|---|---|---|
| `publet-cli` (the `pub` command and the plumbing tools) | the latest | `vX.Y.Z` |
| `publet-core` | the latest | `publet-core-vX.Y.Z` |
| `publet-algorithms` | the latest | `publet-algorithms-vX.Y.Z` |

Below 1.0 there are no backports. A security fix is released as the next
version of the package that needs it.

## What is checked, and how you can tell

**On every pull request and push to `main`, CI runs:**

- `cargo deny check`. It fails on any crate in the dependency tree that has a
  RustSec advisory: known vulnerabilities, crates marked unmaintained or
  unsound, and yanked releases. An advisory can be ignored only by naming it
  in `deny.toml` with the reason. It also restricts dependencies to crates.io
  and to permissive licences (`deny.toml`).
- `cargo vet`. Every dependency must be covered by an audit imported from
  Mozilla, Google, the Bytecode Alliance, Zcash, ISRG or Embark Studios, by
  a publisher those organisations trust, or by an exemption that records the
  evidence that does exist (`supply-chain/`).
- A check that each published crate's `SUPPLY-CHAIN.md` is current.

**In each published crate:** `SUPPLY-CHAIN.md` lists every dependency, says
whether it runs code at build time, and says how its code is vouched for. It
ships in the crate package, so it describes exactly what you install. See
[publet-core](https://github.com/schryer/publet/blob/main/crates/publet-core/SUPPLY-CHAIN.md) and
[publet-algorithms](https://github.com/schryer/publet/blob/main/crates/publet-algorithms/SUPPLY-CHAIN.md).

**In every release:** the checks above run again before the release is cut,
and the release is not cut if one fails. What ran is recorded as a `security`
claim in the release's package publet (`pkg.<package>` in `corpus/`), signed
with the release's key. It records each command, its tool and version, and
the tool's summary, together with the RustSec database revision checked
against and the audit sets imported. The changelog and the GitHub release
list the same record. pubrel refuses to tag a release whose record is
missing or unsigned.

**In the code:** every crate in the workspace forbids `unsafe`
(`unsafe_code = "forbid"` in `Cargo.toml`).

## What is not yet done

- Not every dependency of the published crates has an audit. The exemptions,
  and the evidence for each, are listed in each crate's `SUPPLY-CHAIN.md`.
- The decoders that take untrusted input (canonical CBOR, objects,
  identifiers) are not yet fuzzed.
- Advisory checks run when the code changes, not on a schedule, so an
  advisory published after a release is caught by the next run of CI.
