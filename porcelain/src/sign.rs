//! `pub sign`: generate a signing key, or sign a stored object.
//!
//! Section 4.4 defines a `sig` object and Section 10.1 a `key` object, but
//! until now nothing in this codebase ever produced either: `pub init`
//! seeded `author` with `Cid::of(process_id_bytes)`, a placeholder its own
//! comment says is "the user's to replace" -- and nothing ever did. Every
//! object this corpus has built is self-declared, unauthenticated
//! authorship. This is the replacement.
//!
//! `--generate-key` writes `.publet/signing.key` (the private half, never
//! stored as an object -- Section 10 has no notion of publishing a secret
//! key) and a `key` object (the public half, Section 10.1's shape, whose
//! own CID becomes the identity everything this workspace signs from here
//! on cites as `author`). `--purpose`/`--target` signs an object already in
//! this workspace's store.
//!
//! # `author: <self>` on a key object
//!
//! Section 10.1's pseudocode writes `author: <self>` for a key object, but
//! an object's header `author` field is a CID, and the key object's own
//! CID is not known until its bytes -- header included -- are complete.
//! Making that literal would need the CID to be a fixed point of its own
//! hash, which is infeasible by construction, not merely hard. This build
//! resolves it as a raw Section 6.2 content identifier over the public key
//! bytes alone (`Cid::of(pubkey_bytes)`), distinct from the key object's
//! own CID: a key authors itself, and this is a non-circular way to say
//! so. It is a reading of underspecified spec pseudocode, not a settled
//! answer -- flagged here for whoever formalizes Section 10.1 next.

use publet_core::{Cid, HashAlg, Object, SigAlg, cbor::Value};

use crate::workspace::{DIR, Workspace};

const KEY_FILE: &str = "signing.key";
const PRINCIPALS: [&str; 3] = ["human", "organization", "automated"];

/// Generate a key, or sign a stored object.
///
/// # Errors
///
/// Returns a message if the arguments are incomplete, a key already exists
/// where `--generate-key` would write one, no key exists where signing
/// needs one, or the target is not an object this workspace holds.
pub(crate) fn run(args: &[String]) -> Result<(), String> {
    let here = std::env::current_dir().map_err(|e| e.to_string())?;
    let ws = Workspace::open(&here)?;

    let mut do_generate = false;
    let mut principal = None;
    let mut label = None;
    let mut purpose = None;
    let mut target = None;
    let mut created = "2026-09-16T00:00:00Z".to_owned();

    for arg in args {
        if arg == "--generate-key" {
            do_generate = true;
        } else if let Some(v) = arg.strip_prefix("--principal=") {
            principal = Some(v.to_owned());
        } else if let Some(v) = arg.strip_prefix("--label=") {
            label = Some(v.to_owned());
        } else if let Some(v) = arg.strip_prefix("--purpose=") {
            purpose = Some(v.to_owned());
        } else if let Some(v) = arg.strip_prefix("--target=") {
            target = Some(v.to_owned());
        } else if let Some(v) = arg.strip_prefix("--created=") {
            v.clone_into(&mut created);
        } else {
            return Err(format!("unknown argument: {arg}"));
        }
    }

    if do_generate {
        return generate(&ws, &here, principal, label.as_deref(), &created);
    }

    let purpose = purpose
        .ok_or("--purpose is required; a signature covers a stated purpose (Section 4.4)")?;
    let target = target.ok_or("--target is required: the CID of the object to sign")?;
    sign_target(&ws, &here, &purpose, &target, &created)
}

fn key_path(here: &std::path::Path) -> std::path::PathBuf {
    here.join(DIR).join(KEY_FILE)
}

fn generate(
    ws: &Workspace,
    here: &std::path::Path,
    principal: Option<String>,
    label: Option<&str>,
    created: &str,
) -> Result<(), String> {
    let principal = principal.ok_or(format!(
        "--principal is required: one of {}",
        PRINCIPALS.join(", ")
    ))?;
    if !PRINCIPALS.contains(&principal.as_str()) {
        return Err(format!("unknown principal: {principal}"));
    }

    let path = key_path(here);
    if path.exists() {
        return Err(format!(
            "{} already exists; this command will not overwrite a signing key. \
             Rotation is a signed `key` object naming `rotates` (Section 10.5), \
             not a silent replacement of the file.",
            path.display()
        ));
    }

    let mut seed = [0u8; 32];
    getrandom::getrandom(&mut seed)
        .map_err(|e| format!("could not read system randomness: {e}"))?;
    let pubkey = publet_core::verifying_key(SigAlg::Ed25519, &seed).map_err(|e| e.to_string())?;

    // Non-circular resolution of Section 10.1's `author: <self>` -- see the
    // module doc comment.
    let self_author = Cid::of(&pubkey, HashAlg::Sha2_256);

    let mut builder = Object::builder("key", &self_author.to_string())
        .created(created)
        .field("alg", Value::Text("ed25519".to_owned()))
        .field("pubkey", Value::Bytes(pubkey))
        .field("principal", Value::Text(principal));
    if let Some(label) = label {
        builder = builder.field("label", Value::Text(label.to_owned()));
    }
    let bytes = builder.build().map_err(|e| e.to_string())?;
    let cid = Cid::of(&bytes, HashAlg::Sha2_256);

    let store = ws.store()?;
    store.put(&cid, &bytes).map_err(|e| e.to_string())?;

    write_key_file(&path, &seed)?;
    ws.set("author", &cid.to_string())?;

    println!("key      {cid}");
    println!("secret   {}", path.display());
    println!();
    println!("`author` in this workspace's config now names this key. Every");
    println!("object you compose, relate, or annotate from here on claims it");
    println!("as author -- but nothing is actually signed by it yet; nothing");
    println!("in this build attaches a `sig` object automatically. Do that");
    println!("with `pub sign --purpose=P --target=CID` per object.");
    println!();
    println!(
        "The secret key at {} is yours alone. It is never",
        path.display()
    );
    println!("published as an object, never leaves this machine, and has no");
    println!("recovery mechanism (Section 10.5): losing it means this identity");
    println!("publishes nothing further, not that it can be restored.");
    Ok(())
}

fn write_key_file(path: &std::path::Path, seed: &[u8; 32]) -> Result<(), String> {
    let hex = seed.iter().fold(String::new(), |mut acc, b| {
        use std::fmt::Write as _;
        let _ = write!(acc, "{b:02x}");
        acc
    });
    std::fs::write(path, hex + "\n").map_err(|e| e.to_string())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn read_key_file(path: &std::path::Path) -> Result<[u8; 32], String> {
    let text = std::fs::read_to_string(path).map_err(|_| {
        format!(
            "no signing key at {}; run `pub sign --generate-key --principal=...` first",
            path.display()
        )
    })?;
    let hex = text.trim();
    if !hex.len().is_multiple_of(2) {
        return Err(format!("{} is malformed", path.display()));
    }
    let bytes: Option<Vec<u8>> = hex
        .as_bytes()
        .chunks(2)
        .map(|c| {
            std::str::from_utf8(c)
                .ok()
                .and_then(|pair| u8::from_str_radix(pair, 16).ok())
        })
        .collect();
    bytes
        .and_then(|b| <[u8; 32]>::try_from(b).ok())
        .ok_or_else(|| format!("{} is malformed", path.display()))
}

/// A detached `sig` object (Section 4.4) over `target_bytes`, made with
/// this workspace's secret key, not yet stored.
///
/// # Errors
///
/// Returns a message if there is no key here or signing fails.
pub(crate) fn signature(
    here: &std::path::Path,
    author: &str,
    created: &str,
    purpose: &str,
    target: &Cid,
    target_bytes: &[u8],
) -> Result<(Cid, Vec<u8>), String> {
    let seed = read_key_file(&key_path(here))?;
    let value = publet_core::sign(SigAlg::Ed25519, &seed, purpose, target_bytes)
        .map_err(|e| e.to_string())?;
    let bytes = Object::builder("sig", author)
        .created(created)
        .field("target", Value::Text(target.to_string()))
        .field("alg", Value::Text("ed25519".to_owned()))
        .field("value", Value::Bytes(value))
        .field("purpose", Value::Text(purpose.to_owned()))
        .build()
        .map_err(|e| e.to_string())?;
    Ok((Cid::of(&bytes, HashAlg::Sha2_256), bytes))
}

fn sign_target(
    ws: &Workspace,
    here: &std::path::Path,
    purpose: &str,
    target: &str,
    created: &str,
) -> Result<(), String> {
    let target_cid: Cid = target
        .parse()
        .map_err(|_| format!("--target is not a CID: {target}"))?;

    let store = ws.store()?;
    let Some(target_bytes) = store.get(&target_cid).map_err(|e| e.to_string())? else {
        return Err(format!(
            "{target_cid} is not an object this workspace's store holds, so there \
             are no canonical bytes to sign. Signing a Section 6.2 raw content \
             identifier (code, for instance) needs its own decision about what \
             bytes a signature covers -- the source text `pub-code-tree` spans, \
             or the identifier string itself -- which has not been made yet."
        ));
    };

    let author = ws
        .get("author")
        .ok_or("no author configured; run `pub sign --generate-key` first")?;
    let (cid, bytes) = signature(here, &author, created, purpose, &target_cid, &target_bytes)?;
    store.put(&cid, &bytes).map_err(|e| e.to_string())?;

    println!("{cid}");
    Ok(())
}

#[cfg(test)]
mod tests {
    use publet_core::cbor::Value;

    use super::*;

    fn workspace() -> (tempfile::TempDir, Workspace) {
        let dir = tempfile::tempdir().unwrap();
        let ws = Workspace::create(dir.path()).unwrap();
        (dir, ws)
    }

    #[test]
    fn a_produced_signature_verifies_against_the_generated_keys_public_half() {
        let (dir, ws) = workspace();
        generate(
            &ws,
            dir.path(),
            Some("human".to_owned()),
            None,
            "2026-09-16T00:00:00Z",
        )
        .unwrap();

        let author = ws.get("author").unwrap();
        let key_cid: Cid = author.parse().unwrap();

        // Each block opens and drops its own store handle: `redb` holds an
        // exclusive lock on the file, and `sign_target` below opens its
        // own, so this test must never hold one open across that call.
        let (pubkey, target_bytes, target_cid) = {
            let store = ws.store().unwrap();
            let key_bytes = store.get(&key_cid).unwrap().unwrap();
            let key_object = Object::parse(&key_bytes).unwrap().verify(&key_cid).unwrap();
            let Some(Value::Bytes(pubkey)) = key_object.object().body().get("pubkey").cloned()
            else {
                panic!("pubkey is not bytes");
            };

            let target_bytes = Object::builder("claim.prose", &author)
                .created("2026-09-16T00:00:00Z")
                .field("content", Value::Text("a test claim".into()))
                .build()
                .unwrap();
            let target_cid = Cid::of(&target_bytes, HashAlg::Sha2_256);
            store.put(&target_cid, &target_bytes).unwrap();
            (pubkey, target_bytes, target_cid)
        };

        sign_target(
            &ws,
            dir.path(),
            "authored",
            &target_cid.to_string(),
            "2026-09-16T00:00:00Z",
        )
        .unwrap();

        let (signature, purpose) = {
            let store = ws.store().unwrap();
            let sig_cid = store
                .cids()
                .unwrap()
                .into_iter()
                .filter_map(|c| c.parse::<Cid>().ok())
                .find(|c| *c != key_cid && *c != target_cid)
                .expect("a sig object was stored");
            let sig_bytes = store.get(&sig_cid).unwrap().unwrap();
            let sig_object = Object::parse(&sig_bytes).unwrap().verify(&sig_cid).unwrap();
            let body = sig_object.object().body();
            let Some(Value::Bytes(signature)) = body.get("value").cloned() else {
                panic!("signature value is not bytes");
            };
            let Some(Value::Text(purpose)) = body.get("purpose").cloned() else {
                panic!("purpose is not text");
            };
            (signature, purpose)
        };
        assert_eq!(purpose, "authored");

        assert!(
            publet_core::verify(
                SigAlg::Ed25519,
                &pubkey,
                &signature,
                &purpose,
                "authored",
                &target_bytes,
            )
            .is_ok()
        );
    }

    #[test]
    fn generate_refuses_to_overwrite_an_existing_key() {
        let (dir, ws) = workspace();
        generate(
            &ws,
            dir.path(),
            Some("human".to_owned()),
            None,
            "2026-09-16T00:00:00Z",
        )
        .unwrap();
        let err = generate(
            &ws,
            dir.path(),
            Some("human".to_owned()),
            None,
            "2026-09-16T00:00:00Z",
        )
        .unwrap_err();
        assert!(err.contains("already exists"));
    }

    #[test]
    fn signing_an_object_the_store_does_not_hold_is_refused() {
        let (dir, ws) = workspace();
        generate(
            &ws,
            dir.path(),
            Some("human".to_owned()),
            None,
            "2026-09-16T00:00:00Z",
        )
        .unwrap();
        let not_held = Cid::of(b"never stored", HashAlg::Sha2_256).to_string();
        let err = sign_target(
            &ws,
            dir.path(),
            "authored",
            &not_held,
            "2026-09-16T00:00:00Z",
        )
        .unwrap_err();
        assert!(err.contains("is not an object this workspace's store holds"));
    }
}
