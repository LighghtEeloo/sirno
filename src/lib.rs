//! Core library for Sirno.
//!
//! Sirno keeps design as named Markdown entries with exact metadata.
//! The public model follows the repository design:
//! entries are nominal objects, structural links are explicit,
//! and repository witnesses are discovered by entry address.

pub mod artifact;
pub mod anchor;
pub mod charm;
pub mod check;
pub mod config;
pub mod surface;
pub mod entry;
pub mod freeze;
pub mod identifier;
pub mod lake;
pub mod mcp;
pub mod meta;
pub mod mist;
pub mod query;
pub mod render;
mod skills;
pub mod structural;
pub mod tide;
pub mod upstream;
pub mod witness;

pub use crate::anchor::{
    ANCHOR_FILE_NAME, ANCHOR_SCHEMA, AnchorEntry, AnchorEntryMap, AnchorError, AnchorFile,
    SIRNO_CONTROL_DIR_NAME,
};
pub use crate::artifact::{
    ARTIFACT_DIRECTORY_NAME, EntryArtifact, EntryArtifactPath, EntryArtifactPathError,
};
pub use crate::charm::{
    CHARM_MANIFEST_FILE_NAME, CharmBundle, CharmCommandSpec, CharmError, CharmManifest,
    SPELL_CACHE_DIRECTORY,
};
pub use crate::check::{
    CheckDiagnostic, CheckDiagnosticKind, CheckMode, CheckReport, CheckSeverity,
};
pub use crate::config::{
    CONFIG_FILE_NAME, CharmSettings, CheckSettings, ConfigError, LakeSettings, RepoMember,
    RepoSettings, SirnoConfig, TutorialSettings, UpstreamRef, UpstreamSettings,
    UpstreamSettingsMap, WitnessDelimiterSettings, WitnessSettings,
};
pub use crate::entry::{
    Entry, EntryIntrinsicFields, EntryMeta, EntryMetaType, EntryMetadata, EntryParseError,
    EntryStructuralFields, FROZEN_FIELD, FrozenMarker, META_FIELD, RawEntry,
};
pub use crate::freeze::{FreezeError, FrozenPath};
pub use crate::identifier::{EntryAddress, EntryAddressError, EntryAtom, EntryAtomError};
pub use crate::lake::{
    EntryDirectory, EntryDirectoryCheckSettings, EntryDirectoryError, EntryDirectoryReport,
    EntryDirectoryWritePolicy, EntryFileDiagnostic, EntryRenameReport, GenLinkDirectoryReport,
    GlacierReport,
};
pub use crate::meta::{
    IntrinsicFieldMap, META_FILE_NAME, META_FILE_SCHEMA, MetaFieldNameError, MetaFieldRecord,
    MetaFile, MetaRegistry, MetaRegistryError,
};
pub use crate::mist::{
    DEFAULT_MIST_PROJECTION_PATH, MIST_MANIFEST_FILE_NAME, MIST_MANIFEST_SCHEMA,
    MIST_SPEC_DIR_NAME, MistError, MistManifest, MistManifestEntry, MistProjectionSettings,
    MistRenderSettings, MistSelectionSettings, MistSpec, MistStructuralFieldState,
    MistStructuralRenderMap, MistStructuralStateFilter, MistStructuralTargetFilter,
};
pub use crate::query::{EntryQuery, EntryStructuralMatcher, EntryTextTerm, VagueEntryQuery};
pub use crate::render::{
    BEGIN_LINKS_GUARD, END_LINKS_GUARD, GeneratedLinkBody, GeneratedLinkError,
};
pub use crate::structural::{
    StructuralEdgeDirection, StructuralEdgeDirectionParseError, StructuralEdgeIndex,
    StructuralEdgeSettings, StructuralFieldMap, StructuralFieldSettings, StructuralRelationMap,
    StructuralRelationSettings, StructuralRenderMap, StructuralRenderSettings,
    StructuralRippleSettings, StructuralSettings, StructuralTideSettings,
};
pub use crate::tide::{
    TIDE_FILE_NAME, TIDE_FILE_SCHEMA, Tide, TideEntrySnapshot, TideError, TideFile, TideFileError,
    TideResolution, TideSource, TideStatus, TideWorkitem, TideWorkitemParseError,
};
pub use crate::upstream::{
    UPSTREAM_FILE_NAME, UpstreamCrystallizeReport, UpstreamError, UpstreamFile, UpstreamFileError,
    UpstreamGitCache, UpstreamLock, UpstreamLockMap, UpstreamStatus, UpstreamStatusReport,
    UpstreamStatusState,
};
pub use crate::witness::{
    WitnessCheckSettings, WitnessError, WitnessIndex, WitnessRecord, WitnessSpan,
};
