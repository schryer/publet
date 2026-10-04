//! `pub compose`: build a claim and add it to the workspace.
//!
//! Scope is required, not optional. An assertion that states its own
//! validity conditions does not drift, because nothing was left implicit to
//! drift (R4); one that does not is a different assertion every time it is
//! read. The command therefore refuses to build without one rather than
//! supplying a default that would be a claim the author never made.

use std::collections::BTreeMap;
use std::path::PathBuf;

use publet_core::{Cid, HashAlg, Object, cbor::Value};
use publet_graph::{Class, ProseClaim, check};
use publet_store::Store;

use crate::workspace::Workspace;

/// The creation instant used when `--created` is not given.
pub(crate) const DEFAULT_CREATED: &str = "2026-09-12T00:00:00Z";

/// One `--source=`, with the `--source-*` flags that followed it.
#[derive(Debug, Default, Clone)]
pub(crate) struct SourceArg {
    reference: String,
    revision: Option<String>,
    locator: Option<String>,
    query: Option<String>,
    note: Option<String>,
}

impl SourceArg {
    /// A source given other than by flags, as `pub build` reads one.
    pub(crate) fn new(
        reference: String,
        revision: Option<String>,
        locator: Option<String>,
        query: Option<String>,
        note: Option<String>,
    ) -> Self {
        Self {
            reference,
            revision,
            locator,
            query,
            note,
        }
    }
}

/// The flags `pub compose` and `pub revise` share.
///
/// `pub revise` reads the same flags as `pub compose` and applies only the
/// ones given, so both commands parse them here: two parsers for one set
/// of flags is how the two would come to disagree about what one means.
#[derive(Debug, Default)]
pub(crate) struct ClaimFlags {
    pub(crate) class: Option<String>,
    pub(crate) content: Option<String>,
    pub(crate) scope: Option<String>,
    pub(crate) lang: Option<String>,
    pub(crate) depends: Vec<String>,
    pub(crate) method: Option<String>,
    pub(crate) cites: Vec<String>,
    pub(crate) cite_notes: Vec<String>,
    pub(crate) sources: Vec<SourceArg>,
    pub(crate) data: Option<PathBuf>,
    pub(crate) no_data: bool,
    pub(crate) created: Option<String>,
}

impl ClaimFlags {
    /// Take `arg` if it is one of these flags.
    ///
    /// Returns `Ok(false)` for an argument that is not, so the caller can
    /// accept its own flags or refuse the argument.
    ///
    /// # Errors
    ///
    /// Returns a message if a `--source-*` flag precedes any `--source`.
    pub(crate) fn take(&mut self, arg: &str) -> Result<bool, String> {
        if let Some(v) = arg.strip_prefix("--class=") {
            self.class = Some(v.to_owned());
        } else if let Some(v) = arg.strip_prefix("--content=") {
            self.content = Some(v.to_owned());
        } else if let Some(v) = arg.strip_prefix("--scope=") {
            self.scope = Some(v.to_owned());
        } else if let Some(v) = arg.strip_prefix("--lang=") {
            self.lang = Some(v.to_owned());
        } else if let Some(v) = arg.strip_prefix("--method=") {
            self.method = Some(v.to_owned());
        } else if let Some(v) = arg.strip_prefix("--depends=") {
            self.depends.push(v.to_owned());
        } else if let Some(v) = arg.strip_prefix("--cite=") {
            self.cites.push(v.to_owned());
        } else if let Some(v) = arg.strip_prefix("--cite-note=") {
            self.cite_notes.push(v.to_owned());
        } else if let Some(v) = arg.strip_prefix("--data=") {
            self.data = Some(PathBuf::from(v));
        } else if arg == "--no-data" {
            self.no_data = true;
        } else if let Some(v) = arg.strip_prefix("--source=") {
            self.sources.push(SourceArg {
                reference: v.to_owned(),
                ..SourceArg::default()
            });
        } else if let Some(v) = arg.strip_prefix("--source-revision=") {
            self.current_source("--source-revision")?.revision = Some(v.to_owned());
        } else if let Some(v) = arg.strip_prefix("--source-locator=") {
            self.current_source("--source-locator")?.locator = Some(v.to_owned());
        } else if let Some(v) = arg.strip_prefix("--source-query=") {
            self.current_source("--source-query")?.query = Some(v.to_owned());
        } else if let Some(v) = arg.strip_prefix("--source-note=") {
            self.current_source("--source-note")?.note = Some(v.to_owned());
        } else if let Some(v) = arg.strip_prefix("--created=") {
            self.created = Some(v.to_owned());
        } else {
            return Ok(false);
        }
        Ok(true)
    }

    /// The source the most recent `--source=` started, for a following
    /// `--source-*` flag to attach to -- the same pairing-by-position
    /// `pub document` uses for its items, for the same reason: a CID holds
    /// colons, so one flag cannot carry several fields unambiguously.
    fn current_source(&mut self, flag: &str) -> Result<&mut SourceArg, String> {
        self.sources
            .last_mut()
            .ok_or_else(|| format!("{flag} given before any --source"))
    }
}

/// Compose a claim.
///
/// # Errors
///
/// Returns a message if a required field is missing or the class is unknown.
pub(crate) fn run(args: &[String]) -> Result<(), String> {
    let here = std::env::current_dir().map_err(|e| e.to_string())?;
    let ws = Workspace::open(&here)?;
    let store = ws.store()?;

    let mut flags = ClaimFlags::default();
    for arg in args {
        if !flags.take(arg)? {
            return Err(format!("unknown argument: {arg}"));
        }
    }

    check_cite_notes(&flags.cites, &flags.cite_notes)?;
    if flags.no_data {
        return Err("--no-data is for `pub revise`: a new claim that carries \
                    no data simply omits --data"
            .to_owned());
    }

    let class_id = flags.class.clone().ok_or(
        "--class is required: formal, empirical, attributive, definitional, \
         normative, expressive, archival, or procedural",
    )?;
    let parsed_class =
        Class::from_id(&class_id).ok_or(format!("unknown claim class: {class_id}"))?;
    let content = flags.content.clone().ok_or("--content is required")?;
    let scope = flags.scope.clone().ok_or(
        "--scope is required. State the conditions you assert this under, or \
         \"unconditional\" if you really mean that (Section 5.3)",
    )?;

    // Section 5.5: an empirical claim must name a method others can
    // execute. Supplying a placeholder would assert a reproducibility the
    // author never offered, so the command refuses instead.
    if parsed_class == Class::Empirical && flags.method.is_none() {
        return Err("--method is required for an empirical claim: the CID of a \
             `procedural` claim describing how the observation may be \
             repeated. A measurement without one is a report of an \
             experience (Section 5.5)"
            .to_owned());
    }

    let author = ws
        .get("author")
        .ok_or("no author configured; run `pub init`")?;

    let (source_entries, source_claims) = source_entries(&store, &flags.sources)?;
    let mut depends = flags.depends.clone();
    add_missing(&mut depends, &source_claims);
    let Value::Array(mut evidence) =
        evidence_for(flags.method.as_deref(), &flags.cites, &flags.cite_notes)
    else {
        return Err("evidence is not a list".to_owned());
    };
    evidence.extend(source_entries);

    let mut body = BTreeMap::new();
    body.insert("class".to_owned(), Value::Text(class_id.clone()));
    body.insert(
        "lang".to_owned(),
        Value::Text(flags.lang.clone().unwrap_or_else(|| "en".to_owned())),
    );
    body.insert("content".to_owned(), Value::Text(content));
    body.insert("scope".to_owned(), scope_value(&scope));
    body.insert("depends".to_owned(), text_array(&depends));
    body.insert("evidence".to_owned(), Value::Array(evidence));
    if let Some(path) = &flags.data {
        body.insert(
            "data".to_owned(),
            crate::payload::data_from_file(&store, path)?,
        );
    }

    let created = flags.created.as_deref().unwrap_or(DEFAULT_CREATED);
    let (cid, bytes, claim) = build_claim(&author, created, body)?;
    store.put(&cid, &bytes).map_err(|e| e.to_string())?;

    println!("{cid}");
    warn_about(&claim);

    if parsed_class == Class::Empirical {
        eprintln!();
        eprintln!("This is an empirical claim. It cannot reach `accepted` on");
        eprintln!("endorsement alone: it needs a method others can execute and");
        eprintln!("independent reproductions of it (Section 11.4.1).");
    }
    Ok(())
}

/// Build a prose claim from a body and read it back through the graph's
/// own view of one, so that a body the loader would refuse -- a ragged
/// table, a source claim missing from `depends` -- is refused here before
/// anything is stored, by the same code that would refuse it later.
///
/// # Errors
///
/// Returns a message if the object cannot be built or the graph refuses it.
pub(crate) fn build_claim(
    author: &str,
    created: &str,
    body: BTreeMap<String, Value>,
) -> Result<(Cid, Vec<u8>, ProseClaim), String> {
    let mut builder = Object::builder("claim.prose", author).created(created);
    for (key, value) in body {
        builder = builder.field(&key, value);
    }
    let bytes = builder.build().map_err(|e| e.to_string())?;
    let cid = Cid::of(&bytes, HashAlg::Sha2_256);
    let verified = Object::parse(&bytes)
        .map_err(|e| e.to_string())?
        .verify(&cid)
        .map_err(|e| e.to_string())?;
    let claim = ProseClaim::from_object(cid.clone(), verified.object())
        .map_err(|e| format!("this claim was refused, so it was not stored: {e}"))?;
    Ok((cid, bytes, claim))
}

/// Structural findings and a missing source are warnings, not validity
/// rules: the pressure belongs on the author now, when the fix is cheap.
pub(crate) fn warn_about(claim: &ProseClaim) {
    let findings = check(claim);
    if !findings.is_empty() {
        eprintln!();
        eprintln!("structural findings (warnings, not errors):");
        for finding in findings {
            eprintln!("  {}: {}", finding.test, finding.detail);
        }
    }
    if claim.data().is_some() && claim.sources().is_empty() {
        eprintln!();
        eprintln!("This claim carries data but names no --source it was read");
        eprintln!("from. A reader cannot trace a figure without one (Section 5.5).");
    }
}

/// The `source` evidence entries for `--source` flags, and which of them
/// name claims (Section 5.5).
///
/// The kind is read off what the reference is rather than asked for: a CID
/// held here as an object is a `claim`, one held as a blob is a `blob`,
/// and anything else -- a repository URL, a warehouse table -- is
/// `external` and will be displayed as unverified. A CID held as neither
/// is refused, because nothing here could say what it names.
///
/// # Errors
///
/// Returns a message if a source is a CID this workspace does not hold.
pub(crate) fn source_entries(
    store: &Store,
    sources: &[SourceArg],
) -> Result<(Vec<Value>, Vec<String>), String> {
    let mut entries = Vec::new();
    let mut claims = Vec::new();
    for source in sources {
        let kind = match source.reference.parse::<Cid>() {
            Ok(cid) if store.contains(&cid).map_err(|e| e.to_string())? => {
                claims.push(source.reference.clone());
                "claim"
            }
            Ok(cid) if store.get_blob(&cid).map_err(|e| e.to_string())?.is_some() => "blob",
            Ok(_) => {
                return Err(format!(
                    "--source={} is a CID held here neither as an object nor \
                     as a blob, so nothing could verify what it names",
                    source.reference
                ));
            }
            Err(_) => "external",
        };
        let mut entry = BTreeMap::new();
        entry.insert("kind".to_owned(), Value::Text(kind.to_owned()));
        entry.insert("role".to_owned(), Value::Text("source".to_owned()));
        entry.insert("ref".to_owned(), Value::Text(source.reference.clone()));
        let optional = [
            ("revision", &source.revision),
            ("locator", &source.locator),
            ("query", &source.query),
            ("note", &source.note),
        ];
        for (name, value) in optional {
            if let Some(value) = value {
                entry.insert(name.to_owned(), Value::Text(value.clone()));
            }
        }
        entries.push(Value::Map(entry));
    }
    Ok((entries, claims))
}

/// Append each of `extra` not already in `list`, keeping order.
pub(crate) fn add_missing(list: &mut Vec<String>, extra: &[String]) {
    for item in extra {
        if !list.contains(item) {
            list.push(item.clone());
        }
    }
}

/// An array of text values.
pub(crate) fn text_array(items: &[String]) -> Value {
    Value::Array(items.iter().map(|d| Value::Text(d.clone())).collect())
}

/// A note is paired with the citation at the same position; a citation may
/// be given without one, but a note cannot outnumber the citations it
/// annotates -- there would be nothing left to say it about.
pub(crate) fn check_cite_notes(cites: &[String], cite_notes: &[String]) -> Result<(), String> {
    if cite_notes.len() > cites.len() {
        return Err(format!(
            "{} --cite-note value(s) given but only {} --cite value(s); \
             each --cite-note pairs with the --cite given at the same position",
            cite_notes.len(),
            cites.len()
        ));
    }
    Ok(())
}

/// The evidence list (Section 5.5): a `method` entry when one was named,
/// followed by a `citation` entry for each `--cite`.
///
/// A citation names a source lying outside this corpus's integrity
/// guarantees, so `kind` is `"external"` and `ref` is whatever the author
/// gave -- a DOI, a URL, a stable identifier -- displayed as unverified,
/// per Section 5.5. This is deliberately the only evidence role `pub
/// compose` writes besides `method`: `measurement`, `derivation`, and
/// `replication` name evidence a claim's own author produced, which is a
/// different act from citing someone else's, and conflating them here
/// would let a citation masquerade as one of the roles Section 11.4
/// weighs more heavily.
pub(crate) fn evidence_for(method: Option<&str>, cites: &[String], cite_notes: &[String]) -> Value {
    let mut entries = Vec::new();

    if let Some(method) = method {
        let mut entry = std::collections::BTreeMap::new();
        entry.insert("kind".to_owned(), Value::Text("claim".into()));
        entry.insert("role".to_owned(), Value::Text("method".into()));
        entry.insert("ref".to_owned(), Value::Text(method.to_owned()));
        entries.push(Value::Map(entry));
    }

    for (i, cite) in cites.iter().enumerate() {
        let mut entry = std::collections::BTreeMap::new();
        entry.insert("kind".to_owned(), Value::Text("external".into()));
        entry.insert("role".to_owned(), Value::Text("citation".into()));
        entry.insert("ref".to_owned(), Value::Text(cite.clone()));
        if let Some(note) = cite_notes.get(i) {
            entry.insert("note".to_owned(), Value::Text(note.clone()));
        }
        entries.push(Value::Map(entry));
    }

    Value::Array(entries)
}

/// Offer a new object to the loader against everything already held, and
/// store it only if the loader accepts.
///
/// The rules that decide whether an object may enter a graph live in
/// `publet_graph` (Section 5.2's class rules, Section 6's acyclicity), and
/// checking them a second time here is how the two implementations would
/// come to disagree. `refused` names what the caller was trying to do, so
/// the message reads in terms of the command the author ran.
///
/// # Errors
///
/// Returns a message if the loader refuses the object or the store fails.
pub(crate) fn offer_then_store(
    store: &publet_store::Store,
    cid: &Cid,
    bytes: &[u8],
    refused: &str,
) -> Result<(), String> {
    let mut objects = vec![(cid.clone(), bytes.to_vec())];
    for cid_text in store.cids().map_err(|e| e.to_string())? {
        let Ok(held) = cid_text.parse::<Cid>() else {
            continue;
        };
        if let Ok(Some(held_bytes)) = store.get(&held) {
            objects.push((held, held_bytes));
        }
    }
    publet_graph::load::from_objects(objects)
        .map_err(|e| format!("{refused}, so it was not stored: {e}"))?;
    store.put(cid, bytes).map_err(|e| e.to_string())
}

/// The scope every claim states (R4, Section 5.3).
///
/// One shape for all three grammars: `pub compose`, `pub relate`, and
/// `pub annotate` each state the conditions their author asserts under,
/// and `"unconditional"` is the explicit way of saying there are none.
pub(crate) fn scope_value(domain: &str) -> Value {
    let mut map = std::collections::BTreeMap::new();
    map.insert("domain".to_owned(), Value::Text(domain.to_owned()));
    map.insert("conditions".to_owned(), Value::Array(Vec::new()));
    Value::Map(map)
}

/// A starting policy trusting only the workspace's own key.
///
/// Deliberately minimal and deliberately announced. A policy with no roots
/// assigns zero weight to everything, so one is needed to evaluate at all;
/// one that trusts only yourself is honest about being a placeholder.
///
/// # Errors
///
/// Returns a message if the object cannot be built.
pub(crate) fn default_policy(author: &Cid) -> Result<Vec<u8>, String> {
    let mut root = std::collections::BTreeMap::new();
    root.insert("key".to_owned(), Value::Text(author.to_string()));
    root.insert("weight".to_owned(), Value::Uint(1000));

    Object::builder("policy", &author.to_string())
        .created(DEFAULT_CREATED)
        .field("roots", Value::Array(vec![Value::Map(root)]))
        .field("damping", Value::Uint(850_000))
        .field("iterations", Value::Uint(20))
        .field("tau", Value::Uint(660_000))
        .field("delta_max", Value::Uint(290_000))
        .field("replication_floor", Value::Uint(2))
        .field("independence_distance", Value::Uint(2))
        .build()
        .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entries(value: &Value) -> &[Value] {
        let Value::Array(entries) = value else {
            panic!("evidence is not an array");
        };
        entries
    }

    fn field<'a>(entry: &'a Value, name: &str) -> Option<&'a str> {
        let Value::Map(map) = entry else {
            panic!("evidence entry is not a map");
        };
        map.get(name).and_then(Value::as_text)
    }

    #[test]
    fn no_method_and_no_citations_is_an_empty_list() {
        assert_eq!(evidence_for(None, &[], &[]), Value::Array(Vec::new()));
    }

    #[test]
    fn a_citation_is_external_and_carries_the_citation_role() {
        let cites = vec!["https://doi.org/10.1136/bmj.b2680".to_owned()];
        let value = evidence_for(None, &cites, &[]);
        let entry = &entries(&value)[0];
        assert_eq!(field(entry, "kind"), Some("external"));
        assert_eq!(field(entry, "role"), Some("citation"));
        assert_eq!(field(entry, "ref"), Some(cites[0].as_str()));
        assert_eq!(field(entry, "note"), None);
    }

    #[test]
    fn a_note_pairs_with_the_citation_at_its_position() {
        let cites = vec!["A".to_owned(), "B".to_owned()];
        let notes = vec!["note for A".to_owned()];
        let value = evidence_for(None, &cites, &notes);
        let list = entries(&value);
        assert_eq!(field(&list[0], "ref"), Some("A"));
        assert_eq!(field(&list[0], "note"), Some("note for A"));
        assert_eq!(field(&list[1], "ref"), Some("B"));
        assert_eq!(field(&list[1], "note"), None);
    }

    #[test]
    fn method_precedes_citations_and_keeps_its_own_kind_and_role() {
        let cites = vec!["https://example.org/case".to_owned()];
        let value = evidence_for(Some("pub:sha2-256:abc"), &cites, &[]);
        let list = entries(&value);
        assert_eq!(list.len(), 2);
        assert_eq!(field(&list[0], "kind"), Some("claim"));
        assert_eq!(field(&list[0], "role"), Some("method"));
        assert_eq!(field(&list[0], "ref"), Some("pub:sha2-256:abc"));
        assert_eq!(field(&list[1], "role"), Some("citation"));
    }
}
