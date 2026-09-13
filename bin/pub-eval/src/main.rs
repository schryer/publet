//! Evaluate standing under a declared viewpoint.
//!
//! A pure filter: a vector file in, a standing record out. No clock, no
//! network, no randomness. Running it twice on one machine, or once on each
//! of four architectures, must produce identical bytes -- which is what
//! `tools/matrix.sh` checks and what settlement depends on.
//!
//! Output is a single JSON line per target, with every component of the
//! standing exposed. Section 11.3 forbids reducing a standing to a badge
//! without its components, and a tool that emitted only the outcome would
//! make that impossible downstream.

use std::collections::BTreeSet;
use std::path::PathBuf;
use std::process::ExitCode;

use publet_core::{Object, cbor, cbor::Value};
use publet_eval::{
    Evidence, Fixed6, Policy, Reproducibility, Reproductions, TrustEdge, evaluate, propagate,
};
use publet_graph::Class;

const EXIT_VIOLATION: u8 = 1;
const EXIT_USAGE: u8 = 2;
const EXIT_IO: u8 = 4;

fn main() -> ExitCode {
    let mut vector: Option<PathBuf> = None;
    for arg in std::env::args().skip(1) {
        if let Some(v) = arg.strip_prefix("--vector=") {
            vector = Some(PathBuf::from(v));
        } else if arg.starts_with("--") && arg != "--vector" {
            eprintln!("unknown argument: {arg}");
            eprintln!("usage: pub-eval --vector=FILE");
            return ExitCode::from(EXIT_USAGE);
        } else {
            vector = Some(PathBuf::from(arg));
        }
    }

    let Some(path) = vector else {
        eprintln!("a vector file is required: pub-eval --vector=FILE");
        return ExitCode::from(EXIT_USAGE);
    };

    let bytes = match std::fs::read(&path) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("cannot read {}: {e}", path.display());
            return ExitCode::from(EXIT_IO);
        }
    };

    match run(&bytes) {
        Ok(line) => {
            println!("{line}");
            ExitCode::SUCCESS
        }
        Err(message) => {
            eprintln!("{message}");
            ExitCode::from(EXIT_VIOLATION)
        }
    }
}

/// A vector is one canonical CBOR map: a policy object, trust edges, and
/// the evidence gathered for a claim.
fn run(bytes: &[u8]) -> Result<String, String> {
    let value = cbor::decode(bytes).map_err(|e| format!("{} ({})", e, e.rule()))?;

    let policy_bytes = value
        .get("policy")
        .and_then(Value::as_bytes)
        .ok_or("vector is missing `policy`")?;
    let policy_object = Object::parse(policy_bytes)
        .map_err(|e| format!("policy: {e}"))?
        .peek()
        .clone();
    let policy = Policy::from_object(&policy_object).map_err(|e| format!("policy: {e}"))?;

    let edges = read_edges(&value)?;
    let weights = propagate(&policy, &edges);

    let class_id = value
        .get("class")
        .and_then(Value::as_text)
        .ok_or("vector is missing `class`")?;
    let class = Class::from_id(class_id).ok_or(format!("unknown class: {class_id}"))?;

    let evidence = read_evidence(&value);
    let standing = evaluate(&policy, class, &evidence, &weights);

    Ok(format!(
        concat!(
            r#"{{"result":"{}","class":"{}","affirm":"{}","deny":"{}","abstain":"{}","#,
            r#""active":"{}","delta":"{}","retracted":{},"reproducibility":"{}","#,
            r#""reproductions":{{"consistent":{},"inconsistent":{},"inconclusive":{},"#,
            r#""independent_consistent":{},"independent_inconsistent":{}}}}}"#
        ),
        standing.result.id(),
        standing.class.id(),
        standing.affirm_weight,
        standing.deny_weight,
        standing.abstain_weight,
        standing.active_weight,
        standing.delta,
        standing.retracted,
        standing.reproducibility_class.id(),
        standing.reproductions.consistent,
        standing.reproductions.inconsistent,
        standing.reproductions.inconclusive,
        standing.reproductions.independent_consistent,
        standing.reproductions.independent_inconsistent,
    ))
}

fn read_edges(value: &Value) -> Result<Vec<TrustEdge>, String> {
    let Some(Value::Array(items)) = value.get("edges") else {
        return Ok(Vec::new());
    };
    items
        .iter()
        .map(|item| {
            Ok(TrustEdge {
                // A vector states a trust graph directly; confining an edge
                // to subjects is a graph-level fact with no meaning here.
                subjects: Vec::new(),
                from: item
                    .get("from")
                    .and_then(Value::as_text)
                    .ok_or("edge is missing `from`")?
                    .to_owned(),
                to: item
                    .get("to")
                    .and_then(Value::as_text)
                    .ok_or("edge is missing `to`")?
                    .to_owned(),
                weight: Fixed6::from_scaled(
                    i128::from(item.get("weight").and_then(Value::as_uint).unwrap_or(1))
                        * publet_eval::SCALE,
                ),
                age_days: item.get("age_days").and_then(Value::as_uint).unwrap_or(0),
            })
        })
        .collect()
}

fn read_evidence(value: &Value) -> Evidence {
    let set = |field: &str| -> BTreeSet<String> {
        match value.get(field) {
            Some(Value::Array(items)) => items
                .iter()
                .filter_map(|v| v.as_text().map(ToOwned::to_owned))
                .collect(),
            _ => BTreeSet::new(),
        }
    };
    let count = |field: &str| -> u32 {
        value
            .get("reproductions")
            .and_then(|r| r.get(field))
            .and_then(Value::as_uint)
            .and_then(|n| u32::try_from(n).ok())
            .unwrap_or(0)
    };
    Evidence {
        affirm: set("affirm"),
        deny: set("deny"),
        abstain: set("abstain"),
        argued_disputes: set("argued_disputes"),
        proof_checked: matches!(value.get("proof_checked"), Some(Value::Bool(true))),
        reproductions: Reproductions {
            consistent: count("consistent"),
            inconsistent: count("inconsistent"),
            inconclusive: count("inconclusive"),
            independent_consistent: count("independent_consistent"),
            independent_inconsistent: count("independent_inconsistent"),
        },
        reproducibility: value
            .get("reproducibility")
            .and_then(Value::as_text)
            .and_then(Reproducibility::from_id)
            .unwrap_or(Reproducibility::Open),
        retracted: matches!(value.get("retracted"), Some(Value::Bool(true))),
        usage: set("usage"),
        // A vector states evidence directly. Assessments are judgements
        // rather than inputs, so nothing here reads one.
        assessments: Vec::new(),
    }
}
