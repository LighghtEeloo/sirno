//! Obligations: proof burdens generated when entries are mutated.
//!
//! When an entry changes and a dependency edge exists, downstream entries
//! must be re-examined for consistency. An obligation tracks that burden
//! until it is discharged.

use std::collections::HashMap;

use crate::entry::EntryId;
use crate::mutation::Mutation;

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

impl std::fmt::Display for ObligationId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "obl-{}", self.0)
    }
}

/// Argument for why a mutation to a locked entry is necessary.
///
/// Submitted to a reviewer who grants or withholds approval.
/// The mutation materializes only upon approval.
#[derive(Clone, Debug)]
pub struct Justification {
    /// The locked entry that the agent wants to mutate.
    pub entry: EntryId,
    /// The proposed mutation, deferred until approval.
    pub mutation: Box<Mutation>,
    /// The agent's argument for the change.
    pub argument: String,
}

/// A proof burden on a downstream entry.
#[derive(Clone, Debug)]
pub struct Obligation {
    /// Unique id within the session.
    id: ObligationId,
    /// The entry that must be re-examined.
    target: EntryId,
    /// The entry whose mutation induced this obligation.
    cause: EntryId,
    /// Current discharge status.
    status: ObligationStatus,
}

impl Obligation {
    pub fn id(&self) -> &ObligationId {
        &self.id
    }

    pub fn target(&self) -> &EntryId {
        &self.target
    }

    pub fn cause(&self) -> &EntryId {
        &self.cause
    }

    pub fn status(&self) -> &ObligationStatus {
        &self.status
    }

    /// Update the discharge status.
    pub fn set_status(&mut self, status: ObligationStatus) {
        self.status = status;
    }
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

/// Indexed collection of obligations within a session.
///
/// Owns id generation and provides focused queries. All obligations are
/// created through `generate()`, which assigns a unique id automatically.
#[derive(Clone, Debug, Default)]
pub struct ObligationSet {
    inner: HashMap<ObligationId, Obligation>,
    next_id: u64,
}

impl ObligationSet {
    /// Construct an empty obligation set.
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a new pending obligation and return its id.
    pub fn generate(&mut self, target: EntryId, cause: EntryId) -> ObligationId {
        let id = ObligationId::new(self.next_id);
        self.next_id += 1;
        self.inner.insert(
            id.clone(),
            Obligation { id: id.clone(), target, cause, status: ObligationStatus::Pending },
        );
        id
    }

    /// Look up an obligation by id.
    pub fn get(&self, id: &ObligationId) -> Option<&Obligation> {
        self.inner.get(id)
    }

    /// Mutable access to an obligation by id.
    pub fn get_mut(&mut self, id: &ObligationId) -> Option<&mut Obligation> {
        self.inner.get_mut(id)
    }

    /// All pending obligations.
    pub fn pending(&self) -> impl Iterator<Item = &Obligation> {
        self.inner.values().filter(|o| matches!(o.status, ObligationStatus::Pending))
    }

    /// Whether all obligations have been discharged.
    pub fn is_complete(&self) -> bool {
        self.inner.values().all(|o| matches!(o.status, ObligationStatus::Discharged))
    }

    /// Whether any obligations are awaiting approval.
    pub fn has_pending_approvals(&self) -> bool {
        self.inner.values().any(|o| matches!(o.status, ObligationStatus::AwaitingApproval(_)))
    }

    /// Iterate over all obligations.
    pub fn iter(&self) -> impl Iterator<Item = &Obligation> {
        self.inner.values()
    }

    /// Whether the set is empty.
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }
}
