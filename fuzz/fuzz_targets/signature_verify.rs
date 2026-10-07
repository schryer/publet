//! Signature checking on arbitrary keys, signatures and purposes.
//!
//! Keys and signatures arrive inside objects from anyone. Checking one must
//! never panic, whatever the lengths; it is simply valid or not.
#![no_main]

use arbitrary::Arbitrary;
use libfuzzer_sys::fuzz_target;
use publet_core::{SigAlg, verify};

#[derive(Arbitrary, Debug)]
struct Input {
    public_key: Vec<u8>,
    signature: Vec<u8>,
    found_purpose: String,
    expected_purpose: String,
    target: Vec<u8>,
}

fuzz_target!(|input: Input| {
    let _ = verify(
        SigAlg::Ed25519,
        &input.public_key,
        &input.signature,
        &input.found_purpose,
        &input.expected_purpose,
        &input.target,
    );
});
