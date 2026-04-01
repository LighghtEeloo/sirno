//! Edges connect entries in the knowledge graph.
//!
//! Two kinds exist: directed dependencies that encode causal structure,
//! and undirected affinities that encode conceptual relevance.
//!
//! An edge may point at a separate entry that explains the relation in prose.
//! The edge still owns the operational semantics; the attached entry owns the
//! text.

use crate::entry::EntryId;

/// Directed edge asserting that `to`'s validity is contingent on `from`'s
/// content.
///
/// If `from` changes, `to` must be re-examined. The arrow points in the
/// direction of causal force: `from` → `to`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Dependency {
    /// The entry whose content is depended upon.
    from: EntryId,
    /// The entry whose validity is contingent.
    to: EntryId,
    /// Optional entry that explains what the relation means.
    ///
    /// Note: the attached entry is descriptive only. Dependency propagation is
    /// determined solely by `from` and `to`.
    meaning: Option<EntryId>,
}

impl Dependency {
    /// Construct an unlabeled dependency.
    pub fn new(from: EntryId, to: EntryId) -> Self {
        Self { from, to, meaning: None }
    }

    /// Attach an entry describing the dependency relation.
    pub fn with_meaning(mut self, meaning: EntryId) -> Self {
        self.meaning = Some(meaning);
        self
    }

    /// The entry whose content is depended upon.
    pub fn from(&self) -> &EntryId {
        &self.from
    }

    /// The entry whose validity is contingent.
    pub fn to(&self) -> &EntryId {
        &self.to
    }

    /// The optional entry describing this relation.
    pub fn meaning(&self) -> Option<&EntryId> {
        self.meaning.as_ref()
    }

    pub(crate) fn key(&self) -> (EntryId, EntryId) {
        (self.from.clone(), self.to.clone())
    }

    pub(crate) fn meaning_matches(&self, id: &EntryId) -> bool {
        self.meaning.as_ref().is_some_and(|meaning| meaning == id)
    }

    pub(crate) fn clear_meaning(&mut self) -> bool {
        self.meaning.take().is_some()
    }
}

/// Undirected edge between entries that share conceptual relevance.
///
/// Affinities exist for navigation and epistemic context. They carry no
/// causal force and generate no obligations.
///
/// Invariant: the two entry ids are distinct, stored in canonical order
/// (`a < b`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Affinity {
    a: EntryId,
    b: EntryId,
    /// Optional entry that explains what the affinity means.
    ///
    /// Note: the attached entry is descriptive only. Affinities remain
    /// non-causal regardless of how they are explained.
    meaning: Option<EntryId>,
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
        Some(Self { a, b, meaning: None })
    }

    /// Attach an entry describing the affinity relation.
    pub fn with_meaning(mut self, meaning: EntryId) -> Self {
        self.meaning = Some(meaning);
        self
    }

    /// The canonically smaller entry.
    pub fn a(&self) -> &EntryId {
        &self.a
    }

    /// The canonically larger entry.
    pub fn b(&self) -> &EntryId {
        &self.b
    }

    /// The optional entry describing this relation.
    pub fn meaning(&self) -> Option<&EntryId> {
        self.meaning.as_ref()
    }

    /// Whether this affinity involves the given entry.
    pub fn contains(&self, id: &EntryId) -> bool {
        self.a == *id || self.b == *id
    }

    pub(crate) fn key(&self) -> (EntryId, EntryId) {
        (self.a.clone(), self.b.clone())
    }

    pub(crate) fn meaning_matches(&self, id: &EntryId) -> bool {
        self.meaning.as_ref().is_some_and(|meaning| meaning == id)
    }

    pub(crate) fn clear_meaning(&mut self) -> bool {
        self.meaning.take().is_some()
    }
}
