//! Reading JSON files into object fields: `--data=FILE` for `pub compose`
//! and `pub revise` (Section 5.8), and the manifest `pub doc` builds a
//! document from (Section 8).
//!
//! JSON is the input format because the program that gathers values is
//! the usual author of these files, and every language writes it. What
//! JSON permits and the object format does not is refused here, at the
//! boundary, with the reason: a float is the one that matters, because
//! Section 4.1 excludes floating point from every object and a figure that
//! arrives as `12.5` would otherwise have to be silently re-encoded.

use std::collections::BTreeMap;
use std::path::Path;

use publet_core::{Cid, HashAlg, cbor::Value};
use publet_store::Store;

/// Convert parsed JSON into a CBOR value, refusing floating point.
///
/// # Errors
///
/// Returns a message naming the JSON path of the first non-integer number.
pub(crate) fn to_value(json: &serde_json::Value, path: &str) -> Result<Value, String> {
    Ok(match json {
        serde_json::Value::Null => Value::Null,
        serde_json::Value::Bool(b) => Value::Bool(*b),
        serde_json::Value::Number(n) => {
            if let Some(u) = n.as_u64() {
                Value::Uint(u)
            } else if let Some(i) = n.as_i64() {
                // CBOR stores a negative integer n as -1 - n.
                Value::Nint(i.unsigned_abs() - 1)
            } else {
                return Err(format!(
                    "{path}: {n} is not an integer. Objects carry no floating \
                     point (Section 4.1): write it as a decimal string such \
                     as \"{n}\" and put its unit on the column"
                ));
            }
        }
        serde_json::Value::String(s) => Value::Text(s.clone()),
        serde_json::Value::Array(items) => Value::Array(
            items
                .iter()
                .enumerate()
                .map(|(i, v)| to_value(v, &format!("{path}[{i}]")))
                .collect::<Result<_, _>>()?,
        ),
        serde_json::Value::Object(map) => Value::Map(
            map.iter()
                .map(|(k, v)| Ok((k.clone(), to_value(v, &format!("{path}.{k}"))?)))
                .collect::<Result<BTreeMap<_, _>, String>>()?,
        ),
    })
}

/// Read and parse a JSON file.
///
/// # Errors
///
/// Returns a message if the file cannot be read or is not JSON.
pub(crate) fn read_json(path: &Path) -> Result<serde_json::Value, String> {
    let text = std::fs::read_to_string(path)
        .map_err(|e| format!("cannot read {}: {e}", path.display()))?;
    serde_json::from_str(&text).map_err(|e| format!("{} is not JSON: {e}", path.display()))
}

/// The `data` field from a `--data=FILE` (Section 5.8).
///
/// Three shapes are accepted:
///
/// * a table, `{"columns": [...], "rows": [...]}`, carried as given;
/// * a file to carry, `{"media": TYPE, "file": PATH}`, whose bytes are
///   stored as a blob and replaced by `{media, ref, size}`; a relative
///   `PATH` is read from the data file's own directory;
/// * a blob already held, `{"media": TYPE, "ref": CID, "size": N}`, checked
///   against the store so a claim never cites bytes nobody here can verify.
///
/// The shape of the result is checked by the graph when the claim is
/// built, not here, so there is one reading of Section 5.8 and not two.
///
/// # Errors
///
/// Returns a message if the file is unreadable, holds a float, names a
/// file that cannot be read, or names a blob that is not held or whose
/// size differs.
pub(crate) fn data_from_file(store: &Store, path: &Path) -> Result<Value, String> {
    let json = read_json(path)?;
    let mut value = to_value(&json, "data")?;
    let Value::Map(map) = &mut value else {
        return Err(format!("{}: data must be a JSON object", path.display()));
    };

    if let Some(file) = map.remove("file") {
        let file = file.as_text().ok_or("data.file must be a path")?.to_owned();
        let file_path = path
            .parent()
            .map_or_else(|| Path::new(&file).to_path_buf(), |dir| dir.join(&file));
        let bytes = std::fs::read(&file_path)
            .map_err(|e| format!("cannot read {}: {e}", file_path.display()))?;
        let cid = Cid::of(&bytes, HashAlg::Sha2_256);
        store.put_blob(&cid, &bytes).map_err(|e| e.to_string())?;
        map.insert("ref".to_owned(), Value::Text(cid.to_string()));
        map.insert("size".to_owned(), Value::Uint(bytes.len() as u64));
    } else if let Some(reference) = map.get("ref").and_then(Value::as_text) {
        let cid: Cid = reference
            .parse()
            .map_err(|_| format!("data.ref is not a CID: {reference}"))?;
        check_blob(store, &cid, map.get("size").and_then(Value::as_uint))?;
    }
    Ok(value)
}

/// Confirm a cited blob is held and is the size the citation states
/// (Section 4.7).
///
/// # Errors
///
/// Returns a message if it is not held, no longer matches its identifier,
/// or has a different length.
pub(crate) fn check_blob(store: &Store, cid: &Cid, size: Option<u64>) -> Result<(), String> {
    let bytes = store
        .get_blob(cid)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| {
            format!(
                "blob {cid} is not held here, so nothing could verify the \
                 bytes this claim would cite (Section 4.7)"
            )
        })?;
    if let Some(size) = size
        && bytes.len() as u64 != size
    {
        return Err(format!(
            "blob {cid} is {} bytes, not the {size} the data states (Section 4.7)",
            bytes.len()
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(text: &str) -> serde_json::Value {
        serde_json::from_str(text).unwrap()
    }

    #[test]
    fn integers_text_and_null_convert() {
        let value = to_value(&parse(r#"[1, -1, -20, "12.50", null, true]"#), "data").unwrap();
        assert_eq!(
            value,
            Value::Array(vec![
                Value::Uint(1),
                Value::Nint(0),
                Value::Nint(19),
                Value::Text("12.50".into()),
                Value::Null,
                Value::Bool(true),
            ])
        );
    }

    #[test]
    fn a_float_is_refused_and_its_path_named() {
        let err = to_value(&parse(r#"{"rows": [["trays", 12.5]]}"#), "data").unwrap_err();
        assert!(err.contains("data.rows[0][1]"), "{err}");
        assert!(err.contains("Section 4.1"), "{err}");
    }

    #[test]
    fn a_file_is_stored_as_a_blob_and_cited_by_size() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::open(&dir.path().join("objects.redb")).unwrap();
        std::fs::write(dir.path().join("plan.pdf"), b"%PDF-1.7").unwrap();
        std::fs::write(
            dir.path().join("data.json"),
            r#"{"media": "application/pdf", "file": "plan.pdf"}"#,
        )
        .unwrap();
        let value = data_from_file(&store, &dir.path().join("data.json")).unwrap();
        let cid = Cid::of(b"%PDF-1.7", HashAlg::Sha2_256);
        assert_eq!(
            value.get("ref").and_then(Value::as_text),
            Some(cid.to_string().as_str())
        );
        assert_eq!(value.get("size").and_then(Value::as_uint), Some(8));
        assert!(value.get("file").is_none());
        assert!(store.get_blob(&cid).unwrap().is_some());
    }

    #[test]
    fn a_blob_of_another_size_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::open(&dir.path().join("objects.redb")).unwrap();
        let cid = Cid::of(b"four", HashAlg::Sha2_256);
        store.put_blob(&cid, b"four").unwrap();
        assert!(check_blob(&store, &cid, Some(4)).is_ok());
        assert!(check_blob(&store, &cid, Some(5)).is_err());
    }

    #[test]
    fn a_blob_not_held_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::open(&dir.path().join("objects.redb")).unwrap();
        let cid = Cid::of(b"elsewhere", HashAlg::Sha2_256);
        assert!(check_blob(&store, &cid, None).is_err());
    }
}
