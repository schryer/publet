//! The canonical CBOR decoder on arbitrary bytes.
//!
//! It must never panic, and anything it accepts must be the one canonical
//! encoding of its value: re-encoding gives back exactly the input.
#![no_main]

use libfuzzer_sys::fuzz_target;
use publet_core::cbor::{decode, encode};

fuzz_target!(|data: &[u8]| {
    if let Ok(value) = decode(data) {
        assert_eq!(encode(&value), data, "accepted bytes that are not the canonical encoding");
    }
});
