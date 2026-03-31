//! Edges connect entries in the knowledge graph.
//!
//! Two kinds exist: directed dependencies that encode causal structure,
//! and undirected affinities that encode conceptual relevance.

use crate::entry::EntryId;

/// Directed edge asserting that `to`'s validity is contingent on `from`'s
/// content.
///
/// If `from` changes, `to` must be re-examined. The arrow points in the
/// direction of causal force: `from` → `to`.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Dependency {
    /// The entry whose content is depended upon.
    pub from: EntryId,
    /// The entry whose validity is contingent.
    pub to: EntryId,
}

impl Dependency {
    pub fn new(from: EntryId, to: EntryId) -> Self {
        Self { from, to }
    }
}

/// Undirected edge between entries that share conceptual relevance.
///
/// Affinities exist for navigation and epistemic context. They carry no
/// causal force and generate no obligations.
///
/// Invariant: the two entry ids are distinct, stored in canonical order
/// (`a < b`).
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Affinity {
    a: EntryId,
    b: EntryId,
}

impl Affinity {
    /// Construct an affinity between two distinct entries.
    ///
    /// Returns `None` if the two ids are equal (self-affinity is meaningless).
    /// The ids are stored in canonical order regardless of argument order.
    pub fn new(x: EntryId, y: EntryId) -> Option<Self> {
        if x == y {
            return None;
        }
        let (a, b) = if x < y { (x, y) } else { (y, x) };
        Some(Self { a, b })
    }

    /// The canonically smaller entry.
    pub fn a(&self) -> &EntryId {
        &self.a
    }

    /// The canonically larger entry.
    pub fn b(&self) -> &EntryId {
        &self.b
    }

    /// Whether this affinity involves the given entry.
    pub fn contains(&self, id: &EntryId) -> bool {
        self.a == *id || self.b == *id
    }
}
