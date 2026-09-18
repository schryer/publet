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

    let mut title = None;
    let mut lang = "en".to_owned();
    let mut license = None;
    let mut abstract_cid = None;
    let mut created = "2026-09-12T00:00:00Z".to_owned();
    let mut sections: Vec<SectionArgs> = Vec::new();

    for arg in args {
        if let Some(v) = arg.strip_prefix("--title=") {
            title = Some(v.to_owned());
        } else if let Some(v) = arg.strip_prefix("--lang=") {
            v.clone_into(&mut lang);
        } else if let Some(v) = arg.strip_prefix("--license=") {
            license = Some(v.to_owned());
        } else if let Some(v) = arg.strip_prefix("--abstract=") {
            abstract_cid = Some(v.to_owned());
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
            });
        } else if let Some(v) = arg.strip_prefix("--bind=") {
            current_item(&mut sections, "--bind")?.bind = Some(v.to_owned());
        } else if let Some(v) = arg.strip_prefix("--at=") {
            current_item(&mut sections, "--at")?.at = Some(v.to_owned());
        } else if let Some(v) = arg.strip_prefix("--role=") {
            current_item(&mut sections, "--role")?.role = Some(v.to_owned());
        } else if let Some(v) = arg.strip_prefix("--gloss=") {
            current_item(&mut sections, "--gloss")?.gloss = Some(v.to_owned());
        } else {
            return Err(format!("unknown argument: {arg}"));
        }
    }

    let title = title.ok_or("--title is required")?;
    if sections.is_empty() {
        return Err("at least one --section is required".to_owned());
    }

    let author = ws
        .get("author")
        .ok_or("no author configured; run `pub init`")?;

    let sections_value = Value::Array(
        sections
            .into_iter()
            .map(build_section)
            .collect::<Result<_, _>>()?,
    );

    let mut builder = Object::builder("doc", &author)
        .created(&created)
        .field("title", Value::Text(title))
        .field("lang", Value::Text(lang))
        .field("sections", sections_value);
    if let Some(license) = &license {
        builder = builder.field("license", Value::Text(license.clone()));
    }
    if let Some(abstract_cid) = &abstract_cid {
        abstract_cid
            .parse::<Cid>()
            .map_err(|_| format!("--abstract is not a CID: {abstract_cid}"))?;
        builder = builder.field("abstract", Value::Text(abstract_cid.clone()));
    }
    let bytes = builder.build().map_err(|e| e.to_string())?;
    let cid = Cid::of(&bytes, HashAlg::Sha2_256);

    crate::compose::offer_then_store(&store, &cid, &bytes, "this document was refused")?;
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
    let mut map = std::collections::BTreeMap::new();
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

    let mut map = std::collections::BTreeMap::new();
    map.insert("ref".to_owned(), Value::Text(item.cid));
    map.insert("bind".to_owned(), Value::Text(bind.to_owned()));
    if let Some(at) = item.at {
        map.insert("at".to_owned(), Value::Text(at));
    }
    map.insert("role".to_owned(), Value::Text(role.to_owned()));
    if let Some(gloss) = item.gloss {
        map.insert("gloss".to_owned(), Value::Text(gloss));
    }
    Ok(Value::Map(map))
}
