# Fuzzing regressions

Inputs that once made a target fail. `make fuzz-smoke` replays them before
fuzzing anything new, so a bug they found cannot quietly return.

| Target | Input | What it found |
|---|---|---|
| `cbor_decode` | `padded-null` | `null` written as `f8 16`, a two-byte simple value below 32, was accepted, giving the same object a second encoding and identifier. |
| `cid_parse` | `padding-bits` | Base32 padding bits after the digest were not checked, so one identifier had many spellings. |
| `log_verify` | `size-above-2-63` | A proof claiming a log larger than 2^63 made the verifier loop forever. |
| `log_verify` | `consistency-shift-by-64` | A consistency proof between sizes above 2^63 shifted a `u64` by 64 bits: a panic wherever overflow is checked, as in `pub`'s release builds. |

Each also has a regression test in its crate: the `null in two bytes` and
`non-zero padding bits` test vectors, and `absurd_sizes_are_refused_without_hanging`.
