//! CLI grammar and terminal dispatch for the shared command surface.

mod config;
mod entry;
mod freeze;
mod skills;
mod tide;
mod tui;

use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsString;
use std::io::{self, BufRead, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::str::FromStr;

use clap::{ArgGroup, Args, CommandFactory, Parser, Subcommand, ValueEnum};
use clap_complete::{Shell, generate};
use serde::Deserialize;
use thiserror::Error;

use crate::surface::SurfaceContext;
use crate::surface::context::{default_config_path, default_lake_path};
use crate::surface::dto::{
    AnchorOutputFormat, ArtifactAddRequest, ArtifactRemoveRequest, ArtifactRenameRequest,
    CharmListResult, CharmProcessResult, CharmShowResult, EntryNewRequest, EntryPathsRequest,
    LakeInitRequest, LocalProtectionResult, PathRecord, PathSelection, QueryColumnSelection,
    QueryColumns, QueryOutputFormat, QueryRequest, QueryResponse, QueryRun, RgRequest,
    SkillWrapperResult, SpellListResult, StatusMode, StatusOutputFormat, StatusRequest,
    StructuralFilter, StructuralStateFilter, StructuralTarget, TideOutputFormat,
    TideResolveRequest, TideSelectionRequest, TideStatusMode, UpstreamAddRequest,
    UpstreamCrystallizeRequest,
};
use crate::surface::error::CommandError;
use crate::surface::output::{
    OutputStyle, format_human_table_semantic_with_width, format_intrinsic_summary,
    format_muted_text, format_path_table, format_skill_wrapper_table_for_terminal,
    format_success_text, format_warning_text, print_anchor_update_result, print_cli_error,
    print_config_comment_result, print_entry_directory_report, print_json,
    print_mist_intake_result, print_mist_status_result, print_query_column_options,
    print_query_results, print_render_result, print_status_result,
    print_upstream_crystallize_report, print_upstream_status_report, print_witness_records,
};
use crate::surface::rg::{
    is_rg_preprocessor_invocation, rg_args_to_strings, run_rg_preprocessor_from_env,
};
use crate::{
    CheckMode, EntryAddress, EntryAddressError, EntryAtom, EntryIntrinsicFields, TideSource,
    TideStatus, TideWorkitem, TideWorkitemParseError, UpstreamSettings,
};

/// Sirno command-line entry point.
#[derive(Debug, Parser)]
// sirno:witness:cli-interface:begin
#[command(name = "sirno")]
#[command(about = "Manage Sirno design entries")]
#[command(version)]
// sirno:witness:cli-interface:end
pub struct Cli {
    /// Sirno project config file.
    #[arg(short = 'C', long, global = true)]
    config: Option<PathBuf>,
    /// Sirno Lake path override.
    #[arg(short = 'L', long = "lake-path", global = true)]
    lake_path: Option<PathBuf>,
    /// Mute diagnostic detail blocks in human output.
    #[arg(short = 'q', long, global = true)]
    quiet: bool,
    #[command(subcommand)]
    command: Command,
}

/// Supported Sirno commands.
#[derive(Debug, Subcommand)]
enum Command {
    // sirno:witness:project-setup-commands:begin
    /// Create a Sirno config, lake, and skill wrappers.
    Init {
        /// Run non-interactively with the selected init parts.
        #[arg(long)]
        all: bool,
        /// Lake path written to Sirno.toml.
        #[arg(long)]
        lake: Option<PathBuf>,
        /// Skip lake initialization.
        #[arg(long = "no-lake", conflicts_with = "lake")]
        no_lake: bool,
        /// Skip packaged skill wrapper initialization.
        #[arg(long = "no-skills")]
        no_skills: bool,
        /// Link installed wrappers into Claude skills.
        #[arg(long = "claude-skills", conflicts_with = "no_skills")]
        claude_skills: bool,
    },
    // sirno:witness:project-setup-commands:end
    /// Move an entry or the lake path.
    #[command(visible_alias = "mv")]
    Move {
        /// Move target.
        #[command(subcommand)]
        command: MoveCommand,
    },
    /// Manage Sirno Lake entries.
    Entry {
        /// Entry command.
        #[command(subcommand)]
        command: EntryCommand,
    },
    /// Manage lake storage.
    Lake {
        /// Lake command.
        #[command(subcommand)]
        command: LakeCommand,
    },
    /// Manage mist projections.
    Mist {
        /// Mist command.
        #[command(subcommand)]
        command: MistCommand,
    },
    /// Manage Git-backed upstream lakes.
    Upstream {
        /// Upstream command.
        #[command(subcommand)]
        command: UpstreamCommand,
    },
    /// Manage the accepted lake baseline.
    Anchor {
        /// Anchor command.
        #[command(subcommand)]
        command: AnchorCommand,
    },
    // sirno:witness:charm-and-spell-commands:begin
    /// Manage charm artifact bundles.
    Charm {
        /// Charm command.
        #[command(subcommand)]
        command: CharmCommand,
    },
    /// Run and inspect resolved spells.
    Spell {
        /// Spell command.
        #[command(subcommand)]
        command: SpellCommand,
    },
    // sirno:witness:charm-and-spell-commands:end
    /// Manage dependency review worklists for lake edits.
    // sirno:witness:tide-commands:begin
    Tide {
        /// Tide command.
        #[command(subcommand)]
        command: Option<TideCommand>,
    },
    // sirno:witness:tide-commands:end
    // sirno:witness:cli-interface:begin
    // sirno:witness:project-status-commands:begin
    /// Show the current Sirno project status.
    #[command(visible_alias = "st")]
    Status(StatusArgs),
    // sirno:witness:project-status-commands:end
    /// Run an entry operation at the top level.
    #[command(flatten)]
    TopLevelEntry(TopLevelEntryCommand),
    /// Run a mist operation at the top level.
    #[command(flatten)]
    TopLevelMist(TopLevelMistCommand),
    /// Run a tide review operation at the top level.
    #[command(flatten)]
    TopLevelTide(TideReviewCommand),
    /// Utility commands.
    Util {
        /// Utility command.
        #[command(subcommand)]
        command: UtilCommand,
    },
    // sirno:witness:cli-interface:end
}

/// Arguments for the go-to project status command.
// sirno:witness:project-status-commands:begin
#[derive(Debug, Args)]
struct StatusArgs {
    /// Select summary, normal, or full status detail.
    #[arg(long, value_enum, default_value_t)]
    show: StatusMode,
    /// Check boundary used while assembling status.
    #[arg(short = 'm', long, value_enum, default_value_t = CheckModeArg::Review)]
    mode: CheckModeArg,
    /// Output format.
    #[arg(short = 'o', long, value_enum)]
    format: Option<StatusOutputFormat>,
}
// sirno:witness:project-status-commands:end

/// Supported Sirno Lake entry commands.
#[derive(Debug, Subcommand)]
enum EntryCommand {
    /// Run a top-level entry operation under `sirno entry`.
    #[command(flatten)]
    TopLevel(TopLevelEntryCommand),
    /// Rename one entry address and its Sirno references.
    #[command(visible_aliases = ["mv", "move"])]
    Rename(EntryRenameArgs),
    /// Show filesystem paths related to one entry.
    // sirno:witness:entry-commands:begin
    Path(EntryPathsArgs),
    // sirno:witness:entry-commands:end
}

/// Supported top-level Sirno Lake entry commands.
#[derive(Debug, Subcommand)]
enum TopLevelEntryCommand {
    /// Create one Markdown entry.
    // sirno:witness:entry-commands:begin
    New {
        /// Entry address.
        #[arg(value_name = "ENTRY_ADDRESS")]
        id: String,
        /// Intrinsic metadata as FIELD=VALUE.
        #[arg(long = "intrinsic", alias = "metadata", value_name = "FIELD=VALUE")]
        intrinsic: Vec<IntrinsicArgument>,
        /// Structural link target as FIELD=ENTRY_ADDRESS.
        #[arg(long = "structural", value_name = "FIELD=ENTRY_ADDRESS")]
        structural: Vec<StructuralPredicate>,
        /// Initial Markdown body.
        #[arg(short = 'b', long)]
        body: Option<String>,
    },
    // sirno:witness:entry-commands:end
    /// Freeze one current lake entry and make its file read-only.
    // sirno:witness:entry-commands:begin
    Freeze {
        /// Entry address to freeze. Omit this or pass `tui` to open the entry freeze/melt UI.
        #[arg(value_name = "ENTRY_ADDRESS", conflicts_with = "fix_all")]
        id: Option<String>,
        /// Reapply local protection from frozen metadata and immutable checkout state.
        #[arg(long = "fix-all", conflicts_with = "id")]
        fix_all: bool,
        /// Report paths selected by `--fix-all` without changing permissions.
        #[arg(long = "dry-run", requires = "fix_all")]
        dry_run: bool,
    },
    // sirno:witness:entry-commands:end
    /// Melt one Sirno Lake Markdown entry and make its file writable.
    // sirno:witness:entry-commands:begin
    #[command(visible_alias = "unfreeze")]
    Melt {
        /// Entry address to melt. Omit this or pass `tui` to open the entry freeze/melt UI.
        #[arg(value_name = "ENTRY_ADDRESS", conflicts_with = "unsafe_all")]
        id: Option<String>,
        /// Clear every Sirno local protection guard without editing metadata.
        #[arg(long = "unsafe-all", conflicts_with = "id")]
        unsafe_all: bool,
        /// Report paths selected by `--unsafe-all` without changing permissions.
        #[arg(long = "dry-run", requires = "unsafe_all")]
        dry_run: bool,
    },
    // sirno:witness:entry-commands:end
    /// Query Sirno Lake Markdown entries.
    // sirno:witness:entry-commands:begin
    #[command(visible_alias = "q")]
    Query {
        /// Vague text terms matched against entries and structural link target intrinsic values.
        terms: Vec<String>,
        /// Exact text term matched against id, intrinsic field values, and body.
        #[arg(long = "exact-term")]
        exact_terms: Vec<String>,
        /// Structural link target filter as FIELD=ENTRY_ADDRESS[,ENTRY_ADDRESS].
        ///
        /// Different relations narrow results.
        /// Comma-separated values and repeated same-relation filters are alternatives.
        #[arg(long = "has", value_name = "FIELD=ENTRY_ADDRESS[,ENTRY_ADDRESS]")]
        has: Vec<StructuralFilter>,
        /// Structural link state filter as FIELD=present, FIELD=empty, or FIELD=missing.
        ///
        /// Empty means the relation is present with no targets.
        /// Same-relation target filters and state filters are alternatives.
        #[arg(long = "is", value_name = "FIELD=STATE")]
        is: Vec<StructuralStateFilter>,
        /// Optional comma-separated output columns: id, path, intrinsic fields, or structural relations.
        #[arg(long = "columns", alias = "column", value_name = "COLUMNS", num_args = 0..=1)]
        columns: Option<Option<QueryColumns>>,
        /// Output format.
        #[arg(short = 'o', long, value_enum)]
        format: Option<QueryOutputFormat>,
    },
    // sirno:witness:entry-commands:end
    /// Run ripgrep in the configured Sirno Lake.
    // sirno:witness:entry-commands:begin
    Rg {
        /// Include Sirno-owned generated-footer regions in the search.
        #[arg(long = "with-generated-footer")]
        with_generated_footer: bool,
        /// Arguments forwarded to ripgrep before the lake path.
        #[arg(required = true, trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<OsString>,
    },
    // sirno:witness:entry-commands:end
    /// Manage entry-owned artifact files.
    // sirno:witness:entry-commands:begin
    Artifact {
        /// Artifact command.
        #[command(subcommand)]
        command: ArtifactCommand,
    },
    // sirno:witness:entry-commands:end
    /// Show repository witness blocks for one entry address.
    // sirno:witness:entry-commands:begin
    #[command(visible_aliases = ["w", "wit"])]
    Witness {
        /// Entry address used as the witness query key.
        #[arg(value_name = "ENTRY_ADDRESS")]
        id: String,
        /// Print full witness regions instead of only their locations.
        #[arg(short = 'f', long)]
        full: bool,
    },
    // sirno:witness:entry-commands:end
}

/// Supported Sirno Lake commands.
#[derive(Debug, Subcommand)]
enum LakeCommand {
    /// Create a Sirno config and ordinary seed entries.
    // sirno:witness:lake-commands:begin
    Init {
        /// Lake path written to Sirno.toml.
        lake: Option<PathBuf>,
    },
    /// Move the configured lake path.
    #[command(visible_alias = "mv")]
    Move(LakeMoveArgs),
    // sirno:witness:lake-commands:end
}

// sirno:witness:mist-commands:begin
/// Supported Sirno mist commands.
#[derive(Debug, Subcommand)]
enum MistCommand {
    /// Run a top-level mist operation under `sirno mist`.
    #[command(flatten)]
    TopLevel(TopLevelMistCommand),
    /// Show pending mist ripples and stale projection state.
    Status(MistNameArgs),
    /// Intake edited Markdown entry sources from a misty lake into the reservoir.
    Intake(MistNameArgs),
}

/// Supported top-level Sirno mist commands.
#[derive(Debug, Subcommand)]
enum TopLevelMistCommand {
    /// Render Markdown links for a misty lake projection.
    Render(MistRenderArgs),
}

/// Arguments for one named mist.
#[derive(Debug, Args)]
struct MistNameArgs {
    /// Mist name. Omit for the default mist.
    #[arg(value_name = "MIST")]
    mist: Option<String>,
}

/// Arguments for rendering one mist projection.
#[derive(Debug, Args)]
struct MistRenderArgs {
    /// Mist name. Omit for the default mist.
    #[arg(value_name = "MIST")]
    mist: Option<String>,
    /// Report rendered-footer changes without writing files.
    #[arg(short = 'n', long, visible_alias = "dry-run")]
    dry: bool,
    /// JSON render.structural settings used instead of the mist spec for this run.
    #[arg(long = "override-json", value_name = "JSON")]
    override_json: Option<String>,
    /// Render command.
    #[command(subcommand)]
    command: Option<RenderCommand>,
}
// sirno:witness:mist-commands:end

/// Supported top-level move wrappers.
// sirno:witness:cli-interface:begin
#[derive(Debug, Subcommand)]
enum MoveCommand {
    /// Rename one entry address and its Sirno references.
    Entry(EntryRenameArgs),
    /// Move the configured lake path.
    Lake(LakeMoveArgs),
}

/// Arguments for renaming one entry address and its Sirno references.
#[derive(Debug, Args)]
struct EntryRenameArgs {
    /// Existing entry address.
    #[arg(value_name = "OLD_ENTRY_ADDRESS")]
    old_id: String,
    /// New entry address.
    #[arg(value_name = "NEW_ENTRY_ADDRESS")]
    new_id: String,
}

/// Arguments for moving the configured lake path.
#[derive(Debug, Args)]
struct LakeMoveArgs {
    /// New lake path written to Sirno.toml.
    lake: PathBuf,
}

// sirno:witness:cli-interface:end

/// Arguments for entry address lookup.
// sirno:witness:entry-commands:begin
#[derive(Clone, Debug, Args)]
struct EntryPathsArgs {
    /// Entry address whose paths should be shown.
    #[arg(value_name = "ENTRY_ADDRESS")]
    id: String,
    /// Show the Sirno Lake Markdown entry file path.
    #[arg(long = "entry")]
    show_entry: bool,
    /// Show lake entry artifact paths.
    #[arg(long = "artifact")]
    show_artifact: bool,
    /// Print absolute paths.
    #[arg(long)]
    absolute: bool,
    /// Output format.
    #[arg(short = 'o', long, value_enum)]
    format: Option<PathOutputFormat>,
}
// sirno:witness:entry-commands:end

/// CLI path lookup output renderer.
// sirno:witness:entry-commands:begin
#[derive(Clone, Copy, Debug, Default, ValueEnum)]
enum PathOutputFormat {
    /// Print a JSON array of path records.
    Json,
    /// Print a Unicode table.
    #[default]
    Human,
    /// Print only paths, one per line.
    Paths,
}
// sirno:witness:entry-commands:end

/// Intrinsic metadata parsed from `FIELD=VALUE`.
#[derive(Clone, Debug, PartialEq, Eq)]
struct IntrinsicArgument {
    field: String,
    value: String,
}

impl FromStr for IntrinsicArgument {
    type Err = IntrinsicArgumentParseError;

    fn from_str(raw: &str) -> Result<Self, Self::Err> {
        let Some((field, value)) = raw.split_once('=') else {
            return Err(IntrinsicArgumentParseError::MissingEquals);
        };
        if field.is_empty() {
            return Err(IntrinsicArgumentParseError::EmptyField);
        }
        Ok(Self { field: field.to_owned(), value: value.to_owned() })
    }
}

/// Error raised while parsing one intrinsic metadata `FIELD=VALUE` argument.
#[derive(Debug, Error)]
enum IntrinsicArgumentParseError {
    /// The argument does not contain the field-value separator.
    #[error("expected FIELD=VALUE")]
    MissingEquals,
    /// The intrinsic field name is empty.
    #[error("intrinsic field name must not be empty")]
    EmptyField,
}

/// Structural link predicate parsed from `FIELD=ENTRY_ADDRESS`.
#[derive(Clone, Debug, PartialEq, Eq)]
struct StructuralPredicate {
    field: String,
    target: EntryAddress,
}

impl FromStr for StructuralPredicate {
    type Err = StructuralPredicateParseError;

    fn from_str(raw: &str) -> Result<Self, Self::Err> {
        let Some((field, target)) = raw.split_once('=') else {
            return Err(StructuralPredicateParseError::MissingEquals);
        };
        if field.is_empty() {
            return Err(StructuralPredicateParseError::EmptyField);
        }
        let target = EntryAddress::new(target)?;
        Ok(Self { field: field.to_owned(), target })
    }
}

/// Error raised while parsing one structural link `FIELD=ENTRY_ADDRESS` argument.
#[derive(Debug, Error)]
enum StructuralPredicateParseError {
    /// The argument does not contain the field-target separator.
    #[error("expected FIELD=ENTRY_ADDRESS")]
    MissingEquals,
    /// The link relation name is empty.
    #[error("link relation name must not be empty")]
    EmptyField,
    /// The target entry address is invalid.
    #[error(transparent)]
    EntryAddress(#[from] EntryAddressError),
}

/// Supported entry artifact commands.
// sirno:witness:entry-commands:begin
#[derive(Debug, Subcommand)]
enum ArtifactCommand {
    /// List artifacts owned by one entry.
    List {
        /// Entry address whose artifacts should be listed.
        #[arg(value_name = "ENTRY_ADDRESS")]
        id: String,
    },
    /// Copy a file into one entry's artifact tree.
    Add {
        /// Entry address that will own the artifact.
        #[arg(value_name = "ENTRY_ADDRESS")]
        id: String,
        /// Source file to copy.
        source: PathBuf,
        /// Owner-relative artifact path.
        artifact_path: Option<PathBuf>,
    },
    /// Rename one artifact path owned by an entry.
    #[command(visible_aliases = ["mv", "move"])]
    Rename {
        /// Entry address that owns the artifact.
        #[arg(value_name = "ENTRY_ADDRESS")]
        id: String,
        /// Existing owner-relative artifact path.
        old_path: PathBuf,
        /// New owner-relative artifact path.
        new_path: PathBuf,
    },
    /// Remove one artifact owned by an entry.
    #[command(visible_aliases = ["rm", "delete"])]
    Remove {
        /// Entry address that owns the artifact.
        #[arg(value_name = "ENTRY_ADDRESS")]
        id: String,
        /// Owner-relative artifact path to remove.
        artifact_path: PathBuf,
    },
}
// sirno:witness:entry-commands:end

/// Supported charm commands.
// sirno:witness:charm-and-spell-commands:begin
#[derive(Debug, Subcommand)]
enum CharmCommand {
    /// List entries that contain a charm manifest.
    List,
    /// Show one charm.
    Show {
        /// Entry address that owns the charm.
        #[arg(value_name = "ENTRY_ADDRESS")]
        id: String,
    },
    /// Enable one charm in project config.
    Enable {
        /// Entry address that owns the charm.
        #[arg(value_name = "ENTRY_ADDRESS")]
        id: String,
    },
    /// Disable one charm in project config.
    Disable {
        /// Entry address that owns the charm.
        #[arg(value_name = "ENTRY_ADDRESS")]
        id: String,
    },
    /// Run a charm setup command.
    Setup {
        /// Entry address that owns the charm.
        #[arg(value_name = "ENTRY_ADDRESS")]
        id: String,
    },
    /// Run a charm check command.
    Check {
        /// Entry address that owns the charm.
        #[arg(value_name = "ENTRY_ADDRESS")]
        id: String,
    },
    /// Build one source charm.
    Build {
        /// Entry address that owns the charm.
        #[arg(value_name = "ENTRY_ADDRESS")]
        id: String,
    },
    /// Remove spell cache state for one charm.
    Clean {
        /// Entry address that owns the charm.
        #[arg(value_name = "ENTRY_ADDRESS")]
        id: String,
    },
}

/// Supported spell commands.
#[derive(Debug, Subcommand)]
enum SpellCommand {
    /// List spells from enabled charms.
    List,
    /// Show the spell resolved from one charm.
    Show {
        /// Entry address that owns the charm.
        #[arg(value_name = "ENTRY_ADDRESS")]
        id: String,
    },
    /// Run the spell resolved from one charm.
    Run {
        /// Entry address that owns the charm.
        #[arg(value_name = "ENTRY_ADDRESS")]
        id: String,
    },
}
// sirno:witness:charm-and-spell-commands:end

/// CLI representation of check boundaries.
#[derive(Clone, Copy, Debug, ValueEnum)]
enum CheckModeArg {
    /// Editing boundary: dangling references are warnings.
    Edit,
    /// Review boundary: dangling references are errors.
    Review,
}

impl From<CheckModeArg> for CheckMode {
    fn from(value: CheckModeArg) -> Self {
        match value {
            | CheckModeArg::Edit => CheckMode::Edit,
            | CheckModeArg::Review => CheckMode::Review,
        }
    }
}

// sirno:witness:upstream-commands:begin
/// Supported upstream lake commands.
#[derive(Debug, Subcommand)]
enum UpstreamCommand {
    /// Add or replace one Git-backed upstream and crystallize it.
    Add(UpstreamAddArgs),
    /// Remove one upstream declaration and its glacier.
    Remove {
        /// Glacier domain.
        #[arg(value_name = "DOMAIN")]
        domain: String,
    },
    /// Crystallize configured upstream lakes into glaciers.
    Crystallize {
        /// Glacier domain. Omit to crystallize every upstream.
        #[arg(value_name = "DOMAIN")]
        domain: Option<String>,
        /// Use only existing lock records and cache mirrors.
        #[arg(long)]
        locked: bool,
    },
    /// Refresh upstream locks and glaciers.
    Update {
        /// Glacier domain. Omit to update every upstream.
        #[arg(value_name = "DOMAIN")]
        domain: Option<String>,
    },
    /// Show upstream lock and cache status.
    Status {
        /// Output format.
        #[arg(short = 'o', long, value_enum)]
        format: Option<UpstreamOutputFormat>,
    },
}

/// Arguments for adding or replacing one upstream lake declaration.
#[derive(Debug, Args)]
#[command(group(
    ArgGroup::new("selector")
        .required(true)
        .multiple(false)
        .args(["branch", "tag", "rev"])
))]
struct UpstreamAddArgs {
    /// Glacier domain used as the crystallized entry-address prefix.
    #[arg(value_name = "DOMAIN")]
    domain: String,
    /// Git URI or local Git repository source accepted by Git.
    #[arg(long = "git", value_name = "SOURCE")]
    git: String,
    /// Branch name to resolve.
    #[arg(long)]
    branch: Option<String>,
    /// Tag name to resolve.
    #[arg(long)]
    tag: Option<String>,
    /// Commit-ish to resolve.
    #[arg(long)]
    rev: Option<String>,
    /// Project root directory inside the Git tree.
    #[arg(long)]
    project: Option<PathBuf>,
    /// Project config manifest path relative to `--project`.
    #[arg(long)]
    manifest: Option<PathBuf>,
    /// Upstream mist that selects the crystallized entries.
    #[arg(long)]
    mist: Option<String>,
}
// sirno:witness:upstream-commands:end

/// CLI upstream output renderer.
#[derive(Clone, Copy, Debug, Default, ValueEnum)]
enum UpstreamOutputFormat {
    /// Print JSON for machine-oriented callers.
    Json,
    /// Print terminal-oriented human text.
    #[default]
    Human,
}

/// Tide item selector parsed from one CLI argument.
#[derive(Clone, Debug, PartialEq, Eq)]
enum TideItemSelector {
    /// Select every open workitem whose neighbor matches this entry.
    Neighbor(EntryAddress),
    /// Select one full workitem tuple.
    Workitem(TideWorkitem),
}

impl FromStr for TideItemSelector {
    type Err = TideItemSelectorParseError;

    fn from_str(raw: &str) -> Result<Self, Self::Err> {
        if raw.contains(',') {
            return Ok(Self::Workitem(raw.parse()?));
        }
        Ok(Self::Neighbor(EntryAddress::new(raw)?))
    }
}

/// Error raised while parsing one tide item selector.
#[derive(Debug, Error)]
enum TideItemSelectorParseError {
    /// Entry address parsing failed.
    #[error(transparent)]
    EntryAddress(#[from] EntryAddressError),
    /// Full workitem parsing failed.
    #[error(transparent)]
    Workitem(#[from] TideWorkitemParseError),
}

/// Supported tide commands.
#[derive(Debug, Subcommand)]
// sirno:witness:tide:begin
enum TideCommand {
    /// Show tide review status.
    Status {
        /// Select review entries, full open workitems, or all workitems.
        #[arg(long, value_enum, default_value_t = TideStatusMode::Review)]
        show: TideStatusMode,
        /// Group human output by wave or review entry.
        #[arg(long, value_enum, default_value_t = TideStatusGrouping::Entry)]
        by: TideStatusGrouping,
        /// Output format.
        #[arg(short = 'o', long, value_enum)]
        format: Option<TideOutputFormat>,
    },
    /// Open the interactive tide resolution UI.
    Tui,
    /// Run a tide review operation.
    #[command(flatten)]
    Review(TideReviewCommand),
    /// Clear all tide resolutions from the Tide file.
    Reset,
}
// sirno:witness:tide:end

// sirno:witness:anchor-commands:begin
/// Supported Anchor commands.
#[derive(Debug, Subcommand)]
enum AnchorCommand {
    /// Accept the current lake as the new anchor baseline.
    Update {
        /// Output format.
        #[arg(short = 'o', long, value_enum)]
        format: Option<AnchorOutputFormat>,
    },
}
// sirno:witness:anchor-commands:end

/// Human grouping for tide status output.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, ValueEnum)]
enum TideStatusGrouping {
    /// Group by the changed ripple entry that caused review.
    Wave,
    /// Group by the entry that needs review.
    #[default]
    Entry,
}

/// Supported tide review commands.
#[derive(Debug, Subcommand)]
// sirno:witness:tide-commands:begin
enum TideReviewCommand {
    /// Resolve tide workitems.
    Resolve(ResolveArgs),
    /// Remove resolved marks from tide workitems.
    #[command(visible_alias = "reopen")]
    Unresolve(UnresolveArgs),
}
// sirno:witness:tide-commands:end

/// Arguments for resolving tide workitems.
// sirno:witness:tide-commands:begin
#[derive(Debug, Args)]
struct ResolveArgs {
    /// Resolve workitems whose neighbor also appears in the current ripple set.
    #[arg(long, conflicts_with_all = ["items", "json"])]
    infer: bool,
    /// JSON array of full workitem tuples.
    #[arg(long, conflicts_with_all = ["infer", "items"])]
    json: Option<String>,
    /// Entry addresses or full workitem tuples.
    #[arg(required_unless_present_any = ["infer", "json"])]
    items: Vec<TideItemSelector>,
}

/// Arguments for removing resolved marks from tide workitems.
#[derive(Debug, Args)]
struct UnresolveArgs {
    /// Entry addresses or full workitem tuples.
    #[arg(required = true)]
    items: Vec<TideItemSelector>,
}
// sirno:witness:tide-commands:end

// sirno:witness:mist-commands:begin
/// Supported rendered-footer commands.
#[derive(Debug, Subcommand)]
enum RenderCommand {
    /// Delete generated Markdown link footers.
    Delete,
}
// sirno:witness:mist-commands:end

/// CLI shell target for completion generation.
#[derive(Clone, Copy, Debug, ValueEnum)]
enum CompletionShell {
    /// Bash completion script.
    Bash,
    /// Elvish completion script.
    Elvish,
    /// Fish completion script.
    Fish,
    /// PowerShell completion script.
    #[value(name = "powershell", alias = "power-shell")]
    PowerShell,
    /// Zsh completion script.
    Zsh,
}

impl From<CompletionShell> for Shell {
    fn from(value: CompletionShell) -> Self {
        match value {
            | CompletionShell::Bash => Shell::Bash,
            | CompletionShell::Elvish => Shell::Elvish,
            | CompletionShell::Fish => Shell::Fish,
            | CompletionShell::PowerShell => Shell::PowerShell,
            | CompletionShell::Zsh => Shell::Zsh,
        }
    }
}

/// Supported utility commands.
#[derive(Debug, Subcommand)]
enum UtilCommand {
    // sirno:witness:utility-commands:begin
    /// Maintain Sirno.toml comments and sections.
    Config {
        /// Config utility command.
        #[command(subcommand)]
        command: Option<ConfigUtilityCommand>,
    },
    // sirno:witness:utility-commands:end
    // sirno:witness:utility-commands:begin
    /// Maintain common entry defaults.
    Entry {
        /// Entry utility command.
        #[command(subcommand)]
        command: Option<EntryUtilityCommand>,
    },
    // sirno:witness:utility-commands:end
    /// Generate a shell completion script.
    Completion {
        /// Shell whose completion script should be generated.
        #[arg(value_enum)]
        shell: CompletionShell,
    },
    // sirno:witness:utility-commands:begin
    /// Manage packaged Sirno skill wrappers.
    Skills {
        /// Skill wrapper command.
        #[command(subcommand)]
        command: Option<SkillCommand>,
    },
    // sirno:witness:utility-commands:end
    // sirno:witness:utility-commands:begin
    /// Run the Sirno MCP server over stdio.
    Mcp,
    // sirno:witness:utility-commands:end
}

// sirno:witness:utility-commands:begin
/// Supported config utility commands.
#[derive(Debug, Subcommand)]
enum ConfigUtilityCommand {
    /// Open the interactive Sirno.toml maintenance UI.
    Tui,
    /// Print missing canonical comments without writing the file.
    Check,
    /// Rewrite Sirno.toml with canonical comments when comments are missing.
    Fix,
}
// sirno:witness:utility-commands:end

/// Supported entry-default utility commands.
// sirno:witness:utility-commands:begin
#[derive(Debug, Subcommand)]
enum EntryUtilityCommand {
    /// Open the interactive entry default maintenance UI.
    Tui,
}
// sirno:witness:utility-commands:end

/// Supported skill wrapper utility commands.
// sirno:witness:utility-commands:begin
#[derive(Debug, Subcommand)]
enum SkillCommand {
    /// Open the interactive skill wrapper maintenance UI.
    Tui(SkillCommandArgs),
    /// Install bundled wrappers into `.agents/skills/sirno-*`.
    Init(SkillCommandArgs),
    /// Check installed wrappers against bundled wrappers.
    Check(SkillCommandArgs),
    /// List bundled wrappers and package targets.
    List(SkillCommandArgs),
}
// sirno:witness:utility-commands:end

impl Default for SkillCommand {
    fn default() -> Self {
        Self::Tui(SkillCommandArgs { claude_skills: false })
    }
}

/// Options shared by skill wrapper utility commands.
#[derive(Debug, Args)]
struct SkillCommandArgs {
    /// Include Claude skill links under `.claude/skills`.
    #[arg(long = "claude-skills")]
    claude_skills: bool,
}

/// Run Sirno from the current process environment.
///
/// This is the binary entry point extracted as a library function.
pub fn run_cli_from_env() -> ExitCode {
    if is_rg_preprocessor_invocation() {
        return match run_rg_preprocessor_from_env() {
            | Ok(code) => code,
            | Err(error) => {
                print_cli_error(&error);
                ExitCode::FAILURE
            }
        };
    }

    match Cli::parse().run() {
        | Ok(code) => code,
        | Err(error) => {
            print_cli_error(&error);
            ExitCode::FAILURE
        }
    }
}

impl Cli {
    /// Execute the parsed CLI command.
    pub fn run(self) -> Result<ExitCode, CommandError> {
        let config_path = self.config.unwrap_or_else(default_config_path);
        let lake_path = self.lake_path;
        let quiet = self.quiet;
        match self.command {
            // sirno:witness:project-setup-commands:begin
            | Command::Init { all, lake, no_lake, no_skills, claude_skills } => {
                let request = TopLevelInitRequest {
                    lake,
                    init_lake: !no_lake,
                    init_skills: !no_skills,
                    init_claude_skills: claude_skills,
                };
                if all {
                    run_top_level_init(request, &config_path, lake_path.as_deref())
                } else {
                    run_interactive_top_level_init(request, &config_path, lake_path.as_deref())
                }
            }
            // sirno:witness:project-setup-commands:end
            | Command::Move { command } => command.run(&config_path, lake_path.as_deref()),
            | Command::Entry { command } => command.run(&config_path, lake_path.as_deref()),
            | Command::Lake { command } => command.run(&config_path, lake_path.as_deref()),
            | Command::Mist { command } => command.run(&config_path, lake_path.as_deref()),
            | Command::Upstream { command } => command.run(&config_path, lake_path.as_deref()),
            | Command::Anchor { command } => command.run(&config_path, lake_path.as_deref()),
            | Command::Charm { command } => command.run(&config_path, lake_path.as_deref()),
            | Command::Spell { command } => command.run(&config_path, lake_path.as_deref()),
            | Command::Tide { command } => {
                // sirno:witness:tide-commands:begin
                command.unwrap_or(TideCommand::Tui).run(&config_path, lake_path.as_deref())
                // sirno:witness:tide-commands:end
            }
            // sirno:witness:project-status-commands:begin
            | Command::Status(args) => args.run(&config_path, lake_path.as_deref(), quiet),
            // sirno:witness:project-status-commands:end
            | Command::TopLevelEntry(command) => command.run(&config_path, lake_path.as_deref()),
            | Command::TopLevelMist(command) => command.run(&config_path, lake_path.as_deref()),
            | Command::TopLevelTide(command) => command.run(&config_path, lake_path.as_deref()),
            | Command::Util { command } => command.run(&config_path, lake_path.as_deref()),
        }
    }
}

impl MoveCommand {
    fn run(self, config_path: &Path, lake_path: Option<&Path>) -> Result<ExitCode, CommandError> {
        match self {
            | Self::Entry(args) => args.run(config_path, lake_path),
            | Self::Lake(args) => args.run(config_path),
        }
    }
}

// sirno:witness:project-setup-commands:begin
#[derive(Debug)]
struct TopLevelInitRequest {
    lake: Option<PathBuf>,
    init_lake: bool,
    init_skills: bool,
    init_claude_skills: bool,
}
// sirno:witness:project-setup-commands:end

#[derive(Clone, Copy, Debug)]
enum PromptDefault {
    Yes,
    No,
}

impl PromptDefault {
    fn answer(self) -> bool {
        match self {
            | Self::Yes => true,
            | Self::No => false,
        }
    }

    fn suffix(self, style: OutputStyle) -> String {
        match self {
            | Self::Yes => {
                format!("[{}/{}]", format_success_text("Y", style), format_muted_text("n", style))
            }
            | Self::No => {
                format!("[{}/{}]", format_success_text("y", style), format_muted_text("N", style))
            }
        }
    }
}

fn run_interactive_top_level_init(
    request: TopLevelInitRequest, config_path: &Path, lake_path: Option<&Path>,
) -> Result<ExitCode, CommandError> {
    let stdin = io::stdin();
    let mut input = stdin.lock();
    let mut output = anstream::stdout();
    run_prompted_top_level_init_with_style(
        request,
        config_path,
        lake_path,
        &mut input,
        &mut output,
        OutputStyle::Styled,
    )
}

#[cfg(any())]
fn run_prompted_top_level_init<R: BufRead, W: Write>(
    request: TopLevelInitRequest, config_path: &Path, lake_path: Option<&Path>, input: &mut R,
    output: &mut W,
) -> Result<ExitCode, CommandError> {
    run_prompted_top_level_init_with_style(
        request,
        config_path,
        lake_path,
        input,
        output,
        OutputStyle::Plain,
    )
}

// sirno:witness:project-setup-commands:begin
fn run_prompted_top_level_init_with_style<R: BufRead, W: Write>(
    mut request: TopLevelInitRequest, config_path: &Path, lake_path: Option<&Path>, input: &mut R,
    output: &mut W, style: OutputStyle,
) -> Result<ExitCode, CommandError> {
    writeln!(output, "Interactive Sirno init").map_err(CommandError::InteractiveInit)?;

    if request.init_lake {
        request.init_lake =
            prompt_yes_no(input, output, "Initialize the lake?", PromptDefault::Yes, style)?;
        if request.init_lake && request.lake.is_none() && lake_path.is_none() {
            request.lake = prompt_default_path(
                input,
                output,
                "Use default lake path",
                "lake path",
                default_lake_path(config_path),
                style,
            )?;
        }
    }

    if request.init_skills {
        request.init_skills = prompt_yes_no(
            input,
            output,
            "Install packaged skill wrappers?",
            PromptDefault::Yes,
            style,
        )?;
        if request.init_skills && !request.init_claude_skills {
            request.init_claude_skills = prompt_yes_no(
                input,
                output,
                "Link wrappers into Claude skills?",
                PromptDefault::No,
                style,
            )?;
        }
    } else {
        request.init_claude_skills = false;
    }

    print_init_plan(output, &request, config_path, lake_path, style)?;
    let confirmed =
        prompt_yes_no(input, output, "Apply this init plan?", PromptDefault::No, style)?;
    if !confirmed {
        writeln!(output, "{}", format_warning_text("init cancelled", style))
            .map_err(CommandError::InteractiveInit)?;
        return Ok(ExitCode::SUCCESS);
    }

    run_top_level_init(request, config_path, lake_path)
}
// sirno:witness:project-setup-commands:end

fn prompt_yes_no<R: BufRead, W: Write>(
    input: &mut R, output: &mut W, question: &str, default: PromptDefault, style: OutputStyle,
) -> Result<bool, CommandError> {
    loop {
        write!(output, "{question} {} ", default.suffix(style))
            .map_err(CommandError::InteractiveInit)?;
        output.flush().map_err(CommandError::InteractiveInit)?;

        let answer = read_prompt_line(input)?;
        match answer.trim().to_ascii_lowercase().as_str() {
            | "" => return Ok(default.answer()),
            | "y" | "yes" => return Ok(true),
            | "n" | "no" => return Ok(false),
            | _ => {
                writeln!(output, "{}", format_warning_text("Please answer yes or no.", style))
                    .map_err(CommandError::InteractiveInit)?;
            }
        }
    }
}

fn prompt_default_path<R: BufRead, W: Write>(
    input: &mut R, output: &mut W, default_question: &str, value_question: &str, default: PathBuf,
    style: OutputStyle,
) -> Result<Option<PathBuf>, CommandError> {
    let question = format!("{default_question} `{}`?", default.display());
    if prompt_yes_no(input, output, &question, PromptDefault::Yes, style)? {
        return Ok(None);
    }

    loop {
        write!(output, "{value_question}: ").map_err(CommandError::InteractiveInit)?;
        output.flush().map_err(CommandError::InteractiveInit)?;
        let answer = read_prompt_line(input)?;
        let path = answer.trim();
        if !path.is_empty() {
            return Ok(Some(PathBuf::from(path)));
        }
        writeln!(output, "{}", format_warning_text("Please enter a path.", style))
            .map_err(CommandError::InteractiveInit)?;
    }
}

fn read_prompt_line<R: BufRead>(input: &mut R) -> Result<String, CommandError> {
    let mut answer = String::new();
    let bytes = input.read_line(&mut answer).map_err(CommandError::InteractiveInit)?;
    if bytes == 0 {
        return Err(CommandError::InteractiveInitEof);
    }
    Ok(answer)
}

fn print_init_plan<W: Write>(
    output: &mut W, request: &TopLevelInitRequest, config_path: &Path, lake_path: Option<&Path>,
    style: OutputStyle,
) -> Result<(), CommandError> {
    writeln!(output).map_err(CommandError::InteractiveInit)?;
    writeln!(output, "Init plan:").map_err(CommandError::InteractiveInit)?;
    if request.init_lake {
        writeln!(
            output,
            "  lake: {} ({})",
            format_init_choice(true, style),
            planned_lake_path(request, config_path, lake_path).display()
        )
        .map_err(CommandError::InteractiveInit)?;
    } else {
        writeln!(output, "  lake: {}", format_init_choice(false, style))
            .map_err(CommandError::InteractiveInit)?;
    }

    let skills = format_init_choice(request.init_skills, style);
    writeln!(output, "  skill wrappers: {skills}").map_err(CommandError::InteractiveInit)?;
    let claude_skills = format_init_choice(request.init_claude_skills, style);
    writeln!(output, "  Claude skill links: {claude_skills}").map_err(CommandError::InteractiveInit)
}

fn format_init_choice(value: bool, style: OutputStyle) -> String {
    if value { format_success_text("yes", style) } else { format_muted_text("no", style) }
}

fn planned_lake_path(
    request: &TopLevelInitRequest, config_path: &Path, lake_path: Option<&Path>,
) -> PathBuf {
    request
        .lake
        .clone()
        .or_else(|| lake_path.map(Path::to_path_buf))
        .unwrap_or_else(|| default_lake_path(config_path))
}

// sirno:witness:project-setup-commands:begin
fn run_top_level_init(
    request: TopLevelInitRequest, config_path: &Path, lake_path: Option<&Path>,
) -> Result<ExitCode, CommandError> {
    let mut initialized = false;
    if request.init_lake {
        run_lake_init(request.lake, config_path, lake_path)?;
        initialized = true;
    }
    if request.init_skills {
        run_skill_wrappers_init(config_path, request.init_claude_skills)?;
        initialized = true;
    }
    if !initialized {
        anstream::println!("{}", format_muted_text("nothing initialized", OutputStyle::Styled));
    }
    Ok(ExitCode::SUCCESS)
}
// sirno:witness:project-setup-commands:end

fn run_lake_init(
    lake: Option<PathBuf>, config_path: &Path, lake_path: Option<&Path>,
) -> Result<ExitCode, CommandError> {
    let result = SurfaceContext::from_cli_paths(config_path, lake_path)
        .lake_init(LakeInitRequest { lake })?;
    println!("{}", result.message);
    Ok(ExitCode::SUCCESS)
}

impl EntryCommand {
    fn run(self, config_path: &Path, lake_path: Option<&Path>) -> Result<ExitCode, CommandError> {
        match self {
            | EntryCommand::TopLevel(command) => command.run(config_path, lake_path),
            | EntryCommand::Rename(args) => args.run(config_path, lake_path),
            | EntryCommand::Path(args) => {
                let records = entry_path_records(config_path, lake_path, &args)?;
                print_path_records(&records, args.format.unwrap_or_default())?;
                Ok(ExitCode::SUCCESS)
            }
        }
    }
}

impl EntryRenameArgs {
    fn run(self, config_path: &Path, lake_path: Option<&Path>) -> Result<ExitCode, CommandError> {
        let old_id = EntryAddress::new(&self.old_id)?;
        let new_id = EntryAddress::new(&self.new_id)?;
        let result =
            SurfaceContext::from_cli_paths(config_path, lake_path).entry_rename(old_id, new_id)?;
        println!("{}", result.message);
        println!("updated {} paths", result.updated_paths.len());
        Ok(ExitCode::SUCCESS)
    }
}

fn print_local_protection_result(result: &LocalProtectionResult, warning: &str) {
    println!("{}", format_warning_text(warning, OutputStyle::Styled));
    for path in &result.paths {
        println!("{path}");
    }
    println!("{}", result.message);
}

impl TopLevelEntryCommand {
    fn run(self, config_path: &Path, lake_path: Option<&Path>) -> Result<ExitCode, CommandError> {
        match self {
            | TopLevelEntryCommand::New { id, intrinsic, structural, body } => {
                let id = EntryAddress::new(&id)?;
                let intrinsic = intrinsic
                    .into_iter()
                    .map(|field| (field.field, field.value))
                    .collect::<EntryIntrinsicFields>();
                let structural = structural
                    .into_iter()
                    .map(|target| StructuralTarget { field: target.field, target: target.target })
                    .collect();
                let result = SurfaceContext::from_cli_paths(config_path, lake_path)
                    .entry_new(EntryNewRequest { id, intrinsic, structural, body })?;
                println!("{}", result.message);
                Ok(ExitCode::SUCCESS)
            }
            | TopLevelEntryCommand::Freeze { id, fix_all, dry_run } => {
                if fix_all {
                    let result = SurfaceContext::from_cli_paths(config_path, lake_path)
                        .entry_freeze_fix_all(dry_run)?;
                    print_local_protection_result(
                        &result,
                        "WARNING: freeze --fix-all rewrites Sirno local filesystem protection.",
                    );
                    return Ok(ExitCode::SUCCESS);
                }
                let Some(id) = id else {
                    return freeze::run(
                        config_path,
                        lake_path,
                        freeze::EntryFreezeTuiAction::Freeze,
                    );
                };
                if id == "tui" {
                    return freeze::run(
                        config_path,
                        lake_path,
                        freeze::EntryFreezeTuiAction::Freeze,
                    );
                }
                let id = EntryAddress::new(&id)?;
                let result =
                    SurfaceContext::from_cli_paths(config_path, lake_path).entry_freeze(id)?;
                println!("{}", result.message);
                Ok(ExitCode::SUCCESS)
            }
            | TopLevelEntryCommand::Melt { id, unsafe_all, dry_run } => {
                if unsafe_all {
                    let result = SurfaceContext::from_cli_paths(config_path, lake_path)
                        .entry_melt_unsafe_all(dry_run)?;
                    print_local_protection_result(
                        &result,
                        "DANGER: melt --unsafe-all makes Sirno-protected paths writable and deletable.",
                    );
                    return Ok(ExitCode::SUCCESS);
                }
                let Some(id) = id else {
                    return freeze::run(config_path, lake_path, freeze::EntryFreezeTuiAction::Melt);
                };
                if id == "tui" {
                    return freeze::run(config_path, lake_path, freeze::EntryFreezeTuiAction::Melt);
                }
                let id = EntryAddress::new(&id)?;
                let result =
                    SurfaceContext::from_cli_paths(config_path, lake_path).entry_melt(id)?;
                println!("{}", result.message);
                Ok(ExitCode::SUCCESS)
            }
            | TopLevelEntryCommand::Query { terms, exact_terms, has, is, columns, format } => {
                let request = QueryRequest {
                    terms,
                    exact_terms,
                    has,
                    is,
                    columns: match columns {
                        | None => QueryColumnSelection::Default,
                        | Some(None) => QueryColumnSelection::Options,
                        | Some(Some(columns)) => QueryColumnSelection::Selected(columns),
                    },
                };
                let run = SurfaceContext::from_cli_paths(config_path, lake_path)
                    .query_entries(request)?;
                let format = format.unwrap_or_default();
                let results = match run {
                    | QueryRun::ColumnOptions(columns) => {
                        print_query_column_options(&columns, format)?;
                        return Ok(ExitCode::SUCCESS);
                    }
                    | QueryRun::InvalidLake { columns, report } => {
                        match format {
                            | QueryOutputFormat::Json => {
                                print_json(&QueryResponse::invalid_lake(columns, &report))?;
                            }
                            | QueryOutputFormat::Human => print_entry_directory_report(&report),
                        }
                        return Ok(ExitCode::FAILURE);
                    }
                    | QueryRun::Results(results) => results,
                };
                print_query_results(&results, format)?;
                Ok(ExitCode::SUCCESS)
            }
            | TopLevelEntryCommand::Rg { with_generated_footer, args } => {
                let args = rg_args_to_strings(args)?;
                let result = SurfaceContext::from_cli_paths(config_path, lake_path)
                    .entry_rg(RgRequest { with_generated_footer, args })?;
                print!("{}", result.stdout);
                eprint!("{}", result.stderr);
                Ok(ExitCode::from(result.exit_code))
            }
            | TopLevelEntryCommand::Artifact { command } => command.run(config_path, lake_path),
            | TopLevelEntryCommand::Witness { id, full } => {
                run_witness_command(config_path, lake_path, &id, full)
            }
        }
    }
}

impl LakeCommand {
    fn run(self, config_path: &Path, lake_path: Option<&Path>) -> Result<ExitCode, CommandError> {
        match self {
            | LakeCommand::Init { lake } => run_lake_init(lake, config_path, lake_path),
            | LakeCommand::Move(args) => args.run(config_path),
        }
    }
}

// sirno:witness:mist-commands:begin
impl MistCommand {
    fn run(self, config_path: &Path, lake_path: Option<&Path>) -> Result<ExitCode, CommandError> {
        match self {
            | MistCommand::Status(args) => args.run_status(config_path, lake_path),
            | MistCommand::Intake(args) => args.run_intake(config_path, lake_path),
            | MistCommand::TopLevel(command) => command.run(config_path, lake_path),
        }
    }
}
// sirno:witness:mist-commands:end

impl LakeMoveArgs {
    // sirno:witness:lake-commands:begin
    fn run(self, config_path: &Path) -> Result<ExitCode, CommandError> {
        let result = SurfaceContext::new(config_path.to_path_buf()).lake_move(self.lake)?;
        println!("{}", result.message);
        Ok(ExitCode::SUCCESS)
    }
    // sirno:witness:lake-commands:end
}

// sirno:witness:mist-commands:begin
impl TopLevelMistCommand {
    fn run(self, config_path: &Path, lake_path: Option<&Path>) -> Result<ExitCode, CommandError> {
        match self {
            | TopLevelMistCommand::Render(args) => args.run(config_path, lake_path),
        }
    }
}

impl MistNameArgs {
    fn run_status(
        self, config_path: &Path, lake_path: Option<&Path>,
    ) -> Result<ExitCode, CommandError> {
        let mist = self.mist.as_deref().map(entry_atom).transpose()?;
        let result = SurfaceContext::from_cli_paths(config_path, lake_path).mist_status(mist)?;
        print_mist_status_result(&result);
        if result.ok { Ok(ExitCode::SUCCESS) } else { Ok(ExitCode::FAILURE) }
    }

    fn run_intake(
        self, config_path: &Path, lake_path: Option<&Path>,
    ) -> Result<ExitCode, CommandError> {
        let mist = self.mist.as_deref().map(entry_atom).transpose()?;
        let result = SurfaceContext::from_cli_paths(config_path, lake_path).mist_intake(mist)?;
        print_mist_intake_result(&result);
        Ok(ExitCode::SUCCESS)
    }
}

impl MistRenderArgs {
    fn run(self, config_path: &Path, lake_path: Option<&Path>) -> Result<ExitCode, CommandError> {
        let mist = self.mist.as_deref().map(entry_atom).transpose()?;
        let context = SurfaceContext::from_cli_paths(config_path, lake_path);
        let result = match self.command {
            | None => context.mist_render_with_override_json(
                mist,
                self.dry,
                self.override_json.as_deref(),
            )?,
            | Some(RenderCommand::Delete) => {
                if self.dry {
                    return Err(CommandError::DryWithRenderSubcommand);
                }
                if self.override_json.is_some() {
                    return Err(CommandError::OverrideJsonWithRenderSubcommand);
                }
                context.mist_render_delete(mist)?
            }
        };
        print_render_result(&result);
        if result.ok { Ok(ExitCode::SUCCESS) } else { Ok(ExitCode::FAILURE) }
    }
}
// sirno:witness:mist-commands:end

impl StatusArgs {
    // sirno:witness:project-status-commands:begin
    fn run(
        self, config_path: &Path, lake_path: Option<&Path>, quiet: bool,
    ) -> Result<ExitCode, CommandError> {
        let result = SurfaceContext::from_cli_paths(config_path, lake_path)
            .status(StatusRequest::new(self.show, self.mode.into()))?;
        match self.format.unwrap_or_default() {
            | StatusOutputFormat::Json => print_json(&result)?,
            | StatusOutputFormat::Human => print_status_result(&result, quiet),
        }
        if result.ok { Ok(ExitCode::SUCCESS) } else { Ok(ExitCode::FAILURE) }
    }
    // sirno:witness:project-status-commands:end
}

// sirno:witness:upstream-commands:begin
impl UpstreamCommand {
    fn run(
        self, config_path: &std::path::Path, lake_path: Option<&Path>,
    ) -> Result<ExitCode, CommandError> {
        let context = SurfaceContext::from_cli_paths(config_path, lake_path);
        match self {
            | Self::Add(args) => {
                let result = context.upstream_add(args.into_request()?)?;
                print_upstream_crystallize_report(&result);
                Ok(ExitCode::SUCCESS)
            }
            | Self::Remove { domain } => {
                let result = context.upstream_remove(entry_atom(&domain)?)?;
                print_upstream_crystallize_report(&result);
                Ok(ExitCode::SUCCESS)
            }
            | Self::Crystallize { domain, locked } => {
                let result = context.upstream_crystallize(UpstreamCrystallizeRequest {
                    domains: upstream_domains(domain)?,
                    locked,
                })?;
                print_upstream_crystallize_report(&result);
                Ok(ExitCode::SUCCESS)
            }
            | Self::Update { domain } => {
                let result = context.upstream_update(upstream_domains(domain)?)?;
                print_upstream_crystallize_report(&result);
                Ok(ExitCode::SUCCESS)
            }
            | Self::Status { format } => {
                let result = context.upstream_status()?;
                match format.unwrap_or_default() {
                    | UpstreamOutputFormat::Json => print_json(&result)?,
                    | UpstreamOutputFormat::Human => print_upstream_status_report(&result),
                }
                if result.ok { Ok(ExitCode::SUCCESS) } else { Ok(ExitCode::FAILURE) }
            }
        }
    }
}

impl UpstreamAddArgs {
    fn into_request(self) -> Result<UpstreamAddRequest, CommandError> {
        let mut settings = if let Some(branch) = self.branch {
            UpstreamSettings::branch(self.git, branch)
        } else if let Some(tag) = self.tag {
            UpstreamSettings::tag(self.git, tag)
        } else if let Some(rev) = self.rev {
            UpstreamSettings::rev(self.git, rev)
        } else {
            unreachable!("clap requires one upstream selector")
        };
        if let Some(project) = self.project {
            settings.project = project;
        }
        if let Some(manifest) = self.manifest {
            settings.manifest = manifest;
        }
        if let Some(mist) = self.mist {
            settings.mist = Some(entry_atom(&mist)?);
        }
        Ok(UpstreamAddRequest { domain: entry_atom(&self.domain)?, settings })
    }
}
// sirno:witness:upstream-commands:end

fn upstream_domains(domain: Option<String>) -> Result<Vec<EntryAtom>, CommandError> {
    domain.map(|domain| entry_atom(&domain)).transpose().map(|domain| domain.into_iter().collect())
}

fn entry_atom(raw: &str) -> Result<EntryAtom, CommandError> {
    Ok(EntryAtom::new(raw)?)
}

// sirno:witness:anchor-commands:begin
impl AnchorCommand {
    fn run(self, config_path: &Path, lake_path: Option<&Path>) -> Result<ExitCode, CommandError> {
        let context = SurfaceContext::from_cli_paths(config_path, lake_path);
        match self {
            | Self::Update { format } => {
                let result = context.anchor_update()?;
                match format.unwrap_or_default() {
                    | AnchorOutputFormat::Json => print_json(&result)?,
                    | AnchorOutputFormat::Human => print_anchor_update_result(&result),
                }
                Ok(ExitCode::SUCCESS)
            }
        }
    }
}
// sirno:witness:anchor-commands:end

impl TideCommand {
    fn run(
        self, config_path: &std::path::Path, lake_path: Option<&Path>,
    ) -> Result<ExitCode, CommandError> {
        match self {
            | TideCommand::Status { show, by, format } => {
                let context = SurfaceContext::from_cli_paths(config_path, lake_path);
                let format = format.unwrap_or_default();
                if show.includes_workitems() {
                    let statuses = context.tide_statuses(show)?;
                    print_tide_statuses(&statuses, by, format)?;
                    Ok(if statuses.iter().all(|status| status.resolved) {
                        ExitCode::SUCCESS
                    } else {
                        ExitCode::FAILURE
                    })
                } else {
                    let statuses = context.tide_statuses(show)?;
                    print_tide_review_waves(&statuses, by, format)?;
                    Ok(if statuses.is_empty() { ExitCode::SUCCESS } else { ExitCode::FAILURE })
                }
            }
            // sirno:witness:tide-commands:begin
            | TideCommand::Tui => tide::run(config_path, lake_path),
            // sirno:witness:tide-commands:end
            | TideCommand::Review(command) => command.run(config_path, lake_path),
            | TideCommand::Reset => {
                let result = SurfaceContext::from_cli_paths(config_path, lake_path).tide_reset()?;
                println!("{}", result.message);
                Ok(ExitCode::SUCCESS)
            }
        }
    }
}

impl TideReviewCommand {
    // sirno:witness:tide-commands:begin
    fn run(self, config_path: &Path, lake_path: Option<&Path>) -> Result<ExitCode, CommandError> {
        match self {
            | Self::Resolve(args) => args.run(config_path, lake_path),
            | Self::Unresolve(args) => args.run(config_path, lake_path),
        }
    }
    // sirno:witness:tide-commands:end
}

impl ResolveArgs {
    // sirno:witness:tide-commands:begin
    fn run(self, config_path: &Path, lake_path: Option<&Path>) -> Result<ExitCode, CommandError> {
        let request = if self.infer {
            TideResolveRequest { infer: true, ..TideResolveRequest::default() }
        } else if let Some(json) = self.json {
            TideResolveRequest {
                workitems: tide_workitems_from_json(&json)?,
                ..TideResolveRequest::default()
            }
        } else {
            let selection = tide_selection_from_items(self.items);
            TideResolveRequest {
                neighbors: selection.neighbors,
                workitems: selection.workitems,
                ..TideResolveRequest::default()
            }
        };
        let result =
            SurfaceContext::from_cli_paths(config_path, lake_path).tide_resolve(request)?;
        println!("{}", result.message);
        Ok(ExitCode::SUCCESS)
    }
    // sirno:witness:tide-commands:end
}

impl UnresolveArgs {
    // sirno:witness:tide-commands:begin
    fn run(self, config_path: &Path, lake_path: Option<&Path>) -> Result<ExitCode, CommandError> {
        let request = tide_selection_from_items(self.items);
        let result =
            SurfaceContext::from_cli_paths(config_path, lake_path).tide_unresolve(request)?;
        println!("{}", result.message);
        Ok(ExitCode::SUCCESS)
    }
    // sirno:witness:tide-commands:end
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum TideJsonWorkitems {
    One(TideWorkitem),
    Many(Vec<TideWorkitem>),
}

fn tide_workitems_from_json(source: &str) -> Result<Vec<TideWorkitem>, CommandError> {
    Ok(match serde_json::from_str::<TideJsonWorkitems>(source)? {
        | TideJsonWorkitems::One(workitem) => vec![workitem],
        | TideJsonWorkitems::Many(workitems) => workitems,
    })
}

fn tide_selection_from_items(items: Vec<TideItemSelector>) -> TideSelectionRequest {
    let mut request = TideSelectionRequest::default();
    for item in items {
        match item {
            | TideItemSelector::Neighbor(id) => request.neighbors.push(id),
            | TideItemSelector::Workitem(workitem) => request.workitems.push(workitem),
        }
    }
    request
}

fn print_tide_statuses(
    statuses: &[TideStatus], grouping: TideStatusGrouping, format: TideOutputFormat,
) -> Result<(), CommandError> {
    match format {
        | TideOutputFormat::Json => {
            print_json(statuses)?;
        }
        | TideOutputFormat::Human => {
            anstream::print!(
                "{}",
                format_tide_statuses_grouped_with_style(statuses, grouping, OutputStyle::Styled)
            );
        }
    }
    Ok(())
}

fn print_tide_review_waves(
    statuses: &[TideStatus], grouping: TideStatusGrouping, format: TideOutputFormat,
) -> Result<(), CommandError> {
    match format {
        | TideOutputFormat::Json => {
            let entries = tide_review_entries_from_statuses(statuses);
            print_json(&entries)?;
        }
        | TideOutputFormat::Human => {
            print!("{}", format_tide_review_waves_grouped(statuses, grouping));
        }
    }
    Ok(())
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct TideReviewWave {
    ripple: EntryAddress,
    entries: Vec<EntryAddress>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct TideReviewEntryGroup {
    entry: EntryAddress,
    ripples: Vec<EntryAddress>,
}

fn format_tide_review_waves_grouped(
    statuses: &[TideStatus], grouping: TideStatusGrouping,
) -> String {
    match grouping {
        | TideStatusGrouping::Wave => format_tide_review_waves(statuses),
        | TideStatusGrouping::Entry => format_tide_review_entries(statuses),
    }
}

fn format_tide_review_waves(statuses: &[TideStatus]) -> String {
    let waves = tide_review_waves(statuses);
    if waves.is_empty() {
        return "tide: clear\n".to_owned();
    }

    let open_count = statuses.iter().filter(|status| !status.resolved).count();
    let review_entry_count = tide_review_entries_from_statuses(statuses).len();
    let rows = waves
        .iter()
        .flat_map(|wave| {
            wave.entries.iter().enumerate().map(|(index, entry)| {
                let ripple = if index == 0 { wave.ripple.to_string() } else { String::new() };
                TideGroupedTableRow {
                    starts_group: index == 0,
                    cells: vec![ripple, entry.to_string()],
                }
            })
        })
        .collect::<Vec<_>>();
    let mut output = format_tide_grouped_table(vec!["wave".to_owned(), "entry".to_owned()], rows);
    output.push('\n');
    output.push_str(&tide_summary_sentence(open_count, 0, waves.len(), review_entry_count));
    output.push('\n');

    output
}

fn format_tide_review_entries(statuses: &[TideStatus]) -> String {
    let entries = tide_review_entry_groups(statuses);
    if entries.is_empty() {
        return "tide: clear\n".to_owned();
    }

    let waves = tide_review_waves(statuses);
    let open_count = statuses.iter().filter(|status| !status.resolved).count();
    let review_entry_count = entries.len();
    let rows = entries
        .iter()
        .flat_map(|group| {
            group.ripples.iter().enumerate().map(|(index, ripple)| {
                let entry = if index == 0 { group.entry.to_string() } else { String::new() };
                TideGroupedTableRow {
                    starts_group: index == 0,
                    cells: vec![entry, ripple.to_string()],
                }
            })
        })
        .collect::<Vec<_>>();
    let mut output = format_tide_grouped_table(vec!["entry".to_owned(), "reason".to_owned()], rows);
    output.push('\n');
    output.push_str(&tide_summary_sentence(open_count, 0, waves.len(), review_entry_count));
    output.push('\n');

    output
}

fn format_tide_statuses_grouped_with_style(
    statuses: &[TideStatus], grouping: TideStatusGrouping, style: OutputStyle,
) -> String {
    match grouping {
        | TideStatusGrouping::Wave => format_tide_statuses_with_style(statuses, style),
        | TideStatusGrouping::Entry => format_tide_statuses_by_entry_with_style(statuses, style),
    }
}

#[cfg(any())]
fn format_tide_statuses(statuses: &[TideStatus]) -> String {
    format_tide_statuses_with_style(statuses, OutputStyle::Plain)
}

fn format_tide_statuses_with_style(statuses: &[TideStatus], style: OutputStyle) -> String {
    let waves = tide_status_waves(statuses);
    if waves.is_empty() {
        return "tide: clear\n".to_owned();
    }

    let open_count = statuses.iter().filter(|status| !status.resolved).count();
    let resolved_count = statuses.len() - open_count;
    let review_entry_count = tide_review_entries_from_statuses(statuses).len();
    let rows = waves
        .iter()
        .flat_map(|wave| {
            wave.statuses.iter().enumerate().map(|(index, status)| {
                let ripple = if index == 0 { wave.ripple.to_string() } else { String::new() };
                TideGroupedTableRow {
                    starts_group: index == 0,
                    cells: vec![
                        ripple,
                        status.workitem.neighbor.to_string(),
                        tide_state_label(status).to_owned(),
                        status.workitem.field.clone(),
                        status.workitem.direction.to_string(),
                        tide_sources_label(status),
                    ],
                }
            })
        })
        .collect::<Vec<_>>();
    let mut output = format_tide_grouped_table_with_style(
        vec![
            "wave".to_owned(),
            "entry".to_owned(),
            "state".to_owned(),
            "field".to_owned(),
            "direction".to_owned(),
            "sources".to_owned(),
        ],
        rows,
        style,
    );
    output.push('\n');
    output.push_str(&tide_summary_sentence(
        open_count,
        resolved_count,
        waves.len(),
        review_entry_count,
    ));
    output.push('\n');

    output
}

#[cfg(any())]
fn format_tide_statuses_by_entry(statuses: &[TideStatus]) -> String {
    format_tide_statuses_by_entry_with_style(statuses, OutputStyle::Plain)
}

fn format_tide_statuses_by_entry_with_style(statuses: &[TideStatus], style: OutputStyle) -> String {
    let entries = tide_status_entry_groups(statuses);
    if entries.is_empty() {
        return "tide: clear\n".to_owned();
    }

    let waves = tide_status_waves(statuses);
    let open_count = statuses.iter().filter(|status| !status.resolved).count();
    let resolved_count = statuses.len() - open_count;
    let review_entry_count = tide_review_entries_from_statuses(statuses).len();
    let rows = entries
        .iter()
        .flat_map(|group| {
            group.statuses.iter().enumerate().map(|(index, status)| {
                let entry = if index == 0 { group.entry.to_string() } else { String::new() };
                TideGroupedTableRow {
                    starts_group: index == 0,
                    cells: vec![
                        entry,
                        status.workitem.ripple.to_string(),
                        tide_state_label(status).to_owned(),
                        status.workitem.field.clone(),
                        status.workitem.direction.to_string(),
                        tide_sources_label(status),
                    ],
                }
            })
        })
        .collect::<Vec<_>>();
    let mut output = format_tide_grouped_table_with_style(
        vec![
            "entry".to_owned(),
            "reason".to_owned(),
            "state".to_owned(),
            "field".to_owned(),
            "direction".to_owned(),
            "sources".to_owned(),
        ],
        rows,
        style,
    );
    output.push('\n');
    output.push_str(&tide_summary_sentence(
        open_count,
        resolved_count,
        waves.len(),
        review_entry_count,
    ));
    output.push('\n');

    output
}

fn tide_summary_sentence(
    open_count: usize, resolved_count: usize, wave_count: usize, review_entry_count: usize,
) -> String {
    let resolved = if resolved_count == 0 {
        String::new()
    } else {
        format!(
            " and {resolved_count} resolved {}",
            plural(resolved_count, "workitem", "workitems"),
        )
    };
    format!(
        "The tide has {open_count} open {}{resolved} in {wave_count} {}, \
         with {review_entry_count} unique {}.",
        plural(open_count, "workitem", "workitems"),
        plural(wave_count, "wave", "waves"),
        plural(review_entry_count, "review entry", "review entries"),
    )
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct TideGroupedTableRow {
    cells: Vec<String>,
    starts_group: bool,
}

fn format_tide_grouped_table(headers: Vec<String>, rows: Vec<TideGroupedTableRow>) -> String {
    format_tide_grouped_table_with_style(headers, rows, OutputStyle::Plain)
}

fn format_tide_grouped_table_with_style(
    headers: Vec<String>, rows: Vec<TideGroupedTableRow>, style: OutputStyle,
) -> String {
    let group_start_rows = rows
        .iter()
        .enumerate()
        .filter_map(|(index, row)| row.starts_group.then_some(index))
        .filter(|index| *index > 0)
        .collect::<Vec<_>>();
    let rows = rows.into_iter().map(|row| row.cells).collect::<Vec<_>>();
    let table = format_human_table_semantic_with_width(headers, rows, None, style);
    strengthen_tide_group_separators(&table, &group_start_rows)
}

fn strengthen_tide_group_separators(table: &str, group_start_rows: &[usize]) -> String {
    if group_start_rows.is_empty() {
        return table.to_owned();
    }

    let mut lines = table.lines().map(str::to_owned).collect::<Vec<_>>();
    for row_index in group_start_rows {
        if let Some(separator) = lines.get_mut(tide_row_separator_index(*row_index)) {
            *separator = heavy_table_separator(separator);
        }
    }

    let mut output = lines.join("\n");
    output.push('\n');
    output
}

fn tide_row_separator_index(row_index: usize) -> usize {
    2 * row_index + 2
}

fn heavy_table_separator(separator: &str) -> String {
    let length = separator.chars().count();
    separator
        .chars()
        .enumerate()
        .map(|(index, character)| {
            if index == 0 {
                '╞'
            } else if index + 1 == length {
                '╡'
            } else if character == '┼' {
                '╪'
            } else {
                '═'
            }
        })
        .collect()
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct TideStatusWave<'a> {
    ripple: EntryAddress,
    statuses: Vec<&'a TideStatus>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct TideStatusEntryGroup<'a> {
    entry: EntryAddress,
    statuses: Vec<&'a TideStatus>,
}

fn tide_review_waves(statuses: &[TideStatus]) -> Vec<TideReviewWave> {
    let mut entries_by_ripple = BTreeMap::<EntryAddress, BTreeSet<EntryAddress>>::new();
    for status in statuses.iter().filter(|status| !status.resolved) {
        entries_by_ripple
            .entry(status.workitem.ripple.clone())
            .or_default()
            .insert(status.workitem.neighbor.clone());
    }

    entries_by_ripple
        .into_iter()
        .map(|(ripple, entries)| TideReviewWave { ripple, entries: entries.into_iter().collect() })
        .collect()
}

fn tide_review_entry_groups(statuses: &[TideStatus]) -> Vec<TideReviewEntryGroup> {
    let mut ripples_by_entry = BTreeMap::<EntryAddress, BTreeSet<EntryAddress>>::new();
    for status in statuses.iter().filter(|status| !status.resolved) {
        ripples_by_entry
            .entry(status.workitem.neighbor.clone())
            .or_default()
            .insert(status.workitem.ripple.clone());
    }

    ripples_by_entry
        .into_iter()
        .map(|(entry, ripples)| TideReviewEntryGroup {
            entry,
            ripples: ripples.into_iter().collect(),
        })
        .collect()
}

fn tide_status_waves(statuses: &[TideStatus]) -> Vec<TideStatusWave<'_>> {
    let mut statuses_by_ripple = BTreeMap::<EntryAddress, Vec<&TideStatus>>::new();
    for status in statuses {
        statuses_by_ripple.entry(status.workitem.ripple.clone()).or_default().push(status);
    }

    statuses_by_ripple
        .into_iter()
        .map(|(ripple, statuses)| TideStatusWave { ripple, statuses })
        .collect()
}

fn tide_status_entry_groups(statuses: &[TideStatus]) -> Vec<TideStatusEntryGroup<'_>> {
    let mut statuses_by_entry = BTreeMap::<EntryAddress, Vec<&TideStatus>>::new();
    for status in statuses {
        statuses_by_entry.entry(status.workitem.neighbor.clone()).or_default().push(status);
    }

    statuses_by_entry
        .into_iter()
        .map(|(entry, mut statuses)| {
            statuses.sort_by(|left, right| left.workitem.cmp(&right.workitem));
            TideStatusEntryGroup { entry, statuses }
        })
        .collect()
}

fn tide_review_entries_from_statuses(statuses: &[TideStatus]) -> Vec<EntryAddress> {
    statuses
        .iter()
        .filter(|status| !status.resolved)
        .map(|status| status.workitem.neighbor.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn tide_state_label(status: &TideStatus) -> &'static str {
    if status.resolved { "resolved" } else { "open" }
}

fn tide_sources_label(status: &TideStatus) -> String {
    status
        .sources
        .iter()
        .map(|source| match source {
            | TideSource::Lake => "lake",
            | TideSource::Anchor => "anchor",
        })
        .collect::<Vec<_>>()
        .join(",")
}

fn plural<'a>(count: usize, singular: &'a str, plural: &'a str) -> &'a str {
    if count == 1 { singular } else { plural }
}

impl ArtifactCommand {
    fn run(self, config_path: &Path, lake_path: Option<&Path>) -> Result<ExitCode, CommandError> {
        let context = SurfaceContext::from_cli_paths(config_path, lake_path);
        match self {
            | ArtifactCommand::List { id } => {
                let id = EntryAddress::new(&id)?;
                for artifact in context.entry_artifact_list(id)?.artifacts {
                    println!("{artifact}");
                }
                Ok(ExitCode::SUCCESS)
            }
            | ArtifactCommand::Add { id, source, artifact_path } => {
                let id = EntryAddress::new(&id)?;
                let result =
                    context.entry_artifact_add(ArtifactAddRequest { id, source, artifact_path })?;
                println!("{}", result.message);
                Ok(ExitCode::SUCCESS)
            }
            | ArtifactCommand::Rename { id, old_path, new_path } => {
                let id = EntryAddress::new(&id)?;
                let result = context.entry_artifact_rename(ArtifactRenameRequest {
                    id,
                    old_path,
                    new_path,
                })?;
                println!("{}", result.message);
                Ok(ExitCode::SUCCESS)
            }
            | ArtifactCommand::Remove { id, artifact_path } => {
                let id = EntryAddress::new(&id)?;
                let result =
                    context.entry_artifact_remove(ArtifactRemoveRequest { id, artifact_path })?;
                println!("{}", result.message);
                Ok(ExitCode::SUCCESS)
            }
        }
    }
}

impl CharmCommand {
    fn run(self, config_path: &Path, lake_path: Option<&Path>) -> Result<ExitCode, CommandError> {
        let context = SurfaceContext::from_cli_paths(config_path, lake_path);
        match self {
            | CharmCommand::List => {
                print_charm_list(&context.charm_list()?);
                Ok(ExitCode::SUCCESS)
            }
            | CharmCommand::Show { id } => {
                let id = EntryAddress::new(&id)?;
                print_charm_show(&context.charm_show(id)?);
                Ok(ExitCode::SUCCESS)
            }
            | CharmCommand::Enable { id } => {
                let id = EntryAddress::new(&id)?;
                println!("{}", context.charm_enable(id)?.message);
                Ok(ExitCode::SUCCESS)
            }
            | CharmCommand::Disable { id } => {
                let id = EntryAddress::new(&id)?;
                println!("{}", context.charm_disable(id)?.message);
                Ok(ExitCode::SUCCESS)
            }
            | CharmCommand::Setup { id } => {
                let id = EntryAddress::new(&id)?;
                exit_from_process_result(context.charm_setup(id)?)
            }
            | CharmCommand::Check { id } => {
                let id = EntryAddress::new(&id)?;
                exit_from_process_result(context.charm_check(id)?)
            }
            | CharmCommand::Build { id } => {
                let id = EntryAddress::new(&id)?;
                exit_from_process_result(context.charm_build(id)?)
            }
            | CharmCommand::Clean { id } => {
                let id = EntryAddress::new(&id)?;
                println!("{}", context.charm_clean(id)?.message);
                Ok(ExitCode::SUCCESS)
            }
        }
    }
}

impl SpellCommand {
    fn run(self, config_path: &Path, lake_path: Option<&Path>) -> Result<ExitCode, CommandError> {
        let context = SurfaceContext::from_cli_paths(config_path, lake_path);
        match self {
            | SpellCommand::List => {
                print_spell_list(&context.spell_list()?);
                Ok(ExitCode::SUCCESS)
            }
            | SpellCommand::Show { id } => {
                let id = EntryAddress::new(&id)?;
                print_charm_show(&context.spell_show(id)?);
                Ok(ExitCode::SUCCESS)
            }
            | SpellCommand::Run { id } => {
                let id = EntryAddress::new(&id)?;
                exit_from_process_result(context.spell_run(id)?)
            }
        }
    }
}

fn print_charm_list(result: &CharmListResult) {
    let rows = result
        .charms
        .iter()
        .map(|record| {
            vec![
                record.id.clone(),
                format_intrinsic_summary(&record.intrinsic),
                record.kind.clone(),
                if record.enabled { "yes" } else { "no" }.to_owned(),
            ]
        })
        .collect::<Vec<_>>();
    print!(
        "{}",
        format_human_table_semantic_with_width(
            vec![
                "entry".to_owned(),
                "intrinsic".to_owned(),
                "kind".to_owned(),
                "enabled".to_owned(),
            ],
            rows,
            None,
            OutputStyle::Styled
        )
    );
    println!("{}", result.message);
}

fn print_spell_list(result: &SpellListResult) {
    let rows = result
        .spells
        .iter()
        .map(|record| {
            vec![
                record.id.clone(),
                format_intrinsic_summary(&record.intrinsic),
                record.kind.clone(),
                record.spell_cache_path.clone(),
            ]
        })
        .collect::<Vec<_>>();
    print!(
        "{}",
        format_human_table_semantic_with_width(
            vec!["entry".to_owned(), "intrinsic".to_owned(), "kind".to_owned(), "cache".to_owned(),],
            rows,
            None,
            OutputStyle::Styled
        )
    );
    println!("{}", result.message);
}

fn print_charm_show(result: &CharmShowResult) {
    println!("entry: {}", result.id);
    println!("intrinsic: {}", format_intrinsic_summary(&result.intrinsic));
    println!("kind: {}", result.kind);
    println!("enabled: {}", if result.enabled { "yes" } else { "no" });
    println!("manifest: {}", result.manifest_path);
    println!("artifact root: {}", result.artifact_root);
    println!("spell cache: {}", result.spell_cache_path);
    println!("spell command: {}", result.spell_command.join(" "));
    println!("setup: {}", if result.has_setup { "yes" } else { "no" });
    println!("check: {}", if result.has_check { "yes" } else { "no" });
    println!("build: {}", if result.has_build { "yes" } else { "no" });
    if !result.hooks.is_empty() {
        println!("hooks: {}", result.hooks.join(", "));
    }
}

fn exit_from_process_result(result: CharmProcessResult) -> Result<ExitCode, CommandError> {
    println!("{}", result.message);
    if !result.stdout.is_empty() {
        print!("{}", result.stdout);
    }
    if !result.stderr.is_empty() {
        eprint!("{}", result.stderr);
    }
    if result.ok { Ok(ExitCode::SUCCESS) } else { Ok(ExitCode::FAILURE) }
}

impl UtilCommand {
    fn run(self, config_path: &Path, lake_path: Option<&Path>) -> Result<ExitCode, CommandError> {
        match self {
            | UtilCommand::Config { command } => {
                if lake_path.is_some() {
                    return Err(CommandError::ConfigRejectsLakePath);
                }
                command.unwrap_or(ConfigUtilityCommand::Tui).run(config_path)
            }
            | UtilCommand::Entry { command } => {
                command.unwrap_or(EntryUtilityCommand::Tui).run(config_path, lake_path)
            }
            | UtilCommand::Completion { shell } => {
                let shell = Shell::from(shell);
                let mut command = Cli::command();
                let mut stdout = std::io::stdout();
                generate(shell, &mut command, "sirno", &mut stdout);
                Ok(ExitCode::SUCCESS)
            }
            | UtilCommand::Skills { command } => {
                if lake_path.is_some() {
                    return Err(CommandError::SkillsRejectsLakePath);
                }
                command.unwrap_or_default().run(config_path)
            }
            | UtilCommand::Mcp => {
                if lake_path.is_some() {
                    return Err(CommandError::McpRejectsLakePath);
                }
                let runtime = tokio::runtime::Builder::new_multi_thread()
                    .enable_all()
                    .build()
                    .map_err(CommandError::CreateMcpRuntime)?;
                runtime
                    .block_on(crate::mcp::run_stdio(SurfaceContext::new(config_path.to_path_buf())))
                    .map_err(|error| CommandError::McpServer(error.to_string()))?;
                Ok(ExitCode::SUCCESS)
            }
        }
    }
}

impl ConfigUtilityCommand {
    fn run(self, config_path: &Path) -> Result<ExitCode, CommandError> {
        let context = SurfaceContext::new(config_path.to_path_buf());
        match self {
            | ConfigUtilityCommand::Tui => config::run(config_path),
            | ConfigUtilityCommand::Check => {
                let result = context.config_comments_check()?;
                print_config_comment_result(&result);
                if result.ok { Ok(ExitCode::SUCCESS) } else { Ok(ExitCode::FAILURE) }
            }
            | ConfigUtilityCommand::Fix => {
                let result = context.config_comments_fix()?;
                print_config_comment_result(&result);
                Ok(ExitCode::SUCCESS)
            }
        }
    }
}

impl EntryUtilityCommand {
    fn run(self, config_path: &Path, lake_path: Option<&Path>) -> Result<ExitCode, CommandError> {
        match self {
            | EntryUtilityCommand::Tui => entry::run(config_path, lake_path),
        }
    }
}

impl SkillCommand {
    fn run(self, config_path: &Path) -> Result<ExitCode, CommandError> {
        let context = SurfaceContext::new(config_path.to_path_buf());
        let result = match self {
            | SkillCommand::Tui(args) => {
                return skills::run(config_path, args.claude_skills);
            }
            | SkillCommand::Init(args) => {
                context.skill_wrappers_init_with_claude(args.claude_skills)?
            }
            | SkillCommand::Check(args) => {
                context.skill_wrappers_check_with_claude(args.claude_skills)?
            }
            | SkillCommand::List(args) => {
                context.skill_wrappers_list_with_claude(args.claude_skills)?
            }
        };
        Ok(print_skill_wrapper_result(result))
    }
}

fn run_skill_wrappers_init(config_path: &Path, claude_skills: bool) -> Result<(), CommandError> {
    let result = SurfaceContext::new(config_path.to_path_buf())
        .skill_wrappers_init_with_claude(claude_skills)?;
    print_skill_wrapper_result(result);
    Ok(())
}

fn print_skill_wrapper_result(result: SkillWrapperResult) -> ExitCode {
    anstream::print!("{}", format_skill_wrapper_table_for_terminal(&result.records));
    anstream::println!("{}", result.message);
    if result.ok { ExitCode::SUCCESS } else { ExitCode::FAILURE }
}

fn run_witness_command(
    config_path: &Path, lake_path: Option<&Path>, raw_id: &str, full: bool,
) -> Result<ExitCode, CommandError> {
    let id = EntryAddress::new(raw_id)?;
    let records = SurfaceContext::from_cli_paths(config_path, lake_path).witness_records(&id)?;
    if records.is_empty() {
        println!("no witness found for {id}");
        return Ok(ExitCode::FAILURE);
    }
    print_witness_records(&records, full);
    Ok(ExitCode::SUCCESS)
}

fn entry_path_records(
    config_path: &Path, lake_path: Option<&Path>, args: &EntryPathsArgs,
) -> Result<Vec<PathRecord>, CommandError> {
    let request = EntryPathsRequest::new(
        EntryAddress::new(&args.id)?,
        path_selection_from_args(args),
        args.absolute,
    );
    SurfaceContext::from_cli_paths(config_path, lake_path).entry_paths(request)
}

fn print_path_records(
    records: &[PathRecord], format: PathOutputFormat,
) -> Result<(), CommandError> {
    match format {
        | PathOutputFormat::Json => print_json(records)?,
        | PathOutputFormat::Human => print!("{}", format_path_table(records)),
        | PathOutputFormat::Paths => {
            for record in records {
                println!("{}", record.path);
            }
        }
    }
    Ok(())
}

fn path_selection_from_args(args: &EntryPathsArgs) -> PathSelection {
    let all = !args.show_entry && !args.show_artifact;
    PathSelection::new(all || args.show_entry, all || args.show_artifact)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_defaults_match_the_shared_request() {
        let cli = Cli::try_parse_from(["sirno", "status"]).unwrap();
        let Command::Status(args) = cli.command else {
            panic!("expected status command");
        };

        assert_eq!(StatusRequest::new(args.show, args.mode.into()), StatusRequest::default());
    }

    #[test]
    fn entry_new_accepts_user_intrinsic_fields() {
        let cli = Cli::try_parse_from([
            "sirno",
            "new",
            "alpha",
            "--intrinsic",
            "name=Alpha",
            "--intrinsic",
            "desc=Alpha entry.",
        ])
        .unwrap();

        let Command::TopLevelEntry(TopLevelEntryCommand::New { id, intrinsic, .. }) = cli.command
        else {
            panic!("expected top-level new command");
        };

        assert_eq!(id, "alpha");
        assert_eq!(
            intrinsic,
            vec![
                IntrinsicArgument { field: "name".to_owned(), value: "Alpha".to_owned() },
                IntrinsicArgument { field: "desc".to_owned(), value: "Alpha entry.".to_owned() },
            ]
        );
    }

    #[test]
    fn entry_new_rejects_hardcoded_name_desc_flags() {
        let error = Cli::try_parse_from([
            "sirno",
            "new",
            "alpha",
            "--name",
            "Alpha",
            "--desc",
            "Alpha entry.",
        ])
        .unwrap_err();

        assert_eq!(error.kind(), clap::error::ErrorKind::UnknownArgument);
    }
}
