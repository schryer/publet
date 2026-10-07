//! The test vectors in `testdata/`, run against this crate.
//!
//! Each file's expected values come from somewhere other than this crate
//! (see its `source` field, and `tools/vectors/generate.py`), so passing
//! them shows agreement with the specification and with independent
//! implementations, not only with this one. They are plain JSON so that
//! another implementation can run the same cases.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic
)]

use publet_core::{Cid, HashAlg, SigAlg, cbor, sign, verify, verifying_key};
use serde_json::Value;

fn load(name: &str) -> Value {
    let path = format!("{}/testdata/{name}", env!("CARGO_MANIFEST_DIR"));
    serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap()
}

fn hex(text: &str) -> Vec<u8> {
    (0..text.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&text[i..i + 2], 16).unwrap())
        .collect()
}

fn cases<'a>(file: &'a Value, key: &str) -> &'a Vec<Value> {
    file[key].as_array().unwrap()
}

fn text<'a>(case: &'a Value, key: &str) -> &'a str {
    case[key].as_str().unwrap()
}

// covers: cbor::decode, cbor::encode
#[test]
fn canonical_bytes_are_accepted_and_re_encode_to_themselves() {
    let file = load("canonical.json");
    for case in cases(&file, "accept") {
        let bytes = hex(text(case, "hex"));
        let value =
            cbor::decode(&bytes).unwrap_or_else(|e| panic!("{}: refused: {e}", text(case, "name")));
        assert_eq!(
            cbor::encode(&value),
            bytes,
            "{}: re-encoded differently",
            text(case, "name")
        );
    }
}

// covers: cbor::decode, CanonError::rule
#[test]
fn non_canonical_bytes_are_refused_by_the_rule_they_break() {
    let file = load("canonical.json");
    for case in cases(&file, "reject") {
        let error = cbor::decode(&hex(text(case, "hex")))
            .err()
            .unwrap_or_else(|| panic!("{}: accepted", text(case, "name")));
        assert_eq!(error.rule(), text(case, "rule"), "{}", text(case, "name"));
    }
}

// covers: Cid, Cid::of
#[test]
fn identifiers_of_known_bytes() {
    let file = load("identifiers.json");
    for case in cases(&file, "compute") {
        let cid = Cid::of(&hex(text(case, "hex")), HashAlg::Sha2_256);
        assert_eq!(cid.to_string(), text(case, "cid"), "{}", text(case, "name"));
        assert_eq!(
            text(case, "cid").parse::<Cid>().unwrap(),
            cid,
            "{}",
            text(case, "name")
        );
    }
}

// covers: Cid
#[test]
fn text_that_is_not_an_identifier_is_refused() {
    let file = load("identifiers.json");
    for case in cases(&file, "invalid") {
        let error = text(case, "text")
            .parse::<Cid>()
            .err()
            .unwrap_or_else(|| panic!("{}: accepted", text(case, "name")));
        let variant = format!("{error:?}");
        assert!(
            variant.starts_with(text(case, "error")),
            "{}: {variant}, expected {}",
            text(case, "name"),
            text(case, "error")
        );
    }
}

// covers: verifying_key
#[test]
fn public_keys_match_rfc_8032() {
    let file = load("signatures.json");
    for case in cases(&file, "keys") {
        let public = verifying_key(SigAlg::Ed25519, &hex(text(case, "seed"))).unwrap();
        assert_eq!(public, hex(text(case, "public")), "{}", text(case, "name"));
    }
}

// covers: sign, verify, verifying_key
#[test]
fn signatures_match_an_independent_implementation() {
    let file = load("signatures.json");
    for case in cases(&file, "signatures") {
        let (seed, purpose) = (hex(text(case, "seed")), text(case, "purpose"));
        let target = hex(text(case, "target_hex"));
        let expected = hex(text(case, "signature"));
        let signature = sign(SigAlg::Ed25519, &seed, purpose, &target).unwrap();
        assert_eq!(signature, expected, "{}", text(case, "name"));
        let public = verifying_key(SigAlg::Ed25519, &seed).unwrap();
        verify(
            SigAlg::Ed25519,
            &public,
            &expected,
            purpose,
            purpose,
            &target,
        )
        .unwrap_or_else(|e| panic!("{}: {e}", text(case, "name")));
    }
}
