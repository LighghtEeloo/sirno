//! Sessions, checkpoints, and commit logic.
//!
//! A checkpoint is an immutable snapshot of a coherent graph state.
//! A session is the working interval between two checkpoints, overlaying
//! a mutable patch and pending obligations on top of a frozen base.

use crate::graph::Graph;
use crate::mutation::Patch;
use crate::obligation::Obligation;

/// Immutable snapshot of the graph at a moment of coherence.
///
/// Every checkpoint satisfies the coherence invariant: all obligations
/// discharged and all groundings accurate.
#[derive(Clone, Debug)]
pub struct Checkpoint {
    /// The graph state at the time of the snapshot.
    pub graph: Graph,
}

impl Checkpoint {
    /// Create a checkpoint from a graph that is assumed coherent.
    ///
    /// Note: coherence is not verified at construction. The caller is
    /// responsible for ensuring all obligations are discharged.
    pub fn new(graph: Graph) -> Self {
        Self { graph }
    }
}

/// Working interval between two checkpoints.
///
/// Operates against `base` as a frozen checkpoint, overlaid with an
/// in-progress `patch`. The working state is visible only to the active
/// session; other observers see the last checkpoint.
#[derive(Clone, Debug)]
pub struct Session {
    /// The frozen base checkpoint.
    pub base: Checkpoint,
    /// Accumulated mutations since the base.
    pub patch: Patch,
    /// Outstanding proof burdens.
    pub obligations: Vec<Obligation>,
}

impl Session {
    /// Start a new session from a checkpoint.
    pub fn new(base: Checkpoint) -> Self {
        Self { base, patch: Patch::new(), obligations: Vec::new() }
    }

    /// Whether all obligations have been discharged.
    pub fn is_obligation_complete(&self) -> bool {
        self.obligations
            .iter()
            .all(|o| matches!(o.status, crate::obligation::ObligationStatus::Discharged))
    }

    /// Whether all locked-entry mutations have been approved.
    pub fn is_approval_complete(&self) -> bool {
        self.obligations
            .iter()
            .all(|o| !matches!(o.status, crate::obligation::ObligationStatus::AwaitingApproval(_)))
    }

    /// Whether the patch can be promoted to a new checkpoint.
    ///
    /// Requires both obligation-completeness and approval-completeness.
    pub fn can_commit(&self) -> bool {
        self.is_obligation_complete() && self.is_approval_complete()
    }
}
