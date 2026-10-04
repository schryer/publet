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
/// The argument may also be an object file itself, such as
/// `objects/pub_sha2-256_….cbor`. Its identifier is then computed from
/// its bytes rather than taken on trust, and a filename naming a
/// different identifier is reported: the name says what the file was,
/// the bytes say what it is.
///
/// `--json` prints the whole object instead of the summary: every header
/// and body field, decoded, with the identifier and the mode the read used.
///
/// # Errors
///
/// Returns a message if the workspace, store, or object is unavailable.
pub(crate) fn run(args: &[String]) -> Result<(), String> {
    let mut dir: Option<std::path::PathBuf> = None;
    let mut wanted: Option<&String> = None;
    let mut json = false;
    for arg in args {
        if let Some(v) = arg.strip_prefix("--dir=") {
            dir = Some(std::path::PathBuf::from(v));
        } else if arg == "--json" {
            json = true;
        } else if arg.starts_with("--") {
            return Err(format!("unknown argument: {arg}"));
        } else {
            wanted = Some(arg);
        }
    }

    let wanted = wanted.ok_or("a CID, or an object file, is required")?;
    let (target, bytes, mode) = if let Ok(target) = wanted.parse::<Cid>() {
        let (bytes, mode) = fetch(&target, dir.as_deref())?;
        (target, bytes, mode)
    } else if std::path::Path::new(wanted).is_file() {
        let (target, bytes) = from_file(std::path::Path::new(wanted))?;
        (target, bytes, None)
    } else {
        return Err(format!(
            "{wanted} is neither a CID nor a file holding an object"
        ));
    };

    let object = publet_core::Object::parse(&bytes)
        .map_err(|e| e.to_string())?
        .verify(&target)
        .map_err(|e| e.to_string())?;
    if json {
        return print_json(&target, &bytes, mode);
    }
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

    if let Some(data) = body.get("data") {
        print_data(data);
    }
    print_sources(body.get("evidence"));

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
/// An object's bytes by identifier: from `dir` when given, otherwise from
/// the workspace store, with the mode that read used (Section 14.3).
fn fetch(
    target: &Cid,
    dir: Option<&std::path::Path>,
) -> Result<(Vec<u8>, Option<crate::workspace::Mode>), String> {
    if let Some(path) = dir {
        let graph = publet_graph::load::from_dir(path).map_err(|e| e.to_string())?;
        let object = graph
            .object(target)
            .ok_or_else(|| format!("not in {}: {target}", path.display()))?;
        return Ok((object.bytes().to_vec(), None));
    }
    let here = std::env::current_dir().map_err(|e| e.to_string())?;
    let ws = Workspace::open(&here)?;
    let store = ws.store()?;
    let mode = Workspace::mode_for(&store, target);
    let bytes = store
        .get(target)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("not in your replica: {target}"))?;
    Ok((bytes, Some(mode)))
}

/// An object file's bytes and the identifier they hash to (Section 4.2).
///
/// A filename spelling a different identifier is reported rather than
/// refused: the reader asked for this file, and what it holds is still
/// shown -- under the identifier its bytes actually have.
fn from_file(path: &std::path::Path) -> Result<(Cid, Vec<u8>), String> {
    let bytes = std::fs::read(path).map_err(|e| format!("cannot read {}: {e}", path.display()))?;
    let cid = Cid::of(&bytes, publet_core::HashAlg::Sha2_256);
    let named = path
        .file_stem()
        .and_then(|s| s.to_str())
        .and_then(|s| s.replace('_', ":").parse::<Cid>().ok());
    if let Some(named) = named
        && named != cid
    {
        eprintln!(
            "warning: {} is named for {named}, but its bytes hash to {cid}; \
             the file was altered or misnamed",
            path.display()
        );
    }
    Ok((cid, bytes))
}

/// The whole object as JSON: `{cid, mode, object}`, where `object` is
/// every field of the canonical CBOR, decoded. `mode` is `local` or
/// `query` for a read from the workspace and `null` for a file or `--dir`,
/// which are neither (Section 14.3).
fn print_json(
    target: &Cid,
    bytes: &[u8],
    mode: Option<crate::workspace::Mode>,
) -> Result<(), String> {
    let decoded = publet_core::cbor::decode(bytes).map_err(|e| e.to_string())?;
    let out = serde_json::json!({
        "cid": target.to_string(),
        "mode": mode.map(crate::workspace::Mode::label),
        "object": to_json(&decoded),
    });
    let text = serde_json::to_string_pretty(&out).map_err(|e| e.to_string())?;
    println!("{text}");
    Ok(())
}

/// A CBOR value as JSON. Integers stay integers -- a negative one is
/// `-1 - n`, as CBOR stores it -- and a byte string, which JSON has no
/// form for, becomes `{"$bytes": HEX}`, so it can never be mistaken for
/// text. There are no floats to convert: the profile has none (Section 4.1).
fn to_json(value: &Value) -> serde_json::Value {
    match value {
        Value::Uint(n) => (*n).into(),
        Value::Nint(n) => i64::try_from(*n)
            .ok()
            .and_then(i64::checked_neg)
            .and_then(|n| n.checked_sub(1))
            .map_or_else(
                || serde_json::Value::String(format!("-{}", u128::from(*n) + 1)),
                Into::into,
            ),
        Value::Bytes(b) => {
            let hex = b.iter().fold(String::new(), |mut acc, byte| {
                use std::fmt::Write as _;
                let _ = write!(acc, "{byte:02x}");
                acc
            });
            serde_json::json!({ "$bytes": hex })
        }
        Value::Text(t) => t.clone().into(),
        Value::Array(items) => items.iter().map(to_json).collect::<Vec<_>>().into(),
        Value::Map(map) => {
            serde_json::Value::Object(map.iter().map(|(k, v)| (k.clone(), to_json(v))).collect())
        }
        Value::Bool(b) => (*b).into(),
        _ => serde_json::Value::Null,
    }
}

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
            if let Some(Value::Text(renderer)) = item.get("view").and_then(|v| v.get("renderer")) {
                println!("      view   {renderer}");
            }
            if let Some(Value::Text(gloss)) = item.get("gloss") {
                println!("      gloss  {gloss}");
            }
        }
    }
}

/// Summarize a claim's `data` (Section 5.8): its shape, not its values.
///
/// Showing the rows is rendering, and rendering belongs to whatever shows
/// a document, under that document's view. What a reader of the claim
/// itself needs is what the values are measured in and how many there
/// are, so the table's columns and units are listed and the rows counted.
fn print_data(data: &Value) {
    println!();
    if let Some(Value::Array(columns)) = data.get("columns") {
        let rows = match data.get("rows") {
            Some(Value::Array(rows)) => rows.len(),
            _ => 0,
        };
        println!(
            "data      table, {} column(s), {rows} row(s)",
            columns.len()
        );
        for column in columns {
            let name = column.get("name").and_then(Value::as_text).unwrap_or("");
            match column.get("unit").and_then(Value::as_text) {
                Some(unit) => println!("  {name} ({unit})"),
                None => println!("  {name}"),
            }
        }
    } else if let Some(Value::Text(blob)) = data.get("ref") {
        let media = data.get("media").and_then(Value::as_text).unwrap_or("");
        let size = data.get("size").and_then(Value::as_uint).unwrap_or(0);
        println!("data      file, {media}, {size} bytes");
        println!("  blob    {blob}");
    }
}

/// List where a claim's values were read from (Section 5.5). External
/// sources lie outside the protocol's integrity guarantees and are marked
/// unverified, as that section requires.
fn print_sources(evidence: Option<&Value>) {
    let Some(Value::Array(entries)) = evidence else {
        return;
    };
    let sources: Vec<&Value> = entries
        .iter()
        .filter(|e| e.get("role").and_then(Value::as_text) == Some("source"))
        .collect();
    if sources.is_empty() {
        return;
    }
    println!();
    println!("read from:");
    for source in sources {
        let kind = source.get("kind").and_then(Value::as_text).unwrap_or("");
        let reference = source.get("ref").and_then(Value::as_text).unwrap_or("");
        let mark = if kind == "external" {
            "  (unverified)"
        } else {
            ""
        };
        println!("  {kind:<9}{reference}{mark}");
        for field in ["revision", "locator", "query", "note"] {
            if let Some(value) = source.get(field).and_then(Value::as_text) {
                println!("    {field:<9}{value}");
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

#[cfg(test)]
mod json_tests {
    use super::*;

    #[test]
    fn values_convert_without_losing_what_they_are() {
        assert_eq!(to_json(&Value::Uint(7)), serde_json::json!(7));
        assert_eq!(to_json(&Value::Nint(0)), serde_json::json!(-1));
        assert_eq!(to_json(&Value::Nint(19)), serde_json::json!(-20));
        assert_eq!(
            to_json(&Value::Nint(u64::MAX)),
            serde_json::json!("-18446744073709551616")
        );
        assert_eq!(
            to_json(&Value::Bytes(vec![0, 255])),
            serde_json::json!({ "$bytes": "00ff" })
        );
        assert_eq!(to_json(&Value::Null), serde_json::Value::Null);
    }
}
