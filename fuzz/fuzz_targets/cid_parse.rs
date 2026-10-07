//! Identifier parsing on arbitrary text.
//!
//! It must never panic, and an identifier it accepts must print back as
//! exactly the text it was parsed from: there is one text form.
#![no_main]

use libfuzzer_sys::fuzz_target;
use publet_core::Cid;

fuzz_target!(|text: &str| {
    if let Ok(cid) = text.parse::<Cid>() {
        assert_eq!(cid.to_string(), text, "accepted a second text form of an identifier");
    }
});
