//! Corpora: a workspace whose objects form a Section 14.1 domain, built
//! from named sources, and pinned to the more general corpora it builds on.
//!
//! A corpus is a directory holding `corpus.json` (what it is called, its
//! byte bound, and its parents), `publets/` (the sources `pub build`
//! reads), `objects/` (its members, one file per object -- the durable,
//! committed form), `blobs/`, `corpus.lock`, and a `.publet/` workspace
//! with its own key. Every build that changes the membership advances the
//! corpus's **generation** (Section 14.1.1), so a generation is the
//! corpus's version.
//!
//! Discovery runs from child to parent only. A child names a parent's
//! publet as `alias:slug` -- never by a bare slug, which could otherwise be
//! shadowed by whatever a parent later declares (Section 9.1) -- and is
//! **pinned** to the parent generation it built against: only members of
//! that generation are visible to it, so a parent moving on changes
//! nothing until the child upgrades. `pub corpus status` reports when a
//! parent has moved on, and `pub corpus upgrade` re-pins.
//!
//! Everything a child's objects reach in a parent is copied into the
//! child at identical identifiers, as Section 14.1 expects of a domain
//! ("shared definitions are duplicated across domains at identical CIDs"),
//! so a child reads, builds, and renders with its parents absent.

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::path::{Path, PathBuf};

use publet_core::{Cid, HashAlg, Object, cbor::Value};
use publet_graph::Graph;
use publet_store::Store;

use crate::build::{Built, Lock, Tree};
use crate::workspace::Workspace;

/// The corpus description, beside its sources.
pub(crate) const CONFIG: &str = "corpus.json";
/// The corpus lock: what was built, and what it is pinned to.
pub(crate) const LOCK: &str = "corpus.lock";
const DEFAULT_BOUND: u64 = 64 * 1024 * 1024;

/// `pub corpus SUBCOMMAND`.
///
/// # Errors
///
/// As each subcommand.
pub(crate) fn run(args: &[String]) -> Result<(), String> {
    let rest: Vec<String> = args.iter().skip(1).cloned().collect();
    match args.first().map(String::as_str) {
        Some("init") => init(&rest),
        Some("status") => status(&rest),
        Some("upgrade") => upgrade(&rest),
        Some("map") => map(&rest),
        Some("renderings") => renderings(&rest),
        _ => Err("usage: pub corpus <init|status|upgrade|map|renderings> [options]".to_owned()),
    }
}

// ----------------------------------------------------------------- config

struct ParentConfig {
    alias: String,
    path: PathBuf,
}

struct Config {
    root: PathBuf,
    name: String,
    label: String,
    bound: u64,
    parents: Vec<ParentConfig>,
}

impl Config {
    fn read(root: &Path) -> Result<Self, String> {
        let path = root.join(CONFIG);
        let json = crate::payload::read_json(&path)?;
        let text = |k: &str| json.get(k).and_then(serde_json::Value::as_str);
        let name = text("name")
            .ok_or_else(|| format!("{}: `name` is required", path.display()))?
            .to_owned();
        let mut parents = Vec::new();
        for p in json
            .get("parents")
            .and_then(serde_json::Value::as_array)
            .into_iter()
            .flatten()
        {
            let field = |k: &str| p.get(k).and_then(serde_json::Value::as_str);
            let alias = field("alias")
                .ok_or_else(|| format!("{}: each parent needs an `alias`", path.display()))?;
            let rel = field("path").ok_or_else(|| {
                format!(
                    "{}: parent `{alias}` needs a `path`; other locations are not \
                     supported yet",
                    path.display()
                )
            })?;
            parents.push(ParentConfig {
                alias: alias.to_owned(),
                path: root.join(rel),
            });
        }
        Ok(Self {
            root: root.to_path_buf(),
            label: text("label").unwrap_or(&name).to_owned(),
            name,
            bound: json
                .get("bound")
                .and_then(serde_json::Value::as_u64)
                .unwrap_or(DEFAULT_BOUND),
            parents,
        })
    }

    fn objects(&self) -> PathBuf {
        self.root.join("objects")
    }
}

// ------------------------------------------------------------------- meta

#[derive(Clone)]
struct Pin {
    alias: String,
    index: u64,
    generation: String,
    key: Option<String>,
}

/// The corpus section of `corpus.lock`.
#[derive(Default)]
struct Meta {
    key: Option<String>,
    domain: Option<String>,
    generation: Option<(u64, String)>,
    parents: Vec<Pin>,
    imports: BTreeMap<String, String>,
}

impl Meta {
    fn from_lock(lock: &Lock) -> Self {
        let Some(v) = &lock.corpus else {
            return Self::default();
        };
        let text = |v: &serde_json::Value, k: &str| {
            v.get(k)
                .and_then(serde_json::Value::as_str)
                .map(str::to_owned)
        };
        Self {
            key: text(v, "key"),
            domain: text(v, "domain"),
            generation: v.get("generation").and_then(|g| {
                Some((
                    g.get("index")?.as_u64()?,
                    g.get("cid")?.as_str()?.to_owned(),
                ))
            }),
            parents: v
                .get("parents")
                .and_then(serde_json::Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(|p| {
                    Some(Pin {
                        alias: text(p, "alias")?,
                        index: p.get("index")?.as_u64()?,
                        generation: text(p, "generation")?,
                        key: text(p, "key"),
                    })
                })
                .collect(),
            imports: v
                .get("imports")
                .and_then(serde_json::Value::as_object)
                .map(|m| {
                    m.iter()
                        .filter_map(|(k, v)| Some((k.clone(), v.as_str()?.to_owned())))
                        .collect()
                })
                .unwrap_or_default(),
        }
    }

    fn to_value(&self) -> serde_json::Value {
        let mut m = serde_json::Map::new();
        if let Some(key) = &self.key {
            m.insert("key".into(), key.clone().into());
        }
        if let Some(domain) = &self.domain {
            m.insert("domain".into(), domain.clone().into());
        }
        if let Some((index, cid)) = &self.generation {
            m.insert(
                "generation".into(),
                serde_json::json!({ "index": index, "cid": cid }),
            );
        }
        m.insert(
            "parents".into(),
            self.parents
                .iter()
                .map(|p| {
                    serde_json::json!({
                        "alias": p.alias, "index": p.index,
                        "generation": p.generation, "key": p.key,
                    })
                })
                .collect::<Vec<_>>()
                .into(),
        );
        m.insert(
            "imports".into(),
            serde_json::Value::Object(
                self.imports
                    .iter()
                    .map(|(k, v)| (k.clone(), v.clone().into()))
                    .collect(),
            ),
        );
        serde_json::Value::Object(m)
    }
}

// ---------------------------------------------------------------- parents

/// A parent corpus as one of its children sees it: pinned to a
/// generation, with only that generation's members visible.
pub(crate) struct ParentView {
    alias: String,
    root: PathBuf,
    pin: Pin,
    latest: (u64, String),
    members: BTreeSet<String>,
    /// The parent's current slugs, from its lock.
    slugs: BTreeMap<String, String>,
}

impl ParentView {
    fn load(cfg: &ParentConfig, pin: Option<&Pin>) -> Result<Self, String> {
        let lock = Lock::read(&cfg.path.join(LOCK))?;
        let meta = Meta::from_lock(&lock);
        let latest = meta.generation.clone().ok_or_else(|| {
            format!(
                "parent `{}` at {} has no generation yet; run `pub build` there first",
                cfg.alias,
                cfg.path.display()
            )
        })?;
        let pin = pin.cloned().unwrap_or_else(|| Pin {
            alias: cfg.alias.clone(),
            index: latest.0,
            generation: latest.1.clone(),
            key: meta.key.clone(),
        });
        let members = membership(&cfg.path.join("objects"), &pin.generation)
            .map_err(|e| format!("parent `{}`: {e}", cfg.alias))?;
        Ok(Self {
            alias: cfg.alias.clone(),
            root: cfg.path.clone(),
            pin,
            latest,
            members,
            slugs: lock
                .entries
                .values()
                .map(|e| (e.slug.clone(), e.cid.clone()))
                .collect(),
        })
    }

    /// The pinned generation's members, read from the parent's `objects/`.
    fn objects(&self) -> Result<Vec<(Cid, Vec<u8>)>, String> {
        read_members(&self.root.join("objects"), &self.members)
    }
}

/// Resolves `alias:slug` against the pinned parents, recording each
/// resolution so the next build reuses it rather than re-resolving.
pub(crate) struct Importer {
    parents: Vec<ParentView>,
    imports: RefCell<BTreeMap<String, String>>,
}

impl Importer {
    /// The identifier `alias:slug` names.
    ///
    /// # Errors
    ///
    /// Returns a message if the alias is not a parent, the parent declares
    /// no such slug, or what it declares is newer than the pinned
    /// generation.
    pub(crate) fn resolve(&self, reference: &str) -> Result<Cid, String> {
        if let Some(cid) = self.imports.borrow().get(reference) {
            return cid
                .parse()
                .map_err(|_| format!("{LOCK}: `{reference}` names no identifier"));
        }
        let (alias, slug) = reference
            .split_once(':')
            .ok_or_else(|| format!("`{reference}` is not `alias:slug`"))?;
        let parent = self
            .parents
            .iter()
            .find(|p| p.alias == alias)
            .ok_or_else(|| {
                let known: Vec<&str> = self.parents.iter().map(|p| p.alias.as_str()).collect();
                format!(
                    "`{reference}`: `{alias}` is not a parent of this corpus (parents: {})",
                    if known.is_empty() {
                        "none".to_owned()
                    } else {
                        known.join(", ")
                    }
                )
            })?;
        let cid = parent
            .slugs
            .get(slug)
            .ok_or_else(|| format!("`{reference}`: `{alias}` declares no publet `{slug}`"))?;
        if !parent.members.contains(cid) {
            return Err(format!(
                "`{reference}` is {cid} in `{alias}`'s current lock, which is newer than \
                 the generation {} this corpus is pinned to; run `pub corpus upgrade {alias}`",
                parent.pin.index
            ));
        }
        self.imports
            .borrow_mut()
            .insert(reference.to_owned(), cid.clone());
        cid.parse()
            .map_err(|_| format!("`{alias}`'s lock names no identifier for `{slug}`"))
    }

    /// The alias of a parent declaring `slug`, for a hint when a bare name
    /// is used for it.
    pub(crate) fn alias_declaring(&self, slug: &str) -> Option<&str> {
        self.parents
            .iter()
            .find(|p| p.slugs.contains_key(slug))
            .map(|p| p.alias.as_str())
    }
}

// ------------------------------------------------------------- generations

/// A generation's members: the union of `added` along its chain back to
/// genesis, less what was removed (Section 14.1.1).
fn membership(objects: &Path, generation: &str) -> Result<BTreeSet<String>, String> {
    let mut chain = Vec::new();
    let mut at = Some(generation.to_owned());
    let mut seen = BTreeSet::new();
    while let Some(cid) = at {
        if !seen.insert(cid.clone()) {
            return Err(format!("generation {cid} is its own ancestor"));
        }
        let record = read_generation(objects, &cid)?;
        at = record.parent.as_ref().map(ToString::to_string);
        chain.push(record);
    }
    let mut members = BTreeSet::new();
    for record in chain.iter().rev() {
        members.extend(record.added.iter().map(ToString::to_string));
        for removal in &record.removed {
            members.remove(&removal.cid.to_string());
        }
    }
    Ok(members)
}

fn read_generation(objects: &Path, cid: &str) -> Result<publet_domain::Generation, String> {
    let parsed: Cid = cid
        .parse()
        .map_err(|_| format!("`{cid}` is not an identifier"))?;
    let bytes = std::fs::read(objects.join(file_name(&parsed)))
        .map_err(|_| format!("generation {cid} is not in {}", objects.display()))?;
    let object = Object::parse(&bytes)
        .map_err(|e| e.to_string())?
        .verify(&parsed)
        .map_err(|e| e.to_string())?
        .into_inner();
    publet_domain::Generation::from_object(&object).map_err(|e| e.to_string())
}

fn file_name(cid: &Cid) -> String {
    format!("{}.cbor", cid.to_string().replace(':', "_"))
}

/// Read and verify the named members of an `objects/` directory.
fn read_members(objects: &Path, members: &BTreeSet<String>) -> Result<Vec<(Cid, Vec<u8>)>, String> {
    let mut out = Vec::new();
    for member in members {
        let cid: Cid = member
            .parse()
            .map_err(|_| format!("member `{member}` is not an identifier"))?;
        let bytes = std::fs::read(objects.join(file_name(&cid)))
            .map_err(|_| format!("member {cid} is missing from {}", objects.display()))?;
        if !cid.verifies(&bytes) {
            return Err(format!("{} no longer hashes to {cid}", objects.display()));
        }
        out.push((cid, bytes));
    }
    Ok(out)
}

/// Every object file in `objects/`, keyed by identifier, and which of them
/// are members: everything but the domain's own manifest and generation
/// records, which describe the membership rather than belong to it.
fn scan_objects(objects: &Path) -> Result<(BTreeSet<String>, u64), String> {
    let mut members = BTreeSet::new();
    let mut size = 0;
    let Ok(entries) = std::fs::read_dir(objects) else {
        return Ok((members, 0));
    };
    for entry in entries {
        let path = entry.map_err(|e| e.to_string())?.path();
        if path.extension().is_none_or(|x| x != "cbor") {
            continue;
        }
        let bytes = std::fs::read(&path).map_err(|e| e.to_string())?;
        let cid = Cid::of(&bytes, HashAlg::Sha2_256);
        let object = Object::parse(&bytes)
            .map_err(|e| format!("{}: {e}", path.display()))?
            .verify(&cid)
            .map_err(|e| format!("{}: {e}", path.display()))?;
        if matches!(object.object().kind(), "domain" | "generation") {
            continue;
        }
        size += bytes.len() as u64;
        members.insert(cid.to_string());
    }
    Ok((members, size))
}

/// Advance the corpus's generation if its membership changed. Returns the
/// objects written, for the caller to report.
fn advance(cfg: &Config, author: &str, meta: &mut Meta) -> Result<Option<(u64, Cid)>, String> {
    let objects = cfg.objects();
    let (members, size) = scan_objects(&objects)?;
    if size > cfg.bound {
        return Err(format!(
            "the corpus is {size} bytes against its declared bound of {}; raise `bound` \
             in {CONFIG} or split it (Section 14.1)",
            cfg.bound
        ));
    }
    let graph = publet_graph::load::from_objects(read_members(&objects, &members)?)
        .map_err(|e| e.to_string())?;
    let listed: Vec<String> = members.iter().cloned().collect();
    publet_domain::check_depends_closure(&graph, &listed)
        .map_err(|e| format!("the corpus is not closed under `depends` (Section 14.1): {e}"))?;

    let created = now();
    let written = if let (Some(domain), Some((index, previous))) = (&meta.domain, &meta.generation)
    {
        {
            let before = membership(&objects, previous)?;
            if before == members {
                return Ok(None);
            }
            if let Some(lost) = before.difference(&members).next() {
                return Err(format!(
                    "{lost} was a member of generation {index} and is gone from {}; \
                     a corpus only grows",
                    objects.display()
                ));
            }
            let added: Vec<String> = members.difference(&before).cloned().collect();
            let domain: Cid = domain
                .parse()
                .map_err(|_| "bad domain in lock".to_owned())?;
            let parent: Cid = previous
                .parse()
                .map_err(|_| "bad generation in lock".to_owned())?;
            let (cid, bytes) = crate::domain::next_generation(
                author,
                &created,
                &domain,
                index + 1,
                &parent,
                &listed,
                &added,
            )?;
            // Hold the record to the rules a reader will: it follows its
            // parent, and declares exactly the change in membership.
            let record = Object::parse(&bytes)
                .map_err(|e| e.to_string())?
                .verify(&cid)
                .map_err(|e| e.to_string())?
                .into_inner();
            let next =
                publet_domain::Generation::from_object(&record).map_err(|e| e.to_string())?;
            next.follows(&read_generation(&objects, previous)?)
                .map_err(|e| e.to_string())?;
            next.check_against(
                &publet_algorithms::membership::Membership::new(before),
                &publet_algorithms::membership::Membership::new(listed.clone()),
            )
            .map_err(|e| e.to_string())?;
            std::fs::write(objects.join(file_name(&cid)), &bytes).map_err(|e| e.to_string())?;
            (index + 1, cid)
        }
    } else {
        {
            let ((domain, dbytes), (cid, gbytes)) = crate::domain::manifest_and_genesis(
                author, &cfg.label, cfg.bound, &created, &listed, size,
            )?;
            std::fs::create_dir_all(&objects).map_err(|e| e.to_string())?;
            std::fs::write(objects.join(file_name(&domain)), &dbytes).map_err(|e| e.to_string())?;
            std::fs::write(objects.join(file_name(&cid)), &gbytes).map_err(|e| e.to_string())?;
            meta.domain = Some(domain.to_string());
            (0, cid)
        }
    };
    meta.generation = Some((written.0, written.1.to_string()));
    Ok(Some(written))
}

// ------------------------------------------------------------------ build

/// Where drafts are tracked: in the workspace, never committed.
const DRAFTS: &str = "drafts.json";

/// What a corpus build or publish works from: the published state, the
/// drafts, the pinned parents, and one graph over all of it.
struct Session {
    here: PathBuf,
    cfg: Config,
    author: String,
    store: Store,
    /// `corpus.lock`: what is published.
    lock: Lock,
    /// `.publet/drafts.json`: what is built and not yet published.
    drafts: Lock,
    importer: Importer,
    graph: Graph,
}

impl Session {
    fn open() -> Result<Self, String> {
        let here = std::env::current_dir().map_err(|e| e.to_string())?;
        let ws = Workspace::open(&here)?;
        let cfg = Config::read(&here)?;
        let author = ws
            .get("author")
            .ok_or("no author configured; run `pub sign --generate-key`")?;
        let store = ws.store()?;
        let lock = Lock::read(&here.join(LOCK))?;
        let drafts = Lock::read(&here.join(crate::workspace::DIR).join(DRAFTS))?;
        let published = Meta::from_lock(&lock);
        let drafted = Meta::from_lock(&drafts);

        // A first build pins each parent; until that is published, the pin
        // lives with the drafts.
        let mut parents = Vec::new();
        for parent in &cfg.parents {
            let pin = published
                .parents
                .iter()
                .chain(&drafted.parents)
                .find(|p| p.alias == parent.alias);
            parents.push(ParentView::load(parent, pin)?);
        }
        // What is published wins: an upgrade re-resolves the lock's imports,
        // and a draft's older resolution must not undo it. Drafts only add
        // imports not yet published.
        let mut imports = drafted.imports.clone();
        imports.extend(published.imports.clone());
        let importer = Importer {
            parents,
            imports: RefCell::new(imports),
        };

        // The graph sees what this workspace holds and what the parents'
        // pinned generations hold -- nothing newer.
        let mut objects = held(&store)?;
        for parent in &importer.parents {
            objects.extend(parent.objects()?);
        }
        let graph = publet_graph::load::from_objects(objects).map_err(|e| e.to_string())?;
        Ok(Self {
            here,
            cfg,
            author,
            store,
            lock,
            drafts,
            importer,
            graph,
        })
    }

    /// Build every source against the **published** state: a draft revises
    /// what was published, never an earlier draft.
    fn build(&mut self, tag: bool) -> Result<Built, String> {
        let sources = self.here.join("publets");
        if !sources.is_dir() {
            return Ok(Built {
                pending: Vec::new(),
                owned: Vec::new(),
                report: Vec::new(),
                lock: Lock {
                    entries: BTreeMap::new(),
                    corpus: None,
                },
            });
        }
        crate::build::build_tree(
            &Tree {
                dir: &sources,
                lock_dir: &self.here,
                here: &self.here,
                author: &self.author,
                store: &self.store,
                lock: &self.lock,
                importer: Some(&self.importer),
                sign: false,
                tag,
            },
            &mut self.graph,
        )
    }

    /// Whether a built row differs from what is published.
    fn changed(&self, id: &str, cid: &str) -> bool {
        self.lock.entries.get(id).is_none_or(|e| e.cid != cid)
    }

    fn drafts_path(&self) -> PathBuf {
        self.here.join(crate::workspace::DIR).join(DRAFTS)
    }

    /// The objects of the current drafts.
    fn draft_objects(&self) -> BTreeSet<String> {
        self.drafts
            .corpus
            .as_ref()
            .and_then(|c| c.get("objects"))
            .and_then(serde_json::Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(serde_json::Value::as_str)
            .map(str::to_owned)
            .collect()
    }

    fn pins(&self) -> Vec<Pin> {
        self.importer
            .parents
            .iter()
            .map(|p| p.pin.clone())
            .collect()
    }
}

/// `pub build` inside a corpus: compile the sources into drafts.
///
/// Drafts are the workshop. They are unsigned, live only in this
/// workspace, are never exported, and leave `corpus.lock`, `objects/`, and
/// the generation untouched -- so building, rebuilding, and experimenting
/// leave no history. `pub publish` is what records anything.
///
/// # Errors
///
/// As `pub build`, and if a parent is not built or an import cannot be
/// resolved within its pin.
pub(crate) fn build(args: &[String]) -> Result<(), String> {
    let mut dry_run = false;
    for arg in args {
        match arg.as_str() {
            "--dry-run" => dry_run = true,
            "--sign" => {
                return Err("in a corpus, `pub build` makes unsigned drafts; \
                            `pub publish` signs what you publish"
                    .to_owned());
            }
            other => return Err(format!("unknown argument: {other}")),
        }
    }
    let mut session = Session::open()?;
    let built = session.build(false)?;

    let report = draft_report(&session, &built);
    for line in &report {
        println!("{line}");
    }
    let drafted: Vec<&String> = built
        .lock
        .entries
        .values()
        .filter(|e| session.changed(&e.slug, &e.cid))
        .map(|e| &e.slug)
        .collect();
    if dry_run {
        println!("dry run: {} draft(s); nothing stored", drafted.len());
        return Ok(());
    }

    for (cid, bytes) in &built.pending {
        session.store.put(cid, bytes).map_err(|e| e.to_string())?;
    }
    // An earlier draft this one replaces was never exported, so nothing
    // outside this workspace can know it: drop it rather than keep it.
    let current: BTreeSet<String> = built.owned.iter().map(|(c, _)| c.to_string()).collect();
    let exported = exported(&session.cfg);
    for stale in session.draft_objects().difference(&current) {
        if !exported.contains(stale)
            && let Ok(cid) = stale.parse::<Cid>()
        {
            session.store.remove(&cid).map_err(|e| e.to_string())?;
        }
    }
    write_drafts(&session, &built)?;
    if drafted.is_empty() {
        println!("no drafts: the sources match what is published");
    } else {
        println!(
            "{} draft(s); nothing published. `pub publish SLUG` or `pub publish --all` publishes",
            drafted.len()
        );
    }
    Ok(())
}

/// Each built publet as a draft or as already published.
fn draft_report(session: &Session, built: &Built) -> Vec<String> {
    built
        .report
        .iter()
        .filter(|(id, _, _)| !id.contains(" ["))
        .map(|(id, status, cid)| {
            let state = match *status {
                "unchanged" if !session.changed(id, cid) => "published",
                "new" => "draft-new",
                // Section 6: from a key the published version's author has
                // not delegated to, publishing this would only propose.
                "proposed" => "draft-proposal",
                _ => "draft",
            };
            format!("{state:<10} {id}  {cid}")
        })
        .collect()
}

fn write_drafts(session: &Session, built: &Built) -> Result<(), String> {
    let changed: BTreeSet<&str> = built
        .lock
        .entries
        .values()
        .filter(|e| session.changed(&e.slug, &e.cid))
        .map(|e| e.slug.as_str())
        .collect();
    let objects: Vec<String> = built
        .owned
        .iter()
        .map(|(c, _)| c.to_string())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    let meta = Meta {
        parents: session.pins(),
        imports: session.importer.imports.borrow().clone(),
        ..Meta::default()
    };
    let mut corpus = meta.to_value();
    if let Some(m) = corpus.as_object_mut() {
        m.insert("objects".into(), objects.into());
    }
    let drafts = Lock {
        entries: built
            .lock
            .entries
            .iter()
            .filter(|(id, _)| changed.contains(id.as_str()))
            .map(|(id, e)| (id.clone(), e.clone()))
            .collect(),
        corpus: Some(corpus),
    };
    drafts.write(&session.drafts_path())
}

/// `pub publish SLUG… | --all [--no-rendering]`: publish drafts.
///
/// The one deliberate act. The named publets -- with any changed publet
/// they depend on, so nothing published refers to an unpublished draft --
/// are signed, tagged, exported to `objects/`, and recorded in
/// `corpus.lock`, and the corpus's generation advances. If a current
/// rendering of a published document is beside its source, its lineage is
/// pinned too: what was rendered, with which pipeline and toolchain, into
/// which bytes -- and the accepted output is kept as a blob.
///
/// # Errors
///
/// As `pub build`; and if a named publet is not declared, or the graph
/// refuses what would be published.
pub(crate) fn publish(args: &[String]) -> Result<(), String> {
    let mut named = Vec::new();
    let mut all = false;
    let mut renderings = true;
    for arg in args {
        match arg.as_str() {
            "--all" => all = true,
            "--no-rendering" => renderings = false,
            a if a.starts_with("--") => return Err(format!("unknown argument: {a}")),
            a => named.push(a.to_owned()),
        }
    }
    if named.is_empty() && !all {
        return Err("name the publets to publish, or pass --all".to_owned());
    }
    let mut session = Session::open()?;
    let built = session.build(true)?;

    let selected = select(&session, &built, &named, all)?;
    let mut publishing: Vec<Cid> = built
        .owned
        .iter()
        .filter(|(_, owner)| selected.contains(owner.split(" [").next().unwrap_or(owner)))
        .map(|(c, _)| c.clone())
        .collect();
    publishing.sort_by_key(ToString::to_string);
    publishing.dedup();

    let mut published = session.lock.entries.clone();
    for id in &selected {
        if let Some(row) = built.lock.entries.get(id) {
            published.insert(id.clone(), row.clone());
        }
    }

    let (extra, notes) = if renderings {
        let docs: Vec<crate::build::LockEntry> = published
            .values()
            .filter(|e| e.kind == "doc")
            .filter(|e| all || selected.contains(&e.slug) || named.contains(&e.slug))
            .cloned()
            .collect();
        pin_renderings(&mut session, &docs, &named)?
    } else {
        (Vec::new(), Vec::new())
    };
    publishing.extend(extra.iter().map(|(c, _)| c.clone()));

    let signatures = sign_each(&mut session, &publishing)?;
    for (cid, bytes) in built.pending.iter().chain(&extra).chain(&signatures) {
        session.store.put(cid, bytes).map_err(|e| e.to_string())?;
    }
    publishing.extend(signatures.iter().map(|(c, _)| c.clone()));

    // Everything leaving this workspace, plus what it reaches in parents.
    let unpublished: BTreeSet<String> = session
        .draft_objects()
        .into_iter()
        .chain(built.owned.iter().map(|(c, _)| c.to_string()))
        .filter(|c| !publishing.iter().any(|p| p.to_string() == *c))
        .collect();
    let mut own: BTreeSet<String> = session
        .store
        .cids()
        .map_err(|e| e.to_string())?
        .into_iter()
        .filter(|c| !unpublished.contains(c))
        .collect();
    own.extend(publishing.iter().map(ToString::to_string));
    let copies = reach(&session.graph, &own, &session.author);
    for cid in &copies {
        if let Some(object) = session.graph.object(cid) {
            session
                .store
                .put(cid, object.bytes())
                .map_err(|e| e.to_string())?;
            copy_blobs(object, &session.importer, &session.store)?;
        }
    }
    let exported = export(&session.cfg, &session.store, &unpublished)?;

    let mut meta = Meta::from_lock(&session.lock);
    meta.key = Some(session.author.clone());
    meta.parents = session.pins();
    meta.imports = session.importer.imports.borrow().clone();
    let generation = advance(&session.cfg, &session.author, &mut meta)?;
    let lock = Lock {
        entries: published,
        corpus: Some(meta.to_value()),
    };
    lock.write(&session.here.join(LOCK))?;

    // What was published is no longer a draft.
    session.lock = lock;
    write_drafts(&session, &remaining_drafts(built, &unpublished))?;

    for line in selected.iter().map(|id| format!("published  {id}")) {
        println!("{line}");
    }
    for note in notes {
        println!("{note}");
    }
    summary(
        publishing.len(),
        copies.len(),
        exported,
        generation.as_ref(),
    );
    Ok(())
}

fn summary(published: usize, copied: usize, exported: usize, generation: Option<&(u64, Cid)>) {
    println!(
        "{published} object(s) published, {copied} copied from parents, \
         {exported} exported to objects/"
    );
    match generation {
        Some((index, cid)) => println!("generation {index}  {cid}"),
        None => println!("membership unchanged; generation stays as it was"),
    }
}

/// What remains a draft once `built` is published, less `unpublished`.
fn remaining_drafts(built: Built, unpublished: &BTreeSet<String>) -> Built {
    Built {
        pending: Vec::new(),
        owned: built
            .owned
            .into_iter()
            .filter(|(c, _)| unpublished.contains(&c.to_string()))
            .collect(),
        report: Vec::new(),
        lock: Lock {
            entries: built.lock.entries,
            corpus: None,
        },
    }
}

/// The node ids to publish: the named publets' documents and claims, or
/// every changed node with `all`, closed over changed dependencies.
fn select(
    session: &Session,
    built: &Built,
    named: &[String],
    all: bool,
) -> Result<BTreeSet<String>, String> {
    let rows = &built.lock.entries;
    let mut queue: VecDeque<String> = VecDeque::new();
    if all {
        queue.extend(rows.keys().cloned());
    }
    for name in named {
        if !rows.contains_key(name) {
            return Err(format!("no publet.json declares `{name}`"));
        }
        queue.extend(
            rows.keys()
                .filter(|id| *id == name || id.starts_with(&format!("{name}#")))
                .cloned(),
        );
    }
    let mut selected = BTreeSet::new();
    while let Some(id) = queue.pop_front() {
        let Some(row) = rows.get(&id) else {
            continue;
        };
        if !session.changed(&id, &row.cid) || !selected.insert(id.clone()) {
            continue;
        }
        queue.extend(row.deps.iter().cloned());
    }
    Ok(selected)
}

/// Sign each object this key has not already signed (Section 4.4).
fn sign_each(session: &mut Session, targets: &[Cid]) -> Result<Vec<(Cid, Vec<u8>)>, String> {
    let signed = crate::build::signed_by(&session.graph, &session.author);
    let mut out = Vec::new();
    for target in targets {
        if signed.contains(&target.to_string()) {
            continue;
        }
        let Some(object) = session.graph.object(target) else {
            continue;
        };
        if object.kind() == "sig" {
            continue;
        }
        let bytes = object.bytes().to_vec();
        let (cid, sig) = crate::sign::signature(
            &session.here,
            &session.author,
            crate::compose::DEFAULT_CREATED,
            crate::build::PURPOSE,
            target,
            &bytes,
        )?;
        let verified = Object::parse(&sig)
            .map_err(|e| e.to_string())?
            .verify(&cid)
            .map_err(|e| e.to_string())?;
        session
            .graph
            .insert(&cid, verified)
            .map_err(|e| e.to_string())?;
        out.push((cid, sig));
    }
    Ok(out)
}

/// Pin the renderings beside each of `docs`, with a line saying what
/// happened to each.
fn pin_renderings(
    session: &mut Session,
    docs: &[crate::build::LockEntry],
    named: &[String],
) -> Result<(Vec<crate::domain::Signed>, Vec<String>), String> {
    let mut objects = Vec::new();
    let mut notes = Vec::new();
    for row in docs {
        match pin_rendering(session, row)? {
            Pinned::Filed(filed) => {
                notes.push(format!("rendering  {}  pinned", row.slug));
                objects.extend(filed);
            }
            // A publet nobody rendered and nobody named, such as a
            // pipeline published as a dependency, needs no remark.
            Pinned::Unrendered if !named.contains(&row.slug) => {}
            Pinned::Unrendered => {
                notes.push(format!(
                    "rendering  {}  not rendered; nothing pinned",
                    row.slug
                ));
            }
            Pinned::Skipped(why) => notes.push(format!("rendering  {}  {why}", row.slug)),
        }
    }
    Ok((objects, notes))
}

enum Pinned {
    Filed(Vec<(Cid, Vec<u8>)>),
    Unrendered,
    Skipped(String),
}

/// Pin the rendering beside a published document's source, if it is a
/// current rendering of exactly that document and differs from the
/// rendering already pinned.
fn pin_rendering(session: &mut Session, row: &crate::build::LockEntry) -> Result<Pinned, String> {
    let Some(source) = &row.source else {
        return Ok(Pinned::Skipped("has no source to look beside".to_owned()));
    };
    let dir = session.here.join(source);
    let Ok(record) = crate::payload::read_json(&dir.join(crate::render::RECORD)) else {
        return Ok(Pinned::Unrendered);
    };
    let text = |k: &str| record.get(k).and_then(serde_json::Value::as_str);
    if text("doc") != Some(row.cid.as_str()) {
        return Ok(Pinned::Skipped(
            "the rendering beside it is of another version; re-render to pin one".to_owned(),
        ));
    }
    let out = PathBuf::from(text("out").unwrap_or_default());
    let mut outputs = Vec::new();
    for o in record
        .get("outputs")
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
    {
        let (Some(path), Some(cid)) = (
            o.get("path").and_then(serde_json::Value::as_str),
            o.get("cid").and_then(serde_json::Value::as_str),
        ) else {
            continue;
        };
        let bytes = std::fs::read(out.join(path)).unwrap_or_default();
        if Cid::of(&bytes, HashAlg::Sha2_256).to_string() != cid {
            return Ok(Pinned::Skipped(format!(
                "{path} changed since it was rendered; re-render to pin it"
            )));
        }
        outputs.push((path.to_owned(), cid.to_owned()));
    }
    let doc: Cid = row.cid.parse().map_err(|_| "bad doc cid".to_owned())?;
    if let Some(active) = crate::render::active_rendering(&session.graph, &doc)
        && active.outputs == outputs
    {
        return Ok(Pinned::Skipped(
            "identical to the rendering already pinned; nothing filed".to_owned(),
        ));
    }
    let method: Cid = text("method")
        .and_then(|m| m.parse().ok())
        .ok_or("the rendering record names no method")?;
    let final_path = record
        .get("final")
        .and_then(serde_json::Value::as_array)
        .and_then(|f| f.first())
        .and_then(serde_json::Value::as_str)
        .ok_or("the rendering record names no final output")?;
    let final_bytes = std::fs::read(out.join(final_path)).map_err(|e| e.to_string())?;
    let blob = Cid::of(&final_bytes, HashAlg::Sha2_256);
    session
        .store
        .put_blob(&blob, &final_bytes)
        .map_err(|e| e.to_string())?;

    let tools: Vec<(String, String)> = record
        .get("tools")
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|t| {
            Some((
                t.get("name")?.as_str()?.to_owned(),
                t.get("observed")?.as_str()?.to_owned(),
            ))
        })
        .collect();
    let objects = crate::render::rendering_objects(
        &session.author,
        &now(),
        &crate::render::Rendering {
            doc,
            method,
            outputs,
            tools,
            blob,
            size: final_bytes.len() as u64,
        },
    )?;
    for (cid, bytes) in &objects {
        let verified = Object::parse(bytes)
            .map_err(|e| e.to_string())?
            .verify(cid)
            .map_err(|e| e.to_string())?;
        session
            .graph
            .insert(cid, verified)
            .map_err(|e| format!("the rendering record was refused: {e}"))?;
    }
    Ok(Pinned::Filed(objects))
}

/// Every identifier already in `objects/`.
fn exported(cfg: &Config) -> BTreeSet<String> {
    std::fs::read_dir(cfg.objects())
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .filter_map(|e| e.file_name().to_str().map(str::to_owned))
        .filter_map(|n| n.strip_suffix(".cbor").map(|n| n.replace('_', ":")))
        .collect()
}

/// The graph a reader in `here` sees: the workspace's store and, inside a
/// corpus, its parents' pinned generations -- what `pub build` sees, so
/// that rendering a draft resolves the same objects building it did.
///
/// # Errors
///
/// Returns a message if the store or a parent cannot be read, or the
/// objects do not load.
pub(crate) fn visible_graph(here: &Path, store: &Store) -> Result<Graph, String> {
    let mut objects = held(store)?;
    if here.join(CONFIG).exists() {
        let cfg = Config::read(here)?;
        let published = Meta::from_lock(&Lock::read(&here.join(LOCK))?);
        let drafted = Meta::from_lock(&Lock::read(&here.join(crate::workspace::DIR).join(DRAFTS))?);
        for parent in &cfg.parents {
            let pin = published
                .parents
                .iter()
                .chain(&drafted.parents)
                .find(|p| p.alias == parent.alias);
            objects.extend(ParentView::load(parent, pin)?.objects()?);
        }
    }
    publet_graph::load::from_objects(objects).map_err(|e| e.to_string())
}

fn held(store: &Store) -> Result<Vec<(Cid, Vec<u8>)>, String> {
    let mut out = Vec::new();
    for text in store.cids().map_err(|e| e.to_string())? {
        let Ok(cid) = text.parse::<Cid>() else {
            continue;
        };
        if let Some(bytes) = store.get(&cid).map_err(|e| e.to_string())? {
            out.push((cid, bytes));
        }
    }
    Ok(out)
}

/// The objects this corpus's own objects reach and it does not yet hold:
/// what it cites, depends on, relates, annotates, or signs, the keys that
/// wrote them, and -- so that what is copied reads as it did where it came
/// from -- the tags, signatures, and retractions on what is copied, and the
/// delegations made to this corpus's key.
fn reach(graph: &Graph, own: &BTreeSet<String>, author: &str) -> Vec<Cid> {
    let mut about: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut seeds: Vec<String> = own.iter().cloned().collect();
    for text in graph.cids() {
        let Ok(cid) = text.parse::<Cid>() else {
            continue;
        };
        let Some(object) = graph.object(&cid) else {
            continue;
        };
        let body = object.body();
        let target = match object.kind() {
            "sig" => body.get("target"),
            "claim.annotation" if body.get("kind").and_then(Value::as_text) == Some("tagged") => {
                body.get("target")
            }
            "claim.relation" => match body.get("kind").and_then(Value::as_text) {
                Some("retracts") => body.get("to"),
                // A version pulls in the edge to its predecessor, so a
                // copied lineage keeps its genesis -- where its tag lives.
                Some("supersedes") => body.get("from"),
                Some("delegates") if body.get("to").and_then(Value::as_text) == Some(author) => {
                    seeds.push(text.to_owned());
                    None
                }
                _ => None,
            },
            _ => None,
        };
        if let Some(t) = target.and_then(Value::as_text) {
            about.entry(t.to_owned()).or_default().push(text.to_owned());
        }
        // And forward: a copied version pulls in what supersedes it, so a
        // lineage cited by its genesis arrives with its head -- bounded by
        // the pinned membership, which is all this graph holds of a parent.
        if object.kind() == "claim.relation"
            && body.get("kind").and_then(Value::as_text) == Some("supersedes")
            && let Some(older) = body.get("to").and_then(Value::as_text)
        {
            about
                .entry(older.to_owned())
                .or_default()
                .push(text.to_owned());
        }
    }

    let mut seen: BTreeSet<String> = BTreeSet::new();
    let mut queue: VecDeque<String> = seeds.into_iter().collect();
    while let Some(text) = queue.pop_front() {
        if !seen.insert(text.clone()) {
            continue;
        }
        let Ok(cid) = text.parse::<Cid>() else {
            continue;
        };
        let Some(object) = graph.object(&cid) else {
            continue;
        };
        let mut next = references(object);
        next.push(object.author().to_string());
        next.extend(about.get(&text).cloned().unwrap_or_default());
        queue.extend(next.into_iter().filter(|n| !seen.contains(n)));
    }
    seen.into_iter()
        .filter(|c| !own.contains(c))
        .filter_map(|c| c.parse::<Cid>().ok())
        .filter(|c| graph.object(c).is_some())
        .collect()
}

/// Identifiers an object names in its body.
fn references(object: &Object) -> Vec<String> {
    let body = object.body();
    let mut out = Vec::new();
    let mut text = |v: Option<&Value>| {
        if let Some(t) = v.and_then(Value::as_text) {
            out.push(t.to_owned());
        }
    };
    for key in ["from", "to", "target", "abstract", "at"] {
        text(body.get(key));
    }
    if let Some(Value::Array(depends)) = body.get("depends") {
        for d in depends {
            text(Some(d));
        }
    }
    if let Some(Value::Array(evidence)) = body.get("evidence") {
        for e in evidence {
            if matches!(e.get("kind").and_then(Value::as_text), Some("claim")) {
                text(e.get("ref"));
            }
        }
    }
    if let Some(Value::Array(sections)) = body.get("sections") {
        for s in sections {
            if let Some(Value::Array(items)) = s.get("items") {
                for i in items {
                    text(i.get("ref"));
                    text(i.get("at"));
                }
            }
        }
    }
    out
}

/// Copy the blob a copied claim's `data` cites, from whichever parent
/// holds it in `blobs/` (Section 4.7: blobs travel separately).
fn copy_blobs(object: &Object, importer: &Importer, store: &Store) -> Result<(), String> {
    let Some(reference) = object
        .body()
        .get("data")
        .and_then(|d| d.get("ref"))
        .and_then(Value::as_text)
    else {
        return Ok(());
    };
    let cid: Cid = reference
        .parse()
        .map_err(|_| format!("data.ref is not an identifier: {reference}"))?;
    if store.get_blob(&cid).map_err(|e| e.to_string())?.is_some() {
        return Ok(());
    }
    for parent in &importer.parents {
        let path = parent.root.join("blobs").join(blob_name(&cid));
        if let Ok(bytes) = std::fs::read(&path)
            && cid.verifies(&bytes)
        {
            return store.put_blob(&cid, &bytes).map_err(|e| e.to_string());
        }
    }
    Err(format!(
        "blob {cid} is cited by a copied claim but no parent holds it in blobs/"
    ))
}

/// The blob an object cites: a claim's `data.ref`, or a settlement's
/// kept output in `value.data`.
fn blob_of(object: &Object) -> Option<Cid> {
    let body = object.body();
    body.get("data")
        .and_then(|d| d.get("ref"))
        .or_else(|| body.get("value").and_then(|v| v.get("data")))
        .and_then(Value::as_text)
        .and_then(|t| t.parse().ok())
}

fn blob_name(cid: &Cid) -> String {
    cid.to_string().replace(':', "_")
}

/// Write every held object missing from `objects/`, and every held blob
/// missing from `blobs/` -- except drafts not being published, and objects
/// listed in `.draft-discards`.
fn export(cfg: &Config, store: &Store, drafts: &BTreeSet<String>) -> Result<usize, String> {
    let discards: BTreeSet<String> = std::fs::read_to_string(cfg.root.join(".draft-discards"))
        .unwrap_or_default()
        .lines()
        .filter_map(|l| l.split_once('=').map(|(_, c)| c.trim().to_owned()))
        .collect();
    let objects = cfg.objects();
    std::fs::create_dir_all(&objects).map_err(|e| e.to_string())?;
    let mut written = 0;
    let mut cited: Vec<Cid> = Vec::new();
    for text in store.cids().map_err(|e| e.to_string())? {
        if discards.contains(&text) || drafts.contains(&text) {
            continue;
        }
        let Ok(cid) = text.parse::<Cid>() else {
            continue;
        };
        let path = objects.join(file_name(&cid));
        if path.exists() {
            continue;
        }
        if let Some(bytes) = store.get(&cid).map_err(|e| e.to_string())? {
            if let Ok(object) = Object::parse(&bytes) {
                cited.extend(blob_of(object.peek()));
            }
            std::fs::write(&path, bytes).map_err(|e| e.to_string())?;
            written += 1;
        }
    }
    // Only the blobs what was just exported cites: a kept output pruned
    // since must not come back merely because the store still has it.
    let blobs = cfg.root.join("blobs");
    for cid in cited {
        let path = blobs.join(blob_name(&cid));
        if path.exists() {
            continue;
        }
        if let Some(bytes) = store.get_blob(&cid).map_err(|e| e.to_string())? {
            std::fs::create_dir_all(&blobs).map_err(|e| e.to_string())?;
            std::fs::write(&path, bytes).map_err(|e| e.to_string())?;
        }
    }
    Ok(written)
}

/// Unix seconds for an RFC 3339 UTC instant to the second, as `created`
/// is written (Section 4.3); `None` for any other form.
pub(crate) fn epoch_of(instant: &str) -> Option<u64> {
    let b = instant.as_bytes();
    let at = |i: usize, c: u8| b.get(i) == Some(&c);
    if b.len() != 20 || !at(4, b'-') || !at(7, b'-') || !at(10, b'T') || !at(19, b'Z') {
        return None;
    }
    let num = |r: std::ops::Range<usize>| instant.get(r)?.parse::<u64>().ok();
    let (y, m, d) = (num(0..4)?, num(5..7)?, num(8..10)?);
    let (hh, mm, ss) = (num(11..13)?, num(14..16)?, num(17..19)?);
    if !(1..=12).contains(&m) || !(1..=31).contains(&d) || hh > 23 || mm > 59 || ss > 60 {
        return None;
    }
    // Howard Hinnant's days-from-civil, for years from 1970.
    let div = u64::div_euclid;
    let y = if m <= 2 { y.checked_sub(1)? } else { y };
    let era = div(y, 400);
    let yoe = y - era * 400;
    let mp = if m > 2 { m - 3 } else { m + 9 };
    let doy = div(153 * mp + 2, 5) + d - 1;
    let doe = yoe * 365 + div(yoe, 4) - div(yoe, 100) + doy;
    let days = (era * 146_097 + doe).checked_sub(719_468)?;
    Some(days * 86_400 + hh * 3600 + mm * 60 + ss)
}

/// The current instant, RFC 3339 UTC to the second (Section 4.3).
pub(crate) fn now() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    // `div_euclid` rather than `/`: unsigned and exact, and the
    // workspace denies the operator form outright.
    let d = u64::div_euclid;
    let days = d(secs, 86_400);
    let rem = secs % 86_400;
    // Howard Hinnant's civil-from-days.
    let z = days + 719_468;
    let era = d(z, 146_097);
    let doe = z - era * 146_097;
    let yoe = d(doe - d(doe, 1460) + d(doe, 36_524) - d(doe, 146_096), 365);
    let doy = doe - (365 * yoe + d(yoe, 4) - d(yoe, 100));
    let mp = d(5 * doy + 2, 153);
    let day = doy - d(153 * mp + 2, 5) + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + u64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        d(rem, 3600),
        d(rem % 3600, 60),
        rem % 60
    )
}

// ------------------------------------------------------------ subcommands

fn init(args: &[String]) -> Result<(), String> {
    let here = std::env::current_dir().map_err(|e| e.to_string())?;
    if here.join(CONFIG).exists() {
        return Err(format!("{} already exists", here.join(CONFIG).display()));
    }
    let mut name = here
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("corpus")
        .to_owned();
    let mut label = None;
    let mut bound = DEFAULT_BOUND;
    let mut parents = Vec::new();
    for arg in args {
        if let Some(v) = arg.strip_prefix("--name=") {
            v.clone_into(&mut name);
        } else if let Some(v) = arg.strip_prefix("--label=") {
            label = Some(v.to_owned());
        } else if let Some(v) = arg.strip_prefix("--bound=") {
            bound = v
                .parse()
                .map_err(|_| "--bound is a byte count".to_owned())?;
        } else if let Some(v) = arg.strip_prefix("--parent=") {
            let (alias, path) = v
                .split_once('=')
                .ok_or("--parent takes ALIAS=PATH, e.g. --parent=base=../../publet-corpus")?;
            if !Path::new(&here).join(path).join(CONFIG).exists() {
                return Err(format!("{path} holds no {CONFIG}; it is not a corpus"));
            }
            parents.push(serde_json::json!({ "alias": alias, "path": path }));
        } else {
            return Err(format!("unknown argument: {arg}"));
        }
    }
    if !here.join(crate::workspace::DIR).is_dir() {
        crate::init(&[])?;
    }
    let config = serde_json::json!({
        "name": name,
        "label": label.unwrap_or_else(|| name.clone()),
        "bound": bound,
        "parents": parents,
    });
    std::fs::write(
        here.join(CONFIG),
        serde_json::to_string_pretty(&config).map_err(|e| e.to_string())? + "\n",
    )
    .map_err(|e| e.to_string())?;
    println!("{} written", here.join(CONFIG).display());
    println!("Sources go under publets/; `pub build` builds them, exports objects/,");
    println!("and advances this corpus's generation.");
    Ok(())
}

fn status(args: &[String]) -> Result<(), String> {
    if let Some(arg) = args.first() {
        return Err(format!("unknown argument: {arg}"));
    }
    let here = std::env::current_dir().map_err(|e| e.to_string())?;
    let cfg = Config::read(&here)?;
    let meta = Meta::from_lock(&Lock::read(&here.join(LOCK))?);
    match &meta.generation {
        Some((index, cid)) => println!("{}  generation {index}  {cid}", cfg.name),
        None => println!("{}  not built yet", cfg.name),
    }
    for parent in &cfg.parents {
        let pin = meta.parents.iter().find(|p| p.alias == parent.alias);
        let view = ParentView::load(parent, pin)?;
        let ahead = view.latest.0.saturating_sub(view.pin.index);
        let state = if ahead == 0 {
            "up to date".to_owned()
        } else {
            format!(
                "{ahead} generation(s) ahead; `pub corpus upgrade {}` to move",
                parent.alias
            )
        };
        println!(
            "  {}  pinned to generation {}, latest {}: {state}",
            parent.alias, view.pin.index, view.latest.0
        );
        for (reference, cid) in meta
            .imports
            .iter()
            .filter(|(r, _)| r.starts_with(&format!("{}:", parent.alias)))
        {
            let slug = reference.split_once(':').map_or("", |(_, s)| s);
            match view.slugs.get(slug) {
                Some(current) if current != cid => {
                    println!("    {reference} has a newer version: {cid} -> {current}");
                }
                Some(_) => {}
                None => println!("    {reference} is no longer declared by {}", parent.alias),
            }
        }
    }
    Ok(())
}

fn upgrade(args: &[String]) -> Result<(), String> {
    let here = std::env::current_dir().map_err(|e| e.to_string())?;
    let cfg = Config::read(&here)?;
    let lock_path = here.join(LOCK);
    let mut lock = Lock::read(&lock_path)?;
    let mut meta = Meta::from_lock(&lock);
    let only: Option<&str> = args.first().map(String::as_str);
    if let Some(alias) = only
        && !cfg.parents.iter().any(|p| p.alias == alias)
    {
        return Err(format!("`{alias}` is not a parent of this corpus"));
    }
    for parent in cfg
        .parents
        .iter()
        .filter(|p| only.is_none_or(|a| a == p.alias))
    {
        let latest = ParentView::load(parent, None)?;
        let previous = meta
            .parents
            .iter()
            .find(|p| p.alias == parent.alias)
            .map_or(0, |p| p.index);
        println!(
            "{}  generation {previous} -> {}",
            parent.alias, latest.pin.index
        );
        for (reference, cid) in &mut meta.imports {
            let Some(slug) = reference.strip_prefix(&format!("{}:", parent.alias)) else {
                continue;
            };
            match latest.slugs.get(slug) {
                Some(current) if current != cid => {
                    println!("  {reference}  {cid} -> {current}");
                    cid.clone_from(current);
                }
                Some(_) => {}
                None => println!(
                    "  {reference}  kept: {} no longer declares it",
                    parent.alias
                ),
            }
        }
        meta.parents.retain(|p| p.alias != parent.alias);
        meta.parents.push(latest.pin.clone());
    }
    meta.parents.sort_by(|a, b| a.alias.cmp(&b.alias));
    lock.corpus = Some(meta.to_value());
    lock.write(&lock_path)?;
    println!("re-pinned; `pub build` revises whatever cited what changed");
    Ok(())
}

// ------------------------------------------------------------- renderings

/// `pub corpus renderings [--prune]`: every rendering this corpus has
/// pinned, and whether its output is still kept.
///
/// A rendering is **active** when it is the newest one pinned for the
/// version of its document that is currently published. Any other is kept
/// (`held`) until pruned, after which only its record remains: what was
/// rendered, by which method and tools, into which bytes -- enough to
/// render it again and check the result against the identifier, though
/// byte-for-byte only with the same toolchain.
fn renderings(args: &[String]) -> Result<(), String> {
    let mut prune = false;
    for arg in args {
        match arg.as_str() {
            "--prune" => prune = true,
            other => return Err(format!("unknown argument: {other}")),
        }
    }
    let here = std::env::current_dir().map_err(|e| e.to_string())?;
    let cfg = Config::read(&here)?;
    let lock = Lock::read(&here.join(LOCK))?;
    let graph = publet_graph::load::from_dir(&cfg.objects()).map_err(|e| e.to_string())?;
    let all = crate::render::renderings(&graph);
    let current: BTreeSet<String> = lock.entries.values().map(|e| e.cid.clone()).collect();
    let active: BTreeSet<String> = current
        .iter()
        .filter_map(|c| c.parse::<Cid>().ok())
        .filter_map(|doc| crate::render::active_rendering(&graph, &doc))
        .map(|r| r.settled.to_string())
        .collect();
    let slug_of = |cid: &Cid| {
        lock.entries
            .values()
            .find(|e| e.cid == cid.to_string())
            .map_or_else(|| cid.to_string(), |e| e.slug.clone())
    };
    // Identical bytes are one blob, so an inactive rendering may share its
    // kept output with an active one: never prune what is still active.
    let keep: BTreeSet<String> = all
        .iter()
        .filter(|r| active.contains(&r.settled.to_string()))
        .filter_map(|r| r.blob.as_ref().map(ToString::to_string))
        .collect();
    let blobs = cfg.root.join("blobs");
    let mut removed = 0;
    for r in &all {
        let held = r
            .blob
            .as_ref()
            .is_some_and(|b| blobs.join(blob_name(b)).exists());
        let mut state = if active.contains(&r.settled.to_string()) {
            "active"
        } else if held {
            "held"
        } else {
            "pruned"
        };
        if prune
            && state == "held"
            && let Some(blob) = &r.blob
            && !keep.contains(&blob.to_string())
        {
            std::fs::remove_file(blobs.join(blob_name(blob))).map_err(|e| e.to_string())?;
            removed += 1;
            state = "pruned";
        }
        println!("{state:<7} {}  rendered {}", slug_of(&r.doc), r.created);
        println!("        document  {}", r.doc);
        println!("        method    {}", r.method);
        for (name, seen) in &r.tools {
            println!("        tool      {name}: {seen}");
        }
        for (path, cid) in &r.outputs {
            println!("        output    {path}  {cid}");
        }
        match (&r.blob, state) {
            (Some(blob), "pruned") => println!(
                "        kept      {blob}  unavailable: pruned. Re-render document {} \
                 with method {} and the tools above, and compare against it",
                r.doc, r.method
            ),
            (Some(blob), _) => println!("        kept      {blob}"),
            (None, _) => {}
        }
    }
    if all.is_empty() {
        println!("no renderings pinned; `pub publish` pins the rendering beside a publet");
    }
    if prune {
        println!(
            "{removed} kept output(s) pruned; every record remains. Byte-for-byte \
             re-rendering needs the toolchain each record names."
        );
    }
    Ok(())
}

// -------------------------------------------------------------------- map

struct MapCorpus {
    name: String,
    lock: Lock,
    graph: Graph,
}

/// `pub corpus map`: the publets of this corpus and every corpus above it,
/// the citations between them, and the external works they point at.
fn map(args: &[String]) -> Result<(), String> {
    let mut format = "mermaid".to_owned();
    for arg in args {
        match arg.strip_prefix("--format=") {
            Some(f @ ("mermaid" | "dot" | "json")) => f.clone_into(&mut format),
            _ => {
                return Err(format!(
                    "unknown argument: {arg} (--format=mermaid|dot|json)"
                ));
            }
        }
    }
    let here = std::env::current_dir().map_err(|e| e.to_string())?;
    draw(&chain(&here)?, &format)
}

/// This corpus, then each ancestor once.
fn chain(here: &Path) -> Result<Vec<MapCorpus>, String> {
    let mut corpora = Vec::new();
    let mut queue = VecDeque::from([here.to_path_buf()]);
    let mut seen = BTreeSet::new();
    while let Some(root) = queue.pop_front() {
        let canonical = root
            .canonicalize()
            .map_err(|e| format!("{}: {e}", root.display()))?;
        if !seen.insert(canonical.clone()) {
            continue;
        }
        let cfg = Config::read(&canonical)?;
        for p in &cfg.parents {
            queue.push_back(p.path.clone());
        }
        let graph = publet_graph::load::from_dir(&cfg.objects()).map_err(|e| e.to_string())?;
        corpora.push(MapCorpus {
            name: cfg.name.clone(),
            lock: Lock::read(&canonical.join(LOCK))?,
            graph,
        });
    }
    Ok(corpora)
}

/// Print the publets of `corpora`, the citations between them, and the
/// external works they point at.
fn draw(corpora: &[MapCorpus], format: &str) -> Result<(), String> {
    // Ownership: the most general corpus whose lock names an object owns
    // it, so a child's copy of a parent's publet is drawn in the parent.
    let mut owner: BTreeMap<String, (String, String)> = BTreeMap::new();
    for c in corpora.iter().rev() {
        for e in c.lock.entries.values() {
            let publet = e.slug.split('#').next().unwrap_or(&e.slug).to_owned();
            owner
                .entry(e.cid.clone())
                .or_insert_with(|| (c.name.clone(), publet));
        }
    }

    let mut nodes: BTreeMap<(String, String), String> = BTreeMap::new();
    let mut edges: BTreeSet<((String, String), (String, String))> = BTreeSet::new();
    let mut externals: BTreeSet<((String, String), String)> = BTreeSet::new();
    for c in corpora {
        for e in c.lock.entries.values() {
            let Some(here_owner) = owner.get(&e.cid).cloned() else {
                continue;
            };
            if here_owner.0 != c.name {
                continue;
            }
            if e.kind == "doc" {
                let title = e
                    .cid
                    .parse::<Cid>()
                    .ok()
                    .and_then(|cid| c.graph.document(&cid).map(|d| d.title().to_owned()))
                    .unwrap_or_else(|| e.slug.clone());
                let tag = e
                    .tag
                    .as_deref()
                    .map(|t| format!(" [{t}]"))
                    .unwrap_or_default();
                nodes.insert(here_owner.clone(), format!("{title}{tag}"));
            }
            let Some(object) = e
                .cid
                .parse::<Cid>()
                .ok()
                .and_then(|cid| c.graph.object(&cid))
            else {
                continue;
            };
            for r in references(object) {
                if let Some(other) = owner.get(&r)
                    && *other != here_owner
                {
                    edges.insert((here_owner.clone(), other.clone()));
                }
            }
            if let Some(Value::Array(evidence)) = object.body().get("evidence") {
                for ev in evidence {
                    if ev.get("kind").and_then(Value::as_text) == Some("external")
                        && let Some(url) = ev.get("ref").and_then(Value::as_text)
                        && url.starts_with("http")
                    {
                        externals.insert((here_owner.clone(), url.to_owned()));
                    }
                }
            }
        }
    }

    let names: Vec<String> = corpora.iter().map(|c| c.name.clone()).collect();
    print!(
        "{}",
        match format {
            "json" => map_json(&names, &nodes, &edges, &externals)?,
            "dot" => map_dot(&names, &nodes, &edges, &externals),
            _ => map_mermaid(&names, &nodes, &edges, &externals),
        }
    );
    Ok(())
}

type Node = (String, String);

fn node_id(n: &Node) -> String {
    format!("{}__{}", n.0, n.1)
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect()
}

fn url_id(url: &str) -> String {
    format!(
        "ext_{}",
        url.chars()
            .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
            .collect::<String>()
    )
}

fn map_mermaid(
    corpora: &[String],
    nodes: &BTreeMap<Node, String>,
    edges: &BTreeSet<(Node, Node)>,
    externals: &BTreeSet<(Node, String)>,
) -> String {
    use std::fmt::Write as _;
    let mut out = String::from("flowchart LR\n");
    for corpus in corpora {
        let _ = writeln!(out, "  subgraph {corpus}[\"corpus: {corpus}\"]");
        for (n, label) in nodes.iter().filter(|(n, _)| &n.0 == corpus) {
            let _ = writeln!(out, "    {}[\"{}\"]", node_id(n), label.replace('"', "'"));
        }
        out.push_str("  end\n");
    }
    out.push_str("  subgraph external[\"external\"]\n");
    for url in externals.iter().map(|(_, u)| u).collect::<BTreeSet<_>>() {
        let _ = writeln!(out, "    {}([\"{url}\"])", url_id(url));
    }
    out.push_str("  end\n");
    for (a, b) in edges {
        let _ = writeln!(out, "  {} --> {}", node_id(a), node_id(b));
    }
    for (a, url) in externals {
        let _ = writeln!(out, "  {} -.-> {}", node_id(a), url_id(url));
    }
    out
}

fn map_dot(
    corpora: &[String],
    nodes: &BTreeMap<Node, String>,
    edges: &BTreeSet<(Node, Node)>,
    externals: &BTreeSet<(Node, String)>,
) -> String {
    use std::fmt::Write as _;
    let mut out = String::from("digraph publets {\n  rankdir=LR;\n");
    for corpus in corpora {
        let _ = writeln!(
            out,
            "  subgraph cluster_{corpus} {{ label=\"corpus: {corpus}\";"
        );
        for (n, label) in nodes.iter().filter(|(n, _)| &n.0 == corpus) {
            let _ = writeln!(
                out,
                "    {} [label=\"{}\"];",
                node_id(n),
                label.replace('"', "'")
            );
        }
        out.push_str("  }\n");
    }
    for url in externals.iter().map(|(_, u)| u).collect::<BTreeSet<_>>() {
        let _ = writeln!(out, "  {} [label=\"{url}\", shape=note];", url_id(url));
    }
    for (a, b) in edges {
        let _ = writeln!(out, "  {} -> {};", node_id(a), node_id(b));
    }
    for (a, url) in externals {
        let _ = writeln!(out, "  {} -> {} [style=dashed];", node_id(a), url_id(url));
    }
    out.push_str("}\n");
    out
}

fn map_json(
    corpora: &[String],
    nodes: &BTreeMap<Node, String>,
    edges: &BTreeSet<(Node, Node)>,
    externals: &BTreeSet<(Node, String)>,
) -> Result<String, String> {
    let node = |n: &Node| serde_json::json!({ "corpus": n.0, "publet": n.1 });
    let json = serde_json::json!({
        "corpora": corpora,
        "publets": nodes.iter().map(|(n, label)| serde_json::json!({
            "corpus": n.0, "publet": n.1, "label": label,
        })).collect::<Vec<_>>(),
        "cites": edges.iter().map(|(a, b)| serde_json::json!({
            "from": node(a), "to": node(b),
        })).collect::<Vec<_>>(),
        "external": externals.iter().map(|(a, url)| serde_json::json!({
            "from": node(a), "url": url,
        })).collect::<Vec<_>>(),
    });
    serde_json::to_string_pretty(&json)
        .map(|s| s + "\n")
        .map_err(|e| e.to_string())
}

// --------------------------------------------------------------- delegate

/// `pub delegate --to=KEY [--aspect=lineage] [--effective=immediate]`:
/// authorize another key to continue this key's work (Section 10.6).
///
/// # Errors
///
/// Returns a message if `--to` is missing or not an identifier, an aspect
/// or effective value is unknown, or the loader refuses the relation.
pub(crate) fn delegate(args: &[String]) -> Result<(), String> {
    const ASPECTS: [&str; 3] = ["lineage", "stewardship", "authorship"];
    let here = std::env::current_dir().map_err(|e| e.to_string())?;
    let ws = Workspace::open(&here)?;
    let store = ws.store()?;
    let mut to = None;
    let mut aspect = "lineage".to_owned();
    let mut effective = "immediate".to_owned();
    let mut dormancy: Option<u64> = None;
    for arg in args {
        if let Some(v) = arg.strip_prefix("--to=") {
            to = Some(v.to_owned());
        } else if let Some(v) = arg.strip_prefix("--aspect=") {
            v.clone_into(&mut aspect);
        } else if let Some(v) = arg.strip_prefix("--effective=") {
            v.clone_into(&mut effective);
        } else if let Some(v) = arg.strip_prefix("--dormancy-days=") {
            dormancy = Some(
                v.parse()
                    .map_err(|_| "--dormancy-days is a number".to_owned())?,
            );
        } else {
            return Err(format!("unknown argument: {arg}"));
        }
    }
    let to = to.ok_or("--to=KEY is required: the key being authorized to continue")?;
    to.parse::<Cid>()
        .map_err(|_| format!("--to is not an identifier: {to}"))?;
    if !ASPECTS.contains(&aspect.as_str()) {
        return Err(format!(
            "unknown --aspect: {aspect} ({})",
            ASPECTS.join(", ")
        ));
    }
    if !["immediate", "on-dormancy"].contains(&effective.as_str()) {
        return Err(format!(
            "unknown --effective: {effective} (immediate, on-dormancy)"
        ));
    }
    let author = ws
        .get("author")
        .ok_or("no author configured; run `pub sign --generate-key`")?;
    let mut builder = Object::builder("claim.relation", &author)
        .created(&now())
        .field("kind", Value::Text("delegates".to_owned()))
        .field("from", Value::Text(author.clone()))
        .field("to", Value::Text(to.clone()))
        .field("aspect", Value::Text(aspect.clone()))
        .field("effective", Value::Text(effective.clone()))
        .field("scope", crate::compose::scope_value("unconditional"));
    if let Some(days) = dormancy {
        builder = builder.field("dormancy_days", Value::Uint(days));
    }
    let bytes = builder.build().map_err(|e| e.to_string())?;
    let cid = Cid::of(&bytes, HashAlg::Sha2_256);
    crate::compose::offer_then_store(&store, &cid, &bytes, "this delegation was refused")?;
    println!("{cid}");
    eprintln!();
    eprintln!("{to} may now continue this key's `{aspect}` ({effective}).");
    eprintln!("It reaches a child corpus once this corpus is rebuilt and the child upgrades.");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_instant_converts_to_unix_seconds_and_back() {
        assert_eq!(epoch_of("1970-01-01T00:00:00Z"), Some(0));
        assert_eq!(epoch_of("2026-10-04T00:00:00Z"), Some(1_791_072_000));
        assert_eq!(epoch_of("2000-03-01T12:34:56Z"), Some(951_914_096));
        assert_eq!(epoch_of("2026-10-04"), None);
        assert_eq!(epoch_of("2026-13-04T00:00:00Z"), None);
    }

    #[test]
    fn now_is_rfc3339_utc_to_the_second() {
        let t = now();
        assert_eq!(t.len(), 20, "{t}");
        assert!(t.ends_with('Z') && t.as_bytes()[10] == b'T', "{t}");
    }

    #[test]
    fn a_meta_section_round_trips_through_the_lock() {
        let meta = Meta {
            key: Some("pub:sha2-256:k".into()),
            domain: Some("pub:sha2-256:d".into()),
            generation: Some((2, "pub:sha2-256:g".into())),
            parents: vec![Pin {
                alias: "base".into(),
                index: 7,
                generation: "pub:sha2-256:p".into(),
                key: None,
            }],
            imports: BTreeMap::from([("base:tool.x".into(), "pub:sha2-256:x".into())]),
        };
        let lock = Lock {
            entries: BTreeMap::new(),
            corpus: Some(meta.to_value()),
        };
        let back = Meta::from_lock(&lock);
        assert_eq!(back.generation, meta.generation);
        assert_eq!(back.parents.len(), 1);
        assert_eq!(back.parents[0].index, 7);
        assert_eq!(back.imports, meta.imports);
    }
}
