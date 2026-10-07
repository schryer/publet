#!/usr/bin/env python3
"""Generate the test vectors in crates/*/testdata/ from independent sources.

Nothing here calls publet. Expected values come from:

- hand-encoded CBOR, written from the specification's Section 4.1 profile;
- Python's hashlib and base64, for identifiers (Section 4.2);
- RFC 8032 Section 7.1, for Ed25519 keys, checked here against the
  `cryptography` package, which also makes the domain-separated signatures;
- RFC 6962 Section 2.1, implemented below from the RFC's own definitions,
  checked against the Certificate Transparency project's published roots;
- integer arithmetic with truncation toward zero, for Fixed6;
- Cargo's caret rule and the other requirement forms, for tool versions.

The vectors are committed; this script documents and reproduces them.
Run from the repository root: tools/vectors/generate.py
"""

import base64
import hashlib
import json
from pathlib import Path

from cryptography.hazmat.primitives.asymmetric.ed25519 import Ed25519PrivateKey
from cryptography.hazmat.primitives.serialization import Encoding, PublicFormat

ROOT = Path(__file__).resolve().parents[2]


def write(path, data):
    path = ROOT / path
    path.write_text(json.dumps(data, indent=2, ensure_ascii=False) + "\n")
    print(f"wrote {path.relative_to(ROOT)}")


def cid(data):
    digest = hashlib.sha256(data).digest()
    return "pub:sha2-256:" + base64.b32encode(digest).decode().lower().rstrip("=")


# --- publet-core: canonical CBOR -------------------------------------------

def canonical():
    accept = [
        ("zero", "00", "the integer 0"),
        ("largest one-byte integer", "17", "23 fits in the initial byte"),
        ("smallest two-byte integer", "1818", "24 needs a following byte"),
        ("negative one", "20", "-1 is stored as 0"),
        ("empty byte string", "40", ""),
        ("empty text", "60", ""),
        ("empty array", "80", ""),
        ("empty map", "a0", ""),
        ("false", "f4", ""), ("true", "f5", ""), ("null", "f6", ""),
        ("map, keys sorted by UTF-8 bytes", "a2626161016162" + "02",
         'Section 4.1 sorts "aa" before "b": by bytes, not by encoded length'),
        ("nested", "a16161" + "83" + "01" + "62" + "6869" + "a0",
         '{"a": [1, "hi", {}]}'),
        ("NFC text", "62c3a9", '"é" as the single code point U+00E9'),
    ]
    reject = [
        ("integer not in shortest form", "1801", "shortest-form integers", "1 written in two bytes"),
        ("length not in shortest form", "780161", "shortest-form integers", 'a one-byte text with its length in two bytes'),
        ("indefinite-length array", "9f01ff", "no indefinite length", ""),
        ("indefinite-length text", "7f6161ff", "no indefinite length", ""),
        ("half-precision float", "f93e00", "no floating point", "1.5"),
        ("double-precision float", "fb3ff8000000000000", "no floating point", "1.5"),
        ("integer map key", "a10102", "text map keys", "{1: 2}"),
        ("duplicate map keys", "a2616101616102", "unique map keys", '{"a": 1, "a": 2}'),
        ("keys out of order", "a2616201616102", "sorted map keys", '{"b": 1, "a": 2}'),
        ("keys in encoded-length order", "a26162026261610" + "1", "sorted map keys",
         'RFC 8949 deterministic order puts "b" before "aa"; Section 4.1 does not'),
        ("invalid UTF-8", "61ff", "valid UTF-8", ""),
        ("text not in NFC", "6365cc81", "NFC normalization", '"é" as e plus a combining accent'),
        ("tag", "c06178", "supported item types", 'tag 0 on "x"'),
        ("undefined", "f7", "supported item types", "the simple value undefined"),
        ("null in two bytes", "f816", "shortest-form integers",
         "simple value 22 in the two-byte form, which RFC 8949 3.3 forbids below 32; "
         "found by fuzzing: it gave null a second encoding"),
        ("false in two bytes", "f814", "shortest-form integers", "simple value 20 in the two-byte form"),
        ("two-byte simple value", "f820", "supported item types",
         "simple value 32: well formed, but outside the profile"),
        ("trailing data", "0102", "single top-level item", "1, then 2"),
        ("truncated text", "6261", "complete items", "a two-byte text with one byte present"),
        ("truncated array", "8201", "complete items", "an array of two with one element"),
        ("nesting deeper than 64", "81" * 65 + "80", "nesting limit", "65 arrays inside each other"),
    ]
    write("crates/publet-core/testdata/canonical.json", {
        "description": "Bytes the Section 4.1 profile accepts, which must re-encode to themselves, "
                       "and bytes it refuses, with the rule each breaks.",
        "source": "Hand-encoded from the specification; see tools/vectors/generate.py.",
        "accept": [{"name": n, "hex": h, "why": w} for n, h, w in accept],
        "reject": [{"name": n, "hex": h, "rule": r, "why": w} for n, h, r, w in reject],
    })


def trailing_bits_set(text):
    """The same identifier, spelled with non-zero padding bits.

    A 32-byte digest is 52 base32 characters, and the last carries one bit
    of the digest and four of padding. Setting the padding gives a second
    spelling, which a parser must refuse. Found by fuzzing.
    """
    alphabet = "abcdefghijklmnopqrstuvwxyz234567"
    last = alphabet.index(text[-1])
    assert last & 0b01111 == 0
    return text[:-1] + alphabet[last | 0b00001]


def identifiers():
    compute = [("empty", b""), ("hello", b"hello"), ("CBOR text hello", b"\x65hello"),
               ("one byte", b"\x00")]
    invalid = [
        ("no scheme", "sha2-256:abc", "MissingScheme"),
        ("missing digest", "pub:sha2-256", "Malformed"),
        ("empty digest", "pub:sha2-256:", "Malformed"),
        ("unknown algorithm", "pub:md5:aaaa", "UnknownAlgorithm"),
        ("upper-case digest", cid(b"").upper().replace("PUB:SHA2-256:", "pub:sha2-256:"), "InvalidDigest"),
        ("padded digest", cid(b"") + "====", "InvalidDigest"),
        ("short digest", "pub:sha2-256:aaaa", "WrongDigestLength"),
        ("non-zero padding bits", trailing_bits_set(cid(b"")), "InvalidDigest"),
    ]
    write("crates/publet-core/testdata/identifiers.json", {
        "description": "Identifiers of known bytes (Section 4.2), and text that is not an identifier.",
        "source": "SHA-256 and RFC 4648 base32 from Python's standard library.",
        "compute": [{"name": n, "hex": d.hex(), "cid": cid(d)} for n, d in compute],
        "invalid": [{"name": n, "text": t, "error": e} for n, t, e in invalid],
    })


def signatures():
    # RFC 8032, Section 7.1, TEST 1 to 3: secret key and public key.
    rfc = [
        ("RFC 8032 7.1 TEST 1",
         "9d61b19deffd5a60ba844af492ec2cc44449c5697b326919703bac031cae7f60",
         "d75a980182b10ab7d54bfed3c964073a0ee172f3daa62325af021a68f707511a"),
        ("RFC 8032 7.1 TEST 2",
         "4ccd089b28ff96da9db6c346ec114e0f5b8a319f35aba624da8cf6ed4fb8a6fb",
         "3d4017c3e843895a92b70aa74d1b7ebc9c982ccf2ec4968cc0cd55f12af4660c"),
        ("RFC 8032 7.1 TEST 3",
         "c5aa8df43f9f837bedb7442f31dcb7b166d38535076f094b85ce3a2e0b4458f7",
         "fc51cd8e6218a1a38da47ed00230f0580816ed13ba3303ac5deb911548908025"),
    ]
    for name, seed, public in rfc:
        derived = Ed25519PrivateKey.from_private_bytes(bytes.fromhex(seed)).public_key()
        got = derived.public_bytes(Encoding.Raw, PublicFormat.Raw).hex()
        assert got == public, f"{name}: the cryptography package derives {got}"
    sigs = []
    for name, seed, purpose, target in [
        ("authored, empty target", rfc[0][1], "authored", b""),
        ("authored, an object", rfc[0][1], "authored", b"\xa1\x61\x61\x01"),
        ("endorse, an object", rfc[1][1], "endorse", b"\xa1\x61\x61\x01"),
    ]:
        message = b"pub/v1/sig\x00" + purpose.encode() + b"\x00" + target
        signature = Ed25519PrivateKey.from_private_bytes(bytes.fromhex(seed)).sign(message)
        sigs.append({"name": name, "seed": seed, "purpose": purpose,
                     "target_hex": target.hex(), "signature": signature.hex()})
    write("crates/publet-core/testdata/signatures.json", {
        "description": "Ed25519 keys from RFC 8032, and detached signatures over Section 4.4's "
                       "domain-separated message: \"pub/v1/sig\" 0x00 purpose 0x00 target.",
        "source": "Keys: RFC 8032 Section 7.1, checked against Python's cryptography package, "
                  "which also made the signatures.",
        "keys": [{"name": n, "seed": s, "public": p} for n, s, p in rfc],
        "signatures": sigs,
    })


# --- publet-algorithms: RFC 6962 --------------------------------------------

def H(b):
    return hashlib.sha256(b).digest()


def mth(d):
    n = len(d)
    if n == 0:
        return H(b"")
    if n == 1:
        return H(b"\x00" + d[0])
    k = 1
    while k * 2 < n:
        k *= 2
    return H(b"\x01" + mth(d[:k]) + mth(d[k:]))


def split(n):
    k = 1
    while k * 2 < n:
        k *= 2
    return k


def path(m, d):
    n = len(d)
    if n == 1:
        return []
    k = split(n)
    if m < k:
        return path(m, d[:k]) + [mth(d[k:])]
    return path(m - k, d[k:]) + [mth(d[:k])]


def subproof(m, d, b):
    n = len(d)
    if m == n:
        return [] if b else [mth(d)]
    k = split(n)
    if m <= k:
        return subproof(m, d[:k], b) + [mth(d[k:])]
    return subproof(m - k, d[k:], False) + [mth(d[:k])]


def log():
    # The Certificate Transparency project's test leaves, and the roots it
    # publishes for each prefix of them.
    leaves = ["", "00", "10", "2021", "3031", "40414243", "5051525354555657",
              "606162636465666768696a6b6c6d6e6f"]
    published = [
        "6e340b9cffb37a989ca544e6bb780a2c78901d3fb33738768511a30617afa01d",
        "fac54203e7cc696cf0dfcb42c92a1d9dbaf70ad9e621f4bd8d98662f00e3c125",
        "aeb6bcfe274b70a14fb067a5e5578264db0fa9b51af5e0ba159158f329e06e77",
        "d37ee418976dd95753c1c73862b9398fa2a2cf9b4ff0fdfe8b30cd95209614b7",
        "4e3bbb1f7b478dcfe71fb631631519a3bca12c9aefca1612bfce4c13a86264d4",
        "76e67dadbcdf1e10e1b74ddc608abd2f98dfb16fbce75277b5232a127f2087ef",
        "ddb89be403809e325750d3d263cd78929c2942b7942a34b77e122c9594a74c8c",
        "5dc9da79a70659a9ad559cb701ded9a2ab9d823aad2f4960cfe370eff4604328",
    ]
    data = [bytes.fromhex(x) for x in leaves]
    roots = []
    for size in range(1, len(data) + 1):
        got = mth(data[:size]).hex()
        assert got == published[size - 1], f"size {size}: computed {got}"
        roots.append({"size": size, "root": got})
    inclusion = [{"size": n, "index": m, "path": [h.hex() for h in path(m, data[:n])]}
                 for n in range(1, 9) for m in range(n)]
    consistency = [{"old_size": m, "new_size": n, "proof": [h.hex() for h in subproof(m, data[:n], True)]}
                   for n in range(1, 9) for m in range(1, n)]
    write("crates/publet-algorithms/testdata/log.json", {
        "description": "RFC 6962 Merkle Tree Hashes, audit paths and consistency proofs over the "
                       "Certificate Transparency test leaves.",
        "source": "Roots: published by the Certificate Transparency project for these leaves. "
                  "Paths and proofs: RFC 6962 Section 2.1, implemented in tools/vectors/generate.py.",
        "empty_root": H(b"").hex(),
        "leaves": leaves,
        "roots": roots,
        "inclusion": inclusion,
        "consistency": consistency,
    })


# --- publet-algorithms: Fixed6 and versions -----------------------------------

S = 1_000_000


def tdiv(a, b):
    """Integer division truncating toward zero, as Rust's `/` on integers."""
    q = abs(a) // abs(b)
    return q if (a >= 0) == (b >= 0) else -q


def halve(v, num, den):
    if den == 0 or v == 0:
        return v
    for _ in range(min(num // den, 127)):
        v = tdiv(v, 2)
    if num // den >= 127:
        return 0
    r = num % den
    return v if r == 0 else v - tdiv(v * r, den * 2)


def fixed():
    cases = []
    for a, b, c in [(2 * S, S, 3 * S), (-2 * S, S, 3 * S), (S, S, 0), (7 * S, 22 * S, 7 * S)]:
        cases.append({"op": "mul_div", "a": a, "b": b, "c": c, "expect": 0 if c == 0 else tdiv(a * b, c)})
    for a, b in [(S // 2, S // 2), (-S // 2, S // 3), (1, 1), (3 * S, -S)]:
        cases.append({"op": "times", "a": a, "b": b, "expect": tdiv(a * b, S)})
    for a, b in [(S, 3 * S), (-S, 3 * S), (2 * S, 0), (S, 7)]:
        cases.append({"op": "ratio", "a": a, "b": b, "expect": 0 if b == 0 else tdiv(a * S, b)})
    for a, num, den in [(S, 30, 30), (S, 15, 30), (S, 45, 30), (-S, 15, 30), (S, 3810, 30), (S, 1, 0)]:
        cases.append({"op": "halve_fractional", "a": a, "num": num, "den": den, "expect": halve(a, num, den)})
    write("crates/publet-algorithms/testdata/fixed.json", {
        "description": "Fixed6 operations on scaled values (10^6 is one), truncating toward zero.",
        "source": "Integer arithmetic in tools/vectors/generate.py.",
        "cases": cases,
    })


def parts(v):
    try:
        return [int(p) for p in v.split(".")]
    except ValueError:
        return None


def satisfies(version, requires):
    if requires.startswith("^"):
        have, need = parts(version), parts(requires[1:])
        if have is None or need is None:
            return False
        fixed = next((i + 1 for i, p in enumerate(need) if p != 0), len(need))
        same = all((have[i] if i < len(have) else 0) == need[i] for i in range(fixed))
        return same and have >= need
    if requires.endswith("+"):
        have, need = parts(version), parts(requires[:-1])
        return have is not None and need is not None and have >= need
    return requires == "" or version == requires or version.startswith(requires + ".")


def versions():
    pairs = [("0.15", "0.15"), ("0.15.2", "0.15"), ("0.150", "0.15"), ("0.16.0", "0.15"), ("1.2", ""),
             ("1.98.1", "1.80+"), ("1.79.9", "1.80+"), ("22.22.1", "22+"), ("abc", "1+"),
             ("1.9.0", "^1.2"), ("2.0.0", "^1.2"), ("1.1.9", "^1.2"),
             ("0.1.0", "^0.1"), ("0.1.9", "^0.1"), ("0.2.0", "^0.1"), ("0.0.9", "^0.1"), ("1.0.0", "^0.1"),
             ("0.0.3", "^0.0.3"), ("0.0.4", "^0.0.3"), ("0.0.7", "^0.0"), ("0.1.0", "^0.0"), ("x.1", "^0.1")]
    write("crates/publet-algorithms/testdata/versions.json", {
        "description": "Whether a tool's version meets a requirement: ^X.Y as Cargo reads it, X.Y+, a prefix.",
        "source": "The rules as tools/vectors/generate.py implements them, independently of the crate.",
        "cases": [{"version": v, "requires": r, "expect": satisfies(v, r)} for v, r in pairs],
    })


if __name__ == "__main__":
    canonical()
    identifiers()
    signatures()
    log()
    fixed()
    versions()
