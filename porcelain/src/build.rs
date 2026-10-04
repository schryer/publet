//! `pub build DIR`: publish a tree of named publet sources, and the lock
//! that maps each name to what it became.
//!
//! People link by name and objects link by identifier, and this command is
//! where one becomes the other. Every `publet.json` under `DIR` describes
//! one publet -- a `doc` in the shape `pub doc` reads, plus the claims it
//! bundles -- and refers to anything else by a short **slug**: `tool.typst`
//! names another publet, `tool.typst#identity` a claim inside it, `#usage`
//! a claim in the same file, and a `pub:...` identifier stays what it is.
//!
//! Section 9.1 is why the names stay out here. PUB has no namespace, and a
//! name written into an object's own bytes could never be withdrawn, so a
//! slug never enters any object: the build replaces each with the
//! identifier it resolves to, publishes children before the parents whose
//! identifiers cover theirs, and records the mapping in `publets.lock` --
//! a generated file, the only place a slug and an identifier are written
//! side by side. A publet's citable name is its `tag`, filed as the
//! Section 9.1 `tagged` annotation it always was, on the lineage's genesis
//! so that a revision never needs a new one.
//!
//! A rebuild from unchanged sources publishes nothing: each object is
//! compared against the version the lock names, and a new version, with
//! its `supersedes` edge, exists only where something changed (Section
//! 5.8). Nothing is stored unless the whole tree is admissible.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use publet_core::{Cid, HashAlg, Object, cbor::Value};
use publet_graph::{Class, Graph, RelationKind};
use publet_store::Store;

use crate::workspace::Workspace;

/// The lock file's name, written beside the sources it describes.
pub(crate) const LOCK: &str = "publets.lock";
const SOURCE: &str = "publet.json";

/// Keys a source file may carry: the `pub doc` manifest's, plus naming.
const SOURCE_KEYS: [&str; 10] = [
    "slug",
    "tag",
    "supersedes",
    "created",
    "claims",
    "title",
    "lang",
    "license",
    "abstract",
    "sections",
];
const CLAIM_KEYS: [&str; 11] = [
    "class",
    "content",
    "scope",
    "lang",
    "depends",
    "method",
    "cites",
    "sources",
    "data",
    "created",
    "supersedes",
];

/// Build every publet under a directory.
///
/// # Errors
///
/// Returns a message if a source is malformed, a name does not resolve,
/// the names form a cycle, or the graph refuses anything the build would
/// publish -- in which case nothing is stored.
pub(crate) fn run(args: &[String]) -> Result<(), String> {
    let here = std::env::current_dir().map_err(|e| e.to_string())?;
    // Inside a corpus with no tree named, the corpus is what is built.
    if here.join(crate::corpus::CONFIG).exists() && args.iter().all(|a| a.starts_with("--")) {
        return crate::corpus::build(args);
    }
    let ws = Workspace::open(&here)?;

    let mut dir = None;
    let mut dry_run = false;
    let mut sign = false;
    for arg in args {
        if arg == "--dry-run" {
            dry_run = true;
        } else if arg == "--sign" {
            sign = true;
        } else if arg.starts_with("--") {
            return Err(format!("unknown argument: {arg}"));
        } else {
            dir = Some(PathBuf::from(arg));
        }
    }
    let dir = dir.unwrap_or_else(|| PathBuf::from("publets"));
    let author = ws
        .get("author")
        .ok_or("no author configured; run `pub init`")?;
    let store = ws.store()?;

    let lock_path = dir.join(LOCK);
    let lock = Lock::read(&lock_path)?;
    let mut graph = graph_of(&store)?;
    let tree = build_tree(
        &Tree {
            dir: &dir,
            lock_dir: &dir,
            here: &here,
            author: &author,
            store: &store,
            lock: &lock,
            importer: None,
            sign,
            tag: true,
        },
        &mut graph,
    )?;

    let published = tree.pending.len();
    tree.report_lines();
    if dry_run {
        println!("dry run: {published} object(s) would be published; nothing stored");
        return Ok(());
    }
    for (cid, bytes) in &tree.pending {
        store.put(cid, bytes).map_err(|e| e.to_string())?;
    }
    tree.lock.write(&lock_path)?;
    println!(
        "{published} object(s) published; {} written",
        lock_path.display()
    );
    Ok(())
}

/// What one build of a tree of sources needs.
pub(crate) struct Tree<'a> {
    pub(crate) dir: &'a Path,
    pub(crate) lock_dir: &'a Path,
    pub(crate) here: &'a Path,
    pub(crate) author: &'a str,
    pub(crate) store: &'a Store,
    pub(crate) lock: &'a Lock,
    pub(crate) importer: Option<&'a crate::corpus::Importer>,
    pub(crate) sign: bool,
    /// Whether to file each publet's tag. A draft build does not: a tag
    /// names a published thing.
    pub(crate) tag: bool,
}

/// What one build of a tree produced, before anything is stored.
pub(crate) struct Built {
    pub(crate) pending: Vec<(Cid, Vec<u8>)>,
    /// Every object a changed publet consists of -- its new versions, their
    /// `supersedes` edges, its tags -- whether new to the store or already
    /// held from an earlier draft, with the node it belongs to.
    pub(crate) owned: Vec<(Cid, String)>,
    pub(crate) report: Vec<(String, &'static str, String)>,
    pub(crate) lock: Lock,
}

impl Built {
    pub(crate) fn report_lines(&self) {
        for (id, status, cid) in &self.report {
            println!("{status:<9} {id}  {cid}");
        }
    }
}

/// Build every publet under `tree.dir` into `graph`, without storing
/// anything: the caller stores `pending` once it is satisfied.
///
/// # Errors
///
/// As [`run`].
pub(crate) fn build_tree(tree: &Tree<'_>, graph: &mut Graph) -> Result<Built, String> {
    let sources = load_sources(tree.dir)?;
    let nodes = nodes_of(&sources, tree.importer)?;
    let order = topological(&nodes)?;

    let mut build = Build {
        author: tree.author,
        store: tree.store,
        graph,
        lock: tree.lock,
        importer: tree.importer,
        resolved: BTreeMap::new(),
        pending: Vec::new(),
        owned: Vec::new(),
        report: Vec::new(),
    };
    for node in order.iter().filter_map(|id| nodes.get(id)) {
        build.node(node)?;
    }
    if tree.tag {
        for source in &sources {
            build.tag(source)?;
        }
    }
    if tree.sign {
        build.sign_all(tree.here)?;
    }
    build
        .graph
        .validate()
        .map_err(|e| format!("the built tree was refused, so nothing was stored: {e}"))?;

    let lock = Lock {
        entries: build
            .resolved
            .iter()
            .filter_map(|(id, r)| nodes.get(id).map(|node| (id, r, node)))
            .map(|(id, r, node)| {
                let entry = LockEntry {
                    slug: id.clone(),
                    kind: node.kind.label().to_owned(),
                    cid: r.cid.to_string(),
                    genesis: r.genesis.to_string(),
                    tag: node.tag.clone(),
                    source: node
                        .base
                        .strip_prefix(tree.lock_dir)
                        .ok()
                        .map(|p| p.display().to_string()),
                    deps: node.deps.iter().cloned().collect(),
                };
                (id.clone(), entry)
            })
            .collect(),
        corpus: tree.lock.corpus.clone(),
    };
    Ok(Built {
        pending: build.pending,
        owned: build.owned,
        report: build.report,
        lock,
    })
}

// ---------------------------------------------------------------- sources

/// One `publet.json`.
struct Source {
    path: PathBuf,
    slug: String,
    tag: String,
    created: Option<String>,
    json: serde_json::Map<String, serde_json::Value>,
}

impl Source {
    fn base(&self) -> &Path {
        self.path.parent().unwrap_or_else(|| Path::new("."))
    }
}

fn load_sources(dir: &Path) -> Result<Vec<Source>, String> {
    let mut paths = Vec::new();
    find_sources(dir, &mut paths)?;
    if paths.is_empty() {
        return Err(format!("no {SOURCE} found under {}", dir.display()));
    }
    paths.sort();

    let mut sources = Vec::new();
    let mut slugs: BTreeMap<String, PathBuf> = BTreeMap::new();
    let mut tags: BTreeMap<String, String> = BTreeMap::new();
    for path in paths {
        let where_ = path.display().to_string();
        let serde_json::Value::Object(json) = crate::payload::read_json(&path)? else {
            return Err(format!("{where_}: a publet source is a JSON object"));
        };
        if let Some(key) = json.keys().find(|k| !SOURCE_KEYS.contains(&k.as_str())) {
            return Err(format!(
                "{where_}: unknown key `{key}`; expected one of {}",
                SOURCE_KEYS.join(", ")
            ));
        }
        let text = |key: &str| json.get(key).and_then(serde_json::Value::as_str);
        let slug = text("slug")
            .ok_or_else(|| format!("{where_}: `slug` is required"))?
            .to_owned();
        check_slug(&slug).map_err(|e| format!("{where_}: {e}"))?;
        // Section 9.1: a publet is what a short tag finds, so every one
        // carries a tag. The check is only that it is recognizably one.
        let tag = text("tag")
            .ok_or_else(|| {
                format!("{where_}: `tag` is required: a Section 9.1 citation tag such as PUB-SEMDRI-09-2026")
            })?
            .to_owned();
        crate::annotate::check_tag_format(&tag).map_err(|e| format!("{where_}: {e}"))?;
        if let Some(other) = slugs.insert(slug.clone(), path.clone()) {
            return Err(format!(
                "slug `{slug}` is declared by both {} and {where_}",
                other.display()
            ));
        }
        if let Some(other) = tags.insert(tag.clone(), slug.clone()) {
            return Err(format!(
                "tag {tag} is declared by both `{other}` and `{slug}`"
            ));
        }
        let created = text("created").map(str::to_owned);
        sources.push(Source {
            path,
            slug,
            tag,
            created,
            json,
        });
    }
    Ok(sources)
}

fn find_sources(dir: &Path, out: &mut Vec<PathBuf>) -> Result<(), String> {
    let entries =
        std::fs::read_dir(dir).map_err(|e| format!("cannot read {}: {e}", dir.display()))?;
    for entry in entries {
        let path = entry.map_err(|e| e.to_string())?.path();
        let hidden_or_output = path
            .file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| n.starts_with('.') || n == "_build");
        if path.is_dir() && !hidden_or_output {
            find_sources(&path, out)?;
        } else if path.file_name().is_some_and(|n| n == SOURCE) {
            out.push(path);
        }
    }
    Ok(())
}

/// A slug is lowercase words joined by `-`, in dot-separated segments:
/// `semantic-drift`, `tool.typst`, `xdoc.typst-reference`.
fn check_slug(slug: &str) -> Result<(), String> {
    let ok = !slug.is_empty()
        && slug.split('.').all(|seg| {
            !seg.is_empty()
                && seg
                    .bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
        });
    if ok {
        Ok(())
    } else {
        Err(format!(
            "`{slug}` is not a slug: lowercase letters, digits, and `-`, in \
             `.`-separated segments"
        ))
    }
}

fn check_fragment(fragment: &str) -> Result<(), String> {
    let ok = !fragment.is_empty()
        && fragment
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-');
    if ok {
        Ok(())
    } else {
        Err(format!(
            "`{fragment}` is not a claim name: lowercase letters, digits, and `-`"
        ))
    }
}

// ------------------------------------------------------------------ nodes

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Doc,
    Claim,
}

impl Kind {
    fn label(self) -> &'static str {
        match self {
            Self::Doc => "doc",
            Self::Claim => "claim",
        }
    }
}

/// One object the build produces: a publet's document, or a claim it
/// bundles. `id` is the slug, or `slug#name` for a claim.
struct Node {
    id: String,
    kind: Kind,
    json: serde_json::Map<String, serde_json::Value>,
    base: PathBuf,
    owner: String,
    created: Option<String>,
    supersedes: Option<Cid>,
    tag: Option<String>,
    deps: BTreeSet<String>,
}

/// What a reference in a source names.
enum Ref {
    Cid(Cid),
    Node(String),
    /// `alias:slug` -- a publet of the parent corpus named `alias`.
    Import(String),
}

fn resolve_ref(raw: &str, owner: &str) -> Result<Ref, String> {
    if raw.starts_with("pub:") {
        return raw
            .parse::<Cid>()
            .map(Ref::Cid)
            .map_err(|_| format!("`{raw}` begins like an identifier but is not one"));
    }
    if let Some((alias, rest)) = raw.split_once(':') {
        check_fragment(alias).map_err(|_| format!("`{alias}` is not a parent alias"))?;
        match rest.split_once('#') {
            Some((slug, fragment)) => {
                check_slug(slug)?;
                check_fragment(fragment)?;
            }
            None => check_slug(rest)?,
        }
        return Ok(Ref::Import(raw.to_owned()));
    }
    let id = if let Some(fragment) = raw.strip_prefix('#') {
        check_fragment(fragment)?;
        format!("{owner}#{fragment}")
    } else if let Some((slug, fragment)) = raw.split_once('#') {
        check_slug(slug)?;
        check_fragment(fragment)?;
        raw.to_owned()
    } else {
        check_slug(raw)?;
        raw.to_owned()
    };
    Ok(Ref::Node(id))
}

/// Every reference a node's JSON makes, as (raw text) in source order.
fn refs_of(kind: Kind, json: &serde_json::Map<String, serde_json::Value>) -> Vec<String> {
    let mut out = Vec::new();
    let mut push = |v: Option<&serde_json::Value>| {
        if let Some(s) = v.and_then(serde_json::Value::as_str) {
            out.push(s.to_owned());
        }
    };
    match kind {
        Kind::Doc => {
            push(json.get("abstract"));
            for section in json
                .get("sections")
                .and_then(serde_json::Value::as_array)
                .into_iter()
                .flatten()
            {
                for item in section
                    .get("items")
                    .and_then(serde_json::Value::as_array)
                    .into_iter()
                    .flatten()
                {
                    push(item.get("ref"));
                    push(item.get("at"));
                }
            }
        }
        Kind::Claim => {
            for dep in json
                .get("depends")
                .and_then(serde_json::Value::as_array)
                .into_iter()
                .flatten()
            {
                let bare = dep.as_str().map(|d| {
                    serde_json::Value::String(d.strip_prefix('~').unwrap_or(d).to_owned())
                });
                push(bare.as_ref());
            }
            push(json.get("method"));
        }
    }
    out
}

fn nodes_of(
    sources: &[Source],
    importer: Option<&crate::corpus::Importer>,
) -> Result<BTreeMap<String, Node>, String> {
    let mut nodes = BTreeMap::new();
    for source in sources {
        let where_ = source.path.display().to_string();
        let supersedes = |json: &serde_json::Map<String, serde_json::Value>| {
            json.get("supersedes")
                .map(|v| {
                    v.as_str()
                        .and_then(|s| s.parse::<Cid>().ok())
                        .ok_or_else(|| format!("{where_}: `supersedes` must be an identifier"))
                })
                .transpose()
        };

        let claims = match source.json.get("claims") {
            None => serde_json::Map::new(),
            Some(serde_json::Value::Object(map)) => map.clone(),
            Some(_) => {
                return Err(format!(
                    "{where_}: `claims` must be an object of named claims"
                ));
            }
        };
        for (name, claim) in &claims {
            check_fragment(name).map_err(|e| format!("{where_}: {e}"))?;
            let serde_json::Value::Object(json) = claim else {
                return Err(format!("{where_}: claim `{name}` must be an object"));
            };
            if let Some(key) = json.keys().find(|k| !CLAIM_KEYS.contains(&k.as_str())) {
                return Err(format!(
                    "{where_}: claim `{name}`: unknown key `{key}`; expected one of {}",
                    CLAIM_KEYS.join(", ")
                ));
            }
            let id = format!("{}#{name}", source.slug);
            nodes.insert(
                id.clone(),
                Node {
                    id,
                    kind: Kind::Claim,
                    json: json.clone(),
                    base: source.base().to_path_buf(),
                    owner: source.slug.clone(),
                    created: json
                        .get("created")
                        .and_then(serde_json::Value::as_str)
                        .map(str::to_owned)
                        .or_else(|| source.created.clone()),
                    supersedes: supersedes(json)?,
                    tag: None,
                    deps: BTreeSet::new(),
                },
            );
        }

        let mut doc = source.json.clone();
        for key in ["slug", "tag", "supersedes", "claims"] {
            doc.remove(key);
        }
        nodes.insert(
            source.slug.clone(),
            Node {
                id: source.slug.clone(),
                kind: Kind::Doc,
                json: doc,
                base: source.base().to_path_buf(),
                owner: source.slug.clone(),
                created: source.created.clone(),
                supersedes: supersedes(&source.json)?,
                tag: Some(source.tag.clone()),
                deps: BTreeSet::new(),
            },
        );
    }

    // Resolve every reference now, so a name that names nothing is
    // reported before anything is built.
    let ids: BTreeSet<String> = nodes.keys().cloned().collect();
    for node in nodes.values_mut() {
        for raw in refs_of(node.kind, &node.json) {
            let resolved =
                resolve_ref(&raw, &node.owner).map_err(|e| format!("`{}`: {e}", node.id))?;
            if let Ref::Node(target) = resolved {
                if !ids.contains(&target) {
                    // A parent's publets are never reached by a bare name:
                    // falling through would let a parent shadow a child's
                    // name, namespace capture in miniature (Section 9.1).
                    let hint = importer
                        .and_then(|i| i.alias_declaring(&target))
                        .map(|alias| format!("; a parent declares it -- write `{alias}:{target}`"))
                        .unwrap_or_default();
                    return Err(format!(
                        "`{}` refers to `{target}`, which no {SOURCE} declares{hint}",
                        node.id
                    ));
                }
                node.deps.insert(target);
            }
        }
    }
    Ok(nodes)
}

/// Children before parents. Content addressing forces this order -- a
/// parent's identifier covers its children's -- and makes a cycle a
/// structure no build could ever produce, so one is refused by name.
fn topological(nodes: &BTreeMap<String, Node>) -> Result<Vec<String>, String> {
    #[derive(Clone, Copy, PartialEq)]
    enum Mark {
        Visiting,
        Done,
    }
    fn visit(
        id: &str,
        nodes: &BTreeMap<String, Node>,
        marks: &mut BTreeMap<String, Mark>,
        path: &mut Vec<String>,
        order: &mut Vec<String>,
    ) -> Result<(), String> {
        match marks.get(id) {
            Some(Mark::Done) => return Ok(()),
            Some(Mark::Visiting) => {
                let start = path.iter().position(|p| p == id).unwrap_or(0);
                let mut cycle: Vec<&str> = path.iter().skip(start).map(String::as_str).collect();
                cycle.push(id);
                return Err(format!(
                    "these publets refer to each other in a cycle, which no \
                     content-addressed build can produce: {}",
                    cycle.join(" -> ")
                ));
            }
            None => {}
        }
        marks.insert(id.to_owned(), Mark::Visiting);
        path.push(id.to_owned());
        if let Some(node) = nodes.get(id) {
            for dep in &node.deps {
                visit(dep, nodes, marks, path, order)?;
            }
        }
        path.pop();
        marks.insert(id.to_owned(), Mark::Done);
        order.push(id.to_owned());
        Ok(())
    }

    let mut marks = BTreeMap::new();
    let mut order = Vec::new();
    for id in nodes.keys() {
        visit(id, nodes, &mut marks, &mut Vec::new(), &mut order)?;
    }
    Ok(order)
}

// ------------------------------------------------------------------- lock

#[derive(Clone)]
pub(crate) struct LockEntry {
    pub(crate) slug: String,
    pub(crate) kind: String,
    pub(crate) cid: String,
    pub(crate) genesis: String,
    pub(crate) tag: Option<String>,
    /// Where the publet's source lives, relative to the lock.
    pub(crate) source: Option<String>,
    pub(crate) deps: Vec<String>,
}

/// `publets.lock` (or a corpus's `corpus.lock`): slug to identifier, as
/// last built, and for a corpus what it is pinned to.
pub(crate) struct Lock {
    pub(crate) entries: BTreeMap<String, LockEntry>,
    /// A corpus's own section, owned by `corpus.rs` and kept verbatim here.
    pub(crate) corpus: Option<serde_json::Value>,
}

impl Lock {
    pub(crate) fn read(path: &Path) -> Result<Self, String> {
        if !path.exists() {
            return Ok(Self {
                entries: BTreeMap::new(),
                corpus: None,
            });
        }
        let json = crate::payload::read_json(path)?;
        let bad = || format!("{} is not a publets lock", path.display());
        let rows = json
            .get("publets")
            .and_then(serde_json::Value::as_array)
            .ok_or_else(bad)?;
        let mut entries = BTreeMap::new();
        for row in rows {
            let text = |k: &str| {
                row.get(k)
                    .and_then(serde_json::Value::as_str)
                    .map(str::to_owned)
            };
            let entry = LockEntry {
                slug: text("slug").ok_or_else(bad)?,
                kind: text("kind").ok_or_else(bad)?,
                cid: text("cid").ok_or_else(bad)?,
                genesis: text("genesis").ok_or_else(bad)?,
                tag: text("tag"),
                source: text("source"),
                deps: row
                    .get("deps")
                    .and_then(serde_json::Value::as_array)
                    .map(|d| {
                        d.iter()
                            .filter_map(serde_json::Value::as_str)
                            .map(str::to_owned)
                            .collect()
                    })
                    .unwrap_or_default(),
            };
            entries.insert(entry.slug.clone(), entry);
        }
        Ok(Self {
            entries,
            corpus: json.get("corpus").cloned(),
        })
    }

    /// Sorted by slug, keys sorted, one trailing newline: two builds of one
    /// tree write identical bytes, so the lock diffs only where the tree did.
    /// Sorted by slug, one row per line in a fixed field order -- the slug
    /// first, because it is what a person scans for -- and one trailing
    /// newline: two builds of one tree write identical bytes, so the lock
    /// diffs only where the tree did.
    fn render(&self) -> Result<String, String> {
        let quote = |s: &str| serde_json::to_string(s).map_err(|e| e.to_string());
        let mut rows = Vec::new();
        for e in self.entries.values() {
            let mut fields = vec![
                format!("\"slug\": {}", quote(&e.slug)?),
                format!("\"kind\": {}", quote(&e.kind)?),
            ];
            if let Some(tag) = &e.tag {
                fields.push(format!("\"tag\": {}", quote(tag)?));
            }
            if let Some(source) = &e.source {
                fields.push(format!("\"source\": {}", quote(source)?));
            }
            fields.push(format!("\"cid\": {}", quote(&e.cid)?));
            fields.push(format!("\"genesis\": {}", quote(&e.genesis)?));
            let deps = e
                .deps
                .iter()
                .map(|d| quote(d))
                .collect::<Result<Vec<_>, _>>()?;
            fields.push(format!("\"deps\": [{}]", deps.join(", ")));
            rows.push(format!("    {{{}}}", fields.join(", ")));
        }
        let corpus = match &self.corpus {
            Some(c) => {
                let pretty = serde_json::to_string_pretty(c).map_err(|e| e.to_string())?;
                format!("  \"corpus\": {},\n", pretty.replace('\n', "\n  "))
            }
            None => String::new(),
        };
        Ok(format!(
            "{{\n  \"comment\": \"Generated by `pub build`: each slug and the object it \
             became. Do not edit.\",\n{corpus}  \"publets\": [\n{}\n  ]\n}}\n",
            rows.join(",\n")
        ))
    }

    pub(crate) fn write(&self, path: &Path) -> Result<(), String> {
        std::fs::write(path, self.render()?)
            .map_err(|e| format!("cannot write {}: {e}", path.display()))
    }
}

// ------------------------------------------------------------------ build

struct Resolved {
    cid: Cid,
    genesis: Cid,
}

struct Build<'a> {
    author: &'a str,
    store: &'a Store,
    graph: &'a mut Graph,
    lock: &'a Lock,
    importer: Option<&'a crate::corpus::Importer>,
    resolved: BTreeMap<String, Resolved>,
    pending: Vec<(Cid, Vec<u8>)>,
    owned: Vec<(Cid, String)>,
    report: Vec<(String, &'static str, String)>,
}

impl Build<'_> {
    fn node(&mut self, node: &Node) -> Result<(), String> {
        let prior = self.prior_of(node)?;
        let prior_body = prior
            .as_ref()
            .and_then(|p| self.graph.object(p))
            .map(|o| o.body().clone());
        let json = self.substitute(node, prior_body.as_ref())?;
        let created = node
            .created
            .clone()
            .unwrap_or_else(|| crate::compose::DEFAULT_CREATED.to_owned());
        let body = match node.kind {
            Kind::Claim => claim_body(self.store, &json, &node.base)
                .map_err(|e| format!("`{}`: {e}", node.id))?,
            Kind::Doc => {
                crate::document::body_from_json(&serde_json::Value::Object(json))
                    .map_err(|e| format!("`{}`: {e}", node.id))?
                    .0
            }
        };

        if let Some(prior) = &prior {
            let held = self.graph.object(prior).ok_or_else(|| {
                format!(
                    "`{}` was last built as {prior}, which this workspace does not \
                     hold; a new version needs the old one to supersede",
                    node.id
                )
            })?;
            let expected = match node.kind {
                Kind::Doc => "doc",
                Kind::Claim => "claim.prose",
            };
            if held.kind() != expected {
                return Err(format!(
                    "`{}` would supersede {prior}, a `{}`, with a `{expected}`",
                    node.id,
                    held.kind()
                ));
            }
            if held.body() == &body {
                let genesis = self.genesis_of(&node.id, prior);
                self.resolved.insert(
                    node.id.clone(),
                    Resolved {
                        cid: prior.clone(),
                        genesis,
                    },
                );
                self.report
                    .push((node.id.clone(), "unchanged", prior.to_string()));
                return Ok(());
            }
        }

        let (cid, bytes) = match node.kind {
            Kind::Doc => crate::document::build_document(self.author, &created, body)?,
            Kind::Claim => {
                let (cid, bytes, claim) = crate::compose::build_claim(self.author, &created, body)
                    .map_err(|e| format!("`{}`: {e}", node.id))?;
                crate::compose::warn_about(&claim);
                (cid, bytes)
            }
        };
        let already_held = self.graph.object(&cid).is_some();
        if !already_held {
            self.admit(&cid, bytes, &node.id)?;
        }
        self.owned.push((cid.clone(), node.id.clone()));

        let (status, genesis) = match &prior {
            Some(prior) => {
                let relation = crate::revise::supersedes(self.author, &created, &cid, prior)?;
                let relation_cid = Cid::of(&relation, HashAlg::Sha2_256);
                if self.graph.object(&relation_cid).is_none() {
                    self.admit(&relation_cid, relation, &node.id)?;
                }
                self.owned.push((relation_cid.clone(), node.id.clone()));
                // Section 6: from another key, unless it delegated its
                // lineage to this one, a `supersedes` only proposes.
                let status = if self.graph.supersedes_is_felicitous(&relation_cid) {
                    "revised"
                } else {
                    "proposed"
                };
                (status, self.genesis_of(&node.id, prior))
            }
            None if already_held => ("unchanged", cid.clone()),
            None => ("new", cid.clone()),
        };
        self.report.push((node.id.clone(), status, cid.to_string()));
        self.resolved
            .insert(node.id.clone(), Resolved { cid, genesis });
        Ok(())
    }

    /// The version a node revises: the one the lock records as built --
    /// in a corpus, the one last published -- or the one its source names.
    fn prior_of(&self, node: &Node) -> Result<Option<Cid>, String> {
        match (self.lock.entries.get(&node.id), &node.supersedes) {
            (Some(entry), _) => entry
                .cid
                .parse::<Cid>()
                .map(Some)
                .map_err(|_| format!("{LOCK}: `{}` names no identifier", node.id)),
            (None, Some(cid)) => Ok(Some(cid.clone())),
            (None, None) => Ok(None),
        }
    }

    /// The genesis of the lineage `prior` belongs to: as the lock recorded
    /// it, or by following `supersedes` back from `prior`.
    fn genesis_of(&self, id: &str, prior: &Cid) -> Cid {
        self.lock
            .entries
            .get(id)
            .and_then(|e| e.genesis.parse::<Cid>().ok())
            .unwrap_or_else(|| genesis(self.graph, prior))
    }

    /// Sign, with this workspace's key, every object the tree resolved to
    /// and everything this build adds, that this key has not already
    /// signed. `pub render` runs a pipeline only once its objects carry an
    /// `authored` signature from a trusted key, and a signature that has
    /// to be made by hand per object is one that gets skipped.
    fn sign_all(&mut self, here: &Path) -> Result<(), String> {
        let signed = signed_by(self.graph, self.author);
        let mut targets: BTreeSet<String> =
            self.resolved.values().map(|r| r.cid.to_string()).collect();
        targets.extend(self.pending.iter().map(|(c, _)| c.to_string()));
        let mut count = 0usize;
        for target in targets.difference(&signed) {
            let Ok(target) = target.parse::<Cid>() else {
                continue;
            };
            let Some(object) = self.graph.object(&target) else {
                continue;
            };
            if object.kind() == "sig" {
                continue;
            }
            let bytes = object.bytes().to_vec();
            let (cid, sig) = crate::sign::signature(
                here,
                self.author,
                crate::compose::DEFAULT_CREATED,
                PURPOSE,
                &target,
                &bytes,
            )?;
            self.admit(&cid, sig, "signature")?;
            count += 1;
        }
        self.report.push((
            format!("{count} object(s)"),
            "signed",
            self.author.to_owned(),
        ));
        Ok(())
    }

    /// Offer an object to the graph, keeping it to store once the whole
    /// tree is known to be admissible.
    fn admit(&mut self, cid: &Cid, bytes: Vec<u8>, id: &str) -> Result<(), String> {
        let verified = Object::parse(&bytes)
            .map_err(|e| e.to_string())?
            .verify(cid)
            .map_err(|e| e.to_string())?;
        self.graph
            .insert(cid, verified)
            .map_err(|e| format!("`{id}` was refused, so nothing was stored: {e}"))?;
        self.pending.push((cid.clone(), bytes));
        Ok(())
    }

    /// The node's JSON with every name replaced by the identifier it
    /// resolved to. A lineage-bound item citing a publet by name cites
    /// its genesis, `at` the version this build produced (Section 8).
    fn substitute(
        &self,
        node: &Node,
        prior: Option<&BTreeMap<String, Value>>,
    ) -> Result<serde_json::Map<String, serde_json::Value>, String> {
        let cid_of = |raw: &str, genesis: bool| -> Result<String, String> {
            match resolve_ref(raw, &node.owner)? {
                Ref::Cid(cid) => Ok(cid.to_string()),
                Ref::Node(id) => {
                    let r = self
                        .resolved
                        .get(&id)
                        .ok_or_else(|| format!("`{id}` is not built yet"))?;
                    Ok(if genesis { &r.genesis } else { &r.cid }.to_string())
                }
                Ref::Import(reference) => {
                    let importer = self.importer.ok_or_else(|| {
                        format!(
                            "`{reference}` names a parent corpus, but this tree is \
                             not being built as a corpus"
                        )
                    })?;
                    let cid = importer.resolve(&reference)?;
                    Ok(if genesis {
                        self::genesis(self.graph, &cid)
                    } else {
                        cid
                    }
                    .to_string())
                }
            }
        };
        let swap = |v: &mut serde_json::Value| -> Result<(), String> {
            if let Some(raw) = v.as_str() {
                *v = cid_of(raw, false)?.into();
            }
            Ok(())
        };

        let mut json = node.json.clone();
        match node.kind {
            Kind::Doc => {
                if let Some(v) = json.get_mut("abstract") {
                    swap(v)?;
                }
                for section in json
                    .get_mut("sections")
                    .and_then(serde_json::Value::as_array_mut)
                    .into_iter()
                    .flatten()
                {
                    for item in section
                        .get_mut("items")
                        .and_then(serde_json::Value::as_array_mut)
                        .into_iter()
                        .flatten()
                    {
                        let Some(item) = item.as_object_mut() else {
                            continue;
                        };
                        let lineage =
                            item.get("bind").and_then(serde_json::Value::as_str) == Some("lineage");
                        let raw = item
                            .get("ref")
                            .and_then(serde_json::Value::as_str)
                            .map(str::to_owned);
                        if let Some(raw) = raw {
                            let reference = cid_of(&raw, lineage)?;
                            if lineage && !item.contains_key("at") {
                                // `at` is the version the author read. It
                                // moves when they say so, not whenever the
                                // lineage it tracks does: a revised
                                // pipeline is no reason to revise every
                                // document it renders (Section 8).
                                let at = read_at(prior, &reference)
                                    .map_or_else(|| cid_of(&raw, false), Ok)?;
                                item.insert("at".into(), at.into());
                            }
                            item.insert("ref".into(), reference.into());
                        }
                        if let Some(v) = item.get_mut("at") {
                            swap(v)?;
                        }
                    }
                }
            }
            Kind::Claim => {
                if let Some(v) = json.get_mut("method") {
                    swap(v)?;
                }
                for dep in json
                    .get_mut("depends")
                    .and_then(serde_json::Value::as_array_mut)
                    .into_iter()
                    .flatten()
                {
                    // `~ref` presupposes the lineage, not one version of
                    // it: the genesis, which no revision changes.
                    let raw = dep.as_str().map(str::to_owned);
                    match raw.as_deref().and_then(|r| r.strip_prefix('~')) {
                        Some(lineage) => *dep = cid_of(lineage, true)?.into(),
                        None => swap(dep)?,
                    }
                }
            }
        }
        Ok(json)
    }

    /// Ensure the publet's declared tag names its lineage, and withdraw
    /// the tag this build filed last time if the declaration changed.
    fn tag(&mut self, source: &Source) -> Result<(), String> {
        let Some(resolved) = self.resolved.get(&source.slug) else {
            return Ok(());
        };
        let genesis = resolved.genesis.clone();
        let created = source
            .created
            .clone()
            .unwrap_or_else(|| crate::compose::DEFAULT_CREATED.to_owned());

        let current = self.graph.tags_of(&genesis);
        if current.contains(&source.tag) {
            return Ok(());
        }
        let annotation = tagged(self.author, &created, &genesis, &source.tag)?;
        let annotation_cid = Cid::of(&annotation, HashAlg::Sha2_256);
        self.admit(&annotation_cid, annotation, &source.slug)?;
        self.owned
            .push((annotation_cid.clone(), source.slug.clone()));
        self.report.push((
            format!("{} [{}]", source.slug, source.tag),
            "tagged",
            genesis.to_string(),
        ));

        let previous = self
            .lock
            .entries
            .get(&source.slug)
            .and_then(|e| e.tag.clone());
        let Some(previous) = previous.filter(|p| *p != source.tag) else {
            return Ok(());
        };
        for old in self.own_tags(&genesis, &previous) {
            let relation = Object::builder("claim.relation", self.author)
                .created(&created)
                .field("kind", Value::Text("retracts".to_owned()))
                .field("from", Value::Text(annotation_cid.to_string()))
                .field("to", Value::Text(old.to_string()))
                .field("note", Value::Text(format!("replaced with {}", source.tag)))
                .field("scope", crate::compose::scope_value("unconditional"))
                .build()
                .map_err(|e| e.to_string())?;
            let relation_cid = Cid::of(&relation, HashAlg::Sha2_256);
            self.admit(&relation_cid, relation, &source.slug)?;
            self.owned.push((relation_cid, source.slug.clone()));
            self.report.push((
                format!("{} [{previous}]", source.slug),
                "retracted",
                old.to_string(),
            ));
        }
        Ok(())
    }

    /// This key's live `tagged` annotations giving `target` the tag `tag`.
    /// Only this key's: retraction is felicitous only from the key that
    /// signed what it retracts (Section 6), and anyone else's tag is
    /// theirs to keep.
    fn own_tags(&self, target: &Cid, tag: &str) -> Vec<Cid> {
        self.graph
            .cids()
            .into_iter()
            .filter_map(|c| c.parse::<Cid>().ok())
            .filter(|c| {
                self.graph.object(c).is_some_and(|o| {
                    o.kind() == "claim.annotation"
                        && o.author().to_string() == self.author
                        && o.body().get("kind").and_then(Value::as_text) == Some("tagged")
                        && o.body().get("target").and_then(Value::as_text)
                            == Some(target.to_string().as_str())
                        && o.body()
                            .get("value")
                            .and_then(|v| v.get("tag"))
                            .and_then(Value::as_text)
                            == Some(tag)
                })
            })
            .filter(|c| !self.graph.is_retracted(c))
            .collect()
    }
}

/// The `at` a prior version of a document recorded for the lineage-bound
/// item citing `genesis`, if it had one.
fn read_at(prior: Option<&BTreeMap<String, Value>>, genesis: &str) -> Option<String> {
    let Some(Value::Array(sections)) = prior?.get("sections") else {
        return None;
    };
    sections
        .iter()
        .filter_map(|s| match s.get("items") {
            Some(Value::Array(items)) => Some(items),
            _ => None,
        })
        .flatten()
        .find(|i| {
            i.get("bind").and_then(Value::as_text) == Some("lineage")
                && i.get("ref").and_then(Value::as_text) == Some(genesis)
        })
        .and_then(|i| i.get("at").and_then(Value::as_text))
        .map(str::to_owned)
}

/// The purpose a publet's objects are signed under (Section 4.4).
pub(crate) const PURPOSE: &str = "authored";

/// The genesis of the lineage `cid` belongs to, following `supersedes`
/// edges back from it. Where a lineage merged, the lowest identifier is
/// followed, so the answer does not depend on load order.
pub(crate) fn genesis(graph: &Graph, cid: &Cid) -> Cid {
    let mut at = cid.clone();
    let mut seen = BTreeSet::new();
    while seen.insert(at.to_string()) {
        let mut older = graph.out(RelationKind::Supersedes, &at);
        older.sort();
        match older.first().and_then(|c| c.parse::<Cid>().ok()) {
            Some(next) => at = next,
            None => break,
        }
    }
    at
}

/// Everything `author` has an `authored` signature on, by identifier. The
/// signatures are not verified here: these are this key's own, and
/// `pub render` verifies every one it relies on.
pub(crate) fn signed_by(graph: &Graph, author: &str) -> BTreeSet<String> {
    graph
        .cids()
        .into_iter()
        .filter_map(|c| c.parse::<Cid>().ok())
        .filter_map(|c| graph.object(&c))
        .filter(|o| o.kind() == "sig" && o.author().to_string() == author)
        .filter(|o| o.body().get("purpose").and_then(Value::as_text) == Some(PURPOSE))
        .filter_map(|o| {
            o.body()
                .get("target")
                .and_then(Value::as_text)
                .map(str::to_owned)
        })
        .collect()
}

/// A Section 9.1 `tagged` annotation.
fn tagged(author: &str, created: &str, target: &Cid, tag: &str) -> Result<Vec<u8>, String> {
    let mut value = BTreeMap::new();
    value.insert("tag".to_owned(), Value::Text(tag.to_owned()));
    Object::builder("claim.annotation", author)
        .created(created)
        .field("kind", Value::Text("tagged".to_owned()))
        .field("target", Value::Text(target.to_string()))
        .field("scope", crate::compose::scope_value("unconditional"))
        .field("value", Value::Map(value))
        .build()
        .map_err(|e| e.to_string())
}

/// A claim body from a source's claim, every name already resolved.
///
/// The same fields `pub compose` takes, held to the same rules: scope is
/// required (R4), and an empirical claim names its method (Section 5.5).
fn claim_body(
    store: &Store,
    json: &serde_json::Map<String, serde_json::Value>,
    base: &Path,
) -> Result<BTreeMap<String, Value>, String> {
    let text = |key: &str| json.get(key).and_then(serde_json::Value::as_str);
    let class = text("class").ok_or(
        "`class` is required: formal, empirical, attributive, definitional, \
         normative, expressive, archival, or procedural",
    )?;
    let parsed = Class::from_id(class).ok_or(format!("unknown claim class: {class}"))?;
    let content = text("content").ok_or("`content` is required")?;
    let scope = text("scope").ok_or(
        "`scope` is required. State the conditions you assert this under, or \
         \"unconditional\" if you really mean that (Section 5.3)",
    )?;
    let method = text("method");
    if parsed == Class::Empirical && method.is_none() {
        return Err("`method` is required for an empirical claim (Section 5.5)".to_owned());
    }

    let depends: Vec<String> = match json.get("depends") {
        None => Vec::new(),
        Some(serde_json::Value::Array(items)) => items
            .iter()
            .map(|v| v.as_str().map(str::to_owned).ok_or("`depends` holds names"))
            .collect::<Result<_, _>>()?,
        Some(_) => return Err("`depends` must be a list".to_owned()),
    };

    let Value::Array(mut evidence) = crate::compose::evidence_for(method, &[], &[]) else {
        return Err("evidence is not a list".to_owned());
    };
    for cite in json
        .get("cites")
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
    {
        let (reference, note) = match cite {
            serde_json::Value::String(s) => (s.clone(), None),
            serde_json::Value::Object(c) => (
                c.get("ref")
                    .and_then(serde_json::Value::as_str)
                    .ok_or("each cite needs a `ref`")?
                    .to_owned(),
                c.get("note")
                    .and_then(serde_json::Value::as_str)
                    .map(str::to_owned),
            ),
            _ => return Err("each cite is a string or {ref, note}".to_owned()),
        };
        let notes: Vec<String> = note.into_iter().collect();
        if let Value::Array(entries) = crate::compose::evidence_for(None, &[reference], &notes) {
            evidence.extend(entries);
        }
    }

    let mut sources = Vec::new();
    for source in json
        .get("sources")
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
    {
        let field = |k: &str| {
            source
                .get(k)
                .and_then(serde_json::Value::as_str)
                .map(str::to_owned)
        };
        let reference = match source {
            serde_json::Value::String(s) => s.clone(),
            _ => field("ref")
                .ok_or("each source is a string or {ref, revision, locator, query, note}")?,
        };
        sources.push(crate::compose::SourceArg::new(
            reference,
            field("revision"),
            field("locator"),
            field("query"),
            field("note"),
        ));
    }
    let (source_entries, source_claims) = crate::compose::source_entries(store, &sources)?;
    evidence.extend(source_entries);
    let mut depends = depends;
    crate::compose::add_missing(&mut depends, &source_claims);

    let mut body = BTreeMap::new();
    body.insert("class".to_owned(), Value::Text(class.to_owned()));
    body.insert(
        "lang".to_owned(),
        Value::Text(text("lang").unwrap_or("en").to_owned()),
    );
    body.insert("content".to_owned(), Value::Text(content.to_owned()));
    body.insert("scope".to_owned(), crate::compose::scope_value(scope));
    body.insert("depends".to_owned(), crate::compose::text_array(&depends));
    body.insert("evidence".to_owned(), Value::Array(evidence));
    if let Some(data) = json.get("data") {
        body.insert(
            "data".to_owned(),
            crate::payload::data_from_json(store, data, Some(base))?,
        );
    }
    Ok(body)
}

/// Everything this workspace holds, as one graph.
pub(crate) fn graph_of(store: &Store) -> Result<Graph, String> {
    let mut objects = Vec::new();
    for cid_text in store.cids().map_err(|e| e.to_string())? {
        let Ok(held) = cid_text.parse::<Cid>() else {
            continue;
        };
        if let Ok(Some(bytes)) = store.get(&held) {
            objects.push((held, bytes));
        }
    }
    publet_graph::load::from_objects(objects).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn node(id: &str, deps: &[&str]) -> Node {
        Node {
            id: id.to_owned(),
            kind: Kind::Doc,
            json: serde_json::Map::new(),
            base: PathBuf::new(),
            owner: id.to_owned(),
            created: None,
            supersedes: None,
            tag: None,
            deps: deps.iter().map(|d| (*d).to_owned()).collect(),
        }
    }

    fn graph(nodes: Vec<Node>) -> BTreeMap<String, Node> {
        nodes.into_iter().map(|n| (n.id.clone(), n)).collect()
    }

    #[test]
    fn children_come_before_the_publets_citing_them() {
        let nodes = graph(vec![
            node("semantic-drift", &["render.pdf"]),
            node("render.pdf", &["tool.typst"]),
            node("tool.typst", &[]),
        ]);
        let order = topological(&nodes).unwrap();
        let at = |id: &str| order.iter().position(|o| o == id).unwrap();
        assert!(at("tool.typst") < at("render.pdf"));
        assert!(at("render.pdf") < at("semantic-drift"));
    }

    #[test]
    fn a_cycle_is_refused_and_named() {
        let nodes = graph(vec![node("a", &["b"]), node("b", &["a"])]);
        let err = topological(&nodes).unwrap_err();
        assert!(err.contains("a -> b -> a"), "{err}");
    }

    #[test]
    fn references_resolve_by_shape() {
        assert!(matches!(
            resolve_ref("#usage", "tool.typst").unwrap(),
            Ref::Node(id) if id == "tool.typst#usage"
        ));
        assert!(matches!(
            resolve_ref("tool.mystmd#identity", "x").unwrap(),
            Ref::Node(id) if id == "tool.mystmd#identity"
        ));
        assert!(matches!(
            resolve_ref("semantic-drift", "x").unwrap(),
            Ref::Node(id) if id == "semantic-drift"
        ));
        let cid = Cid::of(b"x", HashAlg::Sha2_256).to_string();
        assert!(matches!(resolve_ref(&cid, "x").unwrap(), Ref::Cid(_)));
        assert!(resolve_ref("pub:not-a-cid", "x").is_err());
        assert!(resolve_ref("Semantic Drift", "x").is_err());
    }

    #[test]
    fn a_parent_publet_is_named_by_its_alias() {
        assert!(matches!(
            resolve_ref("base:tool.typst#identity", "x").unwrap(),
            Ref::Import(r) if r == "base:tool.typst#identity"
        ));
        assert!(matches!(
            resolve_ref("base:tool.typst", "x").unwrap(),
            Ref::Import(_)
        ));
        assert!(resolve_ref("Base:tool.typst", "x").is_err());
        assert!(resolve_ref("base:Tool", "x").is_err());
    }

    #[test]
    fn slugs_are_lowercase_dotted_segments() {
        assert!(check_slug("tool.typst").is_ok());
        assert!(check_slug("xdoc.typst-reference").is_ok());
        assert!(check_slug("Tool.typst").is_err());
        assert!(check_slug("tool..typst").is_err());
        assert!(check_slug("").is_err());
    }

    #[test]
    fn the_lock_renders_identically_whatever_order_it_was_filled_in() {
        let entry = |slug: &str| LockEntry {
            slug: slug.to_owned(),
            kind: "doc".to_owned(),
            cid: "pub:sha2-256:a".to_owned(),
            genesis: "pub:sha2-256:a".to_owned(),
            tag: None,
            source: None,
            deps: Vec::new(),
        };
        let mut one = Lock {
            entries: BTreeMap::new(),
            corpus: None,
        };
        let mut two = Lock {
            entries: BTreeMap::new(),
            corpus: None,
        };
        for slug in ["b", "a"] {
            one.entries.insert(slug.to_owned(), entry(slug));
        }
        for slug in ["a", "b"] {
            two.entries.insert(slug.to_owned(), entry(slug));
        }
        assert_eq!(one.render().unwrap(), two.render().unwrap());
    }
}
