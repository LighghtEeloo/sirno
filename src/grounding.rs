//! Grounding maps entries to locations in the codebase.
//!
//! Groundings are the interpretation function from the abstract knowledge
//! graph into concrete syntax. Two mechanisms are provided: heuristic
//! search (grep) and nominal anchor (telescope).

use crate::entry::EntryId;

/// A grounding mechanism mapping an entry to code locations.
#[derive(Clone, Debug)]
pub enum Grounding {
    Grep(GrepGrounding),
    Telescope(TelescopeGrounding),
}

// ---------------------------------------------------------------------------
// Grep grounding
// ---------------------------------------------------------------------------

/// Heuristic grounding via search patterns.
///
/// Grep groundings are approximate: they may over- or under-approximate
/// the true set of relevant locations. Useful for broad exploration and
/// for entries whose relevance is diffuse across the codebase.
#[derive(Clone, Debug)]
pub struct GrepGrounding {
    /// One or more search patterns that locate relevant code regions.
    patterns: Vec<SearchPattern>,
}

impl GrepGrounding {
    /// Construct a grep grounding from a non-empty list of patterns.
    ///
    /// Note: an empty pattern list is accepted but produces no matches.
    pub fn new(patterns: Vec<SearchPattern>) -> Self {
        Self { patterns }
    }

    pub fn patterns(&self) -> &[SearchPattern] {
        &self.patterns
    }
}

/// A single search pattern used in grep grounding.
#[derive(Clone, Debug)]
pub enum SearchPattern {
    /// Regular expression.
    Regex(String),
    /// Literal string match.
    Literal(String),
    /// File glob pattern.
    Glob(String),
}

// ---------------------------------------------------------------------------
// Telescope grounding
// ---------------------------------------------------------------------------

/// An anchor embedded in a code comment (e.g., `// @sirno:entry-id`)
/// establishing a nominal binding between an entry and a precise code
/// location.
///
/// Telescope anchors survive refactoring as long as the comment moves
/// with the code.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct TelescopeAnchor {
    /// The entry this anchor binds to.
    pub entry_id: EntryId,
}

impl TelescopeAnchor {
    pub fn new(entry_id: EntryId) -> Self {
        Self { entry_id }
    }
}

/// Anchor-based grounding with optional derived views (spans, witnesses).
#[derive(Clone, Debug)]
pub struct TelescopeGrounding {
    /// The anchor establishing the code binding.
    pub anchor: TelescopeAnchor,
    /// Block-level code views derived from this anchor.
    pub spans: Vec<Span>,
    /// Code regions that serve as evidence for the entry's claim.
    pub witnesses: Vec<Witness>,
}

impl TelescopeGrounding {
    /// Construct a telescope grounding from an anchor, with no derived views.
    pub fn new(anchor: TelescopeAnchor) -> Self {
        Self { anchor, spans: Vec::new(), witnesses: Vec::new() }
    }

    /// Attach span views.
    pub fn with_spans(mut self, spans: Vec<Span>) -> Self {
        self.spans = spans;
        self
    }

    /// Attach witness regions.
    pub fn with_witnesses(mut self, witnesses: Vec<Witness>) -> Self {
        self.witnesses = witnesses;
        self
    }
}

/// A region between two anchors, or from an anchor to a scope boundary.
///
/// Provides block-level code views without fragile line references.
#[derive(Clone, Debug)]
pub struct Span {
    /// Where the span begins.
    pub start: SpanBound,
    /// Where the span ends.
    pub end: SpanBound,
}

/// One end of a span: either a telescope anchor or the enclosing scope
/// boundary.
#[derive(Clone, Debug)]
pub enum SpanBound {
    /// An explicit telescope anchor.
    Anchor(TelescopeAnchor),
    /// The nearest enclosing scope boundary (e.g., end of function, block,
    /// or file).
    ScopeBoundary,
}

/// A telescope-grounded code region serving as evidence for an entry's claim.
///
/// During a rewrite session, an agent verifies that witnesses still
/// substantiate their entries.
#[derive(Clone, Debug)]
pub struct Witness {
    /// The code region providing evidence.
    pub span: Span,
}
