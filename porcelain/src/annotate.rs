//! `pub annotate`: author an annotation.
//!
//! Annotations are how anything is said *about* an object without touching
//! it (R3, R6). Four kinds are covered here because four were blocking:
//!
//! * `usage` -- corpus evidence. Section 5.2 permits no verdict on a
//!   `definitional` publet and settles it by usage instead, so without this
//!   a definition had no evidence channel whatsoever.
//! * `classifies` -- subject membership, which Section 9.1 makes an
//!   annotation rather than a property of the object.
//! * `trusts` -- trust graph edges, optionally scoped to subjects.
//! * `affiliated` -- Section 10.4, load-bearing for independence.
//!
//! `verdict` is here too, because a command that writes annotations and
//! cannot write the commonest one would be strange. The class rule that
//! rejects a verdict on a non-truth-apt publet is not reimplemented: as in
//! `pub relate`, the object is offered to the loader and stored only if the
//! loader accepts, so there is one implementation of the rule.

use publet_core::{Cid, HashAlg, Object, cbor::Value};
use publet_graph::load;

use crate::workspace::Workspace;

/// Section 5.2. `sound-in-scope` is the one that carries partial
/// correctness: a claim that holds where its scope says and misleads
/// outside it. There is no number for that, and there should not be --
/// naming the conditions says strictly more than a percentage does.
const ASSESSMENTS: [&str; 6] = [
    "sound",
    "sound-in-scope",
    "superseded",
    "unsupported",
    "refuted",
    "undetermined",
];

const ROLES: [&str; 5] = [
    "employer",
    "funder",
    "host-facility",
    "data-provider",
    "sponsor",
];

/// Author an annotation.
///
/// # Errors
///
/// Returns a message if a required field for the kind is missing, an
/// identifier does not parse, or the loader refuses the result.
#[allow(clippy::too_many_lines)]
pub(crate) fn run(args: &[String]) -> Result<(), String> {
    let here = std::env::current_dir().map_err(|e| e.to_string())?;
    let ws = Workspace::open(&here)?;
    let store = ws.store()?;

    let mut kind = None;
    let mut target = None;
    let mut created = "2026-09-12T00:00:00Z".to_owned();
    let mut flags: std::collections::BTreeMap<String, String> = std::collections::BTreeMap::new();
    let mut subjects: Vec<String> = Vec::new();

    for arg in args {
        if let Some(v) = arg.strip_prefix("--kind=") {
            kind = Some(v.to_owned());
        } else if let Some(v) = arg.strip_prefix("--target=") {
            target = Some(v.to_owned());
        } else if let Some(v) = arg.strip_prefix("--created=") {
            v.clone_into(&mut created);
        } else if let Some(v) = arg.strip_prefix("--subject=") {
            subjects.push(v.to_owned());
        } else if let Some(rest) = arg.strip_prefix("--")
            && let Some((name, value)) = rest.split_once('=')
        {
            flags.insert(name.to_owned(), value.to_owned());
        } else {
            return Err(format!("unknown argument: {arg}"));
        }
    }

    let kind_id =
        kind.ok_or("--kind is required: usage, classifies, trusts, affiliated, or verdict")?;
    let target = target.ok_or("--target is required: the CID being annotated")?;
    let target_cid: Cid = target
        .parse()
        .map_err(|_| format!("--target is not a CID: {target}"))?;

    let need = |name: &str, why: &str| -> Result<String, String> {
        flags
            .get(name)
            .cloned()
            .ok_or_else(|| format!("--{name} is required for `{kind_id}`: {why}"))
    };

    let mut value: std::collections::BTreeMap<String, Value> = std::collections::BTreeMap::new();
    match kind_id.as_str() {
        "usage" => {
            // A citation names where a term is used. It deliberately need
            // not carry the text: recording a location is what keeps a
            // corpus of definitions from becoming a corpus of restatements,
            // and it is why citing a source needs no licence from it.
            let source = need("source", "a citable work this sense appears in")?;
            value.insert("source".to_owned(), Value::Text(source));
            for optional in ["locator", "sense"] {
                if let Some(v) = flags.get(optional) {
                    value.insert(optional.to_owned(), Value::Text(v.clone()));
                }
            }
        }
        "classifies" => {
            let subject = subjects
                .first()
                .cloned()
                .or_else(|| flags.get("subject").cloned())
                .ok_or("--subject is required for `classifies`: the CID of the subject publet")?;
            subject
                .parse::<Cid>()
                .map_err(|_| format!("--subject is not a CID: {subject}"))?;
            value.insert("subject".to_owned(), Value::Text(subject));
        }
        "trusts" => {
            let raw = need("weight", "1..1000")?;
            let weight: u64 = raw
                .parse()
                .map_err(|_| format!("--weight is not a number: {raw}"))?;
            if !(1..=1000).contains(&weight) {
                return Err(format!("--weight must be 1..1000, got {weight}"));
            }
            value.insert("weight".to_owned(), Value::Uint(weight));
            // An unscoped edge trusts the key on everything. Naming
            // subjects confines it, which is the difference between
            // trusting a language reference on that language and trusting
            // it on the world.
            if !subjects.is_empty() {
                for subject in &subjects {
                    subject
                        .parse::<Cid>()
                        .map_err(|_| format!("--subject is not a CID: {subject}"))?;
                }
                value.insert(
                    "subjects".to_owned(),
                    Value::Array(subjects.iter().map(|s| Value::Text(s.clone())).collect()),
                );
            }
        }
        "affiliated" => {
            let org = need("org", "the CID of the organization's key")?;
            org.parse::<Cid>()
                .map_err(|_| format!("--org is not a CID: {org}"))?;
            let role = need("role", &format!("one of {}", ROLES.join(", ")))?;
            if !ROLES.contains(&role.as_str()) {
                return Err(format!("unknown affiliation role: {role}"));
            }
            value.insert("org".to_owned(), Value::Text(org));
            value.insert("role".to_owned(), Value::Text(role));
            let mut period: std::collections::BTreeMap<String, Value> =
                std::collections::BTreeMap::new();
            for bound in ["from", "to"] {
                if let Some(v) = flags.get(bound) {
                    period.insert(bound.to_owned(), Value::Text(v.clone()));
                }
            }
            if !period.is_empty() {
                value.insert("period".to_owned(), Value::Map(period));
            }
            value.insert(
                "disclosed_by".to_owned(),
                Value::Text(flags.get("disclosed_by").cloned().unwrap_or("self".into())),
            );
        }
        "assessment" => {
            let verdict = need("verdict", &format!("one of {}", ASSESSMENTS.join(", ")))?;
            if !ASSESSMENTS.contains(&verdict.as_str()) {
                return Err(format!("unknown assessment: {verdict}"));
            }
            // A judgement with no stated basis is a preference. The reader
            // cannot weigh a preference, so it is required rather than
            // stored and ignored.
            let basis = need("basis", "what this judgement rests on")?;
            value.insert("verdict".to_owned(), Value::Text(verdict));
            value.insert("basis".to_owned(), Value::Text(basis));
        }
        "verdict" => {
            let finding = need("finding", "affirm, deny, or abstain")?;
            if !["affirm", "deny", "abstain"].contains(&finding.as_str()) {
                return Err(format!("unknown finding: {finding}"));
            }
            value.insert("finding".to_owned(), Value::Text(finding));
            if let Some(aspect) = flags.get("aspect") {
                value.insert("aspect".to_owned(), Value::Text(aspect.clone()));
            }
        }
        other => {
            return Err(format!(
                "unknown or unsupported annotation kind: {other}\n\
                 supported: usage, classifies, trusts, affiliated, \
                 assessment, verdict"
            ));
        }
    }

    let author = ws
        .get("author")
        .ok_or("no author configured; run `pub init`")?;

    let bytes = Object::builder("ann", &author)
        .created(&created)
        .field("kind", Value::Text(kind_id.clone()))
        .field("target", Value::Text(target.clone()))
        .field("value", Value::Map(value))
        .build()
        .map_err(|e| e.to_string())?;
    let cid = Cid::of(&bytes, HashAlg::Sha2_256);

    // Offer it to the loader before storing. The class rules of Section 5.2
    // -- no verdict on a definitional, normative, or expressive publet --
    // are enforced there, and enforcing them again here is how the two
    // would come to disagree.
    let mut objects = vec![(cid.clone(), bytes.clone())];
    for cid_text in store.cids().map_err(|e| e.to_string())? {
        let Ok(held) = cid_text.parse::<Cid>() else {
            continue;
        };
        if let Ok(Some(held_bytes)) = store.get(&held) {
            objects.push((held, held_bytes));
        }
    }
    load::from_objects(objects)
        .map_err(|e| format!("this annotation was refused, so it was not stored: {e}"))?;

    store.put(&cid, &bytes).map_err(|e| e.to_string())?;
    println!("{cid}");

    if kind_id == "usage" {
        eprintln!();
        eprintln!(
            "A definition is settled by usage, not by verdict (Section 5.2). \
             Run `pub why {target_cid}` to see what now cites it."
        );
    }
    Ok(())
}
