//! Properties of the canonical encoding over random values and bytes, and
//! agreement with an independent CBOR implementation.
//!
//! `ciborium` is a general CBOR library, lenient where this crate is strict
//! (see the crate docs' Related crates). That makes it a fair oracle for one
//! direction: everything this crate emits must be ordinary CBOR that
//! ciborium reads to the same values. It is a dev-dependency only.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic
)]

use std::collections::BTreeMap;

use ciborium::value::Value as Theirs;
use proptest::prelude::*;
use publet_core::cbor::{Value, decode, encode};

/// Values in the profile: text is drawn from characters already in NFC.
fn values() -> impl Strategy<Value = Value> {
    let leaf = prop_oneof![
        any::<u64>().prop_map(Value::Uint),
        any::<u64>().prop_map(Value::Nint),
        proptest::collection::vec(any::<u8>(), 0..12).prop_map(Value::Bytes),
        "[a-zé°0-9 ]{0,10}".prop_map(Value::Text),
        any::<bool>().prop_map(Value::Bool),
        Just(Value::Null),
    ];
    leaf.prop_recursive(4, 48, 6, |inner| {
        prop_oneof![
            proptest::collection::vec(inner.clone(), 0..6).prop_map(Value::Array),
            proptest::collection::btree_map("[a-z]{0,5}", inner, 0..6).prop_map(Value::Map),
        ]
    })
}

fn as_theirs(value: &Value) -> Theirs {
    match value {
        Value::Uint(n) => Theirs::Integer((*n).into()),
        Value::Nint(n) => Theirs::Integer((-1 - i128::from(*n)).try_into().unwrap()),
        Value::Bytes(b) => Theirs::Bytes(b.clone()),
        Value::Text(t) => Theirs::Text(t.clone()),
        Value::Array(items) => Theirs::Array(items.iter().map(as_theirs).collect()),
        Value::Map(map) => Theirs::Map(
            map.iter()
                .map(|(k, v)| (Theirs::Text(k.clone()), as_theirs(v)))
                .collect(),
        ),
        Value::Bool(b) => Theirs::Bool(*b),
        Value::Null => Theirs::Null,
        other => panic!("a value the profile cannot hold: {other:?}"),
    }
}

proptest! {
    // covers: cbor::encode, cbor::decode
    #[test]
    fn every_value_round_trips(value in values()) {
        let bytes = encode(&value);
        prop_assert_eq!(decode(&bytes).unwrap(), value);
    }

    // covers: cbor::encode, cbor::decode
    #[test]
    fn accepted_bytes_are_the_only_encoding_of_their_value(bytes in proptest::collection::vec(any::<u8>(), 0..24)) {
        // Most random bytes are refused. Any that are accepted must be the
        // one encoding of their value: re-encoding gives them back exactly.
        if let Ok(value) = decode(&bytes) {
            prop_assert_eq!(encode(&value), bytes);
        }
    }

    // covers: cbor::encode
    #[test]
    fn an_independent_decoder_reads_the_same_values(value in values()) {
        let theirs: Theirs = ciborium::de::from_reader(encode(&value).as_slice()).unwrap();
        prop_assert_eq!(theirs, as_theirs(&value));
    }

    // covers: cbor::decode
    #[test]
    fn a_mutated_encoding_is_refused_or_canonical(value in values(), at in any::<prop::sample::Index>(), byte in any::<u8>()) {
        // Change one byte of a valid encoding. The result may decode, but
        // only if it is, again, the one encoding of what it decodes to.
        let mut bytes = encode(&value);
        let i = at.index(bytes.len());
        bytes[i] = byte;
        if let Ok(decoded) = decode(&bytes) {
            prop_assert_eq!(encode(&decoded), bytes);
        }
    }
}

// covers: cbor::encode
#[test]
fn map_keys_are_ordered_by_bytes_not_by_length() {
    // The one place Section 4.1 departs from RFC 8949's deterministic order.
    let map = Value::Map(BTreeMap::from([
        ("b".to_owned(), Value::Uint(2)),
        ("aa".to_owned(), Value::Uint(1)),
    ]));
    assert_eq!(
        encode(&map),
        [0xa2, 0x62, b'a', b'a', 0x01, 0x61, b'b', 0x02]
    );
}
