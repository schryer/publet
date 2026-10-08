//! The test vectors in `testdata/`, run against this crate.
//!
//! Each file's expected values come from somewhere other than this crate
//! (see its `source` field, and `tools/vectors/generate.py`): the RFC 6962
//! roots are the Certificate Transparency project's own. They are plain
//! JSON so that another implementation can run the same cases.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic
)]

use publet_algorithms::Fixed6;
use publet_algorithms::log::{
    Hash, consistency_proof, empty_root, inclusion_proof, leaf_hash, root, to_hex,
    verify_consistency, verify_inclusion,
};
use publet_algorithms::version::satisfies;
use serde_json::Value;

fn load(name: &str) -> Value {
    let path = format!("{}/testdata/{name}", env!("CARGO_MANIFEST_DIR"));
    serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap()
}

fn bytes(text: &str) -> Vec<u8> {
    (0..text.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&text[i..i + 2], 16).unwrap())
        .collect()
}

fn hash(text: &str) -> Hash {
    bytes(text).try_into().unwrap()
}

fn hashes(list: &Value) -> Vec<Hash> {
    list.as_array()
        .unwrap()
        .iter()
        .map(|h| hash(h.as_str().unwrap()))
        .collect()
}

fn size(case: &Value, key: &str) -> usize {
    usize::try_from(case[key].as_u64().unwrap()).unwrap()
}

fn leaves() -> Vec<Hash> {
    load("log.json")["leaves"]
        .as_array()
        .unwrap()
        .iter()
        .map(|l| leaf_hash(&bytes(l.as_str().unwrap())))
        .collect()
}

// covers: log::empty_root, log::root, log::leaf_hash, log::to_hex
#[test]
fn roots_match_the_certificate_transparency_test_vectors() {
    let file = load("log.json");
    assert_eq!(to_hex(&empty_root()), file["empty_root"].as_str().unwrap());
    let leaves = leaves();
    for case in file["roots"].as_array().unwrap() {
        let n = size(case, "size");
        assert_eq!(
            to_hex(&root(&leaves[..n])),
            case["root"].as_str().unwrap(),
            "size {n}"
        );
    }
}

// covers: log::inclusion_proof, log::verify_inclusion
#[test]
fn audit_paths_match_rfc_6962() {
    let file = load("log.json");
    let leaves = leaves();
    for case in file["inclusion"].as_array().unwrap() {
        let (n, m) = (size(case, "size"), size(case, "index"));
        let expected = hashes(&case["path"]);
        assert_eq!(
            inclusion_proof(&leaves[..n], m),
            expected,
            "leaf {m} of {n}"
        );
        assert!(
            verify_inclusion(
                &leaves[m],
                m as u64,
                n as u64,
                &expected,
                &root(&leaves[..n])
            ),
            "leaf {m} of {n}"
        );
    }
}

// covers: log::consistency_proof, log::verify_consistency
#[test]
fn consistency_proofs_match_rfc_6962() {
    let file = load("log.json");
    let leaves = leaves();
    for case in file["consistency"].as_array().unwrap() {
        let (m, n) = (size(case, "old_size"), size(case, "new_size"));
        let expected = hashes(&case["proof"]);
        assert_eq!(consistency_proof(&leaves[..n], m), expected, "{m} to {n}");
        assert!(
            verify_consistency(
                m as u64,
                n as u64,
                &root(&leaves[..m]),
                &root(&leaves[..n]),
                &expected
            ),
            "{m} to {n}"
        );
    }
}

// covers: Fixed6::mul_div, Fixed6::times, Fixed6::ratio, Fixed6::halve_fractional, Fixed6::from_scaled, Fixed6::to_scaled
#[test]
fn fixed_point_results_match_integer_arithmetic() {
    let file = load("fixed.json");
    let int = |case: &Value, key: &str| i128::from(case[key].as_i64().unwrap());
    let fixed = |case: &Value, key: &str| Fixed6::from_scaled(int(case, key));
    for case in file["cases"].as_array().unwrap() {
        let op = case["op"].as_str().unwrap();
        let got = match op {
            "mul_div" => fixed(case, "a").mul_div(fixed(case, "b"), fixed(case, "c")),
            "times" => fixed(case, "a").times(fixed(case, "b")),
            "ratio" => fixed(case, "a").ratio(fixed(case, "b")),
            "halve_fractional" => fixed(case, "a")
                .halve_fractional(case["num"].as_u64().unwrap(), case["den"].as_u64().unwrap()),
            other => panic!("unknown op {other}"),
        };
        assert_eq!(got.to_scaled(), int(case, "expect"), "{op} {case}");
    }
}

// covers: version::satisfies
#[test]
fn version_requirements_match_the_rules() {
    let file = load("versions.json");
    for case in file["cases"].as_array().unwrap() {
        let (version, requires) = (
            case["version"].as_str().unwrap(),
            case["requires"].as_str().unwrap(),
        );
        assert_eq!(
            satisfies(version, requires),
            case["expect"].as_bool().unwrap(),
            "{version} against {requires}"
        );
    }
}
