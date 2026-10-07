//! Traversals over a directed graph of string labels.
//!
//! The graph is never materialized here. Each function takes the start and
//! a `next` function giving a node's neighbours, so the caller decides which
//! edges count -- only authoritative ones, the union of two kinds, edges
//! whose signer a viewpoint trusts -- and this module decides only the
//! order they are walked in. That order is part of the contract: every
//! traversal is breadth-first, visits each node once, and takes neighbours
//! in the order `next` yields them, so the same graph always gives the same
//! sequence, which keeps results that depend on it reproducible.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

/// Every node reachable from `start`, `start` first, in breadth-first
/// order.
///
/// # Example
///
/// ```
/// use publet_algorithms::graph::reachable;
///
/// # use std::collections::BTreeMap;
/// # let edges = BTreeMap::from([("a", vec!["b", "c"]), ("b", vec!["d"]), ("c", vec!["d"])]);
/// # let next = |n: &str| -> Vec<String> {
/// #     edges.get(n).into_iter().flatten().map(|s| s.to_string()).collect()
/// # };
/// // a -> b, a -> c, b -> d, c -> d: breadth-first, each node once.
/// assert_eq!(reachable("a", next), ["a", "b", "c", "d"]);
/// ```
pub fn reachable<F, I>(start: &str, mut next: F) -> Vec<String>
where
    F: FnMut(&str) -> I,
    I: IntoIterator<Item = String>,
{
    let mut out = Vec::new();
    let mut seen = BTreeSet::new();
    let mut queue = VecDeque::from([start.to_owned()]);
    while let Some(current) = queue.pop_front() {
        if !seen.insert(current.clone()) {
            continue;
        }
        queue.extend(next(&current));
        out.push(current);
    }
    out
}

/// Whether `target` can be reached from `start`; a node reaches itself.
///
/// Adding an edge `from -> to` closes a cycle exactly when
/// `reaches(to, from, ..)`.
///
/// # Example
///
/// ```
/// use publet_algorithms::graph::reaches;
///
/// # use std::collections::BTreeMap;
/// # let edges = BTreeMap::from([("a", vec!["b", "c"]), ("b", vec!["d"]), ("c", vec!["d"])]);
/// # let next = |n: &str| -> Vec<String> {
/// #     edges.get(n).into_iter().flatten().map(|s| s.to_string()).collect()
/// # };
/// assert!(reaches("a", "d", next));
/// assert!(!reaches("d", "a", next));
/// // Adding d -> a would close a cycle exactly when a already reaches d.
/// ```
pub fn reaches<F, I>(start: &str, target: &str, mut next: F) -> bool
where
    F: FnMut(&str) -> I,
    I: IntoIterator<Item = String>,
{
    let mut seen = BTreeSet::new();
    let mut queue = VecDeque::from([start.to_owned()]);
    while let Some(current) = queue.pop_front() {
        if current == target {
            return true;
        }
        if !seen.insert(current.clone()) {
            continue;
        }
        queue.extend(next(&current));
    }
    false
}

/// [`closure`] found an edge leading back to its start.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CycleThroughStart;

/// Every node reachable from `start` by at least one edge, in
/// breadth-first order, `start` itself excluded.
///
/// # Errors
///
/// [`CycleThroughStart`] if an edge leads back to `start`: for a relation
/// that must be acyclic, the closure of a node on a cycle is not defined.
///
/// # Example
///
/// ```
/// use publet_algorithms::graph::{CycleThroughStart, closure};
///
/// # use std::collections::BTreeMap;
/// # let edges = BTreeMap::from([("a", vec!["b", "c"]), ("b", vec!["d"]), ("c", vec!["d"])]);
/// # let next = |n: &str| -> Vec<String> {
/// #     edges.get(n).into_iter().flatten().map(|s| s.to_string()).collect()
/// # };
/// assert_eq!(closure("a", next).unwrap(), ["b", "c", "d"]);
///
/// // With d -> a added, the closure of a would contain a itself.
/// let cyclic = |n: &str| if n == "d" { vec!["a".to_string()] } else { next(n) };
/// assert_eq!(closure("a", cyclic), Err(CycleThroughStart));
/// ```
pub fn closure<F, I>(start: &str, mut next: F) -> Result<Vec<String>, CycleThroughStart>
where
    F: FnMut(&str) -> I,
    I: IntoIterator<Item = String>,
{
    let mut out = Vec::new();
    let mut seen = BTreeSet::new();
    let mut queue = VecDeque::from([start.to_owned()]);
    let mut first = true;
    while let Some(current) = queue.pop_front() {
        if current == start && !first {
            return Err(CycleThroughStart);
        }
        first = false;
        if !seen.insert(current.clone()) {
            continue;
        }
        queue.extend(next(&current));
        if current != start {
            out.push(current);
        }
    }
    Ok(out)
}

/// Follow `parent` from `start` until a node has none, and return that
/// node. If the walk returns to a node it has passed, it stops there, so a
/// cycle cannot make it run forever.
///
/// # Example
///
/// ```
/// use publet_algorithms::graph::root;
///
/// // Each version names the one it replaces.
/// let replaces = |v: &str| match v {
///     "v3" => Some("v2".to_string()),
///     "v2" => Some("v1".to_string()),
///     _ => None,
/// };
/// assert_eq!(root("v3", replaces), "v1");
/// ```
pub fn root<F>(start: &str, mut parent: F) -> String
where
    F: FnMut(&str) -> Option<String>,
{
    let mut current = start.to_owned();
    let mut passed = BTreeSet::new();
    loop {
        if !passed.insert(current.clone()) {
            return current;
        }
        match parent(&current) {
            Some(next) => current = next,
            None => return current,
        }
    }
}

/// `nodes` ordered so every node comes after everything `deps` says it
/// depends on.
///
/// Depth-first, taking `nodes` and each node's dependencies in the order
/// given, so the order is deterministic. A dependency that is not among
/// `nodes` is still placed, before the nodes that need it.
///
/// # Errors
///
/// The cycle, as the path that closes it -- `["a", "b", "a"]` -- if the
/// dependencies form one.
///
/// # Example
///
/// ```
/// use std::collections::BTreeMap;
/// use publet_algorithms::graph::topological_order;
///
/// let deps = BTreeMap::from([("app", vec!["lib"]), ("lib", vec!["util"])]);
/// let needs = |id: &str| -> Vec<String> {
///     deps.get(id).into_iter().flatten().map(|d| d.to_string()).collect()
/// };
/// assert_eq!(topological_order(["app", "lib", "util"], needs).unwrap(), ["util", "lib", "app"]);
///
/// // A cycle is reported as the path that closes it.
/// let cyclic = |id: &str| vec![if id == "a" { "b" } else { "a" }.to_string()];
/// assert_eq!(topological_order(["a"], cyclic).unwrap_err(), ["a", "b", "a"]);
/// ```
pub fn topological_order<'a, N, F, I>(nodes: N, mut deps: F) -> Result<Vec<String>, Vec<String>>
where
    N: IntoIterator<Item = &'a str>,
    F: FnMut(&str) -> I,
    I: IntoIterator<Item = String>,
{
    #[derive(Clone, Copy, PartialEq)]
    enum Mark {
        Visiting,
        Done,
    }
    fn visit<F, I>(
        id: &str,
        deps: &mut F,
        marks: &mut BTreeMap<String, Mark>,
        path: &mut Vec<String>,
        order: &mut Vec<String>,
    ) -> Result<(), Vec<String>>
    where
        F: FnMut(&str) -> I,
        I: IntoIterator<Item = String>,
    {
        match marks.get(id) {
            Some(Mark::Done) => return Ok(()),
            Some(Mark::Visiting) => {
                let start = path.iter().position(|p| p == id).unwrap_or(0);
                let mut cycle: Vec<String> = path.iter().skip(start).cloned().collect();
                cycle.push(id.to_owned());
                return Err(cycle);
            }
            None => {}
        }
        marks.insert(id.to_owned(), Mark::Visiting);
        path.push(id.to_owned());
        let needed: Vec<String> = deps(id).into_iter().collect();
        for dep in &needed {
            visit(dep, deps, marks, path, order)?;
        }
        path.pop();
        marks.insert(id.to_owned(), Mark::Done);
        order.push(id.to_owned());
        Ok(())
    }

    let mut marks = BTreeMap::new();
    let mut order = Vec::new();
    for id in nodes {
        visit(id, &mut deps, &mut marks, &mut Vec::new(), &mut order)?;
    }
    Ok(order)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use super::*;

    /// A graph from `"a>b"` edge strings.
    fn graph(edges: &[&str]) -> BTreeMap<String, Vec<String>> {
        let mut g: BTreeMap<String, Vec<String>> = BTreeMap::new();
        for e in edges {
            let (from, to) = e.split_once('>').unwrap();
            g.entry(from.to_owned()).or_default().push(to.to_owned());
        }
        g
    }

    fn next(g: &BTreeMap<String, Vec<String>>) -> impl FnMut(&str) -> Vec<String> + '_ {
        |n: &str| g.get(n).cloned().unwrap_or_default()
    }

    // covers: graph::reachable
    #[test]
    fn reachable_is_breadth_first_from_the_start() {
        let g = graph(&["a>b", "a>c", "b>d", "c>d", "d>a"]);
        assert_eq!(reachable("a", next(&g)), ["a", "b", "c", "d"]);
        assert_eq!(reachable("z", next(&g)), ["z"]);
    }

    // covers: graph::reaches
    #[test]
    fn reaches_finds_paths_and_a_node_reaches_itself() {
        let g = graph(&["a>b", "b>c"]);
        assert!(reaches("a", "c", next(&g)));
        assert!(!reaches("c", "a", next(&g)));
        assert!(reaches("b", "b", next(&g)));
    }

    // covers: graph::closure
    #[test]
    fn closure_excludes_the_start_and_refuses_a_cycle_through_it() {
        let g = graph(&["a>b", "b>c", "c>b"]);
        assert_eq!(closure("a", next(&g)).unwrap(), ["b", "c"]);
        let g = graph(&["a>b", "b>a"]);
        assert_eq!(closure("a", next(&g)), Err(CycleThroughStart));
        let g = graph(&["a>a"]);
        assert_eq!(closure("a", next(&g)), Err(CycleThroughStart));
    }

    // covers: graph::root
    #[test]
    fn root_follows_parents_and_stops_on_a_cycle() {
        let parents = graph(&["c>b", "b>a"]);
        let up = |n: &str| parents.get(n).and_then(|p| p.first().cloned());
        assert_eq!(root("c", up), "a");
        let looped = graph(&["a>b", "b>a"]);
        let up = |n: &str| looped.get(n).and_then(|p| p.first().cloned());
        assert_eq!(root("a", up), "a");
    }

    // covers: graph::topological_order
    #[test]
    fn topological_order_puts_dependencies_first() {
        let deps = graph(&["app>lib", "app>util", "lib>util"]);
        let order = topological_order(["app", "lib", "util"], next(&deps)).unwrap();
        assert_eq!(order, ["util", "lib", "app"]);
        let pos = |n: &str| order.iter().position(|o| o == n).unwrap();
        assert!(pos("util") < pos("lib") && pos("lib") < pos("app"));
    }

    // covers: graph::topological_order
    #[test]
    fn topological_order_names_the_cycle() {
        let deps = graph(&["a>b", "b>c", "c>b"]);
        assert_eq!(
            topological_order(["a"], next(&deps)),
            Err(vec!["b".to_owned(), "c".to_owned(), "b".to_owned()])
        );
    }
}
