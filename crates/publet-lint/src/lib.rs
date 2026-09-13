//! Structural authoring tests for a publet's content (Section 5.7).
//!
//! These are warnings, never validity rules. A publet that fails them is
//! flagged rather than rejected, which puts the pressure on the author at
//! authoring time when the fix is cheap, and leaves the decision about what
//! to do with a flagged publet to a viewpoint.
//!
//! They catch compound assertions. They say nothing about which of two
//! granularities is right: "the rate was 11.9%" and "the rate fell from
//! 18.4% to 11.9%" are both valid publets and are not the same publet
//! (Appendix B.6). The second draws a finding because it carries two
//! quantities, which is a question put to its author, not a verdict.
//!
//! The crate takes text and a caller-supplied list of terms. It has no
//! store, no graph, and no network, so an author can run it on a draft that
//! has not been signed and does not exist anywhere yet.
//!
//! # What this crate deliberately does not do
//!
//! It never compares two publets to each other. Section 5.7 forbids
//! deduplication by content similarity: whether two phrasings assert the
//! same thing is a claim, and claims are made by signing them (R6, R14).
//! Every function here takes exactly one publet's content, which is what
//! makes the prohibition structural rather than a promise.

/// A structural finding about a publet's content.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Finding {
    /// A stable identifier for the test that fired.
    pub test: &'static str,
    /// What the author should look at.
    pub detail: String,
}

/// A term the caller considers contested, and whether this publet declares
/// a dependency on the publet that defines it.
///
/// Contestedness is a graph question -- a term is contested when something
/// disputes its definition -- so the caller answers it. This crate only
/// checks whether a contested term the caller named is used without being
/// declared.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Term<'a> {
    /// The term as it would appear in prose, lowercase.
    pub word: &'a str,
    /// Whether the publet's `depends` names the defining publet.
    pub declared: bool,
}

/// Conjunctions that typically join two independently assertable claims.
const COORDINATORS: [&str; 4] = [" and ", " but ", " whereas ", " while "];

/// Whether a coordinator occurrence actually joins two clauses.
///
/// "while" is the awkward one: contrastive in "the rate fell while the
/// cohort grew", and merely temporal in "tracking depth while scanning".
/// The second has no second subject, so there is no second assertion to
/// separate. A following gerund is the cheap signal for that, and it is
/// the case that arises constantly when describing what code does.
fn joins_two_clauses(lowered: &str, coordinator: &str) -> bool {
    lowered.match_indices(coordinator).any(|(at, _)| {
        let rest = lowered.get(at + coordinator.len()..).unwrap_or("");
        let next = rest
            .split_whitespace()
            .next()
            .unwrap_or("")
            .trim_end_matches(|c: char| !c.is_alphanumeric());
        !(coordinator == " while " && next.ends_with("ing"))
    })
}

/// Openers whose referent cannot be inside a publet that starts with them.
///
/// A publet is quoted on its own, so an opening "it" or "this" resolves to
/// whatever preceded the publet in the work it was cut from -- which is
/// exactly the context that does not travel with it.
const OPENERS: [&str; 8] = [
    "it ", "this ", "that ", "these ", "those ", "they ", "he ", "she ",
];

/// Phrases that point at an enumeration a single assertion cannot contain.
const BACKREFERENCES: [&str; 4] = ["the former", "the latter", "the above", "as mentioned"];

/// The content ceiling, in bytes (Section 4.5).
const CONTENT_CEILING: usize = 4096;

/// Run the structural tests over one publet's content.
///
/// Pass an empty slice for `terms` to run the text-only tests.
///
/// `atomic` says whether the *atomicity* tests apply -- independent
/// clauses, coordinating conjunctions, and quantitative claims. Section 5.7
/// confines those to the classes whose content is the assertion, and
/// forbids them on `procedural`, `attributive`, `archival` and
/// `expressive`. The caller decides, because the caller knows the class and
/// this crate deliberately knows nothing but text.
///
/// The self-containment tests -- dangling anaphora and undeclared contested
/// terms -- run either way. A method directing the reader to "then add it"
/// with no antecedent is defective whatever its class.
///
/// ```
/// # use publet_lint::{check, Term};
/// let findings = check("the rate fell to 11.9%", &[], true);
/// assert!(findings.is_empty());
///
/// let findings = check("the rate fell and the cohort grew", &[], true);
/// assert_eq!(findings.first().map(|f| f.test), Some("single-assertion"));
///
/// // The same content as a method is not two assertions; it is two steps.
/// assert!(check("the rate fell and the cohort grew", &[], false).is_empty());
/// ```
#[must_use]
pub fn check(content: &str, terms: &[Term<'_>], atomic: bool) -> Vec<Finding> {
    let mut findings = Vec::new();

    if content.trim().is_empty() {
        findings.push(Finding {
            test: "non-empty",
            detail: "content is empty".to_owned(),
        });
        return findings;
    }

    if content.len() > CONTENT_CEILING {
        findings.push(Finding {
            test: "size",
            detail: format!(
                "{} bytes exceeds the {CONTENT_CEILING}-byte ceiling",
                content.len()
            ),
        });
    }

    let lowered = content.to_lowercase();
    if atomic {
        findings.extend(atomicity(content, &lowered));
    }

    findings.extend(anaphora(&lowered));

    for term in terms {
        if !term.declared && mentions(&lowered, term.word) {
            findings.push(Finding {
                test: "undeclared-term",
                detail: format!("{:?} is contested and is not named in depends", term.word),
            });
        }
    }

    findings
}

/// The tests that ask whether the content is one assertion (Section 5.7).
fn atomicity(content: &str, lowered: &str) -> Vec<Finding> {
    let mut findings = Vec::new();
    for coordinator in COORDINATORS {
        if lowered.contains(coordinator) && joins_two_clauses(lowered, coordinator) {
            findings.push(Finding {
                test: "single-assertion",
                detail: format!(
                    "{:?} may join two assertions that could each stand alone",
                    coordinator.trim()
                ),
            });
            break;
        }
    }

    let sentences = count_sentences(content);
    if sentences > 1 {
        findings.push(Finding {
            test: "single-assertion",
            detail: format!("{sentences} sentences; a publet asserts one thing"),
        });
    }

    let quantities = count_quantities(content);
    if quantities > 1 {
        findings.push(Finding {
            test: "single-quantity",
            detail: format!("{quantities} quantitative claims; consider separate publets"),
        });
    }

    findings
}

/// Anaphora whose antecedent is necessarily outside the publet.
fn anaphora(lowered: &str) -> Vec<Finding> {
    let mut findings = Vec::new();
    let start = lowered.trim_start();
    for opener in OPENERS {
        if start.starts_with(opener) {
            findings.push(Finding {
                test: "dangling-anaphora",
                detail: format!(
                    "opens with {:?}, whose referent is outside the publet",
                    opener.trim()
                ),
            });
            break;
        }
    }
    for phrase in BACKREFERENCES {
        if lowered.contains(phrase) {
            findings.push(Finding {
                test: "dangling-anaphora",
                detail: format!("{phrase:?} points outside the publet"),
            });
            break;
        }
    }
    findings
}

/// Whether `word` appears in `lowered` as a word rather than inside one.
fn mentions(lowered: &str, word: &str) -> bool {
    if word.is_empty() {
        return false;
    }
    lowered.match_indices(word).any(|(at, _)| {
        let before = lowered[..at].chars().next_back();
        let after = lowered[at + word.len()..].chars().next();
        !before.is_some_and(char::is_alphanumeric) && !after.is_some_and(char::is_alphanumeric)
    })
}

/// Count sentences, treating a decimal point as part of its number.
///
/// A publet is far more likely to carry a decimal than a second sentence,
/// so splitting on every `.` reports "the rate fell to 11.9%" as two
/// assertions and trains the author to ignore the linter.
fn count_sentences(content: &str) -> usize {
    let bytes = content.as_bytes();
    let mut sentences = 0;
    let mut open = false;
    for (at, ch) in content.char_indices() {
        let terminator = matches!(ch, '?' | '!')
            || (ch == '.'
                && !(at
                    .checked_sub(1)
                    .and_then(|prev| bytes.get(prev))
                    .is_some_and(u8::is_ascii_digit)
                    && bytes.get(at + 1).is_some_and(u8::is_ascii_digit)));
        if terminator {
            if open {
                sentences += 1;
                open = false;
            }
        } else if !ch.is_whitespace() {
            open = true;
        }
    }
    if open {
        sentences += 1;
    }
    sentences
}

/// Count distinct numeric literals, so that "18.4% to 11.9%" reads as two.
///
/// Digits inside an identifier are not quantities. A content address, a
/// hash algorithm name, or a digest is one token naming one thing, and
/// reading each run of digits in it as a separate measurement reported
/// `014f9e6a936bf4bf` as five quantitative claims. A corpus about this
/// protocol is mostly content addresses, so that warning would have fired
/// on nearly every publet and been trained away within a day -- which is
/// how an advisory tool stops protecting anything.
fn count_quantities(content: &str) -> usize {
    content
        .split_whitespace()
        .filter(|token| !is_identifier(token))
        .map(numbers_in)
        .sum()
}

/// Whether a token is a name rather than a measurement.
///
/// The test is adjacency: a digit touching a letter is part of a word.
/// `sha2-256` and `f3rbmpe6` are names; `4,182` and `11.9%` are not, and
/// `n=4182` is not either, because the letter does not touch the digits.
fn is_identifier(token: &str) -> bool {
    let chars: Vec<char> = token.chars().collect();
    chars.iter().enumerate().any(|(at, ch)| {
        if !ch.is_ascii_digit() {
            return false;
        }
        let before = at.checked_sub(1).and_then(|i| chars.get(i));
        let after = chars.get(at + 1);
        before.is_some_and(char::is_ascii_alphabetic)
            || after.is_some_and(char::is_ascii_alphabetic)
    })
}

/// Count digit runs in one token, treating a separator between digits as
/// part of the number so that "18.4" and "4,182" each read as one.
fn numbers_in(token: &str) -> usize {
    let mut count = 0;
    let mut in_number = false;
    for ch in token.chars() {
        if ch.is_ascii_digit() {
            if !in_number {
                count += 1;
                in_number = true;
            }
        } else if ch != '.' && ch != ',' {
            in_number = false;
        }
    }
    count
}
