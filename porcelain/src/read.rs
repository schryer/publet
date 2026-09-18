//! `pub read`: show an assertion and the scope it was made under.
//!
//! Section 11.3 requires scope to be rendered wherever a standing is, and
//! the same reasoning applies to the assertion itself: a claim quoted
//! without its validity conditions is the decontextualization the scope
//! field exists to prevent.

use publet_core::{Cid, cbor::Value};

use crate::workspace::Workspace;

/// Show one object.
///
/// Reads from the workspace by default, or from a directory of objects
/// given `--dir`. The second matters for a corpus kept as files: someone
/// who clones one should be able to read it without first building a
/// store, and every other directory-reading command already works that
/// way.
///
/// # Errors
///
/// Returns a message if the workspace, store, or object is unavailable.
pub(crate) fn run(args: &[String]) -> Result<(), String> {
    let mut dir: Option<std::path::PathBuf> = None;
    let mut wanted: Option<&String> = None;
    for arg in args {
        if let Some(v) = arg.strip_prefix("--dir=") {
            dir = Some(std::path::PathBuf::from(v));
        } else if arg.starts_with("--") {
            return Err(format!("unknown argument: {arg}"));
        } else {
            wanted = Some(arg);
        }
    }

    let target: Cid = wanted
        .ok_or("a CID is required")?
        .parse()
        .map_err(|_| "the argument must be a CID".to_owned())?;

    let (bytes, mode) = if let Some(path) = &dir {
        let graph = publet_graph::load::from_dir(path).map_err(|e| e.to_string())?;
        let object = graph
            .object(&target)
            .ok_or_else(|| format!("not in {}: {target}", path.display()))?;
        (object.bytes().to_vec(), None)
    } else {
        let here = std::env::current_dir().map_err(|e| e.to_string())?;
        let ws = Workspace::open(&here)?;
        let store = ws.store()?;
        let mode = Workspace::mode_for(&store, &target);
        let bytes = store
            .get(&target)
            .map_err(|e| e.to_string())?
            .ok_or_else(|| format!("not in your replica: {target}"))?;
        (bytes, Some(mode))
    };

    let object = publet_core::Object::parse(&bytes)
        .map_err(|e| e.to_string())?
        .verify(&target)
        .map_err(|e| e.to_string())?;
    let object = object.object();
    let body = object.body();

    println!("{target}");
    println!("type      {}", object.kind());
    println!("author    {}", object.author());
    println!("created   {}", object.created());

    // A relation or annotation asserts something about two other objects
    // (or one object and a value), and until now this printed nothing past
    // the header for either: a reader -- or a tool trying to tell which
    // relation object asserts a given edge -- had no way to see what it
    // actually said without decoding the CBOR by hand.
    match object.kind() {
        "claim.relation" => print_relation(body),
        "claim.annotation" => print_annotation(body),
        "doc" => print_document(body),
        _ => {}
    }

    if let Some(Value::Text(class)) = body.get("class") {
        println!("class     {class}");
    }
    if let Some(Value::Text(lang)) = body.get("lang") {
        println!("language  {lang}");
    }
    if let Some(Value::Text(content)) = body.get("content") {
        println!();
        println!("{content}");
    }

    // Scope is not an aside. An assertion read without its validity
    // conditions is a different assertion.
    if let Some(scope) = body.get("scope") {
        println!();
        println!("asserted under:");
        if let Some(Value::Text(domain)) = scope.get("domain") {
            println!("  domain     {domain}");
        }
        if let Some(Value::Text(precision)) = scope.get("precision") {
            println!("  precision  {precision}");
        }
        if let Some(temporal) = scope.get("temporal") {
            let from = temporal.get("from").and_then(Value::as_text).unwrap_or("");
            let to = temporal.get("to").and_then(Value::as_text).unwrap_or("");
            println!("  period     {from} to {to}");
        }
    } else if body.get("content").is_some() {
        println!();
        println!("  (no scope declared; the author asserted this without");
        println!("   stating the conditions it holds under)");
    }

    if let Some(Value::Array(depends)) = body.get("depends")
        && !depends.is_empty()
    {
        println!();
        println!("presupposes:");
        for d in depends {
            if let Some(text) = d.as_text() {
                println!("  {text}");
            }
        }
    }

    // Reading from a directory of files is neither local nor remote
    // retrieval, so there is no mode to disclose and none is invented.
    if let Some(mode) = mode {
        println!();
        println!("[{}] {}", mode.label(), mode.note());
    }
    Ok(())
}

/// Print a `rel` object's kind, endpoints, and any qualifiers it carries.
fn print_relation(body: &std::collections::BTreeMap<String, Value>) {
    if let Some(Value::Text(kind)) = body.get("kind") {
        println!("kind      {kind}");
    }
    if let Some(Value::Text(from)) = body.get("from") {
        println!("from      {from}");
    }
    if let Some(Value::Text(to)) = body.get("to") {
        println!("to        {to}");
    }
    if let Some(Value::Text(aspect)) = body.get("aspect") {
        println!("aspect    {aspect}");
    }
    if let Some(Value::Text(method)) = body.get("method") {
        println!("method    {method}");
    }
    if let Some(Value::Text(fidelity)) = body.get("fidelity") {
        println!("fidelity  {fidelity}");
    }
    if let Some(Value::Text(note)) = body.get("note") {
        println!();
        println!("{note}");
    }
}

/// Print a `doc` object's title and its sections of glossed references.
///
/// Section 8: "a document contains no assertions of its own." `gloss` is
/// presentational connective tissue and MUST NOT be mistaken for one, so
/// it is labelled and indented under the reference it explains rather
/// than run into the surrounding text. `role` is printed for the same
/// reason -- `counterpoint` is not endorsement, and a reader needs that
/// distinction at a glance, not just on request.
fn print_document(body: &std::collections::BTreeMap<String, Value>) {
    if let Some(Value::Text(title)) = body.get("title") {
        println!("title     {title}");
    }
    if let Some(Value::Text(license)) = body.get("license") {
        println!("license   {license}");
    }
    if let Some(Value::Text(abstract_cid)) = body.get("abstract") {
        println!("abstract  {abstract_cid}");
    }
    let Some(Value::Array(sections)) = body.get("sections") else {
        return;
    };
    println!();
    println!("sections:");
    for section in sections {
        let Some(Value::Text(heading)) = section.get("heading") else {
            continue;
        };
        println!("  {heading}");
        let Some(Value::Array(items)) = section.get("items") else {
            continue;
        };
        for item in items {
            let Some(Value::Text(reference)) = item.get("ref") else {
                continue;
            };
            println!("    - ref    {reference}");
            if let Some(Value::Text(bind)) = item.get("bind") {
                println!("      bind   {bind}");
            }
            if let Some(Value::Text(at)) = item.get("at") {
                println!("      at     {at}");
            }
            if let Some(Value::Text(role)) = item.get("role") {
                println!("      role   {role}");
            }
            if let Some(Value::Text(gloss)) = item.get("gloss") {
                println!("      gloss  {gloss}");
            }
        }
    }
}

/// Print an `ann` object's kind, target, and value fields.
fn print_annotation(body: &std::collections::BTreeMap<String, Value>) {
    if let Some(Value::Text(kind)) = body.get("kind") {
        println!("kind      {kind}");
    }
    if let Some(Value::Text(target)) = body.get("target") {
        println!("target    {target}");
    }
    let Some(Value::Map(fields)) = body.get("value") else {
        return;
    };
    println!();
    println!("value:");
    for (k, v) in fields {
        if let Some(text) = v.as_text() {
            println!("  {k}  {text}");
        } else if let Value::Array(items) = v {
            // `usage`'s `code` (Section 7.5) is the one array-valued field
            // a citation carries -- one or more content identifiers, never
            // free text, so there is nothing here to render but a list.
            let joined: Vec<&str> = items.iter().filter_map(Value::as_text).collect();
            if !joined.is_empty() {
                println!("  {k}  {}", joined.join(", "));
            }
        }
    }
}
