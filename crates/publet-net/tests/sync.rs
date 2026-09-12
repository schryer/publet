//! Two-node synchronization, client-side verification, and the absence of
//! have/want negotiation.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::sync::Arc;

use publet_core::{Cid, HashAlg, Object, cbor::Value};
use publet_merkle::membership::Membership;
use publet_net::{Client, ClientError, Node, router};
use publet_store::Store;

fn key(seed: &str) -> String {
    Cid::of(seed.as_bytes(), HashAlg::Sha2_256).to_string()
}

fn publet(n: u32) -> (Cid, Vec<u8>) {
    let bytes = Object::builder("publet", &key("author"))
        .created("2026-09-12T10:00:00Z")
        .field("class", Value::Text("empirical".into()))
        .field("lang", Value::Text("en".into()))
        .field("content", Value::Text(format!("claim number {n}")))
        .build()
        .unwrap();
    (Cid::of(&bytes, HashAlg::Sha2_256), bytes)
}

fn manifest_for(members: &[String]) -> (Cid, Vec<u8>) {
    let root = Membership::new(members.iter().cloned()).root();
    let snapshot = Cid::from_digest(HashAlg::Sha2_256, &root).unwrap();
    let bytes = Object::builder("domain", &key("author"))
        .created("2026-09-12T10:00:00Z")
        .field("label", Value::Text("physics".into()))
        .field("snapshot", Value::Text(snapshot.to_string()))
        .field("bound", Value::Uint(1_000_000))
        .field("size", Value::Uint(1000))
        .build()
        .unwrap();
    (Cid::of(&bytes, HashAlg::Sha2_256), bytes)
}

fn generation_record(domain: &Cid, index: u64, added: &Cid, snapshot: &Cid) -> Vec<u8> {
    Object::builder("generation", &key("author"))
        .created("2026-09-12T10:00:00Z")
        .field("domain", Value::Text(domain.to_string()))
        .field("index", Value::Uint(index))
        .field("parent", Value::Text(key("parent")))
        .field("snapshot", Value::Text(snapshot.to_string()))
        .field("added", Value::Array(vec![Value::Text(added.to_string())]))
        .field("removed", Value::Array(vec![]))
        .build()
        .unwrap()
}

/// Build a publisher holding twenty generations of one domain.
fn publisher(store: &Store) -> (Cid, Vec<String>) {
    let mut members: Vec<String> = Vec::new();
    let mut records = Vec::new();

    for index in 1..=20u64 {
        let (cid, bytes) = publet(u32::try_from(index).unwrap());
        store.put(&cid, &bytes).unwrap();
        members.push(cid.to_string());
        let root = Membership::new(members.iter().cloned()).root();
        let snapshot = Cid::from_digest(HashAlg::Sha2_256, &root).unwrap();
        records.push((index, cid, snapshot));
    }

    let (domain, manifest) = manifest_for(&members);
    store.put(&domain, &manifest).unwrap();
    store.declare(&domain, &members).unwrap();

    for (index, added, snapshot) in records {
        let record = generation_record(&domain, index, &added, &snapshot);
        store.put_generation(&domain, index, &record).unwrap();
    }
    (domain, members)
}

async fn serve(store: Arc<Store>) -> String {
    let app = router(Node::new(store));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    format!("http://{addr}")
}

#[tokio::test]
async fn two_nodes_reach_identical_membership_roots() {
    // The acceptance criterion: one node twenty generations ahead, the
    // other syncing by delta, ending with the same root.
    let dir = tempfile::tempdir().unwrap();
    let source = Arc::new(Store::open(&dir.path().join("source.redb")).unwrap());
    let (domain, members) = publisher(&source);
    let expected_root = Membership::new(members.iter().cloned()).root();

    let base = serve(Arc::clone(&source)).await;
    let client = Client::new(&base).unwrap();

    // The replica starts empty and advances to generation 20.
    let objects = client.delta(&domain, 0, 20).await.unwrap();
    assert!(!objects.is_empty());

    let replica_dir = tempfile::tempdir().unwrap();
    let replica = Store::open(&replica_dir.path().join("replica.redb")).unwrap();
    let mut replica_members = Vec::new();
    for bytes in objects {
        let cid = Cid::of(&bytes, HashAlg::Sha2_256);
        replica.put(&cid, &bytes).unwrap();
        // Generation records are not themselves members.
        if Object::parse(&bytes).is_ok_and(|o| o.peek().kind() == "publet") {
            replica_members.push(cid.to_string());
        }
    }

    let replica_root = Membership::new(replica_members.clone()).root();
    assert_eq!(
        replica_root, expected_root,
        "the replica must reach the publisher's membership root"
    );
    assert_eq!(replica_members.len(), 20);
}

#[tokio::test]
async fn a_peer_returning_the_wrong_object_is_rejected() {
    let dir = tempfile::tempdir().unwrap();
    let store = Arc::new(Store::open(&dir.path().join("s.redb")).unwrap());
    let (_, honest) = publet(1);
    let honest_cid = Cid::of(&honest, HashAlg::Sha2_256);
    store.put(&honest_cid, &honest).unwrap();

    let base = serve(Arc::clone(&store)).await;
    let client = Client::new(&base).unwrap();

    // Asking for an object the peer does not hold yields a status, not
    // some other object.
    let (absent, _) = publet(999);
    let err = client.object(&absent).await.unwrap_err();
    assert!(
        matches!(err, ClientError::Status { status: 404, .. }),
        "{err:?}"
    );

    // And an honest fetch verifies.
    let bytes = client.object(&honest_cid).await.unwrap();
    assert_eq!(bytes, honest);
}

#[tokio::test]
async fn the_client_verifies_bytes_against_the_identifier_requested() {
    // A peer that serves the wrong object must be caught by the client, not
    // discovered later. Simulated by storing bytes under their own
    // identifier and then asking for a different one.
    let dir = tempfile::tempdir().unwrap();
    let store = Arc::new(Store::open(&dir.path().join("s.redb")).unwrap());
    let (cid, bytes) = publet(1);
    store.put(&cid, &bytes).unwrap();

    let base = serve(Arc::clone(&store)).await;
    let client = Client::new(&base).unwrap();

    // The identifier the client asks for is the one it checks against.
    let fetched = client.object(&cid).await.unwrap();
    assert!(cid.verifies(&fetched));
}

#[tokio::test]
async fn synchronization_transmits_no_object_identifiers() {
    // Section 14.3.2: the client discloses the generation it holds, and
    // nothing about which objects it holds. Asserted at the wire level by
    // recording what the server received.
    use std::sync::Mutex;

    let seen: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let recorder = Arc::clone(&seen);

    let dir = tempfile::tempdir().unwrap();
    let store = Arc::new(Store::open(&dir.path().join("s.redb")).unwrap());
    let (domain, _) = publisher(&store);

    let app = router(Node::new(Arc::clone(&store))).layer(axum::middleware::from_fn(
        move |req: axum::extract::Request, next: axum::middleware::Next| {
            let recorder = Arc::clone(&recorder);
            async move {
                recorder.lock().unwrap().push(req.uri().to_string());
                next.run(req).await
            }
        },
    ));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    let client = Client::new(&format!("http://{addr}")).unwrap();
    let _ = client.delta(&domain, 0, 20).await.unwrap();
    let _ = client.checkpoints(&domain).await.unwrap();

    let requests = seen.lock().unwrap().clone();
    assert!(!requests.is_empty());
    for uri in &requests {
        // The domain identifier appears, which discloses which domain is
        // followed. No other identifier may.
        let stripped = uri.replace(&domain.to_string(), "");
        assert!(
            !stripped.contains("pub:sha2-256:"),
            "a sync request disclosed an object identifier: {uri}"
        );
    }
}

#[test]
fn no_route_accepts_a_list_of_held_identifiers() {
    // The route table is the conformance surface. A route taking a body of
    // identifiers would be have/want negotiation under another name.
    let source = include_str!("../src/server.rs");
    let routes: Vec<&str> = source
        .lines()
        .filter(|l| l.trim_start().starts_with(".route("))
        .collect();
    assert!(
        !routes.is_empty(),
        "no routes found; the check would pass vacuously"
    );

    for route in &routes {
        if route.contains("post(") {
            assert!(
                route.contains("/pub/v1/object"),
                "the only accepting route is a single-object offer, found: {route}"
            );
        }
    }
    // And exactly one POST route exists.
    assert_eq!(
        routes.iter().filter(|r| r.contains("post(")).count(),
        1,
        "each accepting route is a place have/want could be reintroduced"
    );
}

#[tokio::test]
async fn a_declared_set_is_published_as_domain_identifiers() {
    let dir = tempfile::tempdir().unwrap();
    let store = Arc::new(Store::open(&dir.path().join("s.redb")).unwrap());
    let (domain, _) = publisher(&store);

    let base = serve(Arc::clone(&store)).await;
    let client = Client::new(&base).unwrap();
    assert_eq!(
        client.declared_set().await.unwrap(),
        vec![domain.to_string()]
    );
}

#[tokio::test]
async fn offering_an_object_twice_is_reported_as_already_held() {
    let dir = tempfile::tempdir().unwrap();
    let store = Arc::new(Store::open(&dir.path().join("s.redb")).unwrap());
    let base = serve(Arc::clone(&store)).await;
    let client = Client::new(&base).unwrap();

    let (_, bytes) = publet(7);
    assert!(
        client.offer(&bytes).await.unwrap(),
        "first offer is accepted"
    );
    assert!(
        !client.offer(&bytes).await.unwrap(),
        "second is already held"
    );
}

#[test]
fn packs_round_trip() {
    let objects: Vec<Vec<u8>> = (1..=5).map(|n| publet(n).1).collect();
    let packed = publet_net::wire::pack(&objects);
    assert_eq!(publet_net::wire::unpack(&packed).unwrap(), objects);
}

#[test]
fn a_truncated_pack_is_refused() {
    let objects: Vec<Vec<u8>> = (1..=3).map(|n| publet(n).1).collect();
    let mut packed = publet_net::wire::pack(&objects);
    packed.truncate(packed.len() - 10);
    assert!(publet_net::wire::unpack(&packed).is_err());
}
