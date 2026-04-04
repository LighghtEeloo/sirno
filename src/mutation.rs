//! Mutations, patches, and polarity.
//!
//! A mutation is a single atomic change to the graph. A patch is the ordered
//! sequence of mutations accumulated during a session. Polarity determines
//! whether the graph or the codebase is treated as authoritative for a given
//! entry.

use smol_str::SmolStr;

use crate::edge::{Affinity, Dependency};
use crate::entry::{Entry, EntryId};
use crate::grounding::Grounding;

/// Direction of authority for an entry during a session.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Polarity {
    /// The graph is authoritative; the agent rewrites code to match.
    Actualization,
    /// The codebase is authoritative; the agent updates the entry to match.
    Reflection,
}

/// Describes what to do with a field during an update.
///
/// Replaces `Option<Option<T>>` with explicit intent: leave the field
/// alone, set it to a new value, or clear it (for optional fields).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FieldUpdate<T> {
    /// Leave the field unchanged.
    Unchanged,
    /// Set the field to a new value.
    Set(T),
    /// Clear the field (meaningful only for optional fields).
    Clear,
}

impl<T> Default for FieldUpdate<T> {
    fn default() -> Self {
        Self::Unchanged
    }
}

/// A single atomic change to the graph.
///
/// Mutations fall into two categories:
/// - Entry mutations (`CreateEntry`, `UpdateEntry`, `RemoveEntry`)
///   modify an entry's claims and generate obligations on dependents.
/// - Structural mutations (edge and grounding changes, lock/unlock)
///   modify how entries relate and do not generate obligations.
#[derive(Clone, Debug)]
pub enum Mutation {
    /// Create a new entry.
    ///
    /// Entry mutation. Creates the entry and generates obligations
    /// on any dependents that reference this entry's ID.
    CreateEntry(Entry),
    /// Remove an entry and all its edges, groundings, and locks.
    ///
    /// Entry mutation. Deletes the entry and all incident edges,
    /// and generates obligations on the entry's dependents.
    RemoveEntry(EntryId),
    /// Update an entry's mutable fields.
    ///
    /// Entry mutation. Generates obligations on all entries that
    /// depend on this entry, since the entry's claim has changed.
    UpdateEntry {
        id: EntryId,
        name: FieldUpdate<SmolStr>,
        description: FieldUpdate<String>,
        explanation: FieldUpdate<String>,
    },
    /// Add a dependency edge.
    AddDependency(Dependency),
    /// Remove a dependency edge.
    RemoveDependency(Dependency),
    /// Add an affinity edge.
    AddAffinity(Affinity),
    /// Remove an affinity edge.
    RemoveAffinity(Affinity),
    /// Attach a grounding to an entry.
    AddGrounding { entry: EntryId, grounding: Grounding },
    /// Lock an entry.
    Lock(EntryId),
    /// Unlock an entry.
    Unlock(EntryId),
}
/// Ordered sequence of mutations accumulated during a session.
///
/// Order matters: later mutations may depend on earlier ones (e.g., create
/// an entry, then add an edge to it).
#[derive(Clone, Debug, Default)]
pub struct Patch {
    mutations: Vec<Mutation>,
}

impl Patch {
    /// Construct an empty patch.
    pub fn new() -> Self {
        Self::default()
    }

    /// Append a mutation.
    pub fn push(&mut self, mutation: Mutation) {
        self.mutations.push(mutation);
    }

    /// The mutations in application order.
    pub fn mutations(&self) -> &[Mutation] {
        &self.mutations
    }

    /// Whether the patch contains no mutations.
    pub fn is_empty(&self) -> bool {
        self.mutations.is_empty()
    }
}
