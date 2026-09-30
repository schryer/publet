//! `pub document`: build a document and add it to the workspace.
//!
//! Section 8's own words are the constraint this command enforces
//! structurally rather than by convention: "a document contains no
//! assertions of its own." There is no `--content` flag here, unlike
//! `pub compose` -- a document only ever orders references to claims that
//! already exist, so nothing here can author a new assertion by accident.
//!
//! Sections and items are given as one ordered stream: `--section=`
//! starts a section, and every `--item=`/`--role=`/`--gloss=`/`--bind=`/
//! `--at=` that follows attaches to it, until the next `--section=`. This
//! is the same pairing-by-position discipline `pub compose`'s
//! `--cite`/`--cite-note` already uses, chosen for the same reason: CIDs
//! contain colons (`pub:sha2-256:...`), so packing fields into one flag
//! separated by `:` is not available here without ambiguity.
//!
//! A document has no `scope` (Section 5.3 does not apply to it): R4
//! governs assertions, and a document's own text is explicitly not one.
//!
//! `pub doc MANIFEST` builds the same object from a JSON file instead of
//! a stream of flags, which is what a program publishing documents on
//! every build wants: the manifest is the document's structure written
//! down once, and `pub revise OLD --manifest=FILE` publishes the next
//! version of it.

use std::collections::BTreeMap;
use std::path::Path;

use publet_core::{Cid, HashAlg, Object, cbor::Value};

use crate::workspace::Workspace;

const BINDS: [&str; 2] = ["object", "lineage"];
const ROLES: [&str; 5] = ["assert", "quote", "contrast", "background", "counterpoint"];

struct ItemArgs {
    cid: String,
    bind: Option<String>,
    at: Option<String>,
    role: Option<String>,
    gloss: Option<String>,
    view: Option<Value>,
}

/// The fields of a document body other than its sections.
#[derive(Default)]
struct Header {
    title: Option<String>,
    lang: Option<String>,
    license: Option<String>,
    abstract_cid: Option<String>,
}

struct SectionArgs {
    heading: String,
    items: Vec<ItemArgs>,
}

/// Build a document.
///
/// # Errors
///
/// Returns a message if a required field is missing, an item names an
/// unknown `bind` or `role`, a lineage-bound item omits `--at`, or the
/// loader refuses the result.
pub(crate) fn run(args: &[String]) -> Result<(), String> {
    let here = std::env::current_dir().map_err(|e| e.to_string())?;
    let ws = Workspace::open(&here)?;
    let store = ws.store()?;

    let mut header = Header::default();
    let mut created = crate::compose::DEFAULT_CREATED.to_owned();
    let mut sections: Vec<SectionArgs> = Vec::new();

    for arg in args {
        if let Some(v) = arg.strip_prefix("--title=") {
            header.title = Some(v.to_owned());
        } else if let Some(v) = arg.strip_prefix("--lang=") {
            header.lang = Some(v.to_owned());
        } else if let Some(v) = arg.strip_prefix("--license=") {
            header.license = Some(v.to_owned());
        } else if let Some(v) = arg.strip_prefix("--abstract=") {
            header.abstract_cid = Some(v.to_owned());
        } else if let Some(v) = arg.strip_prefix("--created=") {
            v.clone_into(&mut created);
        } else if let Some(v) = arg.strip_prefix("--section=") {
            sections.push(SectionArgs {
                heading: v.to_owned(),
                items: Vec::new(),
            });
        } else if let Some(v) = arg.strip_prefix("--item=") {
            let section = sections
                .last_mut()
                .ok_or("--item given before any --section")?;
            section.items.push(ItemArgs {
                cid: v.to_owned(),
                bind: None,
                at: None,
                role: None,
                gloss: None,
                view: None,
            });
        } else if let Some(v) = arg.strip_prefix("--bind=") {
            current_item(&mut sections, "--bind")?.bind = Some(v.to_owned());
        } else if let Some(v) = arg.strip_prefix("--at=") {
            current_item(&mut sections, "--at")?.at = Some(v.to_owned());
        } else if let Some(v) = arg.strip_prefix("--role=") {
            current_item(&mut sections, "--role")?.role = Some(v.to_owned());
        } else if let Some(v) = arg.strip_prefix("--gloss=") {
            current_item(&mut sections, "--gloss")?.gloss = Some(v.to_owned());
        } else if let Some(v) = arg.strip_prefix("--view=") {
            let json: serde_json::Value =
                serde_json::from_str(v).map_err(|e| format!("--view is not JSON: {e}"))?;
            current_item(&mut sections, "--view")?.view =
                Some(crate::payload::to_value(&json, "view")?);
        } else {
            return Err(format!("unknown argument: {arg}"));
        }
    }

    let body = build_body(header, sections)?;
    store_document(&ws, &store, &created, body)
}

/// Build a document from a JSON manifest: `pub doc MANIFEST`.
///
/// The manifest is the document body in JSON -- `title`, optional `lang`,
/// `license`, `abstract`, and `sections` of `{heading, items}`, each item
/// `{ref, bind, at, role, view, gloss}` -- plus an optional `created`.
///
/// # Errors
///
/// Returns a message if the manifest is unreadable or malformed, or the
/// loader refuses the document.
pub(crate) fn run_manifest(args: &[String]) -> Result<(), String> {
    let here = std::env::current_dir().map_err(|e| e.to_string())?;
    let ws = Workspace::open(&here)?;
    let store = ws.store()?;

    let mut manifest = None;
    let mut created = None;
    for arg in args {
        if let Some(v) = arg.strip_prefix("--created=") {
            created = Some(v.to_owned());
        } else if arg.starts_with("--") {
            return Err(format!("unknown argument: {arg}"));
        } else {
            manifest = Some(arg.clone());
        }
    }
    let manifest = manifest.ok_or("a manifest file is required: pub doc MANIFEST.json")?;
    let (body, manifest_created) = body_from_manifest(Path::new(&manifest))?;
    let created = created
        .or(manifest_created)
        .unwrap_or_else(|| crate::compose::DEFAULT_CREATED.to_owned());
    store_document(&ws, &store, &created, body)
}

/// Read a manifest into a document body, and the `created` it names.
///
/// # Errors
///
/// Returns a message if the file is unreadable, not JSON, holds a float,
/// names an unknown key, or describes an invalid item.
pub(crate) fn body_from_manifest(
    path: &Path,
) -> Result<(BTreeMap<String, Value>, Option<String>), String> {
    let json = crate::payload::read_json(path)?;
    let Value::Map(mut manifest) = crate::payload::to_value(&json, "manifest")? else {
        return Err(format!("{}: a manifest is a JSON object", path.display()));
    };
    // An unknown key is refused rather than ignored: a misspelled `gloss`
    // silently dropped is a document missing what its author wrote.
    known_keys(
        &manifest,
        &[
            "title", "lang", "license", "abstract", "created", "sections",
        ],
        "manifest",
    )?;
    let text = |map: &mut BTreeMap<String, Value>, key: &str| match map.remove(key) {
        None => Ok(None),
        Some(Value::Text(t)) => Ok(Some(t)),
        Some(_) => Err(format!("manifest.{key} must be a string")),
    };
    let header = Header {
        title: text(&mut manifest, "title")?,
        lang: text(&mut manifest, "lang")?,
        license: text(&mut manifest, "license")?,
        abstract_cid: text(&mut manifest, "abstract")?,
    };
    let created = text(&mut manifest, "created")?;

    let Some(Value::Array(raw_sections)) = manifest.remove("sections") else {
        return Err("manifest.sections must be a list of {heading, items}".to_owned());
    };
    let mut sections = Vec::new();
    for raw in raw_sections {
        let Value::Map(mut section) = raw else {
            return Err("each section must be a {heading, items} object".to_owned());
        };
        known_keys(&section, &["heading", "items"], "section")?;
        let heading = text(&mut section, "heading")?.ok_or("each section needs a heading")?;
        let raw_items = match section.remove("items") {
            None => Vec::new(),
            Some(Value::Array(items)) => items,
            Some(_) => return Err(format!("section {heading:?}: items must be a list")),
        };
        let mut items = Vec::new();
        for raw in raw_items {
            let Value::Map(mut item) = raw else {
                return Err(format!("section {heading:?}: each item must be an object"));
            };
            known_keys(
                &item,
                &["ref", "bind", "at", "role", "view", "gloss"],
                "item",
            )?;
            items.push(ItemArgs {
                cid: text(&mut item, "ref")?
                    .ok_or_else(|| format!("section {heading:?}: an item needs a ref"))?,
                bind: text(&mut item, "bind")?,
                at: text(&mut item, "at")?,
                role: text(&mut item, "role")?,
                gloss: text(&mut item, "gloss")?,
                view: item.remove("view"),
            });
        }
        sections.push(SectionArgs { heading, items });
    }
    Ok((build_body(header, sections)?, created))
}

fn known_keys(map: &BTreeMap<String, Value>, known: &[&str], what: &str) -> Result<(), String> {
    match map.keys().find(|k| !known.contains(&k.as_str())) {
        Some(key) => Err(format!(
            "unknown {what} key `{key}`; expected one of {}",
            known.join(", ")
        )),
        None => Ok(()),
    }
}

fn build_body(
    header: Header,
    sections: Vec<SectionArgs>,
) -> Result<BTreeMap<String, Value>, String> {
    let title = header.title.ok_or("a title is required")?;
    if sections.is_empty() {
        return Err("at least one section is required".to_owned());
    }
    let mut body = BTreeMap::new();
    body.insert("title".to_owned(), Value::Text(title));
    body.insert(
        "lang".to_owned(),
        Value::Text(header.lang.unwrap_or_else(|| "en".to_owned())),
    );
    body.insert(
        "sections".to_owned(),
        Value::Array(
            sections
                .into_iter()
                .map(build_section)
                .collect::<Result<_, _>>()?,
        ),
    );
    if let Some(license) = header.license {
        body.insert("license".to_owned(), Value::Text(license));
    }
    if let Some(abstract_cid) = header.abstract_cid {
        abstract_cid
            .parse::<Cid>()
            .map_err(|_| format!("abstract is not a CID: {abstract_cid}"))?;
        body.insert("abstract".to_owned(), Value::Text(abstract_cid));
    }
    Ok(body)
}

/// Build a document object from a body.
///
/// # Errors
///
/// Returns a message if the object cannot be built.
pub(crate) fn build_document(
    author: &str,
    created: &str,
    body: BTreeMap<String, Value>,
) -> Result<(Cid, Vec<u8>), String> {
    let mut builder = Object::builder("doc", author).created(created);
    for (key, value) in body {
        builder = builder.field(&key, value);
    }
    let bytes = builder.build().map_err(|e| e.to_string())?;
    Ok((Cid::of(&bytes, HashAlg::Sha2_256), bytes))
}

fn store_document(
    ws: &Workspace,
    store: &publet_store::Store,
    created: &str,
    body: BTreeMap<String, Value>,
) -> Result<(), String> {
    let author = ws
        .get("author")
        .ok_or("no author configured; run `pub init`")?;
    let (cid, bytes) = build_document(&author, created, body)?;
    crate::compose::offer_then_store(store, &cid, &bytes, "this document was refused")?;
    println!("{cid}");
    Ok(())
}

/// The item the most recent `--item=` started, for a following
/// `--bind=`/`--at=`/`--role=`/`--gloss=` to attach to.
fn current_item<'a>(
    sections: &'a mut [SectionArgs],
    flag: &'static str,
) -> Result<&'a mut ItemArgs, String> {
    sections
        .last_mut()
        .and_then(|s| s.items.last_mut())
        .ok_or_else(|| format!("{flag} given before any --item"))
}

fn build_section(section: SectionArgs) -> Result<Value, String> {
    let items = section
        .items
        .into_iter()
        .map(build_item)
        .collect::<Result<_, _>>()?;
    let mut map = BTreeMap::new();
    map.insert("heading".to_owned(), Value::Text(section.heading));
    map.insert("items".to_owned(), Value::Array(items));
    Ok(Value::Map(map))
}

fn build_item(item: ItemArgs) -> Result<Value, String> {
    item.cid
        .parse::<Cid>()
        .map_err(|_| format!("--item is not a CID: {}", item.cid))?;

    let bind = item.bind.as_deref().unwrap_or("object");
    if !BINDS.contains(&bind) {
        return Err(format!("unknown --bind: {bind} (object or lineage)"));
    }
    // Section 8: a lineage-bound citation without the head its author read
    // gives a reader only what is current, losing what was actually cited.
    if bind == "lineage" && item.at.is_none() {
        return Err(format!(
            "--item={} is bound to a lineage but names no --at: the head \
             its author actually read (Section 8)",
            item.cid
        ));
    }
    if let Some(at) = &item.at {
        at.parse::<Cid>()
            .map_err(|_| format!("--at is not a CID: {at}"))?;
    }

    let role = item.role.as_deref().unwrap_or("assert");
    if !ROLES.contains(&role) {
        return Err(format!("unknown --role: {role} ({})", ROLES.join(", ")));
    }

    let mut map = BTreeMap::new();
    map.insert("ref".to_owned(), Value::Text(item.cid));
    map.insert("bind".to_owned(), Value::Text(bind.to_owned()));
    if let Some(at) = item.at {
        map.insert("at".to_owned(), Value::Text(at));
    }
    map.insert("role".to_owned(), Value::Text(role.to_owned()));
    if let Some(gloss) = item.gloss {
        map.insert("gloss".to_owned(), Value::Text(gloss));
    }
    // Section 8: presentational, like `gloss`. Its shape is checked by the
    // graph when the document is offered, so there is one reading of it.
    if let Some(view) = item.view {
        map.insert("view".to_owned(), view);
    }
    Ok(Value::Map(map))
}
