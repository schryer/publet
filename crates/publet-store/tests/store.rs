//! Declared-set protection, tombstone discipline, and integrity scanning.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::integer_division
)]

use publet_core::{Cid, HashAlg};
use publet_store::{Store, StoreError};
use redb::{Database, TableDefinition};

fn store(dir: &tempfile::TempDir) -> Store {
    Store::open(&dir.path().join("objects.redb")).unwrap()
}

fn object(seed: &str) -> (Cid, Vec<u8>) {
    let bytes = seed.as_bytes().to_vec();
    (Cid::of(&bytes, HashAlg::Sha2_256), bytes)
}

#[test]
fn stores_and_retrieves_by_identifier() {
    let dir = tempfile::tempdir().unwrap();
    let s = store(&dir);
    let (cid, bytes) = object("hello");
    s.put(&cid, &bytes).unwrap();
    assert_eq!(s.get(&cid).unwrap().as_deref(), Some(bytes.as_slice()));
    assert!(s.contains(&cid).unwrap());
    assert_eq!(s.len().unwrap(), 1);
}

#[test]
fn refuses_bytes_that_do_not_match_their_identifier() {
    // Accepting these would let the store answer a request for one object
    // with a different one.
    let dir = tempfile::tempdir().unwrap();
    let s = store(&dir);
    let (cid, _) = object("hello");
    let err = s.put(&cid, b"something else").unwrap_err();
    assert!(
        matches!(err, StoreError::IdentifierMismatch { .. }),
        "{err:?}"
    );
    assert!(!s.contains(&cid).unwrap());
}

#[test]
fn garbage_collection_never_removes_a_declared_member() {
    let dir = tempfile::tempdir().unwrap();
    let s = store(&dir);
    let (kept, kept_bytes) = object("declared member");
    let (loose, loose_bytes) = object("outside every declared set");
    s.put(&kept, &kept_bytes).unwrap();
    s.put(&loose, &loose_bytes).unwrap();

    let (domain, manifest) = object("a domain manifest");
    s.declare(&domain, &manifest, &[kept.to_string()]).unwrap();

    let collected = s.collect_garbage().unwrap();
    assert_eq!(collected, vec![loose.to_string()]);
    assert!(
        s.contains(&kept).unwrap(),
        "service within a declared set is unconditional"
    );
    assert!(!s.contains(&loose).unwrap());
}

#[test]
fn repeated_collection_is_stable() {
    let dir = tempfile::tempdir().unwrap();
    let s = store(&dir);
    let (cid, bytes) = object("member");
    s.put(&cid, &bytes).unwrap();
    let (domain, manifest) = object("domain");
    s.declare(&domain, &manifest, &[cid.to_string()]).unwrap();
    assert!(s.collect_garbage().unwrap().is_empty());
    assert!(s.collect_garbage().unwrap().is_empty());
    assert_eq!(s.len().unwrap(), 1);
}

#[test]
fn a_declared_member_cannot_be_deleted_silently() {
    let dir = tempfile::tempdir().unwrap();
    let s = store(&dir);
    let (cid, bytes) = object("member");
    s.put(&cid, &bytes).unwrap();
    let (domain, manifest) = object("domain");
    s.declare(&domain, &manifest, &[cid.to_string()]).unwrap();

    let err = s.remove(&cid).unwrap_err();
    assert!(
        matches!(err, StoreError::UndisclosedRemoval { .. }),
        "{err:?}"
    );
    assert!(s.contains(&cid).unwrap());
}

#[test]
fn a_tombstone_permits_removal_and_leaves_a_record() {
    let dir = tempfile::tempdir().unwrap();
    let s = store(&dir);
    let (cid, bytes) = object("member");
    s.put(&cid, &bytes).unwrap();
    let (domain, manifest) = object("domain");
    s.declare(&domain, &manifest, &[cid.to_string()]).unwrap();

    s.tombstone(&cid, b"a signed tombstone object").unwrap();
    assert!(s.is_tombstoned(&cid).unwrap());
    assert!(s.remove(&cid).unwrap());
    assert!(!s.contains(&cid).unwrap());
    // The disclosure outlives the object, which is the point of requiring it.
    assert!(s.is_tombstoned(&cid).unwrap());
}

#[test]
fn an_undeclared_object_may_be_removed_freely() {
    let dir = tempfile::tempdir().unwrap();
    let s = store(&dir);
    let (cid, bytes) = object("loose");
    s.put(&cid, &bytes).unwrap();
    assert!(s.remove(&cid).unwrap());
}

#[test]
fn a_scan_finds_nothing_in_a_healthy_store() {
    let dir = tempfile::tempdir().unwrap();
    let s = store(&dir);
    for i in 0..32 {
        let (cid, bytes) = object(&format!("object {i}"));
        s.put(&cid, &bytes).unwrap();
    }
    assert!(s.scan().unwrap().is_empty());
}

#[test]
fn a_scan_detects_bytes_changed_underneath_the_store() {
    // The store verifies on write, so a mismatch means the database file
    // was corrupted or tampered with after the fact.
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("objects.redb");
    let (cid, bytes) = object("original");
    {
        let s = Store::open(&path).unwrap();
        s.put(&cid, &bytes).unwrap();
    }
    // Write different bytes under the same key, bypassing `put`.
    {
        const OBJECTS: TableDefinition<'_, &str, &[u8]> = TableDefinition::new("objects");
        let db = Database::open(&path).unwrap();
        let tx = db.begin_write().unwrap();
        {
            let mut table = tx.open_table(OBJECTS).unwrap();
            table
                .insert(cid.to_string().as_str(), b"tampered".as_slice())
                .unwrap();
        }
        tx.commit().unwrap();
    }
    let s = Store::open(&path).unwrap();
    let findings = s.scan().unwrap();
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].stored_as, cid.to_string());
    assert_ne!(findings[0].actual, cid.to_string());
}

#[test]
fn declared_sets_are_reported_as_domain_identifiers() {
    let dir = tempfile::tempdir().unwrap();
    let s = store(&dir);
    let (a, ma) = object("domain a");
    let (b, mb) = object("domain b");
    s.declare(&a, &ma, &["pub:sha2-256:x".to_owned()]).unwrap();
    s.declare(&b, &mb, &[]).unwrap();
    let mut declared = s.declared().unwrap();
    declared.sort();
    let mut expected = vec![a.to_string(), b.to_string()];
    expected.sort();
    assert_eq!(declared, expected);
}

#[test]
fn members_of_every_declared_domain_are_protected() {
    let dir = tempfile::tempdir().unwrap();
    let s = store(&dir);
    let (one, one_bytes) = object("in domain one");
    let (two, two_bytes) = object("in domain two");
    s.put(&one, &one_bytes).unwrap();
    s.put(&two, &two_bytes).unwrap();
    let (da, ma) = object("domain a");
    let (db, mb) = object("domain b");
    s.declare(&da, &ma, &[one.to_string()]).unwrap();
    s.declare(&db, &mb, &[two.to_string()]).unwrap();
    assert!(s.collect_garbage().unwrap().is_empty());
}

#[test]
fn reopening_preserves_everything() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("objects.redb");
    let (cid, bytes) = object("durable");
    let (domain, manifest) = object("domain");
    {
        let s = Store::open(&path).unwrap();
        s.put(&cid, &bytes).unwrap();
        s.declare(&domain, &manifest, &[cid.to_string()]).unwrap();
    }
    let s = Store::open(&path).unwrap();
    assert!(s.contains(&cid).unwrap());
    assert_eq!(s.declared().unwrap(), vec![domain.to_string()]);
}

/// The acceptance criterion from the implementation plan.
///
/// Ignored by default because writing a million objects takes minutes;
/// run with `cargo test -p publet-store -- --ignored --nocapture`.
#[test]
#[ignore = "writes 10^6 objects; run on demand"]
fn a_million_objects_open_quickly_and_read_in_constant_time() {
    use std::time::Instant;

    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("big.redb");
    let count = 1_000_000u32;

    {
        let s = Store::open(&path).unwrap();
        for i in 0..count {
            let (cid, bytes) = object(&format!("object {i}"));
            s.put(&cid, &bytes).unwrap();
        }
        println!("wrote {count} objects");
    }

    let opened = Instant::now();
    let s = Store::open(&path).unwrap();
    let open_time = opened.elapsed();
    println!("open: {open_time:?}");
    assert!(open_time.as_secs() < 1, "open took {open_time:?}");

    // Reads at the two ends of the keyspace should not differ materially.
    for probe in [0u32, count / 2, count - 1] {
        let (cid, _) = object(&format!("object {probe}"));
        let start = Instant::now();
        assert!(s.contains(&cid).unwrap());
        println!("read {probe}: {:?}", start.elapsed());
    }
}
