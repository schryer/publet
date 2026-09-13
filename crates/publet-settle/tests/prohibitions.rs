//! What settlement may never do, and the bounty rule that keeps review
//! honest.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use publet_core::{Cid, HashAlg, Object, cbor::Value};
use publet_settle::{
    Bounty, BountyError, BountyKind, Ledger, LedgerError, LedgerProperty, MINIMUM_REVIEW_SHARE,
    NullLedger, Prohibition,
};

fn cid(seed: &str) -> Cid {
    Cid::of(seed.as_bytes(), HashAlg::Sha2_256)
}

fn bounty_object(share: u64, with_policy: bool) -> Object {
    let mut builder = Object::builder("bounty", &cid("poster").to_string())
        .created("2026-09-12T10:00:00Z")
        .field("kind", Value::Text("review".into()))
        .field("target", Value::Text(cid("a claim").to_string()))
        .field("base_share", Value::Uint(share));
    if with_policy {
        builder = builder.field("policy", Value::Text(cid("a policy").to_string()));
    }
    let bytes = builder.build().unwrap();
    let id = Cid::of(&bytes, HashAlg::Sha2_256);
    Object::parse(&bytes)
        .unwrap()
        .verify(&id)
        .unwrap()
        .into_inner()
}

#[test]
fn a_bounty_below_the_review_share_floor_is_refused() {
    // Below the floor the money is buying a conclusion rather than the
    // labour of reaching one.
    let err = Bounty::from_object(&bounty_object(MINIMUM_REVIEW_SHARE - 1, true)).unwrap_err();
    assert!(
        matches!(err, BountyError::ReviewShareTooLow { .. }),
        "{err:?}"
    );
}

#[test]
fn a_bounty_at_or_above_the_floor_is_accepted() {
    for share in [MINIMUM_REVIEW_SHARE, MINIMUM_REVIEW_SHARE + 1, 10_000] {
        let bounty = Bounty::from_object(&bounty_object(share, true)).unwrap();
        assert_eq!(bounty.review_share, share);
        assert_eq!(bounty.kind, BountyKind::Review);
    }
}

#[test]
fn a_bounty_must_name_the_viewpoint_it_settles_against() {
    // The ledger enforces a choice published in advance. It does not decide
    // whose judgement counts, and a bounty naming no policy would be asking
    // it to.
    assert_eq!(
        Bounty::from_object(&bounty_object(MINIMUM_REVIEW_SHARE, false)).unwrap_err(),
        BountyError::NoPolicy
    );
}

#[test]
fn the_null_ledger_settles_nothing_and_says_why() {
    let err = NullLedger.settle(&cid("a bounty"), &[]).unwrap_err();
    assert_eq!(err, LedgerError::NotConfigured);
    assert!(
        err.to_string().contains("settlement is optional"),
        "the message must say the layer is optional: {err}"
    );
}

#[test]
fn an_incomplete_ledger_reports_which_properties_it_lacks() {
    let properties = NullLedger.properties();
    assert!(!properties.complete());
    let missing = properties.missing();
    assert_eq!(missing.len(), LedgerProperty::required().len());
    for property in LedgerProperty::required() {
        assert!(missing.contains(property), "{property:?}");
    }
}

#[test]
fn every_prohibition_carries_its_reason() {
    // Enumerated so they can be reviewed together rather than found
    // scattered through prose. Each is a way this layer could quietly
    // become a market in credibility.
    assert_eq!(Prohibition::all().len(), 4);
    for prohibition in Prohibition::all() {
        assert!(!prohibition.reason().is_empty(), "{prohibition:?}");
    }
}

#[test]
fn the_ledger_trait_cannot_express_a_standing_or_a_read() {
    // The prohibitions of Section 12.5 hold by construction rather than by
    // discipline: there is no method on `Ledger` that returns a standing,
    // influences one, or gates a retrieval, so an implementation cannot
    // perform those operations by accident or by design.
    let source = include_str!("../src/ledger.rs");
    let trait_body = source
        .split("pub trait Ledger {")
        .nth(1)
        .expect("the trait must be present")
        .split("\n}")
        .next()
        .expect("the trait must close");

    for forbidden in ["standing", "Standing", "evaluate", "retrieve", "read"] {
        assert!(
            !trait_body.contains(forbidden),
            "the ledger trait must not mention {forbidden:?}; \
             settlement has no epistemic authority"
        );
    }
    // What it may do, it does.
    assert!(trait_body.contains("fn settle"));
    assert!(trait_body.contains("fn properties"));
}
