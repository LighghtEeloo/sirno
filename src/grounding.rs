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

impl Grounding {
    /// Validate the structural invariants encoded directly in this grounding.
    ///
    /// This check does not inspect the codebase. It only verifies invariants
    /// representable inside the graph data model.
    pub fn validate_structure(&self, entry: &EntryId) -> Result<(), GroundingValidationError> {
        match self {
            | Self::Grep(_) => Ok(()),
            | Self::Telescope(grounding) => grounding.validate_structure(entry),
        }
    }
}

/// Validates whether a grounding still denotes the code claimed by an entry.
///
/// Callers that can inspect the codebase should implement this trait and
/// enforce project-specific grounding checks during commit.
pub trait GroundingValidator {
    /// Validate one grounding attached to `entry`.
    fn validate(
        &self, entry: &EntryId, grounding: &Grounding,
    ) -> Result<(), GroundingValidationError>;
}

/// Baseline grounding validator that checks only graph-internal invariants.
///
/// Note: this validator does not inspect source files. It is sufficient for
/// tests and for invariants encoded directly in the graph, but it does not
/// prove that a grounding still locates the intended code.
#[derive(Clone, Copy, Debug, Default)]
pub struct StructuralGroundingValidator;

impl GroundingValidator for StructuralGroundingValidator {
    fn validate(
        &self, entry: &EntryId, grounding: &Grounding,
    ) -> Result<(), GroundingValidationError> {
        grounding.validate_structure(entry)
    }
}

/// Failure reported while validating a grounding.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum GroundingValidationError {
    /// The telescope anchor names a different entry than the grounding owner.
    #[error("telescope anchor points to {anchor_entry} but grounding belongs to {entry}")]
    AnchorEntryMismatch { entry: EntryId, anchor_entry: EntryId },
    /// A span start anchor names a different entry than the grounding owner.
    #[error("span start anchor points to {anchor_entry} but grounding belongs to {entry}")]
    SpanStartEntryMismatch { entry: EntryId, anchor_entry: EntryId },
    /// A span end anchor names a different entry than the grounding owner.
    #[error("span end anchor points to {anchor_entry} but grounding belongs to {entry}")]
    SpanEndEntryMismatch { entry: EntryId, anchor_entry: EntryId },
    /// A witness span contains an anchor for a different entry than the grounding owner.
    #[error("witness anchor points to {anchor_entry} but grounding belongs to {entry}")]
    WitnessEntryMismatch { entry: EntryId, anchor_entry: EntryId },
    /// An external grep check found no code corresponding to the grounding.
    #[error("grep grounding for {entry} does not match any code")]
    GrepMiss { entry: EntryId },
    /// An external telescope check could not resolve the anchored code location.
    #[error("telescope grounding for {entry} does not resolve in code")]
    MissingAnchor { entry: EntryId },
    /// An external witness check determined that the code no longer supports the claim.
    #[error("witness for {entry} no longer substantiates the entry")]
    WitnessMismatch { entry: EntryId },
}

/// Concrete report of which grounding failed validation.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
#[error("grounding {grounding_index} for entry {entry} is invalid: {source}")]
pub struct GroundingFailure {
    entry: EntryId,
    grounding_index: usize,
    #[source]
    source: GroundingValidationError,
}

impl GroundingFailure {
    /// Construct a grounding failure report.
    pub fn new(entry: EntryId, grounding_index: usize, source: GroundingValidationError) -> Self {
        Self { entry, grounding_index, source }
    }

    /// The entry that owns the invalid grounding.
    pub fn entry(&self) -> &EntryId {
        &self.entry
    }

    /// The grounding's index within the owning entry's grounding list.
    pub fn grounding_index(&self) -> usize {
        self.grounding_index
    }

    /// The specific validation failure.
    pub fn source(&self) -> &GroundingValidationError {
        &self.source
    }
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

    /// The search patterns used by this grounding.
    pub fn patterns(&self) -> &[SearchPattern] {
        &self.patterns
    }
}

/// A single search pattern used in grep grounding.
///
/// Search patterns carry executable search syntax rather than descriptive
/// prose, so they remain inline data instead of pointing at entries.
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
    /// Construct an anchor for the given entry.
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

    /// Validate the structural invariants encoded in this grounding.
    pub fn validate_structure(&self, entry: &EntryId) -> Result<(), GroundingValidationError> {
        if &self.anchor.entry_id != entry {
            return Err(GroundingValidationError::AnchorEntryMismatch {
                entry: entry.clone(),
                anchor_entry: self.anchor.entry_id.clone(),
            });
        }
        for span in &self.spans {
            span.validate_structure(entry)?;
        }
        for witness in &self.witnesses {
            witness.validate_structure(entry)?;
        }
        Ok(())
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

impl Span {
    /// Validate that any anchors in this span are consistent with the owner entry.
    pub fn validate_structure(&self, entry: &EntryId) -> Result<(), GroundingValidationError> {
        self.start.validate_structure(entry, SpanAnchorPosition::Start)?;
        self.end.validate_structure(entry, SpanAnchorPosition::End)?;
        Ok(())
    }
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

impl SpanBound {
    fn validate_structure(
        &self, entry: &EntryId, position: SpanAnchorPosition,
    ) -> Result<(), GroundingValidationError> {
        let anchor = match self {
            | Self::Anchor(anchor) => anchor,
            | Self::ScopeBoundary => return Ok(()),
        };
        if &anchor.entry_id == entry {
            return Ok(());
        }
        match position {
            | SpanAnchorPosition::Start => Err(GroundingValidationError::SpanStartEntryMismatch {
                entry: entry.clone(),
                anchor_entry: anchor.entry_id.clone(),
            }),
            | SpanAnchorPosition::End => Err(GroundingValidationError::SpanEndEntryMismatch {
                entry: entry.clone(),
                anchor_entry: anchor.entry_id.clone(),
            }),
        }
    }
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

impl Witness {
    /// Validate that the witness is structurally attached to the owner entry.
    pub fn validate_structure(&self, entry: &EntryId) -> Result<(), GroundingValidationError> {
        self.span.validate_structure(entry).map_err(|error| match error {
            | GroundingValidationError::SpanStartEntryMismatch { entry, anchor_entry }
            | GroundingValidationError::SpanEndEntryMismatch { entry, anchor_entry } => {
                GroundingValidationError::WitnessEntryMismatch { entry, anchor_entry }
            }
            | other => other,
        })
    }
}

#[derive(Clone, Copy, Debug)]
enum SpanAnchorPosition {
    Start,
    End,
}
