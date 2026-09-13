//! A ledger that settles nothing.
//!
//! The default, and the one every conformance run uses. Its existence makes
//! the optionality of settlement concrete: code that wants to be
//! ledger-agnostic can hold a `dyn Ledger` without any deployment having to
//! choose a chain, and a deployment that never wants settlement never
//! configures anything.

use publet_core::Cid;

use crate::{Ledger, LedgerError, LedgerProperties, Settlement};

/// A ledger that refuses every settlement, having none to make.
#[derive(Debug, Clone, Copy, Default)]
pub struct NullLedger;

impl Ledger for NullLedger {
    fn properties(&self) -> LedgerProperties {
        // Claims nothing, because it provides nothing.
        LedgerProperties::default()
    }

    fn settle(&self, _bounty: &Cid, _qualifying: &[Cid]) -> Result<Settlement, LedgerError> {
        // Not an error condition being papered over: there is genuinely no
        // ledger, and saying so is the correct answer.
        Err(LedgerError::NotConfigured)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use publet_core::{Cid, HashAlg};

    #[test]
    fn the_null_ledger_claims_nothing_and_settles_nothing() {
        let ledger = NullLedger;
        assert!(!ledger.properties().complete());
        let target = Cid::of(b"a bounty", HashAlg::Sha2_256);
        assert_eq!(ledger.settle(&target, &[]), Err(LedgerError::NotConfigured));
    }
}
