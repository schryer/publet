//! The retrieval interface (Section 14.4).
//!
//! Every route is a read of content-addressed data or an offer of one
//! object. **There is deliberately no endpoint that accepts a list of
//! identifiers a client holds.** `have`/`want` negotiation would let a peer
//! compute a minimal transfer, and would also hand it a fingerprint of what
//! the reader has been working with -- forfeiting most of what local-first
//! operation provides (Section 14.3.2). Synchronization discloses one
//! integer instead: the generation the reader is at.

use std::sync::Arc;

use axum::Router;
use axum::body::Bytes;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::routing::{get, post};
use publet_core::{Cid, HashAlg};
use publet_merkle::membership::{Membership, verify as verify_membership};
use publet_store::Store;

/// State shared by the handlers.
#[derive(Clone)]
pub struct Node {
    store: Arc<Store>,
}

impl Node {
    /// Serve from `store`.
    #[must_use]
    pub fn new(store: Arc<Store>) -> Self {
        Self { store }
    }
}

/// Build the router.
///
/// The route table is the conformance surface: adding a route that accepts
/// client-supplied identifiers would violate Section 14.3.2, and the test
/// suite asserts over this list.
pub fn router(node: Node) -> Router {
    Router::new()
        .route("/pub/v1/set", get(declared_set))
        .route("/pub/v1/object/{cid}", get(object))
        .route("/pub/v1/object", post(offer))
        .route("/pub/v1/domain/{cid}", get(manifest))
        .route("/pub/v1/domain/{cid}/pack", get(pack_domain))
        .route("/pub/v1/domain/{cid}/generation/{index}", get(generation))
        .route("/pub/v1/domain/{cid}/delta/{from}/{to}", get(delta))
        .route("/pub/v1/domain/{cid}/checkpoints", get(checkpoints))
        .route("/pub/v1/domain/{cid}/proof/{member}", get(proof))
        .with_state(node)
}

/// The domains this node has declared it serves (Section 13.2).
async fn declared_set(State(node): State<Node>) -> Result<String, StatusCode> {
    let declared = node
        .store
        .declared()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(declared.join("\n") + "\n")
}

/// Query mode: one object by identifier.
///
/// Section 13.2 makes service within a declared set unconditional, so a
/// held object is always returned. Not-found means not held, never a
/// judgement about the object.
async fn object(State(node): State<Node>, Path(cid): Path<String>) -> Result<Vec<u8>, StatusCode> {
    let parsed: Cid = cid.parse().map_err(|_| StatusCode::BAD_REQUEST)?;
    match node.store.get(&parsed) {
        Ok(Some(bytes)) => Ok(bytes),
        Ok(None) => Err(StatusCode::NOT_FOUND),
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

/// Offer one object. Never a list: see the module note.
async fn offer(State(node): State<Node>, body: Bytes) -> StatusCode {
    let cid = Cid::of(&body, HashAlg::Sha2_256);
    match node.store.contains(&cid) {
        Ok(true) => StatusCode::CONFLICT,
        Ok(false) => match node.store.put(&cid, &body) {
            Ok(()) => StatusCode::ACCEPTED,
            Err(_) => StatusCode::BAD_REQUEST,
        },
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

async fn manifest(
    State(node): State<Node>,
    Path(cid): Path<String>,
) -> Result<Vec<u8>, StatusCode> {
    object(State(node), Path(cid)).await
}

/// Every object in a domain, as one stream.
async fn pack_domain(
    State(node): State<Node>,
    Path(cid): Path<String>,
) -> Result<Vec<u8>, StatusCode> {
    let domain: Cid = cid.parse().map_err(|_| StatusCode::BAD_REQUEST)?;
    let members = node
        .store
        .members_of(&domain)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;

    let mut objects = Vec::new();
    for member in members {
        let Ok(parsed) = member.parse::<Cid>() else {
            continue;
        };
        if let Ok(Some(bytes)) = node.store.get(&parsed) {
            objects.push(bytes);
        }
    }
    Ok(crate::wire::pack(&objects))
}

/// One generation record, by index.
async fn generation(
    State(node): State<Node>,
    Path((cid, index)): Path<(String, u64)>,
) -> Result<Vec<u8>, StatusCode> {
    let domain: Cid = cid.parse().map_err(|_| StatusCode::BAD_REQUEST)?;
    node.store
        .generation(&domain, index)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)
}

/// Generation records and the objects they add, from one index to another.
///
/// The request carries two integers. It carries no list of what the client
/// holds, and there is no variant that accepts one.
async fn delta(
    State(node): State<Node>,
    Path((cid, from, to)): Path<(String, u64, u64)>,
) -> Result<Vec<u8>, StatusCode> {
    let domain: Cid = cid.parse().map_err(|_| StatusCode::BAD_REQUEST)?;
    if to < from {
        return Err(StatusCode::BAD_REQUEST);
    }
    let mut objects = Vec::new();
    for index in (from + 1)..=to {
        let record = node
            .store
            .generation(&domain, index)
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
            .ok_or(StatusCode::NOT_FOUND)?;
        objects.push(record);
    }
    // The objects each record adds travel with it, so applying the delta
    // needs no further round trips.
    let added = node
        .store
        .added_between(&domain, from, to)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    for member in added {
        let Ok(parsed) = member.parse::<Cid>() else {
            continue;
        };
        if let Ok(Some(bytes)) = node.store.get(&parsed) {
            objects.push(bytes);
        }
    }
    Ok(crate::wire::pack(&objects))
}

/// Checkpoint origins a client may sync from.
async fn checkpoints(
    State(node): State<Node>,
    Path(cid): Path<String>,
) -> Result<String, StatusCode> {
    let domain: Cid = cid.parse().map_err(|_| StatusCode::BAD_REQUEST)?;
    let head = node
        .store
        .head_generation(&domain)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;
    let points = publet_domain::checkpoints(head);
    Ok(points
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("\n")
        + "\n")
}

/// An inclusion or absence proof for one member.
async fn proof(
    State(node): State<Node>,
    Path((cid, member)): Path<(String, String)>,
) -> Result<String, StatusCode> {
    let domain: Cid = cid.parse().map_err(|_| StatusCode::BAD_REQUEST)?;
    let members = node
        .store
        .members_of(&domain)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;
    let membership = Membership::new(members);
    let proof = membership.prove(&member);
    let holds = verify_membership(&member, &proof, &membership.root());
    let finding = match proof {
        publet_merkle::membership::Proof::Present { .. } => "present",
        publet_merkle::membership::Proof::Absent { .. } => "absent",
        _ => "unknown",
    };
    Ok(format!(
        r#"{{"member":"{member}","finding":"{finding}","verified":{holds},"root":"{}"}}"#,
        publet_merkle::log::to_hex(&membership.root())
    ))
}
