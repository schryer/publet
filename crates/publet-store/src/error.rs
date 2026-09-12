//! Store errors.

use thiserror::Error;

/// Why a store operation failed.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum StoreError {
    /// The underlying database reported a failure.
    #[error("store: {0}")]
    Database(String),

    /// The bytes offered do not hash to the identifier given.
    ///
    /// Accepting them would let the store answer a request for one object
    /// with a different one, which is the single guarantee content
    /// addressing exists to provide.
    #[error("identifier mismatch: bytes hash to {actual}, not {claimed}")]
    IdentifierMismatch {
        /// The identifier under which storage was attempted.
        claimed: String,
        /// The identifier the bytes actually have.
        actual: String,
    },

    /// Another process holds the store open.
    ///
    /// The database takes an exclusive file lock, so two store-backed
    /// commands cannot run concurrently against one store. This matters for
    /// pipelines: `pub-store list | pub-store declare` deadlocks, because
    /// both ends want the same lock. Redirect through a file, or produce
    /// the identifiers with a directory-backed command such as `pub-ls`.
    #[error(
        "the store at {path} is already open in another process; store-backed \
         commands cannot be piped into one another -- redirect through a file, \
         or read identifiers from a directory with pub-ls"
    )]
    Locked {
        /// The store that could not be opened.
        path: String,
    },

    /// Removal of a declared member was attempted without a tombstone.
    #[error(
        "{cid} is in a declared set; ceasing to serve it requires a published \
         tombstone (Section 13.2), not a silent deletion"
    )]
    UndisclosedRemoval {
        /// The member that would have been removed.
        cid: String,
    },
}

macro_rules! from_redb {
    ($($ty:path),* $(,)?) => {
        $(impl From<$ty> for StoreError {
            fn from(value: $ty) -> Self {
                Self::Database(value.to_string())
            }
        })*
    };
}

from_redb!(
    redb::Error,
    redb::DatabaseError,
    redb::TransactionError,
    redb::TableError,
    redb::StorageError,
    redb::CommitError,
);

impl From<std::io::Error> for StoreError {
    fn from(value: std::io::Error) -> Self {
        Self::Database(value.to_string())
    }
}
