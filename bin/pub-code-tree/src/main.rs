//! `pub-code-tree`: parse a Rust source file into a content-addressed
//! tree, one CID per definition.
//!
//! Every top-level item (`fn`, `struct`, `enum`, `trait`, `impl`, `mod`,
//! `const`, `static`, `type`) becomes a node, hashed from its own exact
//! source text -- not a re-serialized or reformatted rendering, the
//! bytes as written, because the source file is still the logical
//! source of truth (git and every standard tool keeps working on it
//! unmodified) and this tool is a read of it, not an alternative to it.
//! `impl` blocks and inline `mod { ... }` blocks additionally become
//! *branches*: their own hash commits to their signature plus every
//! nested item's own hash, via [`graphset::tree::Node`], so
//! `Membership::prove` is addressable as its own node distinct from
//! `Membership` as a whole. A function nested inside another function's
//! body is not walked in this first pass -- real in this codebase, rare,
//! and left for a follow-up rather than guessed at.
//!
//! Emits one JSON object per line (JSON Lines) to stdout: `path` (the
//! `/`-joined name from the file root, e.g. `membership/Membership/prove`),
//! `kind`, `cid` (the node's hash, addressed as a CID the same way a
//! domain manifest's `snapshot` field is -- via [`Cid::from_digest`], not
//! a stored object), and `start`/`end` byte offsets into the file, so a
//! CID and the file together are enough to extract precisely the text
//! that was hashed. For a branch these bound the whole item -- `impl S {
//! ... }`, braces included -- not just its own signature, since that is
//! what its span already is; nothing here recomputes it as some
//! narrower range.

use std::path::PathBuf;

use proc_macro2::LineColumn;
use publet_core::{Cid, HashAlg};
use syn::spanned::Spanned as _;

/// Precomputed byte offset of the start of each line, so a
/// `proc_macro2::LineColumn` (1-indexed line, 0-indexed char column) can
/// be turned into a byte offset into the original file without
/// rescanning from the start every time.
struct LineIndex {
    /// `starts[i]` is the byte offset where line `i + 1` begins.
    starts: Vec<usize>,
}

impl LineIndex {
    fn build(text: &str) -> Self {
        let mut starts = vec![0];
        for (i, b) in text.bytes().enumerate() {
            if b == b'\n' {
                starts.push(i + 1);
            }
        }
        Self { starts }
    }

    /// The byte offset of `pos`, resolving its column as a *character*
    /// offset within the line (what `proc_macro2` reports) rather than
    /// assuming one byte per column, which would be wrong the moment a
    /// line contains anything outside ASCII.
    fn byte_offset(&self, text: &str, pos: LineColumn) -> usize {
        let line_start = self.starts.get(pos.line - 1).copied().unwrap_or(text.len());
        let line_end = self.starts.get(pos.line).copied().unwrap_or(text.len());
        let Some(line) = text.get(line_start..line_end) else {
            return line_start;
        };
        line.char_indices()
            .nth(pos.column)
            .map_or(line_end, |(i, _)| line_start + i)
    }
}

/// One node on its way to becoming a `graphset::tree::Node`, still
/// carrying its file position.
struct Draft {
    name: String,
    kind: &'static str,
    start: usize,
    end: usize,
    children: Vec<Draft>,
}

impl Draft {
    fn leaf(name: String, kind: &'static str, start: usize, end: usize) -> Self {
        Self {
            name,
            kind,
            start,
            end,
            children: Vec::new(),
        }
    }

    /// This node's own hash, computed by borrowing rather than
    /// consuming -- `emit` needs both the hash and, for a branch, the
    /// still-intact `children` list to recurse into afterward.
    fn hash(&self, text: &str) -> graphset::log::Hash {
        self.to_node(text).hash()
    }

    fn to_node(&self, text: &str) -> graphset::tree::Node {
        if self.children.is_empty() {
            graphset::tree::Node::leaf(text.as_bytes().get(self.start..self.end).unwrap_or(&[]))
        } else {
            let mut node = graphset::tree::Node::branch(format!("{}:{}", self.kind, self.name));
            for child in &self.children {
                node = node.with_child(child.name.clone(), child.to_node(text));
            }
            node
        }
    }
}

fn span_bytes(idx: &LineIndex, text: &str, span: proc_macro2::Span) -> (usize, usize) {
    (
        idx.byte_offset(text, span.start()),
        idx.byte_offset(text, span.end()),
    )
}

fn item_name(item: &syn::Item) -> Option<String> {
    match item {
        syn::Item::Fn(i) => Some(i.sig.ident.to_string()),
        syn::Item::Struct(i) => Some(i.ident.to_string()),
        syn::Item::Enum(i) => Some(i.ident.to_string()),
        syn::Item::Trait(i) => Some(i.ident.to_string()),
        syn::Item::Impl(i) => Some(match &*i.self_ty {
            syn::Type::Path(p) => p
                .path
                .segments
                .last()
                .map_or_else(|| quote::quote!(#p).to_string(), |s| s.ident.to_string()),
            other => quote::quote!(#other).to_string(),
        }),
        syn::Item::Mod(i) => Some(i.ident.to_string()),
        syn::Item::Const(i) => Some(i.ident.to_string()),
        syn::Item::Static(i) => Some(i.ident.to_string()),
        syn::Item::Type(i) => Some(i.ident.to_string()),
        _ => None,
    }
}

fn item_kind(item: &syn::Item) -> &'static str {
    match item {
        syn::Item::Fn(_) => "fn",
        syn::Item::Struct(_) => "struct",
        syn::Item::Enum(_) => "enum",
        syn::Item::Trait(_) => "trait",
        syn::Item::Impl(_) => "impl",
        syn::Item::Mod(_) => "mod",
        syn::Item::Const(_) => "const",
        syn::Item::Static(_) => "static",
        syn::Item::Type(_) => "type",
        _ => "other",
    }
}

fn draft_for_item(idx: &LineIndex, text: &str, item: &syn::Item) -> Option<Draft> {
    let name = item_name(item)?;
    let kind = item_kind(item);
    let (start, end) = span_bytes(idx, text, item.span());

    let children = match item {
        syn::Item::Impl(i) => i
            .items
            .iter()
            .filter_map(|ii| draft_for_impl_item(idx, text, ii))
            .collect(),
        syn::Item::Mod(syn::ItemMod {
            content: Some((_, items)),
            ..
        }) => items
            .iter()
            .filter_map(|it| draft_for_item(idx, text, it))
            .collect(),
        _ => Vec::new(),
    };

    Some(Draft {
        name,
        kind,
        start,
        end,
        children,
    })
}

fn draft_for_impl_item(idx: &LineIndex, text: &str, item: &syn::ImplItem) -> Option<Draft> {
    let (name, kind) = match item {
        syn::ImplItem::Fn(f) => (f.sig.ident.to_string(), "fn"),
        syn::ImplItem::Const(c) => (c.ident.to_string(), "const"),
        syn::ImplItem::Type(t) => (t.ident.to_string(), "type"),
        _ => return None,
    };
    let (start, end) = span_bytes(idx, text, item.span());
    Some(Draft::leaf(name, kind, start, end))
}

fn main() -> std::process::ExitCode {
    let mut path: Option<PathBuf> = None;
    for arg in std::env::args().skip(1) {
        if let Some(v) = arg.strip_prefix("--file=") {
            path = Some(PathBuf::from(v));
        } else {
            eprintln!("unknown argument: {arg}");
            return std::process::ExitCode::from(2);
        }
    }
    let Some(path) = path else {
        eprintln!("usage: pub-code-tree --file=PATH");
        return std::process::ExitCode::from(2);
    };

    let text = match std::fs::read_to_string(&path) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("{}: {e}", path.display());
            return std::process::ExitCode::from(4);
        }
    };
    let file = match syn::parse_file(&text) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("{}: {e}", path.display());
            return std::process::ExitCode::from(1);
        }
    };

    let idx = LineIndex::build(&text);
    let drafts: Vec<Draft> = file
        .items
        .iter()
        .filter_map(|it| draft_for_item(&idx, &text, it))
        .collect();

    for draft in &drafts {
        if let Err(e) = emit(draft, &text, "") {
            eprintln!("{e}");
            return std::process::ExitCode::from(1);
        }
    }
    std::process::ExitCode::SUCCESS
}

/// A SHA-256 digest is always exactly 32 bytes, so [`Cid::from_digest`]
/// cannot actually fail here -- but nothing in the type system proves
/// that, so it is treated as a real (if inert) error path rather than
/// asserted away with `expect`.
fn emit(draft: &Draft, text: &str, prefix: &str) -> Result<(), String> {
    let path = if prefix.is_empty() {
        draft.name.clone()
    } else {
        format!("{prefix}/{}", draft.name)
    };
    let cid = Cid::from_digest(HashAlg::Sha2_256, &draft.hash(text))
        .ok_or_else(|| format!("{path}: could not address a 32-byte hash as a CID"))?;
    let (kind, start, end) = (draft.kind, draft.start, draft.end);
    println!(r#"{{"path":{path:?},"kind":{kind:?},"cid":"{cid}","start":{start},"end":{end}}}"#);
    for child in &draft.children {
        emit(child, text, &path)?;
    }
    Ok(())
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing)]
mod tests {
    use super::*;

    fn drafts(src: &str) -> (Vec<Draft>, String) {
        let text = src.to_owned();
        let file = syn::parse_file(&text).expect("fixture must parse");
        let idx = LineIndex::build(&text);
        let drafts = file
            .items
            .iter()
            .filter_map(|it| draft_for_item(&idx, &text, it))
            .collect();
        (drafts, text)
    }

    #[test]
    fn byte_offset_accounts_for_multibyte_characters() {
        // "é" is two UTF-8 bytes but one character (one proc_macro2
        // column), so a naive byte-per-column assumption would land one
        // byte short of `x` on this line.
        let text = "// résumé\nfn x() {}\n";
        let idx = LineIndex::build(text);
        let start = idx.byte_offset(text, LineColumn { line: 2, column: 0 });
        assert_eq!(&text[start..start + 2], "fn");
    }

    #[test]
    fn a_leaf_items_span_is_exactly_its_source_text() {
        let (drafts, text) = drafts("/// doc\npub fn f(x: u32) -> u32 {\n    x\n}\n");
        assert_eq!(drafts.len(), 1);
        let d = &drafts[0];
        assert_eq!(
            &text[d.start..d.end],
            "/// doc\npub fn f(x: u32) -> u32 {\n    x\n}"
        );
    }

    #[test]
    fn an_impl_block_becomes_a_branch_over_its_methods() {
        let (drafts, text) = drafts("struct S;\nimpl S {\n    fn a() {}\n    fn b() {}\n}\n");
        let imp = drafts.iter().find(|d| d.kind == "impl").expect("impl item");
        assert_eq!(imp.name, "S");
        assert_eq!(imp.children.len(), 2);
        assert!(imp.children.iter().any(|c| c.name == "a"));
        assert!(imp.children.iter().any(|c| c.name == "b"));
        let _ = text;
    }

    #[test]
    fn a_branch_items_span_covers_its_whole_body_not_just_its_signature() {
        let (drafts, text) = drafts("struct S;\nimpl S {\n    fn a() {}\n    fn b() {}\n}\n");
        let imp = drafts.iter().find(|d| d.kind == "impl").expect("impl item");
        assert_eq!(
            &text[imp.start..imp.end],
            "impl S {\n    fn a() {}\n    fn b() {}\n}"
        );
    }

    #[test]
    fn identical_bodies_at_different_names_hash_differently() {
        let (drafts, text) = drafts("fn a() { 1 }\nfn b() { 1 }\n");
        assert_ne!(drafts[0].hash(&text), drafts[1].hash(&text));
    }

    #[test]
    fn the_same_file_parsed_twice_hashes_identically() {
        let src = "pub fn f(x: u32) -> u32 { x + 1 }\n";
        let (a, ta) = drafts(src);
        let (b, tb) = drafts(src);
        assert_eq!(a[0].hash(&ta), b[0].hash(&tb));
    }

    #[test]
    fn changing_a_methods_body_changes_the_impl_blocks_hash_too() {
        let (before, tb) = drafts("struct S;\nimpl S {\n    fn a() -> u32 { 1 }\n}\n");
        let (after, ta) = drafts("struct S;\nimpl S {\n    fn a() -> u32 { 2 }\n}\n");
        let impl_before = before.iter().find(|d| d.kind == "impl").unwrap();
        let impl_after = after.iter().find(|d| d.kind == "impl").unwrap();
        assert_ne!(impl_before.hash(&tb), impl_after.hash(&ta));
    }
}
