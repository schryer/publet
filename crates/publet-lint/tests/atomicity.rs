//! Section 5.7: what the linter says, and what it refuses to say.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use publet_lint::{Term, check};

fn tests_that_fired(content: &str) -> Vec<&'static str> {
    let mut out: Vec<&'static str> = check(content, &[]).into_iter().map(|f| f.test).collect();
    out.sort_unstable();
    out.dedup();
    out
}

#[test]
fn one_assertion_passes() {
    assert!(tests_that_fired("the cohort rate fell to 11.9%").is_empty());
    // A before-and-after is two quantities, which Section 5.7 asks the
    // linter to raise. It stays one valid publet either way: the finding is
    // a question for the author, not a verdict.
    assert_eq!(
        tests_that_fired("the rate fell from 18.4% to 11.9%"),
        ["single-quantity"]
    );
}

#[test]
fn a_coordinating_conjunction_is_flagged() {
    assert_eq!(
        tests_that_fired("the rate fell and the cohort grew"),
        ["single-assertion"]
    );
}

#[test]
fn a_second_sentence_is_flagged() {
    assert_eq!(
        tests_that_fired("the rate fell. the cohort grew"),
        ["single-assertion"]
    );
}

#[test]
fn a_decimal_is_not_a_second_sentence() {
    // The linter is advisory, so a false positive costs more than a missed
    // one: an author who learns to ignore it has lost the whole mechanism.
    assert!(tests_that_fired("the rate fell to 11.9%").is_empty());
}

#[test]
fn an_identifier_is_not_a_pile_of_quantities() {
    // The case that found this: a digest is one token naming one thing,
    // and each run of digits in it was read as a separate measurement.
    assert!(tests_that_fired("the vectors digest to 014f9e6a936bf4bf on every target").is_empty());
    assert!(
        tests_that_fired(
            "the identifier is pub:sha2-256:f3rbmpe6i67p465n5mba3sh43e63wbkgweeyqk3wtisy6exzc7xq"
        )
        .is_empty()
    );
    assert!(tests_that_fired("the digest uses sha2-256 throughout").is_empty());
}

#[test]
fn real_quantities_still_count() {
    // The fix must not buy quiet by going blind. A letter that does not
    // touch the digits leaves them a measurement.
    assert_eq!(
        tests_that_fired("the 2031 cohort had n=4,182 members"),
        ["single-quantity"]
    );
    assert_eq!(
        tests_that_fired("the rate fell from 18.4% to 11.9%"),
        ["single-quantity"]
    );
    assert!(tests_that_fired("the cohort had 4,182 members").is_empty());
}

#[test]
fn an_opening_pronoun_is_flagged() {
    // A publet travels alone, so an opening "it" resolves to whatever
    // preceded it in the work it was cut from -- the context that does not
    // come with it.
    assert_eq!(
        tests_that_fired("it rose in the second cohort"),
        ["dangling-anaphora"]
    );
    assert_eq!(
        tests_that_fired("the effect holds for the latter group"),
        ["dangling-anaphora"]
    );
    assert!(tests_that_fired("the rate rose in the second cohort").is_empty());
}

#[test]
fn a_contested_term_must_be_declared() {
    let content = "the cohort shows a quantum effect";
    assert!(
        check(
            content,
            &[Term {
                word: "quantum",
                declared: true
            }]
        )
        .is_empty()
    );

    let findings = check(
        content,
        &[Term {
            word: "quantum",
            declared: false,
        }],
    );
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].test, "undeclared-term");

    // Inside another word it is not a use of the term.
    assert!(
        check(
            "quantumania was released",
            &[Term {
                word: "quantum",
                declared: false
            }]
        )
        .is_empty()
    );
}

#[test]
fn nothing_here_compares_two_publets() {
    // Section 5.7: implementations MUST NOT deduplicate by content
    // similarity. Whether two phrasings assert the same thing is a claim,
    // and claims are made by signing them (R6, R14) -- not by a threshold
    // inside a linter that no one can dispute.
    //
    // The prohibition is kept structurally: every entry point takes one
    // publet's content, so there is no second publet to compare against.
    let source = include_str!("../src/lib.rs");
    for signature in source
        .lines()
        .filter(|l| l.trim_start().starts_with("pub fn "))
    {
        assert!(
            !signature.contains("&[&str]") && !signature.contains("other"),
            "a lint entry point must not take a second publet: {signature}"
        );
    }
    let banned = [
        "levenshtein",
        "jaccard",
        "cosine",
        "similarity",
        "shingle",
        "minhash",
        "simhash",
        "dedup_by",
        "near_duplicate",
    ];
    let lowered = source.to_lowercase();
    for word in banned {
        // The prose above names the prohibition; the code must not implement
        // it. Count only lines that are not comments.
        let in_code = source
            .lines()
            .filter(|l| !l.trim_start().starts_with("//"))
            .any(|l| l.to_lowercase().contains(word));
        assert!(!in_code, "{word} appears in publet-lint's code");
    }
    assert!(
        lowered.contains("deduplication by content similarity"),
        "the module must say why the comparison is absent"
    );
}
