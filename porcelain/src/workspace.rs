//! The local workspace: a store, a policy, and where the peer is.
//!
//! Everything a reader does happens here. R16 makes the replica the normal
//! place to read from, so the workspace is the default subject of every
//! command rather than a cache in front of a server.

use std::path::{Path, PathBuf};

use publet_core::Cid;
use publet_store::Store;

/// Where a workspace keeps its state.
pub(crate) const DIR: &str = ".publet";

/// How a command obtained what it is showing (Section 14.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Mode {
    /// Read from a local replica. No request was issued, so nothing was
    /// disclosed to anyone.
    Local,
    /// Read by asking a peer. The peer learns exactly which proposition was
    /// requested, and a supplied policy discloses the reader's trust roots.
    Query,
}

impl Mode {
    /// The label shown to the user.
    ///
    /// Section 14.3 requires the mode in use to be visible rather than
    /// assumed: the privacy properties of local-first operation are real,
    /// and a reader who does not know which mode they are in cannot know
    /// whether they have them.
    #[must_use]
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Local => "local",
            Self::Query => "query",
        }
    }

    /// A one-line note explaining what the mode discloses.
    #[must_use]
    pub(crate) fn note(self) -> &'static str {
        match self {
            Self::Local => "read from your replica; nothing was disclosed",
            Self::Query => "asked a peer; it learns which object you requested (Section 14.3)",
        }
    }
}

/// A workspace rooted at a directory.
pub(crate) struct Workspace {
    root: PathBuf,
}

impl Workspace {
    /// Open, or report that there is none here.
    ///
    /// # Errors
    ///
    /// Returns a message naming what to run if no workspace is present.
    pub(crate) fn open(start: &Path) -> Result<Self, String> {
        let root = start.join(DIR);
        if !root.is_dir() {
            return Err(format!(
                "no workspace here; run `pub init` to create one in {}",
                start.display()
            ));
        }
        Ok(Self { root })
    }

    /// Create one.
    ///
    /// # Errors
    ///
    /// Returns a message if the directory cannot be created.
    pub(crate) fn create(start: &Path) -> Result<Self, String> {
        let root = start.join(DIR);
        std::fs::create_dir_all(&root).map_err(|e| e.to_string())?;
        Ok(Self { root })
    }

    /// The object store.
    ///
    /// # Errors
    ///
    /// Returns a message if the store cannot be opened.
    pub(crate) fn store(&self) -> Result<Store, String> {
        Store::open(&self.root.join("objects.redb")).map_err(|e| e.to_string())
    }

    /// Read a configuration value.
    #[must_use]
    pub(crate) fn get(&self, key: &str) -> Option<String> {
        let text = std::fs::read_to_string(self.root.join("config")).ok()?;
        for line in text.lines() {
            if let Some((k, v)) = line.split_once('=')
                && k.trim() == key
            {
                return Some(v.trim().to_owned());
            }
        }
        None
    }

    /// Write a configuration value.
    ///
    /// # Errors
    ///
    /// Returns a message if the file cannot be written.
    pub(crate) fn set(&self, key: &str, value: &str) -> Result<(), String> {
        let path = self.root.join("config");
        let mut lines: Vec<String> = std::fs::read_to_string(&path)
            .unwrap_or_default()
            .lines()
            .filter(|l| !l.starts_with(&format!("{key}=")))
            .map(ToOwned::to_owned)
            .collect();
        lines.push(format!("{key}={value}"));
        lines.sort();
        std::fs::write(path, lines.join("\n") + "\n").map_err(|e| e.to_string())
    }

    /// The policy this workspace evaluates under, if one is configured.
    #[must_use]
    pub(crate) fn policy_cid(&self) -> Option<Cid> {
        self.get("policy").and_then(|c| c.parse().ok())
    }

    /// Which mode a read of `cid` would use.
    ///
    /// Holding the object locally is what makes the local path available;
    /// otherwise the only way to see it is to ask someone.
    #[must_use]
    pub(crate) fn mode_for(store: &Store, cid: &Cid) -> Mode {
        if store.contains(cid).unwrap_or(false) {
            Mode::Local
        } else {
            Mode::Query
        }
    }
}
