//! Repository-backed grounding validation.
//!
//! This module binds the abstract grounding model to a filesystem workspace.
//! It validates supported grep and telescope groundings against repository
//! contents while preserving the grounding contract defined in
//! [`crate::grounding`].
//!
//! The repository validator is intentionally narrow. Literal grep patterns,
//! glob path patterns, and telescope anchors are checked against the
//! workspace. Regex grep patterns are reported as warnings and left to agent
//! review rather than treated as commit blockers.

use std::fs;
use std::path::{Path, PathBuf};

use tracing::warn;

use crate::entry::EntryId;
use crate::grounding::{
    GrepGrounding, Grounding, GroundingValidationError, GroundingValidator, SearchPattern, Span,
    SpanBound, TelescopeGrounding, Witness,
};

/// Filesystem-backed grounding validator rooted at a workspace directory.
///
/// Structural grounding invariants are enforced first. The validator then
/// inspects files below `root` to confirm that supported grep and telescope
/// anchors still resolve in source text.
///
/// The validator skips repository metadata and build output directories
/// (`.git`, `.jj`, and `target`) so that coherence checks stay scoped to the
/// authored workspace.
#[derive(Clone, Debug)]
pub struct WorkspaceGroundingValidator {
    root: PathBuf,
}

impl WorkspaceGroundingValidator {
    /// Construct a validator over `root`.
    ///
    /// `root` defines the repository view used for commit-time coherence.
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }

    fn validate_grep(
        &self, entry: &EntryId, grounding: &GrepGrounding,
    ) -> Result<(), GroundingValidationError> {
        let mut supported_patterns = 0usize;
        for pattern in grounding.patterns() {
            match pattern {
                | SearchPattern::Literal(literal) => {
                    supported_patterns += 1;
                    if self.any_file_contains(literal)? {
                        return Ok(());
                    }
                }
                | SearchPattern::Glob(glob) => {
                    supported_patterns += 1;
                    if self.any_path_matches(glob)? {
                        return Ok(());
                    }
                }
                | SearchPattern::Regex(regex) => {
                    warn!(
                        entry = %entry,
                        pattern = regex,
                        "repository grounding validator skips regex search patterns"
                    );
                }
            }
        }

        if supported_patterns == 0 {
            warn!(
                entry = %entry,
                "repository grounding validator found no supported grep patterns to validate"
            );
            return Ok(());
        }

        Err(GroundingValidationError::GrepMiss { entry: entry.clone() })
    }

    fn validate_telescope(
        &self, entry: &EntryId, grounding: &TelescopeGrounding,
    ) -> Result<(), GroundingValidationError> {
        self.ensure_anchor_exists(entry, &grounding.anchor.entry_id)?;
        for span in &grounding.spans {
            self.validate_span(entry, span)?;
        }
        for witness in &grounding.witnesses {
            self.validate_witness(entry, witness)?;
        }
        Ok(())
    }

    fn validate_span(&self, entry: &EntryId, span: &Span) -> Result<(), GroundingValidationError> {
        self.validate_span_bound(entry, &span.start)?;
        self.validate_span_bound(entry, &span.end)?;
        Ok(())
    }

    fn validate_span_bound(
        &self, entry: &EntryId, bound: &SpanBound,
    ) -> Result<(), GroundingValidationError> {
        let anchor = match bound {
            | SpanBound::Anchor(anchor) => anchor,
            | SpanBound::ScopeBoundary => return Ok(()),
        };
        self.ensure_anchor_exists(entry, &anchor.entry_id)
    }

    fn validate_witness(
        &self, entry: &EntryId, witness: &Witness,
    ) -> Result<(), GroundingValidationError> {
        self.validate_span(entry, &witness.span)
            .map_err(|_| GroundingValidationError::WitnessMismatch { entry: entry.clone() })
    }

    fn ensure_anchor_exists(
        &self, entry: &EntryId, anchor_entry: &EntryId,
    ) -> Result<(), GroundingValidationError> {
        let needle = telescope_marker(anchor_entry);
        if self.any_file_contains(&needle)? {
            Ok(())
        } else {
            Err(GroundingValidationError::MissingAnchor { entry: entry.clone() })
        }
    }

    fn any_file_contains(&self, needle: &str) -> Result<bool, GroundingValidationError> {
        self.walk_files(&mut |path| {
            let bytes = fs::read(path).map_err(|error| GroundingValidationError::RepositoryIo {
                path: path.display().to_string(),
                message: error.to_string(),
            })?;
            Ok(String::from_utf8_lossy(&bytes).contains(needle))
        })
    }

    fn any_path_matches(&self, glob: &str) -> Result<bool, GroundingValidationError> {
        self.walk_files(&mut |path| {
            let relative = path.strip_prefix(&self.root).map_err(|error| {
                GroundingValidationError::RepositoryIo {
                    path: path.display().to_string(),
                    message: error.to_string(),
                }
            })?;
            Ok(path_matches_glob(&relative.to_string_lossy(), glob))
        })
    }

    fn walk_files<F>(&self, predicate: &mut F) -> Result<bool, GroundingValidationError>
    where
        F: FnMut(&Path) -> Result<bool, GroundingValidationError>,
    {
        self.walk_dir(&self.root, predicate)
    }

    fn walk_dir<F>(&self, dir: &Path, predicate: &mut F) -> Result<bool, GroundingValidationError>
    where
        F: FnMut(&Path) -> Result<bool, GroundingValidationError>,
    {
        let entries =
            fs::read_dir(dir).map_err(|error| GroundingValidationError::RepositoryIo {
                path: dir.display().to_string(),
                message: error.to_string(),
            })?;

        for entry in entries {
            let entry = entry.map_err(|error| GroundingValidationError::RepositoryIo {
                path: dir.display().to_string(),
                message: error.to_string(),
            })?;
            let path = entry.path();
            let file_type =
                entry.file_type().map_err(|error| GroundingValidationError::RepositoryIo {
                    path: path.display().to_string(),
                    message: error.to_string(),
                })?;
            if file_type.is_dir() {
                if should_skip_dir(&path) {
                    continue;
                }
                if self.walk_dir(&path, predicate)? {
                    return Ok(true);
                }
                continue;
            }
            if file_type.is_file() && predicate(&path)? {
                return Ok(true);
            }
        }
        Ok(false)
    }
}

impl GroundingValidator for WorkspaceGroundingValidator {
    fn validate(
        &self, entry: &EntryId, grounding: &Grounding,
    ) -> Result<(), GroundingValidationError> {
        grounding.validate_structure(entry)?;
        match grounding {
            | Grounding::Grep(grounding) => self.validate_grep(entry, grounding),
            | Grounding::Telescope(grounding) => self.validate_telescope(entry, grounding),
        }
    }
}

fn telescope_marker(entry: &EntryId) -> String {
    format!("@sirno:{entry}")
}

fn should_skip_dir(path: &Path) -> bool {
    matches!(path.file_name().and_then(|name| name.to_str()), Some(".git" | ".jj" | "target"))
}

fn path_matches_glob(path: &str, pattern: &str) -> bool {
    wildcard_matches(
        &path.replace(std::path::MAIN_SEPARATOR, "/"),
        &pattern.replace(std::path::MAIN_SEPARATOR, "/"),
    )
}

/// Match `text` against a shell-style wildcard pattern with `*` and `?`.
///
/// Note: this matcher is intentionally small. It exists to validate path-glob
/// groundings without introducing a richer pattern language into the core
/// repository validator.
fn wildcard_matches(text: &str, pattern: &str) -> bool {
    let text = text.as_bytes();
    let pattern = pattern.as_bytes();
    let (mut text_index, mut pattern_index) = (0usize, 0usize);
    let (mut star_index, mut match_index) = (None, 0usize);

    while text_index < text.len() {
        if pattern_index < pattern.len()
            && (pattern[pattern_index] == b'?' || pattern[pattern_index] == text[text_index])
        {
            text_index += 1;
            pattern_index += 1;
        } else if pattern_index < pattern.len() && pattern[pattern_index] == b'*' {
            star_index = Some(pattern_index);
            pattern_index += 1;
            match_index = text_index;
        } else if let Some(star) = star_index {
            pattern_index = star + 1;
            match_index += 1;
            text_index = match_index;
        } else {
            return false;
        }
    }

    while pattern_index < pattern.len() && pattern[pattern_index] == b'*' {
        pattern_index += 1;
    }

    pattern_index == pattern.len()
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::WorkspaceGroundingValidator;
    use crate::entry::EntryId;
    use crate::grounding::{
        GrepGrounding, Grounding, GroundingValidationError, GroundingValidator, SearchPattern,
        TelescopeAnchor, TelescopeGrounding,
    };

    struct TempWorkspace {
        root: PathBuf,
    }

    impl TempWorkspace {
        fn new(name: &str) -> Self {
            let unique = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
            let root = std::env::temp_dir().join(format!("sirno-{name}-{unique}"));
            fs::create_dir_all(&root).unwrap();
            Self { root }
        }

        fn path(&self) -> PathBuf {
            self.root.clone()
        }

        fn write(&self, relative: &str, contents: &str) {
            let path = self.root.join(relative);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).unwrap();
            }
            fs::write(path, contents).unwrap();
        }
    }

    impl Drop for TempWorkspace {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }

    #[test]
    fn workspace_validator_accepts_literal_and_anchor_matches() {
        let workspace = TempWorkspace::new("grounding-accepts");
        workspace.write(
            "src/lib.rs",
            "// @sirno:entry-a\nfn literal_example() { let value = \"needle\"; }\n",
        );

        let validator = WorkspaceGroundingValidator::new(workspace.path());
        let entry = EntryId::new("entry-a");

        validator
            .validate(
                &entry,
                &Grounding::Grep(GrepGrounding::new(vec![SearchPattern::Literal(
                    "needle".to_owned(),
                )])),
            )
            .unwrap();
        validator
            .validate(
                &entry,
                &Grounding::Telescope(TelescopeGrounding::new(TelescopeAnchor::new(entry.clone()))),
            )
            .unwrap();
    }

    #[test]
    fn workspace_validator_warns_and_skips_regex_only_grep() {
        let workspace = TempWorkspace::new("grounding-regex-warning");
        workspace.write("src/lib.rs", "fn no_anchor_needed() {}\n");

        let validator = WorkspaceGroundingValidator::new(workspace.path());
        let entry = EntryId::new("entry-a");

        validator
            .validate(
                &entry,
                &Grounding::Grep(GrepGrounding::new(vec![SearchPattern::Regex(
                    "needle.*".to_owned(),
                )])),
            )
            .unwrap();
    }

    #[test]
    fn workspace_validator_rejects_missing_anchor() {
        let workspace = TempWorkspace::new("grounding-missing-anchor");
        workspace.write("src/lib.rs", "fn unrelated() {}\n");

        let validator = WorkspaceGroundingValidator::new(workspace.path());
        let entry = EntryId::new("entry-a");
        let error = validator
            .validate(
                &entry,
                &Grounding::Telescope(TelescopeGrounding::new(TelescopeAnchor::new(entry.clone()))),
            )
            .unwrap_err();

        assert_eq!(error, GroundingValidationError::MissingAnchor { entry });
    }
}
