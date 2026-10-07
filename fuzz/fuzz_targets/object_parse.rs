//! Object parsing on arbitrary bytes.
//!
//! It must never panic, and an object it accepts must verify against the
//! identifier of the very bytes it was parsed from.
#![no_main]

use libfuzzer_sys::fuzz_target;
use publet_core::{Cid, HashAlg, Object};

fuzz_target!(|data: &[u8]| {
    if let Ok(unverified) = Object::parse(data) {
        let cid = Cid::of(data, HashAlg::Sha2_256);
        let verified = unverified.verify(&cid).expect("an object fails its own identifier");
        assert_eq!(verified.object().bytes(), data);
    }
});
