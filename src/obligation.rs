//! Obligations: proof burdens generated when entries are mutated.
//!
//! When an entry changes and a dependency edge exists, downstream entries
//! must be re-examined for consistency. An obligation tracks that burden
//! until it is discharged.

use crate::entry::EntryId;
use crate::mutation::Justification;

/// Unique identifier for an obligation within a session.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ObligationId(u64);

impl ObligationId {
    pub fn new(id: u64) -> Self {
        Self(id)
    }

    pub fn as_u64(&self) -> u64 {
        self.0
    }
}

/// A proof burden on a downstream entry.
#[derive(Clone, Debug)]
pub struct Obligation {
    /// Unique id within the session.
    pub id: ObligationId,
    /// The entry that must be re-examined.
    pub target: EntryId,
    /// The entry whose mutation induced this obligation.
    pub cause: EntryId,
    /// Current discharge status.
    pub status: ObligationStatus,
}

/// Discharge status of an obligation.
#[derive(Clone, Debug)]
pub enum ObligationStatus {
    /// Not yet examined.
    Pending,
    /// The agent confirmed the target entry remains valid, or updated it.
    Discharged,
    /// The target entry is locked; a justification has been submitted and
    /// awaits reviewer approval.
    AwaitingApproval(Justification),
}
