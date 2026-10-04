//! Traversal properties over random graphs: a topological order respects
//! every dependency, a cycle is reported as a real cycle, and the closure
//! agrees with reachability.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing)]

use std::collections::BTreeMap;

use proptest::prelude::*;
use publet_algorithms::graph::{CycleThroughStart, closure, reachable, reaches, topological_order};

/// Up to 8 nodes, `n0`..`n7`, with arbitrary directed edges.
fn graphs() -> impl Strategy<Value = BTreeMap<String, Vec<String>>> {
    proptest::collection::vec((0u8..8, 0u8..8), 0..20).prop_map(|edges| {
        let mut g: BTreeMap<String, Vec<String>> = BTreeMap::new();
        for (a, b) in edges {
            g.entry(format!("n{a}")).or_default().push(format!("n{b}"));
        }
        g
    })
}

fn names() -> Vec<String> {
    (0..8).map(|i| format!("n{i}")).collect()
}

proptest! {
    #[test]
    fn a_topological_order_puts_every_dependency_first(g in graphs()) {
        let next = |n: &str| g.get(n).cloned().unwrap_or_default();
        let nodes = names();
        match topological_order(nodes.iter().map(String::as_str), next) {
            Ok(order) => {
                let pos = |n: &str| order.iter().position(|o| o == n).unwrap();
                for (from, tos) in &g {
                    for to in tos {
                        prop_assert!(pos(to) < pos(from), "{to} must precede {from}: {order:?}");
                    }
                }
            }
            Err(cycle) => {
                // A reported cycle starts and ends at the same node, and
                // each step along it is a real edge.
                prop_assert!(cycle.len() >= 2);
                prop_assert_eq!(cycle.first(), cycle.last());
                for pair in cycle.windows(2) {
                    prop_assert!(g.get(&pair[0]).is_some_and(|t| t.contains(&pair[1])));
                }
            }
        }
    }

    #[test]
    fn the_closure_is_what_the_start_reaches_by_at_least_one_edge(g in graphs(), s in 0u8..8) {
        let start = format!("n{s}");
        let next = |n: &str| g.get(n).cloned().unwrap_or_default();
        let back = g
            .get(&start)
            .into_iter()
            .flatten()
            .any(|first| reaches(first, &start, next));
        match closure(&start, next) {
            Ok(found) => {
                prop_assert!(!back);
                let mut expected = reachable(&start, next);
                expected.retain(|n| n != &start);
                prop_assert_eq!(found, expected);
            }
            Err(CycleThroughStart) => prop_assert!(back),
        }
    }
}
