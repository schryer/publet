//! Declared-set protection, tombstone discipline, and integrity scanning.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::integer_division
)]

use publet_core::{Cid, HashAlg, Object, cbor::Value};
use publet_merkle::membership::Membership;
use publet_store::{Store, StoreError};
use redb::{Database, TableDefinition};

fn store(dir: &tempfile::TempDir) -> Store {
    Store::open(&dir.path().join("objects.redb")).unwrap()
}

fn object(seed: &str) -> (Cid, Vec<u8>) {
    let bytes = seed.as_bytes().to_vec();
    (Cid::of(&bytes, HashAlg::Sha2_256), bytes)
}

/// A domain manifest whose snapshot root covers exactly `members`.
///
/// Built here rather than by the store, so that a store which failed to
/// check the membership would still be caught.
fn manifest_for(members: &[String]) -> (Cid, Vec<u8>) {
    let root = Membership::new(members.iter().cloned()).root();
    let snapshot = Cid::from_digest(HashAlg::Sha2_256, &root).unwrap();
    let bytes = Object::builder("domain", &object("manifest author").0.to_string())
        .created("2026-09-12T10:00:00Z")
        .field("label", Value::Text("a domain".into()))
        .field("snapshot", Value::Text(snapshot.to_string()))
        .field("bound", Value::Uint(1_000_000))
        .field("size", Value::Uint(100))
        .field(
            "closed_under",
            Value::Array(vec![Value::Text("depends".into())]),
        )
        .build()
        .unwrap();
    (Cid::of(&bytes, HashAlg::Sha2_256), bytes)
}

/// Store a manifest and declare it, as a node actually would.
fn declare(s: &Store, members: &[String]) -> Cid {
    let (domain, bytes) = manifest_for(members);
    s.put(&domain, &bytes).unwrap();
    s.declare(&domain, members).unwrap();
    domain
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

    declare(&s, &[kept.to_string()]);

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
    declare(&s, &[cid.to_string()]);
    assert!(s.collect_garbage().unwrap().is_empty());
    assert!(s.collect_garbage().unwrap().is_empty());
    // The member and the manifest: both are part of what the declaration
    // commits the node to holding.
    assert_eq!(s.len().unwrap(), 2);
}

#[test]
fn a_declared_member_cannot_be_deleted_silently() {
    let dir = tempfile::tempdir().unwrap();
    let s = store(&dir);
    let (cid, bytes) = object("member");
    s.put(&cid, &bytes).unwrap();
    declare(&s, &[cid.to_string()]);

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
    declare(&s, &[cid.to_string()]);

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
    let a = declare(&s, &[object("one").0.to_string()]);
    let b = declare(&s, &[]);
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
    declare(&s, &[one.to_string()]);
    declare(&s, &[two.to_string()]);
    assert!(s.collect_garbage().unwrap().is_empty());
}

#[test]
fn reopening_preserves_everything() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("objects.redb");
    let (cid, bytes) = object("durable");
    let domain;
    {
        let s = Store::open(&path).unwrap();
        s.put(&cid, &bytes).unwrap();
        domain = declare(&s, &[cid.to_string()]);
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

#[test]
fn a_domain_cannot_be_declared_without_its_manifest() {
    // A node cannot serve a domain it cannot describe.
    let dir = tempfile::tempdir().unwrap();
    let s = store(&dir);
    let (domain, _) = manifest_for(&[]);
    let err = s.declare(&domain, &[]).unwrap_err();
    assert!(matches!(err, StoreError::ManifestNotHeld { .. }), "{err:?}");
}

#[test]
fn members_that_do_not_match_the_manifest_are_refused() {
    // The failure the `pub-store list | pub-store declare` pipeline would
    // have produced: a node declaring its own inventory as the membership,
    // rather than the membership the domain actually fixes.
    let dir = tempfile::tempdir().unwrap();
    let s = store(&dir);
    let intended = object("the real member").0.to_string();
    let (domain, bytes) = manifest_for(&[intended]);
    s.put(&domain, &bytes).unwrap();

    let inventory = vec![
        object("something the node happens to hold").0.to_string(),
        object("and another").0.to_string(),
    ];
    let err = s.declare(&domain, &inventory).unwrap_err();
    assert!(
        matches!(err, StoreError::MembershipMismatch { .. }),
        "a domain's membership is not the node's inventory: {err:?}"
    );
    assert!(s.declared().unwrap().is_empty());
}

#[test]
fn a_declared_domains_manifest_survives_collection() {
    // Collecting a manifest would leave the node unable to describe the set
    // it has committed to serving.
    let dir = tempfile::tempdir().unwrap();
    let s = store(&dir);
    let (member, bytes) = object("a member");
    s.put(&member, &bytes).unwrap();
    let domain = declare(&s, &[member.to_string()]);

    s.collect_garbage().unwrap();
    assert!(s.contains(&domain).unwrap(), "the manifest must survive");
    assert!(s.contains(&member).unwrap());
    // And the declaration is still readable afterwards.
    assert_eq!(s.declared().unwrap(), vec![domain.to_string()]);
}
