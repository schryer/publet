//! `pub read`: show an assertion and the scope it was made under.
//!
//! Section 11.3 requires scope to be rendered wherever a standing is, and
//! the same reasoning applies to the assertion itself: a publet quoted
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
