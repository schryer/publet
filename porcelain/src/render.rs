//! `pub render` and `pub myst`: turn a publet into a document by running
//! the pipeline it names, and only that pipeline.
//!
//! A publet says how it is rendered. Its `Colophon` section cites, as
//! `background`, a render-pipeline publet; that publet bundles one
//! `procedural` claim whose `depends` lists the steps in order (Section
//! 5.7: a method decomposes through `depends`). Each step is itself a
//! `procedural` claim whose `data` is a table of commands -- `program`,
//! `args`, and optionally `cwd` and `outputs` -- and whose `depends` names
//! the `archival` identity claim of every program it runs: `name`,
//! `version`, `check`, `expect`. A file the pipeline uses from this
//! repository is a claim carrying it as a blob, with a `source` whose
//! `locator` says where it lives.
//!
//! `pub render` refuses to run anything until all of it checks out:
//!
//! * every object in the pipeline carries an `authored` signature from a
//!   key the workspace policy trusts -- the commands come out of objects,
//!   and an unsigned object is a command anyone could have written;
//! * every program a step runs has an identity claim, and running its
//!   `check` command prints what the claim says the installed version is;
//! * every repository file the pipeline cites hashes to the blob the claim
//!   carries.
//!
//! A failure names the publet that disagrees with the machine, by name and
//! identifier, and nothing runs. That is what keeps the description of the
//! rendering and the rendering from drifting apart. Commands run without a
//! shell, with only `{doc}`, `{out}`, `{repo}`, `{ws}` (the workspace),
//! and `{lock}` substituted, and a `check` may only invoke the program it
//! identifies. The program `pub` is this binary, so the version checked is
//! the version that runs.
//!
//! `pub myst` is the first step of the pipeline this repository uses: it
//! writes a `MyST` project for a document, showing every cited publet as
//! `Title [TAG] -- identifier` so a reader can both read the name and
//! check the identity, which a name alone cannot give them.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::process::Command;

use publet_core::{Cid, HashAlg, Object, SigAlg, cbor::Value};
use publet_graph::{Class, Data, Graph, Lineage, ProseClaim};

use crate::workspace::Workspace;

const PLACEHOLDERS: [&str; 8] = [
    "{doc}",
    "{out}",
    "{repo}",
    "{ws}",
    "{lock}",
    "{epoch}",
    "{env}",
    "{host:NAME}",
];

/// The record of a rendering, beside its outputs: what `pub publish` pins.
pub(crate) const RECORD: &str = "render.json";

/// Run a publet's render pipeline.
///
/// # Errors
///
/// Returns a message if the publet names no pipeline, the pre-flight finds
/// anything wrong, or a step fails.
pub(crate) fn run(args: &[String]) -> Result<(), String> {
    let here = std::env::current_dir().map_err(|e| e.to_string())?;
    let ws = Workspace::open(&here)?;

    let mut target = None;
    let mut lock = default_lock();
    let mut out = None;
    let mut repo = None;
    let mut check_only = false;
    for arg in args {
        if let Some(v) = arg.strip_prefix("--lock=") {
            lock = PathBuf::from(v);
        } else if let Some(v) = arg.strip_prefix("--out=") {
            out = Some(PathBuf::from(v));
        } else if let Some(v) = arg.strip_prefix("--repo=") {
            repo = Some(PathBuf::from(v));
        } else if arg == "--check" {
            check_only = true;
        } else if arg.starts_with("--") {
            return Err(format!("unknown argument: {arg}"));
        } else {
            target = Some(arg.clone());
        }
    }
    let target = target.ok_or("a publet is required: pub render SLUG|CID")?;
    let names = Names::read(&lock)?;
    let doc = names.resolve(&target)?;
    // A bare `corpus.lock` has an empty parent, not none.
    let lock_dir = absolute(
        lock.parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new(".")),
    )?;
    // Beside the publet's source when the lock says where that is.
    let out = absolute(&out.unwrap_or_else(|| {
        lock_dir.join(
            names
                .source_of(&doc)
                .unwrap_or_else(|| names.slug_or(&doc, &target).to_owned()),
        )
    }))?;
    let repo = absolute(&repo.unwrap_or_else(|| git_root(&lock_dir)))?;

    // Everything is read before anything runs, and the store is released
    // first: a step may itself be `pub`, which needs the store's lock.
    let graph = {
        let store = ws.store()?;
        crate::corpus::visible_graph(&here, &store)?
    };
    // The pinned parents' keys are trusted too: building on a parent
    // corpus is trusting what it published.
    let mut trusted = trusted_keys(&ws, &graph);
    trusted.extend(names.parent_keys.iter().cloned());
    let ctx = Ctx {
        graph: &graph,
        names: &names,
    };

    let pipeline = Pipeline::of(&ctx, &doc)?;
    println!("publet    {}", ctx.label(&doc));
    println!("pipeline  {}", ctx.label(&pipeline.doc));

    // Inside an environment, only what it provisioned runs (Section 8).
    let mut problems = Vec::new();
    let env = match &pipeline.env {
        Some(env) => {
            println!("env       {}", ctx.label(env));
            Some(crate::env::ready(&ctx, env, &trusted, &mut problems)?)
        }
        None => None,
    };
    let bin = env.as_ref().map(|e| e.join("bin"));
    problems.extend(pipeline.unsigned(&ctx, &trusted));
    let observed = pipeline.check_tools(&ctx, bin.as_deref(), &mut problems);
    pipeline.check_files(&ctx, &repo, &mut problems);
    if !problems.is_empty() {
        let mut message = String::from(
            "pre-flight failed, so nothing was run. The pipeline describes \
             something other than this machine:\n",
        );
        for p in &problems {
            message.push_str("  - ");
            message.push_str(p);
            message.push('\n');
        }
        return Err(message.trim_end().to_owned());
    }
    println!("pre-flight passed: signatures, tool versions, and repository files match");
    if check_only {
        return Ok(());
    }

    let values = [
        ("{doc}", doc.to_string()),
        ("{out}", out.display().to_string()),
        ("{repo}", repo.display().to_string()),
        ("{ws}", here.display().to_string()),
        ("{lock}", absolute(&lock)?.display().to_string()),
        // The document's own `created`, so a PDF's embedded date -- and so
        // its bytes -- depend on what was rendered, not on when.
        ("{epoch}", epoch_of(&graph, &doc).to_string()),
        (
            "{env}",
            env.as_ref()
                .map(|e| e.display().to_string())
                .unwrap_or_default(),
        ),
    ];
    let outputs = execute(&ctx, &pipeline, &out, &values, bin.as_deref())?;
    let record = record(&doc, &pipeline, &observed, &out, &outputs)?;
    let record_path = out.join(RECORD);
    std::fs::write(&record_path, record)
        .map_err(|e| format!("cannot write {}: {e}", record_path.display()))?;
    println!("rendered; nothing is recorded until `pub publish` pins it");
    Ok(())
}

/// Write a `MyST` project for a document: `pub myst SLUG|CID --out=DIR
/// --template=PATH [--export=PATH] [--author=NAME]...`.
///
/// # Errors
///
/// Returns a message if the document is not held or the files cannot be
/// written.
pub(crate) fn run_myst(args: &[String]) -> Result<(), String> {
    let here = std::env::current_dir().map_err(|e| e.to_string())?;
    let ws = Workspace::open(&here)?;

    let mut target = None;
    let mut lock = default_lock();
    let mut out = None;
    let mut template = None;
    let mut export = "classicthesis/main.typ".to_owned();
    let mut authors = Vec::new();
    for arg in args {
        if let Some(v) = arg.strip_prefix("--lock=") {
            lock = PathBuf::from(v);
        } else if let Some(v) = arg.strip_prefix("--out=") {
            out = Some(PathBuf::from(v));
        } else if let Some(v) = arg.strip_prefix("--template=") {
            template = Some(v.to_owned());
        } else if let Some(v) = arg.strip_prefix("--export=") {
            v.clone_into(&mut export);
        } else if let Some(v) = arg.strip_prefix("--author=") {
            authors.push(v.to_owned());
        } else if arg.starts_with("--") {
            return Err(format!("unknown argument: {arg}"));
        } else {
            target = Some(arg.clone());
        }
    }
    let target = target.ok_or("a document is required: pub myst SLUG|CID --out=DIR")?;
    let out = out.ok_or("--out=DIR is required: where to write the MyST project")?;
    let template =
        template.ok_or("--template=PATH is required: the jtex template to export with")?;

    let names = Names::read(&lock).unwrap_or_default();
    let doc = names.resolve(&target)?;
    let graph = {
        let store = ws.store()?;
        crate::corpus::visible_graph(&here, &store)?
    };
    let ctx = Ctx {
        graph: &graph,
        names: &names,
    };
    if authors.is_empty() {
        authors = ctx.signers(&doc);
    }

    let page = Page::of(&ctx, &doc)?;
    std::fs::create_dir_all(&out).map_err(|e| format!("cannot create {}: {e}", out.display()))?;
    std::fs::write(
        out.join("myst.yml"),
        page.myst_yml(&authors, &template, &export)?,
    )
    .map_err(|e| e.to_string())?;
    std::fs::write(out.join("index.md"), page.index_md(&ctx)?).map_err(|e| e.to_string())?;
    println!("{}", out.join("myst.yml").display());
    println!("{}", out.join("index.md").display());
    Ok(())
}

/// Run each step's commands in order, without a shell, and collect the
/// outputs they declare.
pub(crate) fn execute(
    ctx: &Ctx<'_>,
    pipeline: &Pipeline,
    out: &Path,
    values: &[(&str, String)],
    bin: Option<&Path>,
) -> Result<Vec<(usize, PathBuf, Cid)>, String> {
    std::fs::create_dir_all(out).map_err(|e| format!("cannot create {}: {e}", out.display()))?;
    let mut outputs = Vec::new();
    for (index, step) in pipeline.steps.iter().enumerate() {
        println!("step      {}", ctx.label(&step.cid));
        for command in &step.commands {
            let args = substitute_all(&command.args, values)?;
            let cwd = match &command.cwd {
                Some(c) => PathBuf::from(substitute(c, values)?),
                None => out.to_path_buf(),
            };
            println!("  $ {} {}", command.program, args.join(" "));
            let status = program(&command.program, bin)?
                .args(&args)
                .current_dir(&cwd)
                .status()
                .map_err(|e| format!("cannot run {}: {e}", command.program))?;
            if !status.success() {
                return Err(format!(
                    "{} exited with {status} in step {}",
                    command.program,
                    ctx.label(&step.cid)
                ));
            }
            for output in &command.outputs {
                // `path@CID` pins what the output must be: a download is
                // checked here, before any later step can use it.
                let (path, pinned) = pinned_output(output)?;
                let path = cwd.join(substitute(path, values)?);
                let bytes = std::fs::read(&path).map_err(|_| {
                    format!(
                        "step {} declares the output {}, which it did not produce",
                        ctx.label(&step.cid),
                        path.display()
                    )
                })?;
                let cid = Cid::of(&bytes, HashAlg::Sha2_256);
                if let Some(pinned) = pinned
                    && pinned != cid
                {
                    return Err(format!(
                        "step {}: {} hashes to {cid}, not the {pinned} the step pins; \
                         nothing further was run",
                        ctx.label(&step.cid),
                        path.display()
                    ));
                }
                outputs.push((index, path, cid));
            }
        }
    }

    Ok(outputs)
}

/// A command for `name`: this binary for `pub`; inside an environment,
/// only what its `bin/` holds, run with `PATH` set to that alone; otherwise
/// found on the host's `PATH`.
pub(crate) fn program(name: &str, bin: Option<&Path>) -> Result<Command, String> {
    if name == "pub" {
        let me = std::env::current_exe().map_err(|e| format!("cannot locate pub itself: {e}"))?;
        return Ok(Command::new(me));
    }
    let Some(bin) = bin else {
        return Ok(Command::new(name));
    };
    let path = bin.join(name);
    if !path.exists() {
        return Err(format!(
            "`{name}` is not in the environment ({}); nothing outside it is run",
            bin.display()
        ));
    }
    let mut command = Command::new(path);
    command.env("PATH", bin);
    Ok(command)
}

/// The absolute path of `name` on the host's `PATH`.
pub(crate) fn which(name: &str) -> Option<PathBuf> {
    std::env::var_os("PATH").and_then(|paths| {
        std::env::split_paths(&paths)
            .map(|d| d.join(name))
            .find(|p| p.is_file())
    })
}

/// An output and the identifier it is pinned to, if written `path@CID`.
fn pinned_output(output: &str) -> Result<(&str, Option<Cid>), String> {
    match output.rsplit_once('@') {
        Some((path, cid)) if cid.starts_with("pub:") => cid
            .parse()
            .map(|c| (path, Some(c)))
            .map_err(|_| format!("`{output}` pins something that is not an identifier")),
        _ => Ok((output, None)),
    }
}

/// `corpus.lock` inside a corpus, otherwise `publets/publets.lock`.
pub(crate) fn default_lock() -> PathBuf {
    if Path::new(crate::corpus::CONFIG).exists() {
        PathBuf::from(crate::corpus::LOCK)
    } else {
        PathBuf::from("publets").join(crate::build::LOCK)
    }
}

/// The git repository enclosing `dir`, or `dir`'s parent if none does:
/// what `{repo}` means to a pipeline citing files in the repository.
fn git_root(dir: &Path) -> PathBuf {
    dir.ancestors()
        .find(|d| d.join(".git").exists())
        .map_or_else(|| dir.join(".."), Path::to_path_buf)
}

fn absolute(path: &Path) -> Result<PathBuf, String> {
    std::path::absolute(path).map_err(|e| format!("{}: {e}", path.display()))
}

// ------------------------------------------------------------------ names

/// The lock read backwards: identifier to slug, and slug to identifier.
#[derive(Default)]
pub(crate) struct Names {
    by_slug: BTreeMap<String, String>,
    by_cid: BTreeMap<String, String>,
    sources: BTreeMap<String, String>,
    parent_keys: Vec<String>,
    /// Objects that are drafts: unsigned, and the author's own.
    drafts: BTreeSet<String>,
}

impl Names {
    pub(crate) fn read(path: &Path) -> Result<Self, String> {
        let drafts = path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join(crate::workspace::DIR)
            .join("drafts.json");
        // A corpus with drafts but nothing yet published has no lock.
        let json = if path.exists() {
            crate::payload::read_json(path)?
        } else if drafts.exists() {
            serde_json::json!({})
        } else {
            return Err(format!(
                "no lock at {}; run `pub build` first, or pass --lock=PATH",
                path.display()
            ));
        };
        let mut names = Self::default();
        names.absorb(&json);
        // Inside a corpus, drafts overlay what is published: rendering a
        // draft is how you see it before publishing it.
        if let Ok(drafted) = crate::payload::read_json(&drafts) {
            names.absorb(&drafted);
            names.drafts = drafted
                .get("corpus")
                .and_then(|c| c.get("objects"))
                .and_then(serde_json::Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(serde_json::Value::as_str)
                .map(str::to_owned)
                .collect();
        }
        // A parent's object reads as how this corpus imports it.
        for (reference, cid) in json
            .get("corpus")
            .and_then(|c| c.get("imports"))
            .and_then(serde_json::Value::as_object)
            .into_iter()
            .flatten()
        {
            if let Some(cid) = cid.as_str() {
                names
                    .by_cid
                    .entry(cid.to_owned())
                    .or_insert_with(|| reference.clone());
                names
                    .by_slug
                    .entry(reference.clone())
                    .or_insert_with(|| cid.to_owned());
            }
        }
        names.parent_keys = json
            .get("corpus")
            .and_then(|c| c.get("parents"))
            .and_then(serde_json::Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|p| p.get("key").and_then(serde_json::Value::as_str))
            .map(str::to_owned)
            .collect();
        Ok(names)
    }

    fn absorb(&mut self, json: &serde_json::Value) {
        for row in json
            .get("publets")
            .and_then(serde_json::Value::as_array)
            .into_iter()
            .flatten()
        {
            let field = |k: &str| row.get(k).and_then(serde_json::Value::as_str);
            if let (Some(slug), Some(cid)) = (field("slug"), field("cid")) {
                self.by_slug.insert(slug.to_owned(), cid.to_owned());
                self.by_cid.insert(cid.to_owned(), slug.to_owned());
                if let Some(source) = field("source") {
                    self.sources.insert(cid.to_owned(), source.to_owned());
                }
            }
        }
    }

    pub(crate) fn resolve(&self, target: &str) -> Result<Cid, String> {
        let text = if target.starts_with("pub:") {
            target
        } else {
            self.by_slug
                .get(target)
                .ok_or_else(|| format!("`{target}` is not a slug the lock knows"))?
        };
        text.parse()
            .map_err(|_| format!("`{text}` is not an identifier"))
    }

    /// Keys of the parent corpora this one is pinned to.
    pub(crate) fn parent_keys(&self) -> Vec<String> {
        self.parent_keys.clone()
    }

    fn source_of(&self, cid: &Cid) -> Option<String> {
        self.sources.get(&cid.to_string()).cloned()
    }

    fn slug_or<'a>(&'a self, cid: &Cid, fallback: &'a str) -> &'a str {
        self.by_cid
            .get(&cid.to_string())
            .map_or(fallback, String::as_str)
    }
}

pub(crate) struct Ctx<'a> {
    pub(crate) graph: &'a Graph,
    pub(crate) names: &'a Names,
}

impl Ctx<'_> {
    /// `Title [TAG] -- identifier` for a publet, `slug -- identifier` for
    /// anything else the lock names, the bare identifier otherwise. The
    /// name is never shown without the identifier: a name can be captured
    /// and the identifier cannot (Section 9.1).
    pub(crate) fn label(&self, cid: &Cid) -> String {
        let title = self
            .graph
            .document(cid)
            .map(|d| d.title().to_owned())
            .or_else(|| self.names.by_cid.get(&cid.to_string()).cloned());
        let tags = self.tags(cid);
        match (title, tags.is_empty()) {
            (Some(t), false) => format!("{t} [{}] — {cid}", tags.join(", ")),
            (Some(t), true) => format!("{t} — {cid}"),
            (None, _) => cid.to_string(),
        }
    }

    /// [`Self::label`] as Markdown: the title bold, the identifier as code.
    fn label_md(&self, cid: &Cid) -> String {
        let title = self
            .graph
            .document(cid)
            .map(|d| d.title().to_owned())
            .or_else(|| self.names.by_cid.get(&cid.to_string()).cloned());
        let tags = self.tags(cid);
        let tags = if tags.is_empty() {
            String::new()
        } else {
            format!(" [{}]", tags.join(", "))
        };
        match title {
            Some(t) => format!("**{t}**{tags} — `{cid}`"),
            None => format!("`{cid}`"),
        }
    }

    /// [`Self::label_md`] with the identifier on a line of its own. An
    /// identifier cannot be broken, and set inline it either overruns the
    /// measure or stretches every other word on the line to make room.
    fn label_block(&self, cid: &Cid) -> String {
        self.label_md(cid).replacen(" — `", "\\\n`", 1)
    }

    /// Every tag on the lineage `cid` belongs to. All of them: two keys
    /// tagging one lineage differently is a collision a viewpoint
    /// resolves, never this renderer (Section 9.1).
    fn tags(&self, cid: &Cid) -> Vec<String> {
        let genesis = crate::build::genesis(self.graph, cid);
        let mut tags = self.graph.tags_of(&genesis);
        if genesis != *cid {
            tags.extend(self.graph.tags_of(cid));
        }
        tags.sort();
        tags.dedup();
        tags
    }

    /// The current head of the lineage `cid` belongs to, under the
    /// author's own revisions and their delegates (Section 6.1). Where it
    /// has branched, the version this workspace names is preferred, then
    /// the lowest identifier, so the answer never depends on load order.
    pub(crate) fn head(&self, cid: &Cid) -> Cid {
        let view = self.graph.lineage(cid, Lineage::Authoritative);
        if let [only] = view.heads.as_slice() {
            return only.clone();
        }
        let mut heads = view.heads;
        heads.sort_by_key(ToString::to_string);
        heads
            .iter()
            .find(|h| self.names.by_cid.contains_key(&h.to_string()))
            .or(heads.first())
            .cloned()
            .unwrap_or_else(|| cid.clone())
    }

    /// The object a cited item resolves to: itself for a fixed citation,
    /// the lineage's current head for a lineage-bound one (Section 8).
    fn resolved(&self, item: &RawItem) -> Cid {
        if item.bind == "lineage" {
            self.head(&item.reference)
        } else {
            item.cited()
        }
    }

    pub(crate) fn claim(&self, cid: &Cid) -> Option<&ProseClaim> {
        self.graph.prose_claim(cid)
    }

    /// Labels of the keys holding an `authored` signature on `cid`, or of
    /// its author key, for the title page.
    fn signers(&self, cid: &Cid) -> Vec<String> {
        let mut keys: Vec<Cid> = self
            .graph
            .cids()
            .into_iter()
            .filter_map(|c| c.parse::<Cid>().ok())
            .filter_map(|c| self.graph.object(&c))
            .filter(|o| o.kind() == "sig")
            .filter(|o| o.body().get("target").and_then(Value::as_text) == Some(&cid.to_string()))
            .map(|o| o.author().clone())
            .collect();
        if let Some(object) = self.graph.object(cid) {
            keys.push(object.author().clone());
        }
        let mut labels = Vec::new();
        for key in keys {
            let label = self
                .graph
                .object(&key)
                .and_then(|k| k.body().get("label").and_then(Value::as_text))
                .map_or_else(|| format!("key {key}"), str::to_owned);
            if !labels.contains(&label) {
                labels.push(label);
            }
        }
        labels
    }
}

// ------------------------------------------------------------- the pipeline

struct Tool {
    /// The identity claim checked: the head of the lineage cited.
    claim: Cid,
    /// What the steps depend on: the lineage's genesis, or a version.
    cited: Cid,
    name: String,
    version: String,
    check: String,
    expect: String,
}

struct RepoFile {
    claim: Cid,
    blob: Cid,
    locator: String,
}

struct CommandSpec {
    program: String,
    /// The version prefix the program must satisfy; empty means any.
    requires: String,
    args: Vec<String>,
    cwd: Option<String>,
    outputs: Vec<String>,
}

struct Step {
    cid: Cid,
    commands: Vec<CommandSpec>,
    tools: BTreeSet<String>,
}

pub(crate) struct Pipeline {
    /// The environment the pipeline runs in, if it cites one: the head of
    /// the lineage its `Environment` section names.
    env: Option<Cid>,
    doc: Cid,
    method: Cid,
    steps: Vec<Step>,
    tools: Vec<Tool>,
    files: Vec<RepoFile>,
    objects: Vec<Cid>,
}

impl Pipeline {
    /// Every program the pipeline's steps run.
    pub(crate) fn programs(&self) -> BTreeSet<String> {
        self.steps
            .iter()
            .flat_map(|s| s.commands.iter().map(|c| c.program.clone()))
            .collect()
    }

    /// The pipeline a document names in its `Colophon`.
    fn of(ctx: &Ctx<'_>, doc: &Cid) -> Result<Self, String> {
        let object = ctx
            .graph
            .object(doc)
            .ok_or_else(|| format!("{doc} is not held here"))?;
        let pipeline_doc = sections(object)
            .into_iter()
            .filter(|s| s.heading.eq_ignore_ascii_case("colophon"))
            .flat_map(|s| s.items)
            .filter(|i| i.role == "background")
            .find_map(|i| {
                ctx.graph
                    .document(&ctx.resolved(&i))
                    .map(|d| d.cid().clone())
            })
            .ok_or_else(|| {
                format!(
                    "{} names no render pipeline: it needs a `Colophon` section \
                     citing one as `background`",
                    ctx.label(doc)
                )
            })?;
        Self::from_publet(ctx, &pipeline_doc)
    }

    /// A pipeline read from its own publet: the procedural claim it
    /// bundles, whose `depends` are the steps in order.
    pub(crate) fn from_publet(ctx: &Ctx<'_>, pipeline_doc: &Cid) -> Result<Self, String> {
        let method = ctx
            .graph
            .document(pipeline_doc)
            .into_iter()
            .flat_map(publet_graph::Document::items)
            .filter_map(|i| ctx.claim(&i.reference))
            .find(|c| c.class() == Class::Procedural && !c.depends().is_empty())
            .ok_or_else(|| {
                format!(
                    "{} cites no procedural claim whose `depends` lists its steps",
                    ctx.label(pipeline_doc)
                )
            })?;
        let closure = ctx
            .graph
            .depends_closure(method.cid())
            .map_err(|e| e.to_string())?;
        let (tools, files) = components(ctx, &closure);

        let mut steps = Vec::new();
        for step_cid in method.depends() {
            let step = ctx
                .claim(step_cid)
                .filter(|c| c.class() == Class::Procedural)
                .ok_or_else(|| {
                    format!(
                        "step {} is not a procedural claim held here",
                        ctx.label(step_cid)
                    )
                })?;
            let commands =
                commands_of(step).map_err(|e| format!("{}: {e}", ctx.label(step_cid)))?;
            let reach: BTreeSet<String> = ctx
                .graph
                .depends_closure(step_cid)
                .map_err(|e| e.to_string())?
                .iter()
                .map(ToString::to_string)
                .collect();
            let step_tools = tools
                .iter()
                .filter(|t| reach.contains(&t.cited.to_string()))
                .map(|t| t.name.clone())
                .collect();
            steps.push(Step {
                cid: step_cid.clone(),
                commands,
                tools: step_tools,
            });
        }

        let env = ctx.graph.object(pipeline_doc).and_then(|o| {
            sections(o)
                .into_iter()
                .filter(|s| s.heading.eq_ignore_ascii_case("environment"))
                .flat_map(|s| s.items)
                .find(|i| i.role == "background")
                .map(|i| ctx.resolved(&i))
        });
        let mut objects = vec![pipeline_doc.clone()];
        objects.extend(tools.iter().map(|t| t.claim.clone()));
        objects.extend(closure);
        objects.sort_by_key(ToString::to_string);
        objects.dedup();
        Ok(Self {
            env,
            doc: pipeline_doc.clone(),
            method: method.cid().clone(),
            steps,
            tools,
            files,
            objects,
        })
    }

    /// Objects lacking a valid `authored` signature from a trusted key.
    pub(crate) fn unsigned(&self, ctx: &Ctx<'_>, trusted: &BTreeSet<String>) -> Vec<String> {
        if trusted.is_empty() {
            return vec![
                "the workspace policy trusts no key, so no signature on the \
                 pipeline could count; set one with `pub policy --root=KEY`"
                    .to_owned(),
            ];
        }
        let sigs: Vec<&Object> = ctx
            .graph
            .cids()
            .into_iter()
            .filter_map(|c| c.parse::<Cid>().ok())
            .filter_map(|c| ctx.graph.object(&c))
            .filter(|o| o.kind() == "sig" && trusted.contains(&o.author().to_string()))
            .collect();
        // A draft is unsigned by design: it is this workspace's own and has
        // left nowhere. What is published must carry its signature.
        self.objects
            .iter()
            .filter(|cid| !ctx.names.drafts.contains(&cid.to_string()))
            .filter(|cid| {
                let Some(target) = ctx.graph.object(cid) else {
                    return true;
                };
                !sigs.iter().any(|sig| verifies(ctx.graph, sig, cid, target))
            })
            .map(|cid| {
                format!(
                    "{} carries no `{}` signature from a key the policy trusts",
                    ctx.label(cid),
                    crate::build::PURPOSE
                )
            })
            .collect()
    }

    /// Each program a step runs is identified, and its `check` prints the
    /// version its identity claim states. Returns what each check printed.
    fn check_tools(
        &self,
        ctx: &Ctx<'_>,
        bin: Option<&Path>,
        problems: &mut Vec<String>,
    ) -> Vec<(String, String)> {
        for step in &self.steps {
            for command in &step.commands {
                if !step.tools.contains(&command.program) {
                    problems.push(format!(
                        "step {} runs `{}`, which no identity claim it depends on names",
                        ctx.label(&step.cid),
                        command.program
                    ));
                }
            }
        }
        let used: BTreeSet<&str> = self
            .steps
            .iter()
            .flat_map(|s| s.commands.iter().map(|c| c.program.as_str()))
            .collect();
        // A step states the versions it accepts; the identity claim states
        // the version in use. Both must agree before anything runs.
        for step in &self.steps {
            for command in step.commands.iter().filter(|c| !c.requires.is_empty()) {
                for tool in self.tools.iter().filter(|t| t.name == command.program) {
                    if !satisfies(&tool.version, &command.requires) {
                        problems.push(format!(
                            "step {} requires {} {}, but {} states {}",
                            ctx.label(&step.cid),
                            command.program,
                            command.requires,
                            ctx.label(&tool.claim),
                            tool.version
                        ));
                    }
                }
            }
        }
        let mut observed = Vec::new();
        for tool in self.tools.iter().filter(|t| used.contains(t.name.as_str())) {
            let who = format!(
                "{} ({} {})",
                ctx.label(&tool.claim),
                tool.name,
                tool.version
            );
            let mut words = tool.check.split_whitespace();
            if words.next() != Some(tool.name.as_str()) {
                problems.push(format!(
                    "{who}: its `check` must invoke `{}` itself, not `{}`",
                    tool.name, tool.check
                ));
                continue;
            }
            let output = match program(&tool.name, bin)
                .and_then(|mut c| c.args(words).output().map_err(|e| e.to_string()))
            {
                Ok(o) => {
                    let mut text = String::from_utf8_lossy(&o.stdout).into_owned();
                    text.push_str(&String::from_utf8_lossy(&o.stderr));
                    text
                }
                Err(e) => {
                    problems.push(format!("{who}: `{}` could not run here: {e}", tool.check));
                    continue;
                }
            };
            let expect = if tool.expect.is_empty() {
                &tool.version
            } else {
                &tool.expect
            };
            if !output.contains(expect.as_str()) {
                problems.push(format!(
                    "{who}: the claim states {expect:?}, but `{}` on this machine printed \
                     {:?}; revise the identity claim, or install what it states",
                    tool.check,
                    output.trim()
                ));
            }
            observed.push((tool.name.clone(), output.trim().to_owned()));
        }
        observed
    }

    /// Each repository file the pipeline cites hashes to its claimed blob.
    fn check_files(&self, ctx: &Ctx<'_>, repo: &Path, problems: &mut Vec<String>) {
        for file in &self.files {
            let path = repo.join(&file.locator);
            match std::fs::read(&path) {
                Ok(bytes) if file.blob.verifies(&bytes) => {}
                Ok(_) => problems.push(format!(
                    "{}: {} no longer hashes to {}",
                    ctx.label(&file.claim),
                    path.display(),
                    file.blob
                )),
                Err(e) => problems.push(format!(
                    "{}: cannot read {}: {e}",
                    ctx.label(&file.claim),
                    path.display()
                )),
            }
        }
    }
}

/// The tools and repository files among a pipeline's claims: `archival`
/// tables with `name` and `version` columns, one tool per row, and claims
/// carrying a file whose `source` says where in the repository it lives.
fn components(ctx: &Ctx<'_>, closure: &[Cid]) -> (Vec<Tool>, Vec<RepoFile>) {
    // A step may depend on a tool's lineage rather than one version of its
    // identity claim; what is checked is the lineage's current head.
    let heads: Vec<(Cid, Cid)> = closure.iter().map(|c| (ctx.head(c), c.clone())).collect();
    let mut tools = Vec::new();
    let mut files = Vec::new();
    for (cid, cited) in &heads {
        let Some(claim) = ctx.claim(cid) else {
            continue;
        };
        match claim.data() {
            Some(Data::Table { columns, rows }) if claim.class() == Class::Archival => {
                let at = |name: &str| columns.iter().position(|c| c.name == name);
                let (Some(n), Some(v)) = (at("name"), at("version")) else {
                    continue;
                };
                for row in rows {
                    let cell = |i: Option<usize>| i.and_then(|i| row.get(i)).map(cell_text);
                    tools.push(Tool {
                        claim: cid.clone(),
                        cited: cited.clone(),
                        name: cell(Some(n)).unwrap_or_default(),
                        version: cell(Some(v)).unwrap_or_default(),
                        check: cell(at("check")).unwrap_or_default(),
                        expect: cell(at("expect")).unwrap_or_default(),
                    });
                }
            }
            Some(Data::File { blob, .. }) => {
                if let Some(locator) = claim.sources().iter().find_map(|s| s.locator.clone()) {
                    files.push(RepoFile {
                        claim: cid.clone(),
                        blob: blob.clone(),
                        locator,
                    });
                }
            }
            _ => {}
        }
    }
    (tools, files)
}

/// Whether `sig` is a valid `authored` signature on `target` by a key
/// object held here.
fn verifies(graph: &Graph, sig: &Object, cid: &Cid, target: &Object) -> bool {
    let body = sig.body();
    if body.get("target").and_then(Value::as_text) != Some(cid.to_string().as_str()) {
        return false;
    }
    let (Some(value), Some(purpose)) = (
        body.get("value").and_then(Value::as_bytes),
        body.get("purpose").and_then(Value::as_text),
    ) else {
        return false;
    };
    let Some(pubkey) = graph
        .object(sig.author())
        .filter(|k| k.kind() == "key")
        .and_then(|k| k.body().get("pubkey").and_then(Value::as_bytes))
    else {
        return false;
    };
    publet_core::verify(
        SigAlg::Ed25519,
        pubkey,
        value,
        purpose,
        crate::build::PURPOSE,
        target.bytes(),
    )
    .is_ok()
}

/// The keys the workspace policy names as roots.
pub(crate) fn trusted_keys(ws: &Workspace, graph: &Graph) -> BTreeSet<String> {
    ws.policy_cid()
        .and_then(|p| graph.object(&p).cloned())
        .and_then(|p| match p.body().get("roots") {
            Some(Value::Array(roots)) => Some(
                roots
                    .iter()
                    .filter_map(|r| r.get("key").and_then(Value::as_text))
                    .map(str::to_owned)
                    .collect(),
            ),
            _ => None,
        })
        .unwrap_or_default()
}

fn commands_of(step: &ProseClaim) -> Result<Vec<CommandSpec>, String> {
    let Some(Data::Table { columns, rows }) = step.data() else {
        return Err("a step carries its commands as a table in `data`".to_owned());
    };
    let at = |name: &str| columns.iter().position(|c| c.name == name);
    let (Some(p), Some(a)) = (at("program"), at("args")) else {
        return Err("a step's table needs `program` and `args` columns".to_owned());
    };
    let words = |s: Option<String>| -> Vec<String> {
        s.map(|s| s.split_whitespace().map(str::to_owned).collect())
            .unwrap_or_default()
    };
    Ok(rows
        .iter()
        .map(|row| {
            let cell = |i: Option<usize>| {
                i.and_then(|i| row.get(i))
                    .map(cell_text)
                    .filter(|s| !s.is_empty())
            };
            CommandSpec {
                program: cell(Some(p)).unwrap_or_default(),
                requires: cell(at("requires")).unwrap_or_default(),
                args: words(cell(Some(a))),
                cwd: cell(at("cwd")),
                outputs: words(cell(at("outputs"))),
            }
        })
        .collect())
}

/// Whether `version` satisfies `requires`, written either as a prefix --
/// `0.15` accepts `0.15` and `0.15.2`, not `0.150` -- or as a minimum,
/// `1.80+`, which accepts any version not below it, compared numerically
/// part by part.
pub(crate) fn satisfies(version: &str, requires: &str) -> bool {
    if let Some(minimum) = requires.strip_suffix('+') {
        let parts =
            |v: &str| -> Option<Vec<u64>> { v.split('.').map(|p| p.parse::<u64>().ok()).collect() };
        return match (parts(version), parts(minimum)) {
            (Some(have), Some(need)) => have >= need,
            _ => false,
        };
    }
    requires.is_empty()
        || version == requires
        || version
            .strip_prefix(requires)
            .is_some_and(|rest| rest.starts_with('.'))
}

pub(crate) fn cell_text(cell: &publet_graph::Cell) -> String {
    match cell {
        publet_graph::Cell::Text(t) => t.clone(),
        publet_graph::Cell::Uint(n) => n.to_string(),
        publet_graph::Cell::Nint(n) => format!("-{}", u128::from(*n) + 1),
        _ => String::new(),
    }
}

/// Replace the placeholders; any other `{...}` is refused rather than
/// passed through, so a typo cannot reach a program as a literal.
pub(crate) fn substitute(text: &str, values: &[(&str, String)]) -> Result<String, String> {
    let mut out = text.to_owned();
    for (key, value) in values {
        out = out.replace(key, value);
    }
    // `{host:NAME}`: where NAME is on this host -- how an environment links
    // in what it requires the host to provide.
    while let Some(start) = out.find("{host:") {
        let end = out[start..]
            .find('}')
            .map(|e| start + e)
            .ok_or_else(|| format!("`{text}` opens a placeholder it never closes"))?;
        let name = &out[start + 6..end];
        let found = which(name)
            .ok_or_else(|| format!("`{name}` is not on this host's PATH, as `{text}` needs"))?;
        out.replace_range(start..=end, &found.display().to_string());
    }
    if out.contains('{') && out.contains('}') {
        return Err(format!(
            "`{text}` holds a placeholder other than {}",
            PLACEHOLDERS.join(", ")
        ));
    }
    Ok(out)
}

fn substitute_all(args: &[String], values: &[(&str, String)]) -> Result<Vec<String>, String> {
    args.iter().map(|a| substitute(a, values)).collect()
}

fn record(
    doc: &Cid,
    pipeline: &Pipeline,
    observed: &[(String, String)],
    out: &Path,
    outputs: &[(usize, PathBuf, Cid)],
) -> Result<String, String> {
    let relative = |p: &Path| p.strip_prefix(out).unwrap_or(p).display().to_string();
    let last = outputs.iter().map(|(i, _, _)| *i).max();
    let json = serde_json::json!({
        "doc": doc.to_string(),
        "pipeline": pipeline.doc.to_string(),
        "method": pipeline.method.to_string(),
        "out": out.display().to_string(),
        "tools": observed.iter().map(|(name, seen)| serde_json::json!({
            "name": name, "observed": seen,
        })).collect::<Vec<_>>(),
        "outputs": outputs.iter().map(|(_, path, cid)| serde_json::json!({
            "path": relative(path), "cid": cid.to_string(),
        })).collect::<Vec<_>>(),
        "final": outputs.iter()
            .filter(|(i, _, _)| Some(*i) == last)
            .map(|(_, path, _)| relative(path))
            .collect::<Vec<_>>(),
    });
    serde_json::to_string_pretty(&json)
        .map(|s| s + "\n")
        .map_err(|e| e.to_string())
}

/// The rendered document's `created`, as Unix seconds.
fn epoch_of(graph: &Graph, doc: &Cid) -> u64 {
    graph
        .object(doc)
        .and_then(|o| crate::corpus::epoch_of(o.created()))
        .unwrap_or(0)
}

/// A rendering being pinned: what was rendered, by which method, into
/// which outputs, with which tools, and the final output kept as a blob.
pub(crate) struct Rendering {
    pub(crate) doc: Cid,
    pub(crate) method: Cid,
    pub(crate) outputs: Vec<(String, String)>,
    pub(crate) tools: Vec<(String, String)>,
    pub(crate) blob: Cid,
    pub(crate) size: u64,
}

/// The two objects that pin a rendering: an `archival` claim stating what
/// it produced, and a `settled` annotation on the method that produced it
/// (Section 7.2), whose `data` is the kept output.
///
/// # Errors
///
/// Returns a message if either object cannot be built.
pub(crate) fn rendering_objects(
    author: &str,
    created: &str,
    r: &Rendering,
) -> Result<Vec<(Cid, Vec<u8>)>, String> {
    let mut columns = Vec::new();
    for name in ["kind", "name", "value"] {
        let mut c = BTreeMap::new();
        c.insert("name".to_owned(), Value::Text(name.to_owned()));
        columns.push(Value::Map(c));
    }
    let row = |k: &str, n: &str, v: &str| {
        Value::Array(vec![
            Value::Text(k.to_owned()),
            Value::Text(n.to_owned()),
            Value::Text(v.to_owned()),
        ])
    };
    let mut rows: Vec<Value> = r
        .outputs
        .iter()
        .map(|(path, cid)| row("output", path, cid))
        .collect();
    rows.extend(r.tools.iter().map(|(name, seen)| row("tool", name, seen)));
    let mut data = BTreeMap::new();
    data.insert("columns".to_owned(), Value::Array(columns));
    data.insert("rows".to_owned(), Value::Array(rows));

    let mut body = BTreeMap::new();
    body.insert("class".to_owned(), Value::Text("archival".to_owned()));
    body.insert("lang".to_owned(), Value::Text("en".to_owned()));
    body.insert(
        "content".to_owned(),
        Value::Text(format!(
            "Rendering {} by the method {} produced these outputs with these tools.",
            r.doc, r.method
        )),
    );
    body.insert(
        "scope".to_owned(),
        crate::compose::scope_value(&format!("the accepted rendering of {}", r.doc)),
    );
    body.insert(
        "depends".to_owned(),
        crate::compose::text_array(&[r.doc.to_string(), r.method.to_string()]),
    );
    body.insert("evidence".to_owned(), Value::Array(Vec::new()));
    body.insert("data".to_owned(), Value::Map(data));
    let (result, result_bytes, _) = crate::compose::build_claim(author, created, body)?;

    let mut value = BTreeMap::new();
    value.insert("method".to_owned(), Value::Text(r.method.to_string()));
    value.insert("outcome".to_owned(), Value::Text("consistent".to_owned()));
    value.insert("result".to_owned(), Value::Text(result.to_string()));
    value.insert("data".to_owned(), Value::Text(r.blob.to_string()));
    value.insert("size".to_owned(), Value::Uint(r.size));
    let settled = Object::builder("claim.annotation", author)
        .created(created)
        .field("kind", Value::Text("settled".to_owned()))
        .field("target", Value::Text(r.method.to_string()))
        .field(
            "scope",
            crate::compose::scope_value(&format!("rendering {}", r.doc)),
        )
        .field("value", Value::Map(value))
        .build()
        .map_err(|e| e.to_string())?;
    let settled_cid = Cid::of(&settled, HashAlg::Sha2_256);
    Ok(vec![(result, result_bytes), (settled_cid, settled)])
}

/// A pinned rendering, read back from the graph.
pub(crate) struct Pinned {
    pub(crate) settled: Cid,
    pub(crate) doc: Cid,
    pub(crate) method: Cid,
    pub(crate) created: String,
    pub(crate) outputs: Vec<(String, String)>,
    pub(crate) tools: Vec<(String, String)>,
    pub(crate) blob: Option<Cid>,
}

/// Every rendering pinned in `graph`, oldest first.
pub(crate) fn renderings(graph: &Graph) -> Vec<Pinned> {
    let mut out = Vec::new();
    for text in graph.cids() {
        let Ok(cid) = text.parse::<Cid>() else {
            continue;
        };
        let Some(object) = graph.object(&cid) else {
            continue;
        };
        let body = object.body();
        if object.kind() != "claim.annotation"
            || body.get("kind").and_then(Value::as_text) != Some("settled")
        {
            continue;
        }
        let value = body.get("value");
        let field = |k: &str| {
            value
                .and_then(|v| v.get(k))
                .and_then(Value::as_text)
                .and_then(|t| t.parse::<Cid>().ok())
        };
        let (Some(method), Some(result)) = (field("method"), field("result")) else {
            continue;
        };
        let Some(claim) = graph.prose_claim(&result) else {
            continue;
        };
        let Some(Data::Table { columns, rows }) = claim.data() else {
            continue;
        };
        let names: Vec<&str> = columns.iter().map(|c| c.name.as_str()).collect();
        let ([doc, _],) = (claim.depends(),) else {
            continue;
        };
        if names != ["kind", "name", "value"] {
            continue;
        }
        let pick = |kind: &str| -> Vec<(String, String)> {
            rows.iter()
                .filter(|r| r.first().map(cell_text).as_deref() == Some(kind))
                .map(|r| {
                    (
                        r.get(1).map(cell_text).unwrap_or_default(),
                        r.get(2).map(cell_text).unwrap_or_default(),
                    )
                })
                .collect()
        };
        out.push(Pinned {
            settled: cid,
            doc: doc.clone(),
            method,
            created: object.created().to_owned(),
            outputs: pick("output"),
            tools: pick("tool"),
            blob: field("data"),
        });
    }
    out.sort_by(|a, b| {
        (a.created.as_str(), a.settled.to_string())
            .cmp(&(b.created.as_str(), b.settled.to_string()))
    });
    out
}

/// The newest rendering pinned for exactly this document version.
pub(crate) fn active_rendering(graph: &Graph, doc: &Cid) -> Option<Pinned> {
    renderings(graph).into_iter().rev().find(|r| r.doc == *doc)
}

// ------------------------------------------------------------- sections

struct RawItem {
    reference: Cid,
    at: Option<Cid>,
    bind: String,
    role: String,
    gloss: Option<String>,
}

impl RawItem {
    /// The object a reader sees: the head read, for a lineage citation.
    fn cited(&self) -> Cid {
        self.at.clone().unwrap_or_else(|| self.reference.clone())
    }
}

struct Section {
    heading: String,
    items: Vec<RawItem>,
}

/// A document's sections with their headings, which the typed
/// [`publet_graph::Document`] flattens away.
fn sections(object: &Object) -> Vec<Section> {
    let Some(Value::Array(raw)) = object.body().get("sections") else {
        return Vec::new();
    };
    raw.iter()
        .map(|s| Section {
            heading: s
                .get("heading")
                .and_then(Value::as_text)
                .unwrap_or_default()
                .to_owned(),
            items: match s.get("items") {
                Some(Value::Array(items)) => items
                    .iter()
                    .filter_map(|i| {
                        let text = |k: &str| i.get(k).and_then(Value::as_text);
                        Some(RawItem {
                            reference: text("ref")?.parse().ok()?,
                            at: text("at").and_then(|a| a.parse().ok()),
                            bind: text("bind").unwrap_or("object").to_owned(),
                            role: text("role").unwrap_or("assert").to_owned(),
                            gloss: text("gloss").map(str::to_owned),
                        })
                    })
                    .collect(),
                _ => Vec::new(),
            },
        })
        .collect()
}

// ------------------------------------------------------------------ myst

struct Page {
    doc: Cid,
    title: String,
    description: String,
    tags: Vec<String>,
    sections: Vec<Section>,
}

impl Page {
    fn of(ctx: &Ctx<'_>, doc: &Cid) -> Result<Self, String> {
        let object = ctx
            .graph
            .object(doc)
            .filter(|o| o.kind() == "doc")
            .ok_or_else(|| format!("{doc} is not a document held here"))?;
        let title = object
            .body()
            .get("title")
            .and_then(Value::as_text)
            .unwrap_or_default()
            .to_owned();
        let sections = sections(object);
        let claims: Vec<&ProseClaim> = sections
            .iter()
            .flat_map(|s| &s.items)
            .filter_map(|i| ctx.claim(&i.cited()))
            .collect();
        let description = object
            .body()
            .get("abstract")
            .and_then(Value::as_text)
            .and_then(|a| a.parse::<Cid>().ok())
            .and_then(|a| ctx.claim(&a))
            .or_else(|| {
                claims
                    .iter()
                    .copied()
                    .find(|c| c.class() == Class::Definitional)
            })
            .or_else(|| claims.first().copied())
            .map(|c| c.content().to_owned())
            .unwrap_or_default();
        Ok(Self {
            doc: doc.clone(),
            title,
            description,
            tags: ctx.tags(doc),
            sections,
        })
    }

    fn myst_yml(&self, authors: &[String], template: &str, export: &str) -> Result<String, String> {
        let q = |s: &str| serde_json::to_string(s).map_err(|e| e.to_string());
        let mut out = String::from("# Generated by `pub myst`; edit the publet, not this file.\n");
        out.push_str("version: 1\nproject:\n");
        let _ = writeln!(out, "  title: {}", q(&self.title)?);
        let _ = writeln!(out, "  description: {}", q(&self.description)?);
        out.push_str("  keywords:\n");
        for tag in &self.tags {
            let _ = writeln!(out, "    - {}", q(tag)?);
        }
        out.push_str("    - publet\n  authors:\n");
        for author in authors {
            let _ = writeln!(out, "    - name: {}", q(author)?);
        }
        out.push_str("  exports:\n    - format: typst\n");
        let _ = writeln!(out, "      template: {}", q(template)?);
        let _ = writeln!(out, "      output: {}", q(export)?);
        let _ = writeln!(out, "site:\n  title: {}\n  nav: []", q(&self.title)?);
        Ok(out)
    }

    fn index_md(&self, ctx: &Ctx<'_>) -> Result<String, String> {
        let q = |s: &str| serde_json::to_string(s).map_err(|e| e.to_string());
        let mut out = String::from("---\n");
        let _ = writeln!(out, "title: {}", q(&self.title)?);
        let _ = writeln!(out, "description: {}", q(&self.description)?);
        out.push_str("---\n\n");
        let _ = writeln!(
            out,
            "*{}* — `{}`",
            if self.tags.is_empty() {
                "untagged".to_owned()
            } else {
                self.tags.join(", ")
            },
            self.doc
        );

        for section in &self.sections {
            let _ = writeln!(out, "\n## {}", section.heading);
            for item in &section.items {
                out.push('\n');
                if let Some(gloss) = &item.gloss {
                    let _ = write!(out, "{}\n\n", text_md(gloss));
                }
                out.push_str(&item_md(ctx, item));
            }
        }
        Ok(out)
    }
}

/// Text from a claim, made inert for `MyST`: `@name` would otherwise be
/// read as a citation of a reference nobody declared.
fn text_md(text: &str) -> String {
    text.replace('@', "\\@")
}

fn role_label(role: &str) -> &'static str {
    match role {
        "quote" => "Quoted",
        "contrast" => "Contrasted",
        "background" => "Background",
        "counterpoint" => "Counterpoint",
        _ => "Asserted",
    }
}

/// One cited item: a claim quoted with its class, or a publet named by
/// title and tag -- and for a render pipeline, what it is made of.
fn item_md(ctx: &Ctx<'_>, item: &RawItem) -> String {
    // Section 8: show the resolved object, keep `at` reachable, and say
    // when the two differ.
    let cited = ctx.resolved(item);
    let lineage = match (&item.at, item.bind.as_str()) {
        (Some(at), "lineage") if *at != cited => {
            format!(" (cited as read at `{at}`; this is the current version)")
        }
        (_, "lineage") => " (the version cited is the current one)".to_owned(),
        _ => String::new(),
    };
    if let Some(claim) = ctx.claim(&cited) {
        let quoted = claim.content().lines().fold(String::new(), |mut acc, l| {
            let _ = writeln!(acc, "> {}", text_md(l));
            acc
        });
        return format!(
            "{quoted}\n*{}, {}* — `{cited}`{lineage}\n",
            role_label(&item.role),
            claim.class().id()
        );
    }
    if ctx.graph.document(&cited).is_some() {
        let mut out = format!(
            "*{}:* {}{lineage}\n",
            role_label(&item.role),
            ctx.label_block(&cited)
        );
        if let Ok(pipeline) = Pipeline::from_publet(ctx, &cited) {
            out.push_str(&pipeline.colophon_md(ctx));
        }
        return out;
    }
    format!("*{}* — `{cited}`{lineage}\n", role_label(&item.role))
}

impl Pipeline {
    /// The components and their help, as a reader of the colophon wants
    /// them: what each is, which version, and where its documentation is.
    fn colophon_md(&self, ctx: &Ctx<'_>) -> String {
        // A component is named by the publet bundling its identity claim,
        // so the table reads as names a person recognizes.
        let publet_of = |claim: &Cid| {
            ctx.graph
                .documents()
                .find(|d| d.items().iter().any(|i| i.reference == *claim))
                .map_or_else(|| format!("`{claim}`"), |d| ctx.label_block(d.cid()))
        };
        let mut out = String::from("\nComponents:\n\n");
        for tool in &self.tools {
            let _ = writeln!(
                out,
                "- {} {} — {}",
                text_md(&tool.name),
                text_md(&tool.version),
                publet_of(&tool.claim).replacen("\\\n", "\\\n  ", 1)
            );
        }
        // Help: publets citing a component's identity claim, and the
        // publets those cite in turn.
        let identity: BTreeSet<String> = self.tools.iter().map(|t| t.claim.to_string()).collect();
        let mut help = Vec::new();
        for component in ctx.graph.documents() {
            let cites_identity = component
                .items()
                .iter()
                .any(|i| identity.contains(&i.reference.to_string()));
            if !cites_identity {
                continue;
            }
            for item in component.items() {
                let target = item.at.clone().unwrap_or_else(|| item.reference.clone());
                let Some(helpdoc) = ctx.graph.document(&target) else {
                    continue;
                };
                let detail = helpdoc
                    .items()
                    .iter()
                    .filter_map(|i| ctx.claim(&i.reference))
                    .map(|c| text_md(c.content()))
                    .next()
                    .unwrap_or_default();
                let line = format!(
                    "- {}\\\n  {detail}\n",
                    ctx.label_block(&target).replacen("\\\n", "\\\n  ", 1)
                );
                if !help.contains(&line) {
                    help.push(line);
                }
            }
        }
        if !help.is_empty() {
            out.push_str("\nHelp:\n\n");
            out.extend(help);
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_requirement_is_a_version_prefix_at_a_dot_boundary() {
        assert!(satisfies("0.15", "0.15"));
        assert!(satisfies("0.15.2", "0.15"));
        assert!(satisfies("0.15.2", ""));
        assert!(!satisfies("0.150", "0.15"));
        assert!(!satisfies("0.16.0", "0.15"));
        assert!(satisfies("1.98.1", "1.80+"));
        assert!(satisfies("22.22.1", "22+"));
        assert!(!satisfies("1.79.0", "1.80+"));
        assert!(!satisfies("abc", "1+"));
    }

    #[test]
    fn a_handle_in_claim_text_is_not_read_as_a_citation() {
        assert_eq!(
            text_md("the @preview/x package"),
            "the \\@preview/x package"
        );
    }

    #[test]
    fn only_the_named_placeholders_are_substituted() {
        let values = [
            ("{doc}", "D".to_owned()),
            ("{out}", "/o".to_owned()),
            ("{repo}", "/r".to_owned()),
        ];
        assert_eq!(
            substitute("{out}/main.typ", &values).unwrap(),
            "/o/main.typ"
        );
        assert_eq!(substitute("--doc={doc}", &values).unwrap(), "--doc=D");
        assert!(substitute("{home}/x", &values).is_err());
    }

    #[test]
    fn arguments_are_words_never_a_shell_line() {
        let args = substitute_all(&["a;".to_owned(), "$(x)".to_owned()], &[]).unwrap();
        // Passed through as literal argv entries, which no shell ever sees.
        assert_eq!(args, vec!["a;".to_owned(), "$(x)".to_owned()]);
    }
}
