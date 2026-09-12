//! Canonical-form rejection tests.
#![allow(clippy::unwrap_used, clippy::expect_used)]

//! Every rejection rule in Section 4.1, exercised with crafted bytes.
//!
//! A decoder that accepts what it should reject fails silently in
//! production and is invisible to positive tests, so each rule has an
//! input constructed to violate exactly it.

use publet_core::cbor::{Value, decode, encode};
use std::collections::BTreeMap;

fn rule_for(bytes: &[u8]) -> &'static str {
    decode(bytes).expect_err("expected rejection").rule()
}

#[test]
fn accepts_canonical_forms() {
    assert_eq!(decode(&[0x05]).unwrap(), Value::Uint(5));
    assert_eq!(decode(&[0x18, 0x64]).unwrap(), Value::Uint(100));
    assert_eq!(decode(&[0x20]).unwrap(), Value::Nint(0)); // -1
    assert_eq!(decode(&[0xf4]).unwrap(), Value::Bool(false));
    assert_eq!(decode(&[0xf5]).unwrap(), Value::Bool(true));
    assert_eq!(decode(&[0xf6]).unwrap(), Value::Null);
    assert_eq!(
        decode(&[0x43, 1, 2, 3]).unwrap(),
        Value::Bytes(vec![1, 2, 3])
    );
    assert_eq!(decode(b"\x63abc").unwrap(), Value::Text("abc".into()));
}

#[test]
fn rejects_non_shortest_integer() {
    // 5 encoded in two bytes rather than one.
    assert_eq!(rule_for(&[0x18, 0x05]), "shortest-form integers");
    // 100 encoded in three bytes rather than two.
    assert_eq!(rule_for(&[0x19, 0x00, 0x64]), "shortest-form integers");
    // 0 encoded in nine bytes.
    assert_eq!(
        rule_for(&[0x1b, 0, 0, 0, 0, 0, 0, 0, 0]),
        "shortest-form integers"
    );
}

#[test]
fn rejects_indefinite_length() {
    assert_eq!(rule_for(&[0x9f, 0x01, 0xff]), "no indefinite length"); // array
    assert_eq!(rule_for(&[0xbf, 0xff]), "no indefinite length"); // map
    assert_eq!(rule_for(&[0x5f, 0xff]), "no indefinite length"); // bytes
}

#[test]
fn rejects_floats() {
    assert_eq!(rule_for(&[0xf9, 0x3c, 0x00]), "no floating point"); // half 1.0
    assert_eq!(
        rule_for(&[0xfa, 0x3f, 0x80, 0x00, 0x00]),
        "no floating point"
    ); // single 1.0
    assert_eq!(
        rule_for(&[0xfb, 0x3f, 0xf0, 0, 0, 0, 0, 0, 0]),
        "no floating point"
    ); // double 1.0
}

#[test]
fn rejects_non_text_map_keys() {
    // {1: 2}
    assert_eq!(rule_for(&[0xa1, 0x01, 0x02]), "text map keys");
}

#[test]
fn rejects_unsorted_map_keys() {
    // {"b": 1, "a": 2}
    assert_eq!(rule_for(b"\xa2\x61b\x01\x61a\x02"), "sorted map keys");
}

#[test]
fn rejects_duplicate_map_keys() {
    // {"a": 1, "a": 2}
    assert_eq!(rule_for(b"\xa2\x61a\x01\x61a\x02"), "unique map keys");
}

#[test]
fn rejects_invalid_utf8() {
    assert_eq!(rule_for(&[0x62, 0xff, 0xfe]), "valid UTF-8");
}

#[test]
fn rejects_non_nfc_text() {
    // "e" followed by COMBINING ACUTE ACCENT is NFD; NFC is U+00E9.
    let mut bytes = vec![0x63];
    bytes.extend_from_slice("e\u{0301}".as_bytes());
    assert_eq!(rule_for(&bytes), "NFC normalization");
    // The NFC form of the same character is accepted.
    let mut ok = vec![0x62];
    ok.extend_from_slice("\u{00e9}".as_bytes());
    assert_eq!(decode(&ok).unwrap(), Value::Text("é".into()));
}

#[test]
fn rejects_tags() {
    assert_eq!(rule_for(&[0xc0, 0x01]), "supported item types");
}

#[test]
fn rejects_trailing_data() {
    assert_eq!(rule_for(&[0x01, 0x02]), "single top-level item");
}

#[test]
fn rejects_truncated_input() {
    assert_eq!(rule_for(&[0x18]), "complete items");
    assert_eq!(rule_for(&[0x43, 1, 2]), "complete items");
    assert_eq!(rule_for(&[]), "complete items");
}

#[test]
fn rejects_oversized_input() {
    let big = vec![0u8; 64 * 1024 + 1];
    assert_eq!(rule_for(&big), "object size limit");
}

#[test]
fn rejects_deep_nesting() {
    // 100 nested single-element arrays exceeds the default depth of 64.
    let mut bytes = vec![0x81; 100];
    bytes.push(0x01);
    assert_eq!(rule_for(&bytes), "nesting limit");
}

#[test]
fn round_trips_a_realistic_object() {
    let mut body = BTreeMap::new();
    body.insert("class".to_owned(), Value::Text("empirical".into()));
    body.insert("lang".to_owned(), Value::Text("en".into()));

    let mut obj = BTreeMap::new();
    obj.insert("pub".to_owned(), Value::Text("1".into()));
    obj.insert("type".to_owned(), Value::Text("publet".into()));
    obj.insert("body".to_owned(), Value::Map(body));

    let value = Value::Map(obj);
    let bytes = encode(&value);
    assert_eq!(decode(&bytes).unwrap(), value);
    // Re-encoding the decoded value reproduces the exact bytes.
    assert_eq!(encode(&decode(&bytes).unwrap()), bytes);
}

#[test]
fn encoder_emits_sorted_keys_regardless_of_insertion_order() {
    let mut m = BTreeMap::new();
    m.insert("zebra".to_owned(), Value::Uint(1));
    m.insert("apple".to_owned(), Value::Uint(2));
    let bytes = encode(&Value::Map(m));
    // "apple" must precede "zebra" in the encoding.
    let apple = bytes.windows(5).position(|w| w == b"apple").unwrap();
    let zebra = bytes.windows(5).position(|w| w == b"zebra").unwrap();
    assert!(apple < zebra, "keys must be emitted in UTF-8 byte order");
    assert!(decode(&bytes).is_ok());
}
