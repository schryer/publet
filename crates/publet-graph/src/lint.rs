//! Structural authoring tests (Section 5.7).
//!
//! These are warnings, never validity rules. A publet that fails them is
//! flagged rather than rejected, which puts the pressure on the author at
//! authoring time when the fix is cheap, and leaves the decision about what
//! to do with a flagged publet to a viewpoint.
//!
//! They catch compound assertions. They say nothing about granularity: "the
//! rate was 11.9%" and "the rate fell from 18.4% to 11.9%" both pass, and
//! they are not the same publet (Appendix B.6).

use crate::view::Publet;

/// A structural finding about a publet's content.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Finding {
    /// A stable identifier for the test that fired.
    pub test: &'static str,
    /// What the author should look at.
    pub detail: String,
}

/// Conjunctions that typically join two independently assertable claims.
const COORDINATORS: [&str; 4] = [" and ", " but ", " whereas ", " while "];

/// Run the structural tests over a publet's content.
#[must_use]
pub fn check(publet: &Publet) -> Vec<Finding> {
    let content = publet.content();
    let mut findings = Vec::new();

    if content.trim().is_empty() {
        findings.push(Finding {
            test: "non-empty",
            detail: "content is empty".to_owned(),
        });
        return findings;
    }

    if content.len() > 4096 {
        findings.push(Finding {
            test: "size",
            detail: format!("{} bytes exceeds the 4096-byte ceiling", content.len()),
        });
    }

    let lowered = content.to_lowercase();
    for coordinator in COORDINATORS {
        if lowered.contains(coordinator) {
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

    let sentences = content
        .split(['.', '?', '!'])
        .filter(|s| !s.trim().is_empty())
        .count();
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

/// Count distinct numeric literals, so that "18.4% to 11.9%" reads as two.
fn count_quantities(content: &str) -> usize {
    let mut count = 0;
    let mut in_number = false;
    for ch in content.chars() {
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
