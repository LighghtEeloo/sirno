//! Entry: the primitive unit of knowledge in Sirno.
//!
//! Each entry represents a single claim about the codebase — an invariant,
//! a design decision, a module's purpose, a data representation choice, or
//! any other isolable piece of understanding.
//!
//! Entries are the only durable owner of dedicated explanatory text in the
//! graph. Other concepts may carry executable syntax or identifiers, but they
//! refer back to entries whenever they need human-readable narration.

use smol_str::SmolStr;

/// Unique, agent-assigned nominal identifier for an entry.
///
/// Opaque by construction: the inner representation is not exposed.
/// Two entries are the same entry if and only if their ids are equal.
///
/// Note: uses `SmolStr` internally. Identifiers ≤ 22 bytes are stored
/// inline, avoiding heap allocation and making clones a memcpy. This
/// matters because entry ids are the most-cloned value in the system
/// (HashMap keys, edge storage, mutation records).
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct EntryId(SmolStr);

impl EntryId {
    /// Create an entry id from any string-like value.
    pub fn new(id: impl Into<SmolStr>) -> Self {
        Self(id.into())
    }

    /// Borrow the underlying identifier.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for EntryId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// A single claim about the codebase with nominal identity.
///
/// Invariant: `id` is immutable after construction. Name, description, and
/// explanation are the entry's text payload and may be updated through
/// controlled mutation.
#[derive(Clone, Debug)]
pub struct Entry {
    /// Unique nominal identifier. Immutable after construction.
    id: EntryId,
    /// Optional human-readable concept name.
    name: Option<SmolStr>,
    /// Concise summary of the entry's claim.
    description: String,
    /// Full account of the entry's content, rationale, and context.
    explanation: String,
}

impl Entry {
    /// Construct a new entry with a required id, description, and explanation.
    pub fn new(
        id: EntryId, description: impl Into<String>, explanation: impl Into<String>,
    ) -> Self {
        Self { id, name: None, description: description.into(), explanation: explanation.into() }
    }

    /// Attach an optional human-readable name.
    pub fn with_name(mut self, name: impl Into<SmolStr>) -> Self {
        self.name = Some(name.into());
        self
    }

    /// The entry's nominal identifier.
    pub fn id(&self) -> &EntryId {
        &self.id
    }

    /// The optional human-readable concept name.
    pub fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    /// The concise summary of the entry's claim.
    pub fn description(&self) -> &str {
        &self.description
    }

    /// The full account of the entry's content and rationale.
    pub fn explanation(&self) -> &str {
        &self.explanation
    }

    /// Replace the human-readable name.
    pub fn set_name(&mut self, name: Option<SmolStr>) {
        self.name = name;
    }

    /// Replace the description.
    pub fn set_description(&mut self, description: impl Into<String>) {
        self.description = description.into();
    }

    /// Replace the explanation.
    pub fn set_explanation(&mut self, explanation: impl Into<String>) {
        self.explanation = explanation.into();
    }
}
