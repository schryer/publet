//! Object persistence with declared sets, tombstones, and integrity scans.
//!
//! Two rules from Section 13.2 are enforced here rather than left to
//! callers, because both fail silently otherwise:
//!
//! * **Stored bytes always match their identifier.** Verified on write and
//!   re-verified by [`Store::scan`], so a corrupted file is found rather
//!   than served.
//! * **A declared member cannot be deleted silently.** Ceasing to serve
//!   something inside a declared set requires a published tombstone.
//!   Garbage collection therefore cannot touch declared members at all.

use std::collections::BTreeSet;
use std::path::Path;

use publet_core::{Cid, HashAlg, Object};
use publet_merkle::membership::Membership;
use redb::{
    Database, ReadableDatabase as _, ReadableTable as _, ReadableTableMetadata as _,
    TableDefinition,
};

use crate::StoreError;

/// Objects, keyed by identifier.
const OBJECTS: TableDefinition<'_, &str, &[u8]> = TableDefinition::new("objects");
/// Domain manifests the node has declared it serves.
const DECLARED: TableDefinition<'_, &str, &[u8]> = TableDefinition::new("declared");
/// Membership of each declared domain, as newline-separated identifiers.
const MEMBERS: TableDefinition<'_, &str, &str> = TableDefinition::new("members");
/// Tombstones, keyed by the identifier they disclose the removal of.
const TOMBSTONES: TableDefinition<'_, &str, &[u8]> = TableDefinition::new("tombstones");

/// An object whose stored bytes no longer match its key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Corruption {
    /// The identifier the object is stored under.
    pub stored_as: String,
    /// The identifier its bytes actually have.
    pub actual: String,
}

/// A persistent object store.
#[derive(Debug)]
pub struct Store {
    db: Database,
}

impl Store {
    /// Open or create a store at `path`.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError::Locked`] if another process holds the store,
    /// or [`StoreError::Database`] if the file cannot be opened.
    pub fn open(path: &Path) -> Result<Self, StoreError> {
        let db = Database::create(path).map_err(|e| match e {
            redb::DatabaseError::DatabaseAlreadyOpen => StoreError::Locked {
                path: path.display().to_string(),
            },
            other => StoreError::Database(other.to_string()),
        })?;
        // Creating the tables up front means every read path can assume
        // they exist, rather than treating "absent" and "empty" separately.
        let tx = db.begin_write()?;
        {
            let _ = tx.open_table(OBJECTS)?;
            let _ = tx.open_table(DECLARED)?;
            let _ = tx.open_table(MEMBERS)?;
            let _ = tx.open_table(TOMBSTONES)?;
        }
        tx.commit()?;
        Ok(Self { db })
    }

    /// Store an object, verifying its bytes against `cid`.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError::IdentifierMismatch`] if the bytes do not hash
    /// to `cid`.
    pub fn put(&self, cid: &Cid, bytes: &[u8]) -> Result<(), StoreError> {
        if !cid.verifies(bytes) {
            return Err(StoreError::IdentifierMismatch {
                claimed: cid.to_string(),
                actual: Cid::of(bytes, cid.alg()).to_string(),
            });
        }
        let tx = self.db.begin_write()?;
        {
            let mut table = tx.open_table(OBJECTS)?;
            table.insert(cid.to_string().as_str(), bytes)?;
        }
        tx.commit()?;
        Ok(())
    }

    /// Retrieve an object.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError::Database`] on a read failure.
    pub fn get(&self, cid: &Cid) -> Result<Option<Vec<u8>>, StoreError> {
        let tx = self.db.begin_read()?;
        let table = tx.open_table(OBJECTS)?;
        Ok(table
            .get(cid.to_string().as_str())?
            .map(|v| v.value().to_vec()))
    }

    /// Whether an object is held.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError::Database`] on a read failure.
    pub fn contains(&self, cid: &Cid) -> Result<bool, StoreError> {
        Ok(self.get(cid)?.is_some())
    }

    /// How many objects are held.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError::Database`] on a read failure.
    pub fn len(&self) -> Result<u64, StoreError> {
        let tx = self.db.begin_read()?;
        Ok(tx.open_table(OBJECTS)?.len()?)
    }

    /// Whether the store holds nothing.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError::Database`] on a read failure.
    pub fn is_empty(&self) -> Result<bool, StoreError> {
        Ok(self.len()? == 0)
    }

    /// Every identifier held, in sorted order.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError::Database`] on a read failure.
    pub fn cids(&self) -> Result<Vec<String>, StoreError> {
        let tx = self.db.begin_read()?;
        let table = tx.open_table(OBJECTS)?;
        let mut out = Vec::new();
        for entry in table.iter()? {
            let (key, _) = entry?;
            out.push(key.value().to_owned());
        }
        Ok(out)
    }

    /// Declare that this node serves a domain.
    ///
    /// The domain's manifest must already be held, and `members` must hash
    /// to the snapshot root that manifest declares. A domain's membership is
    /// fixed by the domain, not by whatever the node happens to have: a
    /// declaration derived from local inventory would invert that, and
    /// nothing downstream could detect the difference.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError::ManifestNotHeld`] if the manifest is absent,
    /// [`StoreError::MalformedManifest`] if it cannot be read, or
    /// [`StoreError::MembershipMismatch`] if the members do not match it.
    pub fn declare(&self, domain: &Cid, members: &[String]) -> Result<(), StoreError> {
        let manifest_bytes = self
            .get(domain)?
            .ok_or_else(|| StoreError::ManifestNotHeld {
                domain: domain.to_string(),
            })?;

        let object = Object::parse(&manifest_bytes)
            .map_err(|e| StoreError::MalformedManifest {
                domain: domain.to_string(),
                reason: e.to_string(),
            })?
            .verify(domain)
            .map_err(|e| StoreError::MalformedManifest {
                domain: domain.to_string(),
                reason: e.to_string(),
            })?;

        let manifest = publet_domain::Manifest::from_object(object.object()).map_err(|e| {
            StoreError::MalformedManifest {
                domain: domain.to_string(),
                reason: e.to_string(),
            }
        })?;

        let membership = Membership::new(members.iter().cloned());
        let computed = Cid::from_digest(HashAlg::Sha2_256, &membership.root());
        if computed.as_ref() != Some(&manifest.snapshot) {
            return Err(StoreError::MembershipMismatch {
                computed: computed.map_or_else(|| "<malformed>".to_owned(), |c| c.to_string()),
                expected: manifest.snapshot.to_string(),
            });
        }

        let tx = self.db.begin_write()?;
        {
            let mut declared = tx.open_table(DECLARED)?;
            declared.insert(domain.to_string().as_str(), manifest_bytes.as_slice())?;
            let mut table = tx.open_table(MEMBERS)?;
            table.insert(domain.to_string().as_str(), members.join("\n").as_str())?;
        }
        tx.commit()?;
        Ok(())
    }

    /// The domains this node has declared, in sorted order.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError::Database`] on a read failure.
    pub fn declared(&self) -> Result<Vec<String>, StoreError> {
        let tx = self.db.begin_read()?;
        let table = tx.open_table(DECLARED)?;
        let mut out = Vec::new();
        for entry in table.iter()? {
            let (key, _) = entry?;
            out.push(key.value().to_owned());
        }
        Ok(out)
    }

    /// Everything a declared set commits this node to holding.
    ///
    /// This is the members of every declared domain *and the manifests
    /// themselves*. A manifest is what fixes its domain's membership, so a
    /// node that collected one would lose the ability to describe what it
    /// serves, and its declaration would become unreadable.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError::Database`] on a read failure.
    pub fn declared_members(&self) -> Result<BTreeSet<String>, StoreError> {
        let tx = self.db.begin_read()?;
        let mut out = BTreeSet::new();

        let members = tx.open_table(MEMBERS)?;
        for entry in members.iter()? {
            let (_, value) = entry?;
            out.extend(
                value
                    .value()
                    .split('\n')
                    .filter(|s| !s.is_empty())
                    .map(ToOwned::to_owned),
            );
        }

        let declared = tx.open_table(DECLARED)?;
        for entry in declared.iter()? {
            let (key, _) = entry?;
            out.insert(key.value().to_owned());
        }

        Ok(out)
    }

    /// Record a tombstone disclosing that an object is no longer served.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] if the tombstone cannot be written.
    pub fn tombstone(&self, target: &Cid, tombstone: &[u8]) -> Result<(), StoreError> {
        let tx = self.db.begin_write()?;
        {
            let mut table = tx.open_table(TOMBSTONES)?;
            table.insert(target.to_string().as_str(), tombstone)?;
        }
        tx.commit()?;
        Ok(())
    }

    /// Whether a tombstone has been published for an object.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError::Database`] on a read failure.
    pub fn is_tombstoned(&self, target: &Cid) -> Result<bool, StoreError> {
        let tx = self.db.begin_read()?;
        let table = tx.open_table(TOMBSTONES)?;
        Ok(table.get(target.to_string().as_str())?.is_some())
    }

    /// Remove an object.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError::UndisclosedRemoval`] if the object is a member
    /// of a declared set and no tombstone has been published for it.
    pub fn remove(&self, cid: &Cid) -> Result<bool, StoreError> {
        let key = cid.to_string();
        if self.declared_members()?.contains(&key) && !self.is_tombstoned(cid)? {
            return Err(StoreError::UndisclosedRemoval { cid: key });
        }
        let tx = self.db.begin_write()?;
        let removed;
        {
            let mut table = tx.open_table(OBJECTS)?;
            removed = table.remove(key.as_str())?.is_some();
        }
        tx.commit()?;
        Ok(removed)
    }

    /// Discard objects outside every declared set.
    ///
    /// Returns the identifiers removed. Declared members are never
    /// candidates: Section 13.2 makes service within a declared set
    /// unconditional, so collecting one would be a violation rather than
    /// housekeeping.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError::Database`] on a read or write failure.
    pub fn collect_garbage(&self) -> Result<Vec<String>, StoreError> {
        let declared = self.declared_members()?;
        let candidates: Vec<String> = self
            .cids()?
            .into_iter()
            .filter(|c| !declared.contains(c))
            .collect();

        if candidates.is_empty() {
            return Ok(Vec::new());
        }
        let tx = self.db.begin_write()?;
        {
            let mut table = tx.open_table(OBJECTS)?;
            for cid in &candidates {
                table.remove(cid.as_str())?;
            }
        }
        tx.commit()?;
        Ok(candidates)
    }

    /// Re-hash every stored object and report those that no longer match.
    ///
    /// The store verifies on write, so a mismatch found here means the
    /// bytes changed underneath it -- disk corruption, or tampering with
    /// the database file. Either way the object must not be served.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError::Database`] on a read failure.
    pub fn scan(&self) -> Result<Vec<Corruption>, StoreError> {
        let tx = self.db.begin_read()?;
        let table = tx.open_table(OBJECTS)?;
        let mut out = Vec::new();
        for entry in table.iter()? {
            let (key, value) = entry?;
            let stored_as = key.value().to_owned();
            let Ok(cid) = stored_as.parse::<Cid>() else {
                out.push(Corruption {
                    stored_as: stored_as.clone(),
                    actual: "<unparseable key>".to_owned(),
                });
                continue;
            };
            if !cid.verifies(value.value()) {
                out.push(Corruption {
                    actual: Cid::of(value.value(), HashAlg::Sha2_256).to_string(),
                    stored_as,
                });
            }
        }
        Ok(out)
    }
}
