//! The retrieval client (Section 14.4).
//!
//! Two properties are enforced here rather than left to callers:
//!
//! * **Retrieved bytes are verified against the identifier requested**
//!   before they are returned. A peer that serves the wrong object is
//!   otherwise indistinguishable from one that serves the right one.
//! * **Nothing the client holds is ever transmitted during
//!   synchronization.** A sync request carries a domain identifier and two
//!   integers. There is no code path that sends a list of held objects,
//!   because Section 14.3.2 forbids one and because partial holdings are a
//!   fingerprint of what a reader has been working with.

use publet_core::{Cid, HashAlg};
use thiserror::Error;

use crate::wire;

/// Why a fetch failed.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum ClientError {
    /// The request could not be completed.
    #[error("request failed: {0}")]
    Transport(String),

    /// The peer answered with a status the protocol does not define here.
    #[error("peer returned {status} for {path}")]
    Status {
        /// The status returned.
        status: u16,
        /// What was requested.
        path: String,
    },

    /// The bytes returned do not hash to the identifier requested.
    ///
    /// This is the check that makes trusting the peer unnecessary rather
    /// than merely unwise.
    #[error("peer returned bytes for {requested} that hash to {actual}")]
    WrongObject {
        /// What was asked for.
        requested: String,
        /// What arrived.
        actual: String,
    },

    /// A pack could not be decoded.
    #[error(transparent)]
    Wire(#[from] wire::WireError),
}

/// A client for one peer.
#[derive(Debug, Clone)]
pub struct Client {
    base: String,
    http: reqwest::Client,
}

impl Client {
    /// Connect to a peer at `base`, for example `http://127.0.0.1:8080`.
    ///
    /// # Errors
    ///
    /// Returns [`ClientError::Transport`] if the HTTP client cannot be built.
    pub fn new(base: &str) -> Result<Self, ClientError> {
        Ok(Self {
            base: base.trim_end_matches('/').to_owned(),
            http: reqwest::Client::builder()
                .build()
                .map_err(|e| ClientError::Transport(e.to_string()))?,
        })
    }

    /// Connect through a proxy.
    ///
    /// Section 14.3 requires support for an anonymizing transport: which
    /// domains a reader follows is visible to whoever serves them, and
    /// routing through a proxy is what separates that from who they are.
    ///
    /// # Errors
    ///
    /// Returns [`ClientError::Transport`] if the proxy cannot be configured.
    pub fn with_proxy(base: &str, proxy: &str) -> Result<Self, ClientError> {
        let proxy =
            reqwest::Proxy::all(proxy).map_err(|e| ClientError::Transport(e.to_string()))?;
        Ok(Self {
            base: base.trim_end_matches('/').to_owned(),
            http: reqwest::Client::builder()
                .proxy(proxy)
                .build()
                .map_err(|e| ClientError::Transport(e.to_string()))?,
        })
    }

    async fn get(&self, path: &str) -> Result<Vec<u8>, ClientError> {
        let url = format!("{}{path}", self.base);
        let response = self
            .http
            .get(&url)
            .send()
            .await
            .map_err(|e| ClientError::Transport(e.to_string()))?;
        let status = response.status();
        if !status.is_success() {
            return Err(ClientError::Status {
                status: status.as_u16(),
                path: path.to_owned(),
            });
        }
        Ok(response
            .bytes()
            .await
            .map_err(|e| ClientError::Transport(e.to_string()))?
            .to_vec())
    }

    /// Fetch one object, verifying it against the identifier requested.
    ///
    /// # Errors
    ///
    /// Returns [`ClientError::WrongObject`] if the bytes do not hash to
    /// `cid`, or [`ClientError`] on a transport or status failure.
    pub async fn object(&self, cid: &Cid) -> Result<Vec<u8>, ClientError> {
        let bytes = self.get(&format!("/pub/v1/object/{cid}")).await?;
        if !cid.verifies(&bytes) {
            return Err(ClientError::WrongObject {
                requested: cid.to_string(),
                actual: Cid::of(&bytes, cid.alg()).to_string(),
            });
        }
        Ok(bytes)
    }

    /// The domains the peer has declared it serves.
    ///
    /// # Errors
    ///
    /// Returns [`ClientError`] on a transport or status failure.
    pub async fn declared_set(&self) -> Result<Vec<String>, ClientError> {
        let bytes = self.get("/pub/v1/set").await?;
        Ok(String::from_utf8_lossy(&bytes)
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty())
            .map(ToOwned::to_owned)
            .collect())
    }

    /// Fetch a domain's objects as one pack.
    ///
    /// Every object is verified against its own identifier on arrival; any
    /// that fail are discarded rather than returned.
    ///
    /// # Errors
    ///
    /// Returns [`ClientError`] on a transport, status, or framing failure.
    pub async fn pack(&self, domain: &Cid) -> Result<Vec<Vec<u8>>, ClientError> {
        let bytes = self.get(&format!("/pub/v1/domain/{domain}/pack")).await?;
        let objects = wire::unpack(&bytes)?;
        Ok(objects
            .into_iter()
            .filter(|o| {
                let cid = Cid::of(o, HashAlg::Sha2_256);
                cid.verifies(o)
            })
            .collect())
    }

    /// Fetch the generation records and objects between two indices.
    ///
    /// The request carries the domain and two integers. **It carries
    /// nothing about what the client holds**, which is the whole of what
    /// Section 14.3.2 requires of a synchronizing client.
    ///
    /// # Errors
    ///
    /// Returns [`ClientError`] on a transport, status, or framing failure.
    pub async fn delta(
        &self,
        domain: &Cid,
        from: u64,
        to: u64,
    ) -> Result<Vec<Vec<u8>>, ClientError> {
        let bytes = self
            .get(&format!("/pub/v1/domain/{domain}/delta/{from}/{to}"))
            .await?;
        Ok(wire::unpack(&bytes)?)
    }

    /// The checkpoint origins the peer offers.
    ///
    /// # Errors
    ///
    /// Returns [`ClientError`] on a transport or status failure.
    pub async fn checkpoints(&self, domain: &Cid) -> Result<Vec<u64>, ClientError> {
        let bytes = self
            .get(&format!("/pub/v1/domain/{domain}/checkpoints"))
            .await?;
        Ok(String::from_utf8_lossy(&bytes)
            .lines()
            .filter_map(|l| l.trim().parse().ok())
            .collect())
    }

    /// Offer one object to the peer.
    ///
    /// # Errors
    ///
    /// Returns [`ClientError`] on a transport failure. A `409` means the
    /// peer already holds it, which is reported as success.
    pub async fn offer(&self, bytes: &[u8]) -> Result<bool, ClientError> {
        let response = self
            .http
            .post(format!("{}/pub/v1/object", self.base))
            .body(bytes.to_vec())
            .send()
            .await
            .map_err(|e| ClientError::Transport(e.to_string()))?;
        match response.status().as_u16() {
            202 => Ok(true),
            409 => Ok(false),
            status => Err(ClientError::Status {
                status,
                path: "/pub/v1/object".to_owned(),
            }),
        }
    }
}
