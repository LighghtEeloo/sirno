//! Typed command execution shared by CLI and tool adapters.

use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::ffi::OsString;
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::process::Command as ProcessCommand;

use indexmap::IndexMap;

use crate::charm::{
    CharmBuildSpec, CharmBundle, CharmCommandSpec, CharmManifest, artifact_map,
    manifest_artifact_path,
};
use crate::skills::{BundledSkill, BundledSkillWrapper};
use crate::surface::dto::{
    AnchorRippleKind, AnchorRippleRecord, AnchorUpdateResult, ArtifactAddRequest,
    ArtifactChangeResult, ArtifactListResult, ArtifactRemoveRequest, ArtifactRenameRequest,
    CharmCleanResult, CharmEnablementResult, CharmListResult, CharmProcessResult, CharmRecord,
    CharmShowResult, ConfigCommentResult, CwdResult, EntryFileResult, EntryNewRequest,
    EntryPathsRequest, EntryReadMetadataFields, EntryReadMetadataValue, EntryReadRequest,
    EntryReadResult, EntryRenameResult, LakeCheckResult, LakeInitRequest, LakeInitResult,
    LocalProtectionResult, MistIntakeResult, MistStatusResult, MovePathResult, PathRecord,
    QueryColumn, QueryColumnSelection, QueryColumns, QueryRequest, QueryResponse, QueryResults,
    QueryRun, RenderResult, RgRequest, RgResult, SkillResourceContext, SkillWrapperRecord,
    SkillWrapperResult, SpellListResult, SpellRecord, StatusAnchor, StatusBlockers,
    StatusCheckPolicy, StatusRequest, StatusResult, StatusTide, StructuralEdgeStatus,
    StructuralFieldStatus, StructuralFilter, StructuralStateFilter, StructuralTarget,
    TideChangeResult, TideResolveRequest, TideSelectionRequest, TideStatusMode, TideStatusResult,
    UpstreamAddRequest, UpstreamCrystallizeRequest, WitnessRecordResult, WitnessResult,
};
use crate::surface::error::CommandError;
use crate::surface::output::{display_path, display_paths, output_path, query_result_rows};
use crate::surface::rg::{RgPreprocessorLink, resolve_lake_path_for_rg};
use crate::{
    AnchorFile, CHARM_MANIFEST_FILE_NAME, CONFIG_FILE_NAME, CheckMode, Entry, EntryAddress,
    EntryArtifactPath, EntryAtom, EntryDirectory, EntryDirectoryCheckSettings, EntryDirectoryError,
    EntryDirectoryReport, EntryDirectoryWritePolicy, EntryIntrinsicFields, EntryMetadata,
    EntryParseError, EntryQuery, EntryStructuralMatcher, GenLinkDirectoryReport, GeneratedLinkBody,
    MetaFile, MetaRegistry, MistManifest, MistManifestEntry, MistRenderSettings, MistSpec,
    SIRNO_CONTROL_DIR_NAME, SPELL_CACHE_DIRECTORY, SirnoConfig, StructuralEdgeIndex,
    StructuralSettings, Tide, TideEntrySnapshot, TideFile, TideStatus, UpstreamCrystallizeReport,
    UpstreamFile, UpstreamGitCache, UpstreamStatusReport, VagueEntryQuery, WitnessCheckSettings,
    WitnessRecord,
};

#[derive(Clone, Debug)]
pub struct SurfaceContext {
    config_path: PathBuf,
    lake_path: Option<PathBuf>,
    upstream_store_path: Option<PathBuf>,
}

impl SurfaceContext {
    /// Create a context rooted at one Sirno config path.
    pub fn new(config_path: impl Into<PathBuf>) -> Self {
        Self { config_path: config_path.into(), lake_path: None, upstream_store_path: None }
    }

    /// Override the Sirno Lake path used by lake-backed operations.
    pub fn with_lake_path(mut self, lake_path: impl Into<PathBuf>) -> Self {
        self.lake_path = Some(lake_path.into());
        self
    }

    /// Override the upstream Git cache root.
    pub fn with_upstream_store_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.upstream_store_path = Some(path.into());
        self
    }

    pub(crate) fn from_cli_paths(config_path: &Path, lake_path: Option<&Path>) -> Self {
        let mut context = Self::new(config_path.to_path_buf());
        if let Some(lake_path) = lake_path {
            context = context.with_lake_path(lake_path.to_path_buf());
        }
        context
    }

    /// Read or change the process current working directory.
    pub fn cwd(&self, path: Option<PathBuf>) -> Result<CwdResult, CommandError> {
        let changed = path.is_some();
        if let Some(path) = path {
            env::set_current_dir(&path).map_err(|source| CommandError::ChangeCurrentDirectory {
                path: path.to_path_buf(),
                source,
            })?;
        }

        let cwd = env::current_dir().map_err(CommandError::CurrentDirectory)?;
        let path = display_path(&cwd);
        let message = if changed {
            format!("changed current working directory to {path}")
        } else {
            format!("current working directory is {path}")
        };

        Ok(CwdResult { ok: true, changed, path, message })
    }

    /// Query entries and return structured rows before presentation rendering.
    pub fn query_entries(&self, request: QueryRequest) -> Result<QueryRun, CommandError> {
        let (lake, mut settings) =
            resolve_lake_directory(self.lake_path.as_deref(), &self.config_path)?;
        settings.render = false;
        settings.witness = None;
        let report = EntryDirectory::new(&lake).check_with_settings(CheckMode::Edit, &settings)?;
        let meta = report.meta();
        let structural = report.structural();
        // sirno:witness:query:begin
        let columns = match request.columns {
            | QueryColumnSelection::Default => QueryColumns::default_output(),
            | QueryColumnSelection::Options => {
                return Ok(QueryRun::ColumnOptions(query_column_options(meta, structural)));
            }
            | QueryColumnSelection::Selected(columns) => columns,
        };
        // sirno:witness:query:end
        let columns = validate_query_columns(columns, meta, structural)?;
        if report.has_errors() {
            return Ok(QueryRun::InvalidLake { columns, report: Box::new(report) });
        }

        let vague_query = VagueEntryQuery::new().with_text_terms(request.terms);
        let filtered_query = entry_query_from_filters(
            EntryQuery::new().with_text_terms(request.exact_terms),
            request.has,
            request.is,
            structural,
        )?;
        let vague_matches = vague_query.select_entries(report.entries());
        let matches = filtered_query.select_entries(vague_matches);
        let rows = query_result_rows(&report, &matches, &columns)?;
        Ok(QueryRun::Results(QueryResults::new(columns, rows)))
    }

    /// Return filesystem paths related to one entry.
    pub fn entry_paths(&self, request: EntryPathsRequest) -> Result<Vec<PathRecord>, CommandError> {
        let config = SirnoConfig::from_file(&self.config_path)?;
        let lake = resolve_lake_path(self.lake_path.as_deref(), &self.config_path, &config);
        let directory = EntryDirectory::new(&lake);
        directory.read_entry(&request.id)?;
        let artifacts = directory.read_entry_artifacts(&request.id)?;
        let mut records = Vec::new();

        if request.selection.entry {
            records.push(PathRecord::new(
                "entry",
                output_path(directory.entry_file_path(&request.id), request.absolute)?,
            ));
        }
        if request.selection.artifact {
            records.push(PathRecord::new(
                "artifact-root",
                output_path(directory.entry_artifact_root_path(&request.id), request.absolute)?,
            ));
            for artifact in &artifacts {
                records.push(PathRecord::new(
                    "artifact",
                    output_path(
                        directory.entry_artifact_path(&request.id, &artifact.path),
                        request.absolute,
                    )?,
                ));
            }
        }
        Ok(records)
    }

    // sirno:witness:mcp-interface:begin
    /// Read one Sirno Lake Markdown entry with selected content.
    pub fn entry_read(&self, request: EntryReadRequest) -> Result<EntryReadResult, CommandError> {
        let id = request.id;
        let (lake, mut settings) =
            resolve_lake_directory(self.lake_path.as_deref(), &self.config_path)?;
        settings.render = false;
        settings.witness = None;
        let directory = EntryDirectory::new(&lake);
        let path = directory.entry_file_path(&id);
        let source = if request.content.includes_source() {
            Some(directory.read_entry_source(&id)?)
        } else {
            None
        };
        let report = directory.check_with_settings(CheckMode::Edit, &settings)?;
        let entry = report
            .entries()
            .iter()
            .find(|entry| entry.id == id)
            .ok_or_else(|| EntryDirectoryError::EntryNotFound(id.clone()))?;
        Ok(EntryReadResult {
            ok: true,
            id: id.to_string(),
            path: display_path(&path),
            metadata: entry_read_metadata(entry),
            body: request.content.includes_body().then(|| entry.body.clone()),
            source,
            message: format!("read entry {id} from {}", path.display()),
        })
    }
    // sirno:witness:mcp-interface:end

    /// Return tide statuses in structured form.
    pub fn tide_statuses(&self, mode: TideStatusMode) -> Result<Vec<TideStatus>, CommandError> {
        let context = TideContext::load(&self.config_path, self.lake_path.as_deref())?;
        let tide_file = context.load_tide_file_or_current()?;
        let tide = context.tide(&tide_file)?;
        Ok(tide_statuses_for_output(&tide, mode.includes_resolved()))
    }

    /// Return entry addresses that still need tide review.
    pub fn tide_review_entries(&self) -> Result<Vec<EntryAddress>, CommandError> {
        let context = TideContext::load(&self.config_path, self.lake_path.as_deref())?;
        let tide_file = context.load_tide_file_or_current()?;
        let tide = context.tide(&tide_file)?;
        Ok(tide.review_entries())
    }

    /// Return tide review entries and optional full statuses as a JSON-first command result.
    pub fn tide_status(&self, mode: TideStatusMode) -> Result<TideStatusResult, CommandError> {
        let context = TideContext::load(&self.config_path, self.lake_path.as_deref())?;
        let tide_file = context.load_tide_file_or_current()?;
        let tide = context.tide(&tide_file)?;
        let review_entries = tide.review_entries();
        let statuses = if mode.includes_workitems() {
            tide_statuses_for_output(&tide, mode.includes_resolved())
        } else {
            Vec::new()
        };
        Ok(TideStatusResult { ok: review_entries.is_empty(), review_entries, statuses })
    }

    /// Return repository witness records for one entry.
    pub fn witness_records(&self, id: &EntryAddress) -> Result<Vec<WitnessRecord>, CommandError> {
        let config = SirnoConfig::from_file(&self.config_path)?;
        let lake = resolve_lake_path(self.lake_path.as_deref(), &self.config_path, &config);
        if !EntryDirectory::new(&lake).entry_exists(id)? {
            return Err(CommandError::MissingWitnessEntry(id.clone()));
        }
        let Some(settings) = witness_check_settings(&self.config_path, &config) else {
            return Err(CommandError::RepoMembersNotConfigured);
        };
        let index = settings.scan()?;
        Ok(index.records_for(id).to_vec())
    }

    /// Create a Sirno config and ordinary seed entries.
    // sirno:witness:lake-commands:begin
    pub fn lake_init(&self, request: LakeInitRequest) -> Result<LakeInitResult, CommandError> {
        let config = SirnoConfig::new(
            request
                .lake
                .or_else(|| self.lake_path.clone())
                .unwrap_or_else(|| default_lake_path(&self.config_path)),
        );
        let lake_path = config.resolve_lake(&self.config_path);
        config.write_new(&self.config_path)?;
        let paths = EntryDirectory::new(&lake_path).init()?;
        Ok(LakeInitResult {
            ok: true,
            config_path: display_path(&self.config_path),
            lake_path: display_path(&lake_path),
            entry_count: paths.len(),
            message: format!(
                "initialized {} with {} entries in {}",
                self.config_path.display(),
                paths.len(),
                lake_path.display()
            ),
        })
    }
    // sirno:witness:lake-commands:end

    /// Create one Markdown entry.
    pub fn entry_new(&self, request: EntryNewRequest) -> Result<EntryFileResult, CommandError> {
        let (lake, mut settings) =
            resolve_lake_directory(self.lake_path.as_deref(), &self.config_path)?;
        settings.render = false;
        settings.witness = None;
        let report = EntryDirectory::new(&lake).check_with_settings(CheckMode::Edit, &settings)?;
        if report.has_errors() {
            return Err(EntryDirectoryError::InvalidEntryDirectory(lake).into());
        }
        let mut metadata = EntryMetadata::from_intrinsic_fields(intrinsic_fields_from_request(
            request.intrinsic,
            report.meta(),
        )?)?;
        for (field, targets) in
            structural_targets_by_target(request.structural, report.structural())?
        {
            metadata.set_structural_targets(field, targets);
        }

        let entry = Entry::new(request.id.clone(), metadata, request.body.unwrap_or_default());
        let path = EntryDirectory::new(&lake).create_entry(&entry)?;
        Ok(EntryFileResult {
            ok: true,
            id: request.id.to_string(),
            path: display_path(&path),
            message: format!("created {}", path.display()),
        })
    }

    /// Rename one entry address and its Sirno references.
    pub fn entry_rename(
        &self, old_id: EntryAddress, new_id: EntryAddress,
    ) -> Result<EntryRenameResult, CommandError> {
        let (lake, settings) =
            resolve_lake_directory(self.lake_path.as_deref(), &self.config_path)?;
        let report = EntryDirectory::new(&lake).rename_entry(&old_id, &new_id, &settings)?;
        let mut changed_paths = report.changed_paths().to_vec();
        if let Some(witness) = &settings.witness {
            changed_paths.extend(witness.rename_entry_references(&old_id, &new_id)?);
        }
        changed_paths.sort();
        changed_paths.dedup();
        let changed_paths = display_paths(&changed_paths);
        Ok(EntryRenameResult {
            ok: true,
            old_id: old_id.to_string(),
            new_id: new_id.to_string(),
            updated_paths: changed_paths,
            message: format!("renamed entry {old_id} to {new_id}"),
        })
    }

    /// Freeze one current lake entry and make its file read-only.
    pub fn entry_freeze(&self, id: EntryAddress) -> Result<EntryFileResult, CommandError> {
        let (lake, _) = resolve_lake_directory(self.lake_path.as_deref(), &self.config_path)?;
        let directory = EntryDirectory::new(&lake);
        directory.read_entry(&id)?;
        let path = directory.freeze_entry(&id)?;
        Ok(EntryFileResult {
            ok: true,
            id: id.to_string(),
            path: display_path(&path),
            message: format!("froze entry {id} at {}", path.display()),
        })
    }

    /// Melt one Sirno Lake Markdown entry and make its file writable.
    pub fn entry_melt(&self, id: EntryAddress) -> Result<EntryFileResult, CommandError> {
        let (lake, _) = resolve_lake_directory(self.lake_path.as_deref(), &self.config_path)?;
        let path = EntryDirectory::new(&lake).melt_entry(&id)?;
        Ok(EntryFileResult {
            ok: true,
            id: id.to_string(),
            path: display_path(&path),
            message: format!("melted entry {id} at {}", path.display()),
        })
    }

    /// Clear all Sirno local filesystem protection in the active lake.
    pub fn entry_melt_unsafe_all(
        &self, dry_run: bool,
    ) -> Result<LocalProtectionResult, CommandError> {
        let (lake, mut settings) =
            resolve_lake_directory(self.lake_path.as_deref(), &self.config_path)?;
        settings.render = false;
        settings.witness = None;
        let report = EntryDirectory::new(&lake).clear_local_protection(&settings, dry_run)?;
        Ok(local_protection_result(report.root(), report.paths(), dry_run, "clear"))
    }

    /// Reapply Sirno local filesystem protection from frozen metadata.
    pub fn entry_freeze_fix_all(
        &self, dry_run: bool,
    ) -> Result<LocalProtectionResult, CommandError> {
        let config = SirnoConfig::from_file(&self.config_path)?;
        let lake = resolve_lake_path(self.lake_path.as_deref(), &self.config_path, &config);
        let mut settings = entry_directory_check_settings(&self.config_path, &config)?;
        settings.render = false;
        settings.witness = None;
        let upstream_path = UpstreamFile::path_for_config(&self.config_path);
        let _upstream_file = UpstreamFile::from_file_if_exists(upstream_path)?;
        let report = EntryDirectory::new(&lake).fix_local_protection(&settings, false, dry_run)?;
        Ok(local_protection_result(report.root(), report.paths(), dry_run, "repair"))
    }

    /// Query entries and return an MCP-friendly JSON result.
    pub fn entry_query(&self, request: QueryRequest) -> Result<QueryResponse, CommandError> {
        match self.query_entries(request)? {
            | QueryRun::ColumnOptions(columns) => Ok(QueryResponse::column_options(columns)),
            | QueryRun::InvalidLake { columns, report } => {
                Ok(QueryResponse::invalid_lake(columns, &report))
            }
            | QueryRun::Results(results) => Ok(QueryResponse::results(results)),
        }
    }

    /// Run ripgrep in the configured Sirno Lake and capture its output.
    pub fn entry_rg(&self, request: RgRequest) -> Result<RgResult, CommandError> {
        if !request.with_generated_footer
            && request.args.iter().any(|arg| arg == "--pre" || arg.starts_with("--pre="))
        {
            return Err(CommandError::RgPreprocessorConflict);
        }

        let lake = resolve_lake_path_for_rg(self.lake_path.as_deref(), &self.config_path)?;
        let preprocessor =
            if request.with_generated_footer { None } else { Some(RgPreprocessorLink::create()?) };

        let mut command = ProcessCommand::new("rg");
        if let Some(preprocessor) = &preprocessor {
            command.arg("--pre").arg(preprocessor.path()).arg("--pre-glob").arg("*.md");
        }
        let output = command.args(&request.args).arg(lake).output().map_err(CommandError::RunRg)?;
        let exit_code = output.status.code().and_then(|code| u8::try_from(code).ok()).unwrap_or(1);
        Ok(RgResult {
            ok: output.status.success(),
            exit_code,
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        })
    }

    /// Return repository witness blocks for one entry as a JSON-first command result.
    pub fn entry_witness(
        &self, id: EntryAddress, verbose_json: bool,
    ) -> Result<WitnessResult, CommandError> {
        let records = self.witness_records(&id)?;
        Ok(WitnessResult {
            ok: !records.is_empty(),
            id: id.to_string(),
            records: records
                .iter()
                .map(|record| WitnessRecordResult::from_record(record, verbose_json))
                .collect(),
            message: if records.is_empty() {
                format!("no witness found for {id}")
            } else {
                format!("found {} witness records for {id}", records.len())
            },
        })
    }

    // sirno:witness:upstream-commands:begin
    /// Add or replace one upstream and crystallize it.
    pub fn upstream_add(
        &self, request: UpstreamAddRequest,
    ) -> Result<UpstreamCrystallizeReport, CommandError> {
        let mut config = SirnoConfig::from_file(&self.config_path)?;
        config.upstreams.insert(request.domain.clone(), request.settings);
        config.validate_for_file(&self.config_path)?;
        let lake_path = resolve_lake_path(self.lake_path.as_deref(), &self.config_path, &config);
        let mut settings = entry_directory_check_settings(&self.config_path, &config)?;
        settings.render = false;
        settings.witness = None;
        EntryDirectory::new(lake_path).ensure_glacier_replaceable(&request.domain, &settings)?;
        config.write(&self.config_path)?;
        self.upstream_crystallize(UpstreamCrystallizeRequest {
            domains: vec![request.domain],
            locked: false,
        })
    }

    /// Remove one upstream declaration and its glacier.
    pub fn upstream_remove(
        &self, domain: crate::EntryAtom,
    ) -> Result<UpstreamCrystallizeReport, CommandError> {
        let mut config = SirnoConfig::from_file(&self.config_path)?;
        if config.remove_upstream(&domain).is_none() {
            return Err(crate::UpstreamError::UnknownDomain(domain).into());
        }
        config.write(&self.config_path)?;
        let mut settings = entry_directory_check_settings(&self.config_path, &config)?;
        settings.render = false;
        settings.witness = None;
        let lake = EntryDirectory::new(resolve_lake_path(
            self.lake_path.as_deref(),
            &self.config_path,
            &config,
        ));
        let report = lake.replace_glacier(&domain, &[], &[], &settings)?;
        let upstream_path = UpstreamFile::path_for_config(&self.config_path);
        let mut upstream_file =
            UpstreamFile::from_file_if_exists(&upstream_path)?.unwrap_or_default();
        upstream_file.upstreams.shift_remove(&domain);
        write_upstream_file_or_remove(&upstream_path, &upstream_file)?;
        Ok(UpstreamCrystallizeReport {
            ok: true,
            domains: vec![domain.to_string()],
            changed_paths: display_paths(report.changed_paths()),
            message: format!("removed upstream {domain}"),
        })
    }

    /// Crystallize configured upstream lakes into glaciers.
    pub fn upstream_crystallize(
        &self, request: UpstreamCrystallizeRequest,
    ) -> Result<UpstreamCrystallizeReport, CommandError> {
        let config = SirnoConfig::from_file(&self.config_path)?;
        let lake_path = resolve_lake_path(self.lake_path.as_deref(), &self.config_path, &config);
        let mut settings = entry_directory_check_settings(&self.config_path, &config)?;
        settings.render = false;
        settings.witness = None;
        let upstream_path = UpstreamFile::path_for_config(&self.config_path);
        let mut upstream_file =
            UpstreamFile::from_file_if_exists(&upstream_path)?.unwrap_or_default();
        let cache = self.upstream_cache()?;
        let directory = EntryDirectory::new(&lake_path);
        let (mut report, _) =
            crate::upstream::crystallize_upstreams(crate::upstream::CrystallizeUpstreams {
                config_path: &self.config_path,
                config: &config,
                upstream_file: &mut upstream_file,
                lake: &directory,
                settings: &settings,
                cache: &cache,
                domains: &request.domains,
                locked: request.locked,
            })?;
        write_upstream_file_or_remove(&upstream_path, &upstream_file)?;
        let render = directory.generate_links_for_crystallization(&settings)?;
        report.changed_paths.extend(display_paths(render.changed_paths()));
        let protection = directory.fix_local_protection(&settings, false, false)?;
        report.changed_paths.extend(display_paths(protection.paths()));
        report.changed_paths.sort();
        report.changed_paths.dedup();
        Ok(report)
    }

    /// Update upstream locks and glaciers.
    pub fn upstream_update(
        &self, domains: Vec<crate::EntryAtom>,
    ) -> Result<UpstreamCrystallizeReport, CommandError> {
        self.upstream_crystallize(UpstreamCrystallizeRequest { domains, locked: false })
    }

    /// Return upstream status.
    pub fn upstream_status(&self) -> Result<UpstreamStatusReport, CommandError> {
        let config = SirnoConfig::from_file(&self.config_path)?;
        let upstream_file =
            UpstreamFile::from_file_if_exists(UpstreamFile::path_for_config(&self.config_path))?;
        let cache = self.upstream_cache()?;
        let lake_path = resolve_lake_path(self.lake_path.as_deref(), &self.config_path, &config);
        let mut settings = entry_directory_check_settings(&self.config_path, &config)?;
        settings.render = false;
        settings.witness = None;
        let lake = EntryDirectory::new(lake_path);
        Ok(crate::upstream::upstream_status(
            &self.config_path,
            &config,
            upstream_file.as_ref(),
            &cache,
            Some((&lake, &settings)),
        )?)
    }
    // sirno:witness:upstream-commands:end

    // sirno:witness:project-config-comments:begin
    /// Check whether `Sirno.toml` contains every canonical config comment.
    pub fn config_comments_check(&self) -> Result<ConfigCommentResult, CommandError> {
        self.config_comments(false)
    }

    /// Rewrite `Sirno.toml` through the canonical commented renderer when comments are missing.
    pub fn config_comments_fix(&self) -> Result<ConfigCommentResult, CommandError> {
        self.config_comments(true)
    }

    fn config_comments(&self, fix: bool) -> Result<ConfigCommentResult, CommandError> {
        let source = fs::read_to_string(&self.config_path).map_err(|source| {
            crate::ConfigError::Read { path: self.config_path.clone(), source }
        })?;
        let config = SirnoConfig::from_file(&self.config_path)?;
        let missing_comments = config.missing_comments_in(&source)?;
        let missing_count = missing_comments.len();
        let changed = fix && missing_count > 0;
        if changed {
            config.write(&self.config_path)?;
        }

        let message = match (missing_count, fix) {
            | (0, _) => format!("config comments ok in {}", self.config_path.display()),
            | (_, true) => format!(
                "updated config comments in {} ({missing_count} missing)",
                self.config_path.display()
            ),
            | (_, false) => format!(
                "config comments missing in {} ({missing_count} missing); run `sirno util config fix`",
                self.config_path.display()
            ),
        };

        Ok(ConfigCommentResult {
            ok: missing_count == 0 || fix,
            changed,
            config_path: display_path(&self.config_path),
            missing_comments,
            message,
        })
    }
    // sirno:witness:project-config-comments:end

    /// List artifacts owned by one entry.
    pub fn entry_artifact_list(
        &self, id: EntryAddress,
    ) -> Result<ArtifactListResult, CommandError> {
        let (lake, _) = resolve_lake_directory(self.lake_path.as_deref(), &self.config_path)?;
        let directory = EntryDirectory::new(&lake);
        directory.read_entry(&id)?;
        let artifacts = directory
            .read_entry_artifacts(&id)?
            .into_iter()
            .map(|artifact| artifact.path.to_string())
            .collect::<Vec<_>>();
        Ok(ArtifactListResult { ok: true, id: id.to_string(), artifacts })
    }

    /// Copy a file into one entry's artifact tree.
    pub fn entry_artifact_add(
        &self, request: ArtifactAddRequest,
    ) -> Result<ArtifactChangeResult, CommandError> {
        let (lake, _) = resolve_lake_directory(self.lake_path.as_deref(), &self.config_path)?;
        let directory = EntryDirectory::new(&lake);
        let artifact_path = match request.artifact_path {
            | Some(path) => artifact_path_from_cli(&path)?,
            | None => default_artifact_path_from_source(&request.source)?,
        };
        let path = directory.add_entry_artifact(&request.id, &request.source, &artifact_path)?;
        Ok(ArtifactChangeResult {
            ok: true,
            id: request.id.to_string(),
            artifact_path: artifact_path.to_string(),
            path: display_path(&path),
            message: format!("added artifact {artifact_path} at {}", path.display()),
        })
    }

    /// Rename one artifact path owned by an entry.
    pub fn entry_artifact_rename(
        &self, request: ArtifactRenameRequest,
    ) -> Result<ArtifactChangeResult, CommandError> {
        let (lake, _) = resolve_lake_directory(self.lake_path.as_deref(), &self.config_path)?;
        let directory = EntryDirectory::new(&lake);
        let old_path = artifact_path_from_cli(&request.old_path)?;
        let new_path = artifact_path_from_cli(&request.new_path)?;
        let path = directory.rename_entry_artifact(&request.id, &old_path, &new_path)?;
        Ok(ArtifactChangeResult {
            ok: true,
            id: request.id.to_string(),
            artifact_path: new_path.to_string(),
            path: display_path(&path),
            message: format!("renamed artifact {old_path} to {new_path} at {}", path.display()),
        })
    }

    /// Remove one artifact owned by an entry.
    pub fn entry_artifact_remove(
        &self, request: ArtifactRemoveRequest,
    ) -> Result<ArtifactChangeResult, CommandError> {
        let (lake, _) = resolve_lake_directory(self.lake_path.as_deref(), &self.config_path)?;
        let directory = EntryDirectory::new(&lake);
        let artifact_path = artifact_path_from_cli(&request.artifact_path)?;
        let path = directory.remove_entry_artifact(&request.id, &artifact_path)?;
        Ok(ArtifactChangeResult {
            ok: true,
            id: request.id.to_string(),
            artifact_path: artifact_path.to_string(),
            path: display_path(&path),
            message: format!("removed artifact {artifact_path} at {}", path.display()),
        })
    }

    // sirno:witness:charm-and-spell-commands:begin
    /// List discovered charm entries.
    pub fn charm_list(&self) -> Result<CharmListResult, CommandError> {
        let config = SirnoConfig::from_file(&self.config_path)?;
        let discovered = self.discover_charms(&config)?;
        let charms = discovered
            .into_iter()
            .map(|bundle| self.charm_record(&config, &bundle))
            .collect::<Vec<_>>();
        Ok(CharmListResult {
            ok: true,
            message: format!("found {} {}", charms.len(), plural(charms.len(), "charm", "charms")),
            charms,
        })
    }

    /// Show one discovered charm.
    pub fn charm_show(&self, id: EntryAddress) -> Result<CharmShowResult, CommandError> {
        let config = SirnoConfig::from_file(&self.config_path)?;
        let bundle = self.load_charm(&id)?;
        Ok(self.charm_show_result(&config, &bundle))
    }

    /// Enable one charm entry in project config.
    pub fn charm_enable(&self, id: EntryAddress) -> Result<CharmEnablementResult, CommandError> {
        self.load_charm(&id)?;
        let mut config = SirnoConfig::from_file(&self.config_path)?;
        let changed = config.charm.enable(id.clone());
        if changed {
            config.write(&self.config_path)?;
        }
        let message = if changed {
            format!("enabled charm {id} in {}", self.config_path.display())
        } else {
            format!("charm {id} is already enabled in {}", self.config_path.display())
        };
        Ok(CharmEnablementResult {
            ok: true,
            changed,
            id: id.to_string(),
            config_path: display_path(&self.config_path),
            message,
        })
    }

    /// Disable one charm entry in project config.
    pub fn charm_disable(&self, id: EntryAddress) -> Result<CharmEnablementResult, CommandError> {
        let mut config = SirnoConfig::from_file(&self.config_path)?;
        let changed = config.charm.disable(&id);
        if changed {
            config.write(&self.config_path)?;
        }
        let message = if changed {
            format!("disabled charm {id} in {}", self.config_path.display())
        } else {
            format!("charm {id} is not enabled in {}", self.config_path.display())
        };
        Ok(CharmEnablementResult {
            ok: true,
            changed,
            id: id.to_string(),
            config_path: display_path(&self.config_path),
            message,
        })
    }

    /// Run a charm setup command.
    pub fn charm_setup(&self, id: EntryAddress) -> Result<CharmProcessResult, CommandError> {
        let (config, bundle) = self.load_enabled_charm(&id)?;
        let cache = self.spell_cache_path(&bundle);
        let command = bundle.manifest.charm.setup.as_ref();
        self.run_optional_charm_command(&config, &bundle, &cache, "setup", command)
    }

    /// Run a charm check command.
    pub fn charm_check(&self, id: EntryAddress) -> Result<CharmProcessResult, CommandError> {
        let (config, bundle) = self.load_enabled_charm(&id)?;
        let cache = self.spell_cache_path(&bundle);
        let command = bundle.manifest.charm.check.as_ref();
        self.run_optional_charm_command(&config, &bundle, &cache, "check", command)
    }

    /// Build one source charm, or report that a direct charm needs no build.
    pub fn charm_build(&self, id: EntryAddress) -> Result<CharmProcessResult, CommandError> {
        let (config, bundle) = self.load_enabled_charm(&id)?;
        let cache = self.spell_cache_path(&bundle);
        self.run_optional_build_command(&config, &bundle, &cache, false)
    }

    /// Remove spell cache state for one charm entry.
    pub fn charm_clean(&self, id: EntryAddress) -> Result<CharmCleanResult, CommandError> {
        let bundle = self.load_charm(&id)?;
        let path = self.spell_cache_owner_path(&bundle.entry.id);
        let removed = match fs::remove_dir_all(&path) {
            | Ok(()) => true,
            | Err(source) if source.kind() == ErrorKind::NotFound => false,
            | Err(source) => return Err(CommandError::RemoveSpellCache { path, source }),
        };
        let message = if removed {
            format!("removed spell cache for charm {id} at {}", path.display())
        } else {
            format!("spell cache for charm {id} is already clean at {}", path.display())
        };
        Ok(CharmCleanResult {
            ok: true,
            removed,
            id: id.to_string(),
            path: display_path(&path),
            message,
        })
    }

    /// List spells resolved from enabled charms.
    pub fn spell_list(&self) -> Result<SpellListResult, CommandError> {
        let config = SirnoConfig::from_file(&self.config_path)?;
        let mut spells = Vec::new();
        for id in &config.charm.enabled {
            let bundle = self.load_charm(id)?;
            spells.push(self.spell_record(&bundle));
        }
        Ok(SpellListResult {
            ok: true,
            message: format!("found {} {}", spells.len(), plural(spells.len(), "spell", "spells")),
            spells,
        })
    }

    /// Show the spell resolved from one charm.
    pub fn spell_show(&self, id: EntryAddress) -> Result<CharmShowResult, CommandError> {
        self.charm_show(id)
    }

    /// Resolve and run one spell.
    pub fn spell_run(&self, id: EntryAddress) -> Result<CharmProcessResult, CommandError> {
        let (config, bundle) = self.load_enabled_charm(&id)?;
        let cache = self.spell_cache_path(&bundle);
        self.run_optional_build_command(&config, &bundle, &cache, true)?;
        self.run_required_command(
            &config,
            &bundle,
            &cache,
            "spell",
            &bundle.manifest.spell.command,
            true,
        )
    }
    // sirno:witness:charm-and-spell-commands:end

    // sirno:witness:charm-resolution:begin
    fn discover_charms(&self, config: &SirnoConfig) -> Result<Vec<CharmBundle>, CommandError> {
        let lake = resolve_lake_path(self.lake_path.as_deref(), &self.config_path, config);
        let mut settings = entry_directory_check_settings(&self.config_path, config)?;
        settings.render = false;
        settings.witness = None;
        let directory = EntryDirectory::new(&lake);
        let report = directory.check_with_settings(CheckMode::Edit, &settings)?;
        let manifest_path = manifest_artifact_path();
        let mut bundles = Vec::new();

        for entry in report.entries() {
            let artifacts = report
                .artifacts()
                .iter()
                .filter(|artifact| artifact.owner == entry.id)
                .cloned()
                .collect::<Vec<_>>();
            let Some(manifest_artifact) =
                artifacts.iter().find(|artifact| artifact.path == manifest_path)
            else {
                continue;
            };
            let manifest = CharmManifest::from_bytes(&manifest_artifact.content)?;
            bundles.push(CharmBundle {
                entry: entry.clone(),
                manifest,
                artifact_root: directory.entry_artifact_root_path(&entry.id),
                artifacts: artifact_map(artifacts),
            });
        }

        Ok(bundles)
    }

    fn load_charm(&self, id: &EntryAddress) -> Result<CharmBundle, CommandError> {
        let config = SirnoConfig::from_file(&self.config_path)?;
        let lake = resolve_lake_path(self.lake_path.as_deref(), &self.config_path, &config);
        let directory = EntryDirectory::new(&lake);
        let entry = directory.read_entry(id)?;
        let artifacts = directory.read_entry_artifacts(id)?;
        let manifest_path = manifest_artifact_path();
        let manifest_artifact = artifacts
            .iter()
            .find(|artifact| artifact.path == manifest_path)
            .ok_or_else(|| crate::CharmError::MissingManifest(id.clone()))?;
        let manifest = CharmManifest::from_bytes(&manifest_artifact.content)?;
        Ok(CharmBundle {
            entry,
            manifest,
            artifact_root: directory.entry_artifact_root_path(id),
            artifacts: artifact_map(artifacts),
        })
    }
    // sirno:witness:charm-resolution:end

    // sirno:witness:charm-enablement:begin
    fn load_enabled_charm(
        &self, id: &EntryAddress,
    ) -> Result<(SirnoConfig, CharmBundle), CommandError> {
        let config = SirnoConfig::from_file(&self.config_path)?;
        if !config.charm.contains(id) {
            return Err(CommandError::CharmNotEnabled(id.clone()));
        }
        let bundle = self.load_charm(id)?;
        Ok((config, bundle))
    }
    // sirno:witness:charm-enablement:end

    fn charm_record(&self, config: &SirnoConfig, bundle: &CharmBundle) -> CharmRecord {
        CharmRecord {
            id: bundle.entry.id.to_string(),
            intrinsic: bundle.entry.metadata.intrinsic.clone(),
            enabled: config.charm.contains(&bundle.entry.id),
            kind: bundle.kind_label().to_owned(),
            manifest_path: display_path(&bundle.artifact_root.join(CHARM_MANIFEST_FILE_NAME)),
        }
    }

    fn charm_show_result(&self, config: &SirnoConfig, bundle: &CharmBundle) -> CharmShowResult {
        CharmShowResult {
            ok: true,
            id: bundle.entry.id.to_string(),
            intrinsic: bundle.entry.metadata.intrinsic.clone(),
            enabled: config.charm.contains(&bundle.entry.id),
            kind: bundle.kind_label().to_owned(),
            manifest_path: display_path(&bundle.artifact_root.join(CHARM_MANIFEST_FILE_NAME)),
            artifact_root: display_path(&bundle.artifact_root),
            spell_cache_path: display_path(&self.spell_cache_path(bundle)),
            spell_command: bundle.manifest.spell.command.clone(),
            has_setup: bundle.manifest.charm.setup.is_some(),
            has_check: bundle.manifest.charm.check.is_some(),
            has_build: bundle.manifest.charm.build.is_some(),
            hooks: bundle.manifest.hooks.clone(),
        }
    }

    fn spell_record(&self, bundle: &CharmBundle) -> SpellRecord {
        SpellRecord {
            id: bundle.entry.id.to_string(),
            intrinsic: bundle.entry.metadata.intrinsic.clone(),
            kind: bundle.kind_label().to_owned(),
            spell_cache_path: display_path(&self.spell_cache_path(bundle)),
        }
    }

    fn run_optional_charm_command(
        &self, config: &SirnoConfig, bundle: &CharmBundle, cache: &Path, phase: &'static str,
        command: Option<&CharmCommandSpec>,
    ) -> Result<CharmProcessResult, CommandError> {
        let Some(command) = command else {
            return Ok(CharmProcessResult {
                ok: true,
                id: bundle.entry.id.to_string(),
                phase: phase.to_owned(),
                skipped: true,
                exit_code: None,
                stdout: String::new(),
                stderr: String::new(),
                message: format!("no {phase} command declared for charm {}", bundle.entry.id),
            });
        };
        self.run_required_command(config, bundle, cache, phase, &command.command, false)
    }

    fn run_optional_build_command(
        &self, config: &SirnoConfig, bundle: &CharmBundle, cache: &Path, allow_cached: bool,
    ) -> Result<CharmProcessResult, CommandError> {
        let Some(build) = &bundle.manifest.charm.build else {
            return Ok(CharmProcessResult {
                ok: true,
                id: bundle.entry.id.to_string(),
                phase: "build".to_owned(),
                skipped: true,
                exit_code: None,
                stdout: String::new(),
                stderr: String::new(),
                message: format!("direct charm {} does not need a build", bundle.entry.id),
            });
        };

        if allow_cached
            && let Some(output) = &build.output
            && cache.join(output).exists()
        {
            return Ok(CharmProcessResult {
                ok: true,
                id: bundle.entry.id.to_string(),
                phase: "build".to_owned(),
                skipped: true,
                exit_code: None,
                stdout: String::new(),
                stderr: String::new(),
                message: format!("spell cache already contains {}", cache.join(output).display()),
            });
        }

        fs::create_dir_all(cache).map_err(|source| CommandError::CreateSpellCache {
            path: cache.to_path_buf(),
            source,
        })?;
        let mut result = self.run_build_command(config, bundle, cache, "build", build)?;
        if result.ok
            && let Some(output) = &build.output
            && !cache.join(output).exists()
            && !bundle.artifact_root.join(output).exists()
        {
            result.ok = false;
            result.message = format!(
                "build command for charm {} did not produce {}",
                bundle.entry.id,
                output.display()
            );
        }
        Ok(result)
    }

    fn run_build_command(
        &self, config: &SirnoConfig, bundle: &CharmBundle, cache: &Path, phase: &'static str,
        build: &CharmBuildSpec,
    ) -> Result<CharmProcessResult, CommandError> {
        self.run_required_command(config, bundle, cache, phase, &build.command, false)
    }

    // sirno:witness:spell:begin
    fn run_required_command(
        &self, config: &SirnoConfig, bundle: &CharmBundle, cache: &Path, phase: &'static str,
        argv: &[String], prefer_cache: bool,
    ) -> Result<CharmProcessResult, CommandError> {
        fs::create_dir_all(cache).map_err(|source| CommandError::CreateSpellCache {
            path: cache.to_path_buf(),
            source,
        })?;
        let project_root = config_parent(&self.config_path);
        let lake = resolve_lake_path(self.lake_path.as_deref(), &self.config_path, config);
        let argv =
            resolve_manifest_argv(argv, &bundle.artifact_root, cache, &project_root, prefer_cache);
        let mut command = ProcessCommand::new(&argv[0]);
        command.args(&argv[1..]);
        command.current_dir(&bundle.artifact_root);
        command.env("SIRNO_CONFIG", &self.config_path);
        command.env("SIRNO_PROJECT_ROOT", &project_root);
        command.env("SIRNO_LAKE", lake);
        command.env("SIRNO_CHARM", bundle.entry.id.as_str());
        command.env("SIRNO_CHARM_ROOT", &bundle.artifact_root);
        command.env("SIRNO_SPELL_DIR", cache);
        let output = command.output().map_err(|source| CommandError::RunCharmProcess {
            id: bundle.entry.id.clone(),
            phase,
            source,
        })?;
        let ok = output.status.success();
        let exit_code = output.status.code();
        let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
        let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
        let message = if ok {
            format!("{phase} command for charm {} succeeded", bundle.entry.id)
        } else {
            format!(
                "{phase} command for charm {} failed with {}",
                bundle.entry.id,
                exit_code
                    .map(|code| format!("exit code {code}"))
                    .unwrap_or_else(|| "no exit code".to_owned())
            )
        };
        Ok(CharmProcessResult {
            ok,
            id: bundle.entry.id.to_string(),
            phase: phase.to_owned(),
            skipped: false,
            exit_code,
            stdout,
            stderr,
            message,
        })
    }
    // sirno:witness:spell:end

    fn spell_cache_owner_path(&self, id: &EntryAddress) -> PathBuf {
        config_parent(&self.config_path).join(SPELL_CACHE_DIRECTORY).join(id.as_str())
    }

    fn spell_cache_path(&self, bundle: &CharmBundle) -> PathBuf {
        self.spell_cache_owner_path(&bundle.entry.id).join(bundle.fingerprint())
    }

    // sirno:witness:agent-skills:begin
    /// List bundled Sirno skill wrapper constants and package targets.
    pub fn skill_wrappers_list(&self) -> Result<SkillWrapperResult, CommandError> {
        let records = BundledSkill::wrapper_packages()
            .map(|source| source.record("source", false))
            .collect::<Vec<_>>();
        Ok(SkillWrapperResult {
            ok: true,
            message: format!("found {} Sirno skill wrappers", records.len()),
            records,
        })
    }

    /// List bundled wrappers and optional Claude skill links.
    pub fn skill_wrappers_list_with_claude(
        &self, claude_skills: bool,
    ) -> Result<SkillWrapperResult, CommandError> {
        if !claude_skills {
            return self.skill_wrappers_list();
        }

        let mut result = self.skill_wrappers_list()?;
        result.records.extend(
            BundledSkill::wrapper_packages().map(|source| source.claude_record("link", false)),
        );
        result.message = format!(
            "found {} Sirno skill wrappers and {} Claude skill links",
            BundledSkill::wrapper_packages().count(),
            BundledSkill::wrapper_packages().count()
        );
        Ok(result)
    }

    /// Check installed Sirno skill wrapper packages against bundled constants.
    pub fn skill_wrappers_check(&self) -> Result<SkillWrapperResult, CommandError> {
        let root = config_parent(&self.config_path);
        let mut records = Vec::new();
        for source in BundledSkill::wrapper_packages() {
            let target = root.join(source.target_path());
            let status = match fs::read(&target) {
                | Ok(current) if current == source.content.as_bytes() => "ok",
                | Ok(_) => "drifted",
                | Err(error) if error.kind() == ErrorKind::NotFound => "missing",
                | Err(source) => {
                    return Err(CommandError::ReadSkillWrapperTarget { path: target, source });
                }
            };
            records.push(source.record(status, status != "ok"));
        }

        let changed = records.iter().filter(|record| record.changed).count();
        Ok(SkillWrapperResult {
            ok: changed == 0,
            message: if changed == 0 {
                format!("all {} Sirno skill wrappers match artifacts", records.len())
            } else {
                format!("{changed} Sirno skill wrappers differ from artifacts")
            },
            records,
        })
    }

    /// Check installed wrappers and optional Claude skill links.
    pub fn skill_wrappers_check_with_claude(
        &self, claude_skills: bool,
    ) -> Result<SkillWrapperResult, CommandError> {
        if !claude_skills {
            return self.skill_wrappers_check();
        }

        let root = config_parent(&self.config_path);
        let mut result = self.skill_wrappers_check()?;
        for source in BundledSkill::wrapper_packages() {
            let status = check_claude_skill_link(&root, &source)?;
            result.records.push(source.claude_record(status, status != "ok"));
        }

        let changed = result.records.iter().filter(|record| record.changed).count();
        result.ok = changed == 0;
        result.message = if changed == 0 {
            format!(
                "all {} Sirno skill wrappers and Claude links match artifacts",
                BundledSkill::wrapper_packages().count()
            )
        } else {
            format!("{changed} Sirno skill wrappers or Claude links differ from artifacts")
        };
        Ok(result)
    }

    /// Install bundled Sirno skill wrapper constants into their package targets.
    pub fn skill_wrappers_init(&self) -> Result<SkillWrapperResult, CommandError> {
        let root = config_parent(&self.config_path);
        let mut records = Vec::new();
        for source in BundledSkill::wrapper_packages() {
            let target = root.join(source.target_path());
            let status = match fs::read(&target) {
                | Ok(current) if current == source.content.as_bytes() => "unchanged",
                | Ok(_) => {
                    write_skill_wrapper_target(&target, source.content.as_bytes())?;
                    "wrote"
                }
                | Err(error) if error.kind() == ErrorKind::NotFound => {
                    write_skill_wrapper_target(&target, source.content.as_bytes())?;
                    "wrote"
                }
                | Err(source) => {
                    return Err(CommandError::ReadSkillWrapperTarget { path: target, source });
                }
            };
            records.push(source.record(status, status == "wrote"));
        }

        let changed = records.iter().filter(|record| record.changed).count();
        Ok(SkillWrapperResult {
            ok: true,
            message: format!(
                "installed {} Sirno skill wrappers ({changed} changed)",
                records.len()
            ),
            records,
        })
    }

    /// Install bundled wrappers and optional Claude skill links.
    pub fn skill_wrappers_init_with_claude(
        &self, claude_skills: bool,
    ) -> Result<SkillWrapperResult, CommandError> {
        if !claude_skills {
            return self.skill_wrappers_init();
        }

        let root = config_parent(&self.config_path);
        let mut result = self.skill_wrappers_init()?;
        for source in BundledSkill::wrapper_packages() {
            let status = init_claude_skill_link(&root, &source)?;
            result.records.push(source.claude_record(status, status == "linked"));
        }

        let changed = result.records.iter().filter(|record| record.changed).count();
        result.message = format!(
            "installed {} Sirno skill wrappers and Claude links ({changed} changed)",
            BundledSkill::wrapper_packages().count()
        );
        Ok(result)
    }
    // sirno:witness:agent-skills:end

    fn upstream_cache(&self) -> Result<UpstreamGitCache, CommandError> {
        Ok(match &self.upstream_store_path {
            | Some(path) => UpstreamGitCache::new(path),
            | None => UpstreamGitCache::default_global()?,
        })
    }

    /// Move the configured Sirno Lake.
    // sirno:witness:lake-commands:begin
    pub fn lake_move(&self, lake: PathBuf) -> Result<MovePathResult, CommandError> {
        let config = SirnoConfig::from_file(&self.config_path)?;
        let old_lake = config.resolve_lake(&self.config_path);
        let config = config.with_lake(lake);
        config.validate_for_file(&self.config_path)?;
        let new_lake = config.resolve_lake(&self.config_path);
        let moved = move_configured_path_and_write_config(
            &old_lake,
            &new_lake,
            &config,
            &self.config_path,
        )?;
        Ok(MovePathResult {
            ok: true,
            moved,
            old_path: display_path(&old_lake),
            new_path: display_path(&new_lake),
            message: format!("moved lake {} to {}", old_lake.display(), new_lake.display()),
        })
    }
    // sirno:witness:lake-commands:end

    // sirno:witness:mist-commands:begin
    /// Render Markdown links for one misty lake projection.
    pub fn mist_render(
        &self, mist: Option<EntryAtom>, dry: bool,
    ) -> Result<RenderResult, CommandError> {
        self.mist_render_with_override_json(mist, dry, None)
    }

    /// Render Markdown links for one misty lake projection with optional JSON settings.
    // sirno:witness:misty-lake:begin
    pub fn mist_render_with_override_json(
        &self, mist: Option<EntryAtom>, dry: bool, override_json: Option<&str>,
    ) -> Result<RenderResult, CommandError> {
        let mut mist = ResolvedMist::load(&self.config_path, self.lake_path.as_deref(), mist)?;
        if let Some(override_json) = override_json {
            apply_structural_override_json(&mut mist.spec.render, override_json)?;
        }

        let reservoir_report = mist.reservoir_report(CheckMode::Review)?;
        if reservoir_report.has_errors() {
            return Ok(RenderResult::blocked(&reservoir_report));
        }
        let structural = mist.spec.render.structural_settings(reservoir_report.structural())?;
        let selected = mist
            .spec
            .select
            .select_entries(reservoir_report.entries(), reservoir_report.structural())?;
        let selected_ids = selected.iter().map(|entry| entry.id.clone()).collect::<BTreeSet<_>>();
        let selected_entries = selected
            .iter()
            .map(|entry| entry_without_generated_links(entry))
            .collect::<Result<Vec<_>, CommandError>>()?;
        let selected_artifacts = reservoir_report
            .artifacts()
            .iter()
            .filter(|artifact| selected_ids.contains(&artifact.owner))
            .cloned()
            .collect::<Vec<_>>();
        let rendered_entries =
            mist_projection_entries(&selected_entries, reservoir_report.entries(), &structural)?;

        let directory = EntryDirectory::new(&mist.projection_path);
        if dry {
            let report =
                mist_projection_dry_report(&mist.projection_path, &directory, &rendered_entries)?;
            return Ok(RenderResult::from_report(&report, dry));
        }

        let mut extra_changed_paths = directory.write_with_artifacts(
            &rendered_entries,
            &selected_artifacts,
            EntryDirectoryWritePolicy::ReplaceDirectory {
                ignore: mist.projection_settings.ignore.clone(),
            },
        )?;
        let report = GenLinkDirectoryReport::new(
            &mist.projection_path,
            rendered_entries.len(),
            rendered_entries.len(),
            Vec::new(),
        );
        let manifest_path = MistManifest::path_for_projection(&mist.projection_path);
        let manifest = MistManifest::from_entries(
            mist.name,
            mist.spec_path,
            mist.reservoir_path,
            mist.spec.projection,
            mist.spec.select,
            mist.spec.render,
            &selected_entries,
        )?;
        let manifest_changed = manifest.write_if_changed(&manifest_path)?;
        if manifest_changed {
            extra_changed_paths.push(manifest_path);
        }
        Ok(RenderResult::from_report_with_extra_changed_paths(&report, dry, &extra_changed_paths))
    }
    // sirno:witness:misty-lake:end

    /// Delete generated Markdown link footers for one misty lake projection.
    pub fn mist_render_delete(
        &self, mist: Option<EntryAtom>,
    ) -> Result<RenderResult, CommandError> {
        let mist = ResolvedMist::load(&self.config_path, self.lake_path.as_deref(), mist)?;
        let reservoir_report = mist.reservoir_report(CheckMode::Edit)?;
        if reservoir_report.has_errors() {
            return Ok(RenderResult::blocked(&reservoir_report));
        }
        let projection = EntryDirectory::new(&mist.projection_path);
        let projected_entries = projection
            .read_entries_with_registry(reservoir_report.meta(), mist.projection_settings.ignore)?;
        let report =
            mist_projection_delete_report(&mist.projection_path, &projection, &projected_entries)?;
        let manifest_path = MistManifest::path_for_projection(&mist.projection_path);
        let manifest_changed = MistManifest::remove_if_exists(&manifest_path)?;
        let extra_changed_paths = if manifest_changed { vec![manifest_path] } else { Vec::new() };
        Ok(RenderResult::from_report_with_extra_changed_paths(&report, false, &extra_changed_paths))
    }

    /// Show pending mist ripples and stale projection state.
    pub fn mist_status(&self, mist: Option<EntryAtom>) -> Result<MistStatusResult, CommandError> {
        let mist = ResolvedMist::load(&self.config_path, self.lake_path.as_deref(), mist)?;
        mist_status_for(&self.config_path, &mist)
    }

    /// Intake edited Markdown entry sources from one misty lake into the reservoir.
    // sirno:witness:misty-lake:begin
    pub fn mist_intake(&self, mist: Option<EntryAtom>) -> Result<MistIntakeResult, CommandError> {
        let mist = ResolvedMist::load(&self.config_path, self.lake_path.as_deref(), mist)?;
        let status = mist_status_for(&self.config_path, &mist)?;
        let blockers = mist_intake_blockers(&status);
        if !blockers.is_empty() {
            return Err(CommandError::MistIntakeBlocked(blockers.join("; ")));
        }

        let reservoir = EntryDirectory::new(&mist.reservoir_path);
        let projection = EntryDirectory::new(&mist.projection_path);
        let mut updated_entries = Vec::new();
        let mut changed_paths = Vec::new();
        for id in &status.changed_entries {
            let id = EntryAddress::new(id)?;
            let projected = projection.read_entry_source(&id)?;
            let clean = entry_source_without_generated_links(&id, &projected)?;
            changed_paths.push(reservoir.write_entry_source(&id, &clean)?);
            updated_entries.push(id.to_string());
        }

        let render = self.mist_render(Some(mist.name.clone()), false)?;
        changed_paths.extend(render.changed_paths.iter().map(PathBuf::from));
        changed_paths.sort();
        changed_paths.dedup();
        let message = if updated_entries.is_empty() {
            format!("mist {} has no ripples to intake", mist.name)
        } else {
            format!(
                "intook {} {} from mist {}",
                updated_entries.len(),
                plural(updated_entries.len(), "entry", "entries"),
                mist.name
            )
        };
        Ok(MistIntakeResult {
            ok: true,
            mist: mist.name.to_string(),
            reservoir_path: display_path(&mist.reservoir_path),
            projection_path: display_path(&mist.projection_path),
            updated_entries,
            changed_paths: display_paths(&changed_paths),
            message,
        })
    }
    // sirno:witness:misty-lake:end
    // sirno:witness:mist-commands:end

    // sirno:witness:anchor-commands:begin
    /// Accept the current lake as the new anchor baseline.
    // sirno:witness:versioning:begin
    pub fn anchor_update(&self) -> Result<AnchorUpdateResult, CommandError> {
        let context = TideContext::load(&self.config_path, self.lake_path.as_deref())?;
        let report = context.checked_report(CheckMode::Review)?;
        let anchor_exists = context.anchor_path.exists();
        let tide_file = context.load_tide_file_or_current()?;
        if anchor_exists {
            let tide = context.tide(&tide_file)?;
            let open_workitems = tide.open_statuses().count();
            if open_workitems > 0 {
                return Err(CommandError::AnchorUpdateOpenTide { open_workitems });
            }
        }
        let mist_status = self.mist_status(Some(MistSpec::default_name()))?;
        if mist_status.has_ripples_or_blockers() {
            return Err(CommandError::AnchorUpdateMist(mist_status.message));
        }

        let anchor = context.anchor_from_report(&report)?;
        anchor.write(&context.anchor_path)?;

        let cleared_tide_resolutions = tide_file.resolved.len();
        TideFile::remove_if_exists(&context.tide_path)?;

        Ok(AnchorUpdateResult {
            ok: true,
            anchor_path: display_path(&context.anchor_path),
            lake_path: display_path(report.root()),
            entry_count: anchor.entries.len(),
            cleared_tide_resolutions,
            message: format!(
                "anchored {} entries in {}",
                anchor.entries.len(),
                context.anchor_path.display()
            ),
        })
    }
    // sirno:witness:versioning:end
    // sirno:witness:anchor-commands:end

    // sirno:witness:project-status-commands:begin
    /// Show the current Sirno project status.
    pub fn status(&self, request: StatusRequest) -> Result<StatusResult, CommandError> {
        let (lake, settings) =
            resolve_lake_directory(self.lake_path.as_deref(), &self.config_path)?;
        let report = EntryDirectory::new(&lake).check_with_settings(request.mode, &settings)?;
        let check = LakeCheckResult::from_report(&report);
        let tide_context = if !check.has_errors {
            Some(TideContext::load(&self.config_path, self.lake_path.as_deref())?)
        } else {
            None
        };
        let tide = if let Some(tide_context) = &tide_context {
            let tide_file = tide_context.load_tide_file_or_current()?;
            let tide = tide_context.tide(&tide_file)?;
            Some(StatusTide::from_tide(&tide))
        } else {
            None
        };
        let include_ripples = request.show.includes_full_details();
        let anchor = if let Some(tide_context) = &tide_context {
            Some(status_anchor_from_report(tide_context, &report, include_ripples)?)
        } else {
            None
        };
        let mist = if !check.has_errors {
            Some(self.mist_status(Some(MistSpec::default_name()))?)
        } else {
            None
        };
        let ok = check.ok
            && tide.as_ref().is_none_or(|tide| tide.clear)
            && anchor.as_ref().is_none_or(|anchor| anchor.ok)
            && mist.as_ref().is_none_or(|mist| mist.ok || !mist.manifest_present);
        let structural_fields = report
            .structural()
            .with_tide_policies_from_entries(report.entries())
            .fields()
            .map(|(field, settings)| StructuralFieldStatus {
                field: field.to_owned(),
                entry: report
                    .structural()
                    .entry_for_field(field)
                    .expect("effective relation has discovered entry")
                    .to_string(),
                to: StructuralEdgeStatus::from_settings(&settings.to),
                from: StructuralEdgeStatus::from_settings(&settings.from),
                clique: StructuralEdgeStatus::from_settings(&settings.clique),
            })
            .collect::<Vec<_>>();
        let structural_field_count = structural_fields.len();
        let check_policy = StatusCheckPolicy { mode: request.mode, render: settings.render };
        let blockers =
            StatusBlockers::from_parts(&check, tide.as_ref(), anchor.as_ref(), mist.as_ref());
        let message = status_message(ok, report.entries().len(), report.root(), &blockers);
        let show = request.show;
        Ok(StatusResult {
            ok,
            config_path: display_path(&self.config_path),
            lake_path: display_path(report.root()),
            entry_count: report.entries().len(),
            structural_field_count,
            blockers,
            message,
            check_policy: show.includes_domain_details().then_some(check_policy),
            structural_fields: if show.includes_full_details() {
                structural_fields
            } else {
                Vec::new()
            },
            tide: show.includes_domain_details().then_some(tide).flatten(),
            anchor: show.includes_domain_details().then_some(anchor).flatten(),
            mist: show.includes_domain_details().then_some(mist).flatten(),
            // Note: a failed check always carries its diagnostics, at every show mode,
            // so a failing status can explain itself. Full mode also keeps the clean check.
            check: (show.includes_full_details() || !check.ok).then_some(check),
        })
    }
    // sirno:witness:project-status-commands:end

    /// Return active project metadata needed to render MCP skill resource templates.
    pub fn skill_resource_context(&self) -> Result<SkillResourceContext, CommandError> {
        let config = SirnoConfig::from_file(&self.config_path)?;
        let lake = resolve_lake_path(self.lake_path.as_deref(), &self.config_path, &config);
        let meta_path = meta_file_path_for_config(&self.config_path);
        let meta = MetaFile::from_file(&meta_path)?;
        Ok(SkillResourceContext {
            config_path: display_path(&self.config_path),
            lake_path: display_path(&lake),
            meta_path: display_path(&meta_path),
            intrinsic_fields: meta.intrinsics,
            structural_fields: meta.structural,
        })
    }

    /// Resolve tide workitems.
    pub fn tide_resolve(
        &self, request: TideResolveRequest,
    ) -> Result<TideChangeResult, CommandError> {
        let context = TideContext::load(&self.config_path, self.lake_path.as_deref())?;
        let mut tide_file = context.load_tide_file_or_current()?;
        let tide = context.tide(&tide_file)?;
        let (resolutions, count) = if request.infer {
            tide.resolve_where(|status| tide.ripple_ids().contains(&status.workitem.neighbor))
        } else {
            tide.resolve_where(|status| tide_selection_matches(&request, status))
        };
        tide_file.set_resolved(resolutions);
        context.write_tide_file_or_remove(&tide_file)?;
        Ok(TideChangeResult {
            ok: true,
            count,
            message: format!("resolved {count} tide workitems"),
        })
    }

    /// Remove resolved marks from tide workitems.
    pub fn tide_unresolve(
        &self, request: TideSelectionRequest,
    ) -> Result<TideChangeResult, CommandError> {
        let context = TideContext::load(&self.config_path, self.lake_path.as_deref())?;
        let mut tide_file = context.load_tide_file_or_current()?;
        let tide = context.tide(&tide_file)?;
        let (resolutions, count) =
            tide.reopen_where(|status| tide_selection_request_matches(&request, status));
        tide_file.set_resolved(resolutions);
        context.write_tide_file_or_remove(&tide_file)?;
        Ok(TideChangeResult {
            ok: true,
            count,
            message: format!("unresolved {count} tide workitems"),
        })
    }

    /// Clear all tide resolutions from the Tide file.
    pub fn tide_reset(&self) -> Result<TideChangeResult, CommandError> {
        let context = TideContext::load(&self.config_path, self.lake_path.as_deref())?;
        let tide_file = context.load_tide_file_or_current()?;
        let count = tide_file.resolved.len();
        TideFile::remove_if_exists(&context.tide_path)?;
        Ok(TideChangeResult {
            ok: true,
            count,
            message: format!("cleared {count} tide resolutions"),
        })
    }
}

fn status_message(ok: bool, entry_count: usize, lake: &Path, blockers: &StatusBlockers) -> String {
    if ok {
        return format!(
            "status clean: {entry_count} {} in {}",
            plural(entry_count, "entry", "entries"),
            lake.display()
        );
    }

    let mut parts = Vec::new();
    if blockers.check_errors > 0 {
        parts.push(format!(
            "{} check {}",
            blockers.check_errors,
            plural(blockers.check_errors, "error", "errors")
        ));
    }
    if let Some(open_workitems) =
        blockers.tide_open_workitems.filter(|open_workitems| *open_workitems > 0)
    {
        parts.push(format!(
            "{open_workitems} tide {}",
            plural(open_workitems, "workitem", "workitems")
        ));
    }
    if blockers.anchor_missing.unwrap_or(false) {
        parts.push("anchor missing".to_owned());
    } else if let Some(ripples) = blockers.anchor_ripples.filter(|ripples| *ripples > 0) {
        parts.push(format!("{ripples} anchor {}", plural(ripples, "ripple", "ripples")));
    }
    let mist_blockers = blockers.mist_blocker_count();
    if mist_blockers > 0 {
        parts
            .push(format!("{mist_blockers} mist {}", plural(mist_blockers, "blocker", "blockers")));
    }
    if parts.is_empty() {
        "status has blockers".to_owned()
    } else {
        format!("status has blockers: {}", parts.join(", "))
    }
}

fn write_upstream_file_or_remove(path: &Path, file: &UpstreamFile) -> Result<(), CommandError> {
    if file.is_empty() {
        UpstreamFile::remove_if_exists(path)?;
    } else {
        file.write(path)?;
    }
    Ok(())
}

fn move_configured_path_and_write_config(
    source: &Path, destination: &Path, config: &SirnoConfig, config_path: &Path,
) -> Result<bool, CommandError> {
    let move_result = move_configured_path(source, destination)?;
    if let Err(config_error) = config.write(config_path) {
        if move_result.moved()
            && let Err(rollback) = rollback_configured_path(source, destination, move_result)
        {
            return Err(CommandError::MoveConfigWriteRollback {
                source_path: source.to_path_buf(),
                destination_path: destination.to_path_buf(),
                source: Box::new(config_error),
                rollback,
            });
        }
        return Err(CommandError::Config(config_error));
    }
    Ok(move_result.moved())
}

fn local_protection_result(
    root: &Path, paths: &[PathBuf], dry_run: bool, operation: &str,
) -> LocalProtectionResult {
    let action = match (operation, dry_run) {
        | ("clear", true) => "would clear local protection from",
        | ("clear", false) => "cleared local protection from",
        | ("repair", true) => "would repair local protection on",
        | ("repair", false) => "repaired local protection on",
        | (_, true) => "would update local protection on",
        | (_, false) => "updated local protection on",
    };
    let root = display_path(root);
    LocalProtectionResult {
        ok: true,
        dry_run,
        lake_path: root.clone(),
        paths: display_paths(paths),
        message: format!("{action} {} paths in {root}", paths.len()),
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ConfiguredPathMove {
    Unchanged,
    Direct,
    Nested,
}

impl ConfiguredPathMove {
    fn moved(self) -> bool {
        !matches!(self, Self::Unchanged)
    }
}

fn move_configured_path(
    source: &Path, destination: &Path,
) -> Result<ConfiguredPathMove, CommandError> {
    if source == destination {
        return Ok(ConfiguredPathMove::Unchanged);
    }

    ensure_move_destination_missing(destination)?;
    if destination.starts_with(source) {
        move_configured_path_nested(source, destination)?;
        return Ok(ConfiguredPathMove::Nested);
    }

    create_move_destination_parent(destination)?;
    fs::rename(source, destination).map_err(|error| CommandError::MovePath {
        source_path: source.to_path_buf(),
        destination_path: destination.to_path_buf(),
        source: error,
    })?;
    Ok(ConfiguredPathMove::Direct)
}

fn ensure_move_destination_missing(destination: &Path) -> Result<(), CommandError> {
    match fs::symlink_metadata(destination) {
        | Ok(_) => Err(CommandError::MoveDestinationExists(destination.to_path_buf())),
        | Err(source) if source.kind() == ErrorKind::NotFound => Ok(()),
        | Err(source) => {
            Err(CommandError::ReadMoveDestination { path: destination.to_path_buf(), source })
        }
    }
}

fn create_move_destination_parent(destination: &Path) -> Result<(), CommandError> {
    let Some(parent) = move_destination_parent(destination) else {
        return Ok(());
    };
    create_move_destination_parent_io(destination).map_err(|source| {
        CommandError::CreateMoveDestinationParent { path: parent.to_path_buf(), source }
    })
}

fn move_configured_path_nested(source: &Path, destination: &Path) -> Result<(), CommandError> {
    let staging_parent = move_staging_parent(source);
    let staging = move_staging_path(source).map_err(|source| {
        CommandError::PrepareMoveStagingPath { path: staging_parent.to_path_buf(), source }
    })?;

    fs::rename(source, &staging).map_err(|error| CommandError::MovePath {
        source_path: source.to_path_buf(),
        destination_path: destination.to_path_buf(),
        source: error,
    })?;

    if let Err(error) = create_move_destination_parent_io(destination) {
        if let Err(rollback) = rollback_nested_staging_path(&staging, source, destination) {
            return Err(CommandError::MovePathRollback {
                source_path: source.to_path_buf(),
                destination_path: destination.to_path_buf(),
                staging_path: staging,
                source: error,
                rollback,
            });
        }
        let parent = move_destination_parent(destination)
            .expect("destination parent exists after create failed");
        return Err(CommandError::CreateMoveDestinationParent {
            path: parent.to_path_buf(),
            source: error,
        });
    }

    if let Err(error) = fs::rename(&staging, destination) {
        if let Err(rollback) = rollback_nested_staging_path(&staging, source, destination) {
            return Err(CommandError::MovePathRollback {
                source_path: source.to_path_buf(),
                destination_path: destination.to_path_buf(),
                staging_path: staging,
                source: error,
                rollback,
            });
        }
        return Err(CommandError::MovePath {
            source_path: source.to_path_buf(),
            destination_path: destination.to_path_buf(),
            source: error,
        });
    }

    Ok(())
}

fn rollback_configured_path(
    source: &Path, destination: &Path, move_result: ConfiguredPathMove,
) -> std::io::Result<()> {
    match move_result {
        | ConfiguredPathMove::Unchanged => Ok(()),
        | ConfiguredPathMove::Direct => fs::rename(destination, source),
        | ConfiguredPathMove::Nested => rollback_nested_destination(source, destination),
    }
}

fn rollback_nested_destination(source: &Path, destination: &Path) -> std::io::Result<()> {
    let staging = move_staging_path(source)?;
    fs::rename(destination, &staging)?;
    remove_empty_nested_destination_parent(source, destination)?;
    fs::rename(staging, source)
}

fn rollback_nested_staging_path(
    staging: &Path, source: &Path, destination: &Path,
) -> std::io::Result<()> {
    remove_empty_nested_destination_parent(source, destination)
        .and_then(|()| fs::rename(staging, source))
}

fn remove_empty_nested_destination_parent(
    source: &Path, destination: &Path,
) -> std::io::Result<()> {
    let mut path = destination.parent();
    while let Some(parent) = path {
        if !parent.starts_with(source) {
            break;
        }

        match fs::remove_dir(parent) {
            | Ok(()) => {}
            | Err(error) if error.kind() == ErrorKind::NotFound => {}
            | Err(error) => return Err(error),
        }

        if parent == source {
            break;
        }
        path = parent.parent();
    }
    Ok(())
}

fn move_destination_parent(destination: &Path) -> Option<&Path> {
    destination.parent().filter(|parent| !parent.as_os_str().is_empty())
}

fn create_move_destination_parent_io(destination: &Path) -> std::io::Result<()> {
    let Some(parent) = move_destination_parent(destination) else {
        return Ok(());
    };
    fs::create_dir_all(parent)
}

fn move_staging_parent(source: &Path) -> &Path {
    source
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."))
}

fn move_staging_path(source: &Path) -> std::io::Result<PathBuf> {
    let parent = move_staging_parent(source);
    let source_name = source.file_name().map(OsString::from).unwrap_or_else(|| "path".into());

    for index in 0..1000 {
        let mut name = OsString::from(".sirno-move-");
        name.push(&source_name);
        name.push(format!("-{}-{index}", std::process::id()));
        let candidate = parent.join(name);
        match fs::symlink_metadata(&candidate) {
            | Ok(_) => {}
            | Err(error) if error.kind() == ErrorKind::NotFound => return Ok(candidate),
            | Err(error) => return Err(error),
        }
    }

    Err(std::io::Error::new(
        ErrorKind::AlreadyExists,
        format!("move staging paths are unavailable near {}", parent.display()),
    ))
}

struct TideContext {
    tide_path: PathBuf,
    anchor_path: PathBuf,
    anchor_lake_path: PathBuf,
    settings: EntryDirectoryCheckSettings,
    lake_path: PathBuf,
}

impl TideContext {
    fn load(config_path: &Path, lake_path: Option<&Path>) -> Result<Self, CommandError> {
        let config = SirnoConfig::from_file(config_path)?;
        Ok(Self {
            tide_path: TideFile::path_for_config(config_path),
            anchor_path: AnchorFile::path_for_config(config_path),
            anchor_lake_path: lake_path.map(Path::to_path_buf).unwrap_or(config.lake.path.clone()),
            settings: entry_directory_check_settings(config_path, &config)?,
            lake_path: resolve_lake_path(lake_path, config_path, &config),
        })
    }

    fn load_tide_file_or_current(&self) -> Result<TideFile, CommandError> {
        Ok(TideFile::from_file_if_exists(&self.tide_path)?.unwrap_or_default())
    }

    fn write_tide_file_or_remove(&self, file: &TideFile) -> Result<(), CommandError> {
        if file.is_empty() {
            TideFile::remove_if_exists(&self.tide_path)?;
        } else {
            file.write(&self.tide_path)?;
        }
        Ok(())
    }

    fn checked_report(&self, mode: CheckMode) -> Result<EntryDirectoryReport, CommandError> {
        let mut settings = self.settings.clone();
        if mode != CheckMode::Review {
            settings.render = false;
            settings.witness = None;
        }
        let report = EntryDirectory::new(&self.lake_path).check_with_settings(mode, &settings)?;
        if report.has_errors() {
            return Err(EntryDirectoryError::InvalidEntryDirectory(self.lake_path.clone()).into());
        }
        Ok(report)
    }

    fn anchor_from_report(
        &self, report: &EntryDirectoryReport,
    ) -> Result<AnchorFile, CommandError> {
        let structural = report.structural().with_tide_policies_from_entries(report.entries());
        Ok(AnchorFile::from_report(&self.anchor_lake_path, report, &structural)?)
    }

    fn tide(&self, file: &TideFile) -> Result<Tide, CommandError> {
        let report = self.checked_report(CheckMode::Edit)?;
        let structural = report.structural().with_tide_policies_from_entries(report.entries());
        let anchor = AnchorFile::from_file_if_exists(&self.anchor_path)?;
        let anchor_snapshots =
            anchor.as_ref().map(anchor_snapshots).transpose()?.unwrap_or_default();
        let waterline = report
            .entries()
            .iter()
            .map(TideEntrySnapshot::from_entry)
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Tide::from_snapshots(&anchor_snapshots, &waterline, &structural, &file.resolved)?)
    }
}

fn anchor_snapshots(anchor: &AnchorFile) -> Result<Vec<TideEntrySnapshot>, CommandError> {
    anchor
        .entries
        .iter()
        .map(|(id, entry)| {
            let id = EntryAddress::new(id.as_str())?;
            Ok(TideEntrySnapshot::from_anchor_entry(id, entry)?)
        })
        .collect()
}

fn anchor_ripples(
    anchor: &AnchorFile, current: &AnchorFile,
) -> Result<Vec<AnchorRippleRecord>, CommandError> {
    let mut ids = BTreeSet::new();
    ids.extend(anchor.entries.keys().cloned());
    ids.extend(current.entries.keys().cloned());

    let mut ripples = Vec::new();
    for id in ids {
        let kind = match (anchor.entries.get(&id), current.entries.get(&id)) {
            | (None, Some(_)) => Some(AnchorRippleKind::Added),
            | (Some(_), None) => Some(AnchorRippleKind::Deleted),
            | (Some(left), Some(right)) if left != right => Some(AnchorRippleKind::Changed),
            | (Some(_), Some(_)) | (None, None) => None,
        };
        if let Some(kind) = kind {
            ripples.push(AnchorRippleRecord { id: EntryAddress::new(id)?, kind });
        }
    }
    Ok(ripples)
}

fn unanchored_ripples(current: &AnchorFile) -> Result<Vec<AnchorRippleRecord>, CommandError> {
    current
        .entries
        .keys()
        .map(|id| {
            Ok(AnchorRippleRecord {
                id: EntryAddress::new(id.as_str())?,
                kind: AnchorRippleKind::Added,
            })
        })
        .collect()
}

fn status_anchor_from_report(
    context: &TideContext, report: &EntryDirectoryReport, include_ripples: bool,
) -> Result<StatusAnchor, CommandError> {
    let current = context.anchor_from_report(report)?;
    let anchor = AnchorFile::from_file_if_exists(&context.anchor_path)?;
    let (initialized, ripples) = match anchor {
        | Some(anchor) => (true, anchor_ripples(&anchor, &current)?),
        | None => (false, unanchored_ripples(&current)?),
    };
    let ripple_count = ripples.len();
    let ok = initialized && ripple_count == 0;
    Ok(StatusAnchor {
        ok,
        initialized,
        anchor_path: display_path(&context.anchor_path),
        lake_path: display_path(report.root()),
        entry_count: report.entries().len(),
        ripple_count,
        ripples: if include_ripples { ripples } else { Vec::new() },
        message: anchor_status_message(ok, initialized, current.entries.len(), ripple_count),
    })
}

fn anchor_status_message(
    ok: bool, initialized: bool, entry_count: usize, ripple_count: usize,
) -> String {
    if ok {
        format!("anchor is current for {entry_count} entries")
    } else if initialized {
        format!("{ripple_count} anchor {}", plural(ripple_count, "ripple", "ripples"))
    } else {
        "anchor is not initialized; run `sirno anchor update`".to_owned()
    }
}

impl BundledSkillWrapper<'_> {
    fn record(&self, status: impl Into<String>, changed: bool) -> SkillWrapperRecord {
        SkillWrapperRecord {
            name: self.skill.name.to_owned(),
            entry_id: self.skill.entry_id.to_owned(),
            wrapper_path: self.artifact_path(),
            full_path: self.skill.template_path(),
            target_path: self.target_path(),
            status: status.into(),
            changed,
        }
    }

    fn claude_record(&self, status: impl Into<String>, changed: bool) -> SkillWrapperRecord {
        SkillWrapperRecord {
            name: self.skill.name.to_owned(),
            entry_id: self.skill.entry_id.to_owned(),
            wrapper_path: self.agent_skill_path().display().to_string(),
            full_path: self.skill.template_path(),
            target_path: self.claude_skill_path().display().to_string(),
            status: status.into(),
            changed,
        }
    }
}

fn config_parent(config_path: &Path) -> PathBuf {
    config_path
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."))
        .to_path_buf()
}

fn write_skill_wrapper_target(target: &Path, content: &[u8]) -> Result<(), CommandError> {
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent).map_err(|source| {
            CommandError::CreateSkillWrapperTargetDirectory { path: parent.to_path_buf(), source }
        })?;
    }
    fs::write(target, content).map_err(|source| CommandError::WriteSkillWrapperTarget {
        path: target.to_path_buf(),
        source,
    })
}

fn check_claude_skill_link(
    root: &Path, source: &BundledSkillWrapper<'_>,
) -> Result<&'static str, CommandError> {
    let target = root.join(source.claude_skill_path());
    let expected = source.claude_link_source();
    let metadata = match fs::symlink_metadata(&target) {
        | Ok(metadata) => metadata,
        | Err(error) if error.kind() == ErrorKind::NotFound => return Ok("missing"),
        | Err(source) => {
            return Err(CommandError::ReadSkillWrapperTarget { path: target, source });
        }
    };
    if !metadata.file_type().is_symlink() {
        return Ok("drifted");
    }
    let current = fs::read_link(&target)
        .map_err(|source| CommandError::ReadSkillWrapperTarget { path: target, source })?;
    if current == expected { Ok("ok") } else { Ok("drifted") }
}

fn init_claude_skill_link(
    root: &Path, source: &BundledSkillWrapper<'_>,
) -> Result<&'static str, CommandError> {
    let target = root.join(source.claude_skill_path());
    let expected = source.claude_link_source();
    let status = check_claude_skill_link(root, source)?;
    match status {
        | "ok" => Ok("unchanged"),
        | "missing" => {
            create_claude_skill_link(&target, &expected)?;
            Ok("linked")
        }
        | "drifted" => {
            replace_claude_skill_link(&target, &expected)?;
            Ok("linked")
        }
        | _ => unreachable!("unexpected Claude skill link status"),
    }
}

fn replace_claude_skill_link(target: &Path, source: &Path) -> Result<(), CommandError> {
    let metadata = fs::symlink_metadata(target).map_err(|error| {
        CommandError::ReadSkillWrapperTarget { path: target.to_path_buf(), source: error }
    })?;
    if !metadata.file_type().is_symlink() {
        return Err(CommandError::SkillWrapperTargetExists(target.to_path_buf()));
    }
    fs::remove_file(target).map_err(|source| CommandError::RemoveSkillWrapperTarget {
        path: target.to_path_buf(),
        source,
    })?;
    create_claude_skill_link(target, source)
}

fn create_claude_skill_link(target: &Path, source: &Path) -> Result<(), CommandError> {
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent).map_err(|source| {
            CommandError::CreateSkillWrapperTargetDirectory { path: parent.to_path_buf(), source }
        })?;
    }
    symlink_skill_directory(source, target).map_err(|error| CommandError::LinkSkillWrapperTarget {
        source_path: source.to_path_buf(),
        target_path: target.to_path_buf(),
        source: error,
    })
}

#[cfg(unix)]
fn symlink_skill_directory(source: &Path, target: &Path) -> std::io::Result<()> {
    std::os::unix::fs::symlink(source, target)
}

#[cfg(windows)]
fn symlink_skill_directory(source: &Path, target: &Path) -> std::io::Result<()> {
    std::os::windows::fs::symlink_dir(source, target)
}

pub(crate) fn default_config_path() -> PathBuf {
    PathBuf::from(CONFIG_FILE_NAME)
}

pub(crate) fn default_lake_path(config_path: &Path) -> PathBuf {
    config_path.parent().unwrap_or_else(|| Path::new(".")).join(SIRNO_CONTROL_DIR_NAME).join("lake")
}

struct ResolvedMist {
    name: EntryAtom,
    spec_path: PathBuf,
    spec: MistSpec,
    reservoir_path: PathBuf,
    reservoir_settings: EntryDirectoryCheckSettings,
    projection_path: PathBuf,
    projection_settings: EntryDirectoryCheckSettings,
}

impl ResolvedMist {
    fn load(
        config_path: &Path, lake_path: Option<&Path>, name: Option<EntryAtom>,
    ) -> Result<Self, CommandError> {
        let config = SirnoConfig::from_file(config_path)?;
        let name = name.unwrap_or_else(MistSpec::default_name);
        let spec_path = MistSpec::path_for_config(config_path, &name);
        let spec = if name == MistSpec::default_name() && !spec_path.exists() {
            MistSpec::default()
        } else {
            MistSpec::from_file(&spec_path)?
        };
        let reservoir_path = resolve_lake_path(lake_path, config_path, &config);
        let reservoir_settings = entry_directory_check_settings(config_path, &config)?;
        let projection_path = resolve_projection_path(config_path, &spec.projection.path);
        let projection_settings = projection_check_settings(config_path, &config, &spec);
        Ok(Self {
            name,
            spec_path,
            spec,
            reservoir_path,
            reservoir_settings,
            projection_path,
            projection_settings,
        })
    }

    fn reservoir_report(&self, mode: CheckMode) -> Result<EntryDirectoryReport, CommandError> {
        let mut settings = self.reservoir_settings.clone();
        settings.render = false;
        settings.witness = if mode == CheckMode::Review { settings.witness } else { None };
        let report =
            EntryDirectory::new(&self.reservoir_path).check_with_settings(mode, &settings)?;
        Ok(report)
    }
}

fn resolve_projection_path(config_path: &Path, projection_path: &Path) -> PathBuf {
    if projection_path.is_absolute() {
        return projection_path.to_path_buf();
    }
    config_parent(config_path).join(projection_path)
}

fn entry_without_generated_links(entry: &Entry) -> Result<Entry, CommandError> {
    let mut entry = entry.clone();
    entry.body = remove_generated_footer_divider(&GeneratedLinkBody::new(&entry.body).delete()?);
    Ok(entry)
}

fn entry_source_without_generated_links(
    id: &EntryAddress, source: &str,
) -> Result<String, CommandError> {
    let entry = crate::RawEntry::from_markdown(id.clone(), source)?;
    let body = remove_generated_footer_divider(&GeneratedLinkBody::new(&entry.body).delete()?);
    Ok(Entry::replace_markdown_body(source, &body)?)
}

fn remove_generated_footer_divider(body: &str) -> String {
    let trimmed = body.trim_end_matches('\n');
    let Some(before) = trimmed.strip_suffix("\n---") else {
        return body.to_owned();
    };
    let before = before.trim_end_matches('\n');
    if before.is_empty() { String::new() } else { format!("{before}\n") }
}

// sirno:witness:mist:begin
fn mist_projection_entries(
    selected_entries: &[Entry], reservoir_entries: &[Entry], structural: &StructuralSettings,
) -> Result<Vec<Entry>, CommandError> {
    let index = StructuralEdgeIndex::from_entries(reservoir_entries);
    let mut rendered = Vec::new();
    for entry in selected_entries {
        let mut entry = entry.clone();
        let footer = index.render_entry(&entry, structural);
        entry.body = GeneratedLinkBody::new(&entry.body).apply(&footer)?;
        rendered.push(entry);
    }
    Ok(rendered)
}

fn mist_projection_dry_report(
    projection_root: &Path, projection: &EntryDirectory, rendered_entries: &[Entry],
) -> Result<GenLinkDirectoryReport, CommandError> {
    let mut changed_paths = Vec::new();
    for entry in rendered_entries {
        let path = projection.entry_file_path(&entry.id);
        let rendered = entry.to_markdown().map_err(EntryDirectoryError::from)?;
        match fs::read_to_string(&path) {
            | Ok(current) if current == rendered => {}
            | Ok(_) => changed_paths.push(path),
            | Err(source) if source.kind() == ErrorKind::NotFound => changed_paths.push(path),
            | Err(source) => return Err(EntryDirectoryError::from(source).into()),
        }
    }
    let changed_entry_count = changed_paths.len();
    Ok(GenLinkDirectoryReport::new(
        projection_root,
        rendered_entries.len(),
        changed_entry_count,
        changed_paths,
    ))
}

fn mist_projection_delete_report(
    projection_root: &Path, projection: &EntryDirectory, projected_entries: &[Entry],
) -> Result<GenLinkDirectoryReport, CommandError> {
    let mut changed_paths = Vec::new();
    for entry in projected_entries {
        if entry.metadata.meta.frozen.is_some() {
            return Err(EntryDirectoryError::FrozenEntryProtected(entry.id.clone()).into());
        }
        let path = projection.entry_file_path(&entry.id);
        let source = fs::read_to_string(&path).map_err(EntryDirectoryError::from)?;
        let body = GeneratedLinkBody::new(&entry.body).delete()?;
        let rendered = Entry::replace_markdown_body(&source, &body)?;
        if rendered != source {
            fs::write(&path, rendered)
                .map_err(|source| EntryDirectoryError::WriteFile { path: path.clone(), source })?;
            changed_paths.push(path);
        }
    }
    let changed_entry_count = changed_paths.len();
    Ok(GenLinkDirectoryReport::new(
        projection_root,
        projected_entries.len(),
        changed_entry_count,
        changed_paths,
    ))
}
// sirno:witness:mist:end

fn mist_status_for(
    config_path: &Path, mist: &ResolvedMist,
) -> Result<MistStatusResult, CommandError> {
    let manifest_path = MistManifest::path_for_projection(&mist.projection_path);
    let manifest = match MistManifest::from_file(&manifest_path) {
        | Ok(manifest) => manifest,
        | Err(crate::MistError::Read { source, .. }) if source.kind() == ErrorKind::NotFound => {
            return Ok(MistStatusResult {
                ok: false,
                manifest_present: false,
                mist: mist.name.to_string(),
                spec_path: display_path(&mist.spec_path),
                reservoir_path: display_path(&mist.reservoir_path),
                projection_path: display_path(&mist.projection_path),
                editable: mist.spec.projection.editable,
                entry_count: 0,
                changed_entries: Vec::new(),
                stale_entries: Vec::new(),
                missing_entries: Vec::new(),
                staged_paths: git_staged_paths_under(config_path, &mist.projection_path)
                    .map(|paths| display_paths(&paths))?,
                message: format!(
                    "mist {} has no projection manifest; run `sirno mist render`",
                    mist.name
                ),
            });
        }
        | Err(error) => return Err(error.into()),
    };

    let reservoir_report = mist.reservoir_report(CheckMode::Edit)?;
    if reservoir_report.has_errors() {
        return Err(EntryDirectoryError::InvalidEntryDirectory(mist.reservoir_path.clone()).into());
    }
    let reservoir = EntryDirectory::new(&mist.reservoir_path);
    let projection = EntryDirectory::new(&mist.projection_path);
    let manifest_ids = manifest
        .entries
        .iter()
        .map(|record| EntryAddress::new(&record.id))
        .collect::<Result<BTreeSet<_>, _>>()?;
    let entries_by_id = reservoir_report
        .entries()
        .iter()
        .map(|entry| (entry.id.clone(), entry))
        .collect::<BTreeMap<_, _>>();
    let mut changed_entries = Vec::new();
    let mut stale_entries = Vec::new();
    let mut missing_entries = Vec::new();

    for record in &manifest.entries {
        let id = EntryAddress::new(&record.id)?;
        let Some(reservoir_entry) = entries_by_id.get(&id) else {
            stale_entries.push(id.to_string());
            continue;
        };
        let current =
            MistManifestEntry::from_entry(&entry_without_generated_links(reservoir_entry)?)?;
        if current.fingerprint != record.fingerprint {
            stale_entries.push(id.to_string());
            continue;
        }

        let projected_source = match projection.read_entry_source(&id) {
            | Ok(source) => source,
            | Err(EntryDirectoryError::EntryNotFound(_))
            | Err(EntryDirectoryError::MissingDirectory(_)) => {
                missing_entries.push(id.to_string());
                continue;
            }
            | Err(error) => return Err(error.into()),
        };
        let projected_clean = entry_source_without_generated_links(&id, &projected_source)?;
        let reservoir_source = reservoir.read_entry_source(&id)?;
        let reservoir_clean = entry_source_without_generated_links(&id, &reservoir_source)?;
        let projected_entry = Entry::from_markdown_with_registry(
            id.clone(),
            &projected_clean,
            reservoir_report.meta(),
        )?;
        let reservoir_entry = Entry::from_markdown_with_registry(
            id.clone(),
            &reservoir_clean,
            reservoir_report.meta(),
        )?;
        if projected_entry != reservoir_entry {
            changed_entries.push(id.to_string());
        }
    }

    match projection.read_entries_with_registry(
        reservoir_report.meta(),
        mist.projection_settings.ignore.clone(),
    ) {
        | Ok(entries) => {
            for entry in entries {
                if !manifest_ids.contains(&entry.id)
                    && !changed_entries.iter().any(|id| id == entry.id.as_str())
                {
                    changed_entries.push(entry.id.to_string());
                }
            }
        }
        | Err(EntryDirectoryError::MissingDirectory(_)) => {}
        | Err(error) => return Err(error.into()),
    }

    let staged_paths = display_paths(&git_staged_paths_under(config_path, &mist.projection_path)?);
    let ok = changed_entries.is_empty()
        && stale_entries.is_empty()
        && missing_entries.is_empty()
        && staged_paths.is_empty();
    let message = mist_status_message(
        &mist.name,
        ok,
        changed_entries.len(),
        stale_entries.len(),
        missing_entries.len(),
        staged_paths.len(),
    );
    Ok(MistStatusResult {
        ok,
        manifest_present: true,
        mist: mist.name.to_string(),
        spec_path: display_path(&mist.spec_path),
        reservoir_path: display_path(&mist.reservoir_path),
        projection_path: display_path(&mist.projection_path),
        editable: mist.spec.projection.editable,
        entry_count: manifest.entries.len(),
        changed_entries,
        stale_entries,
        missing_entries,
        staged_paths,
        message,
    })
}

fn mist_status_message(
    name: &EntryAtom, ok: bool, changed: usize, stale: usize, missing: usize, staged: usize,
) -> String {
    if ok {
        return format!("mist {name} is clean");
    }

    let mut parts = Vec::new();
    if changed > 0 {
        parts.push(format!("{changed} changed {}", plural(changed, "entry", "entries")));
    }
    if stale > 0 {
        parts.push(format!("{stale} stale {}", plural(stale, "entry", "entries")));
    }
    if missing > 0 {
        parts.push(format!("{missing} missing {}", plural(missing, "entry", "entries")));
    }
    if staged > 0 {
        parts.push(format!("{staged} staged {}", plural(staged, "path", "paths")));
    }
    format!("mist {name} has {}", parts.join(", "))
}

fn mist_intake_blockers(status: &MistStatusResult) -> Vec<String> {
    let mut blockers = Vec::new();
    if !status.editable {
        blockers.push("projection is not editable".to_owned());
    }
    if !status.manifest_present {
        blockers.push("projection manifest is missing".to_owned());
    }
    if !status.stale_entries.is_empty() {
        blockers.push(format!(
            "{} stale {}",
            status.stale_entries.len(),
            plural(status.stale_entries.len(), "entry", "entries")
        ));
    }
    if !status.missing_entries.is_empty() {
        blockers.push(format!(
            "{} missing {}",
            status.missing_entries.len(),
            plural(status.missing_entries.len(), "entry", "entries")
        ));
    }
    if !status.staged_paths.is_empty() {
        blockers.push(format!(
            "{} staged {}",
            status.staged_paths.len(),
            plural(status.staged_paths.len(), "path", "paths")
        ));
    }
    blockers
}

fn git_staged_paths_under(
    config_path: &Path, projection_path: &Path,
) -> Result<Vec<PathBuf>, CommandError> {
    // sirno:witness:versioning:begin
    let root = config_parent(config_path);
    let probe = ProcessCommand::new("git")
        .arg("-C")
        .arg(&root)
        .arg("rev-parse")
        .arg("--is-inside-work-tree")
        .output()
        .map_err(CommandError::RunGit)?;
    if !probe.status.success() {
        return Ok(Vec::new());
    }
    let pathspec = projection_path.strip_prefix(&root).unwrap_or(projection_path);
    let output = ProcessCommand::new("git")
        .arg("-C")
        .arg(&root)
        .arg("diff")
        .arg("--name-only")
        .arg("--cached")
        .arg("--")
        .arg(pathspec)
        .output()
        .map_err(CommandError::RunGit)?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
        if stderr.contains("not a git repository") {
            return Ok(Vec::new());
        }
        return Err(CommandError::GitFailed { stderr });
    }
    let stdout = String::from_utf8(output.stdout).map_err(CommandError::GitOutput)?;
    let paths =
        stdout.lines().filter(|line| !line.trim().is_empty()).map(|line| root.join(line)).collect();
    // sirno:witness:versioning:end
    Ok(paths)
}

fn apply_structural_override_json(
    render: &mut MistRenderSettings, override_json: &str,
) -> Result<(), CommandError> {
    let structural_render = serde_json::from_str(override_json)?;
    *render = MistRenderSettings { structural: structural_render };
    Ok(())
}

fn artifact_path_from_cli(path: &Path) -> Result<EntryArtifactPath, CommandError> {
    Ok(EntryArtifactPath::new(path)?)
}

fn default_artifact_path_from_source(source: &Path) -> Result<EntryArtifactPath, CommandError> {
    let Some(file_name) = source.file_name() else {
        return Err(CommandError::ArtifactSourceHasNoFileName(source.to_path_buf()));
    };
    Ok(EntryArtifactPath::new(Path::new(file_name))?)
}

fn resolve_manifest_argv(
    argv: &[String], artifact_root: &Path, cache: &Path, project_root: &Path, prefer_cache: bool,
) -> Vec<String> {
    let mut resolved = argv.to_vec();
    let Some(first) = resolved.first_mut() else {
        panic!("validated charm commands are non-empty");
    };
    let first_path = Path::new(first);
    if first_path.is_absolute() {
        return resolved;
    }

    let candidates = if prefer_cache {
        [cache.join(first_path), artifact_root.join(first_path), project_root.join(first_path)]
    } else {
        [artifact_root.join(first_path), project_root.join(first_path), cache.join(first_path)]
    };
    if let Some(path) = candidates.into_iter().find(|path| path.exists()) {
        *first = path.display().to_string();
    }
    resolved
}

fn explicit_lake_check_settings(
    config_path: &std::path::Path,
) -> Result<EntryDirectoryCheckSettings, CommandError> {
    if config_path.exists() {
        let config = SirnoConfig::from_file(config_path)?;
        entry_directory_check_settings(config_path, &config)
    } else {
        Ok(EntryDirectoryCheckSettings::default())
    }
}

fn entry_directory_check_settings(
    config_path: &Path, config: &SirnoConfig,
) -> Result<EntryDirectoryCheckSettings, CommandError> {
    Ok(EntryDirectoryCheckSettings {
        render: false,
        structural_render: Default::default(),
        meta_path: Some(meta_file_path_for_config(config_path)),
        ignore: config.lake.ignore.clone(),
        witness: witness_check_settings(config_path, config),
    })
}

fn projection_check_settings(
    config_path: &Path, config: &SirnoConfig, mist: &MistSpec,
) -> EntryDirectoryCheckSettings {
    EntryDirectoryCheckSettings {
        render: config.check.render_enabled(),
        structural_render: mist.render.structural.clone(),
        meta_path: None,
        ignore: projection_ignore_paths(config),
        witness: witness_check_settings(config_path, config),
    }
}

fn meta_file_path_for_config(config_path: &Path) -> PathBuf {
    config_parent(config_path).join(SIRNO_CONTROL_DIR_NAME).join(crate::META_FILE_NAME)
}

fn projection_ignore_paths(config: &SirnoConfig) -> Vec<PathBuf> {
    let mut ignore = config.lake.ignore.clone();
    let control = PathBuf::from(SIRNO_CONTROL_DIR_NAME);
    if !ignore.iter().any(|path| path == &control) {
        ignore.push(control);
    }
    ignore
}

fn witness_check_settings(
    config_path: &Path, config: &SirnoConfig,
) -> Option<WitnessCheckSettings> {
    let repo = config.repo.as_ref()?;
    if repo.members.is_empty() {
        return None;
    }
    Some(WitnessCheckSettings::new(
        config_path.parent().unwrap_or_else(|| Path::new(".")),
        repo.members.clone(),
        config.witness.clone(),
    ))
}

fn resolve_lake_path(
    lake_path: Option<&Path>, config_path: &Path, config: &SirnoConfig,
) -> PathBuf {
    lake_path.map(Path::to_path_buf).unwrap_or_else(|| config.resolve_lake(config_path))
}

pub(crate) fn resolve_lake_directory(
    lake_path: Option<&Path>, config_path: &std::path::Path,
) -> Result<(PathBuf, EntryDirectoryCheckSettings), CommandError> {
    if let Some(lake_path) = lake_path {
        return Ok((lake_path.to_path_buf(), explicit_lake_check_settings(config_path)?));
    }

    let config = SirnoConfig::from_file(config_path)?;
    Ok((config.resolve_lake(config_path), entry_directory_check_settings(config_path, &config)?))
}

pub(crate) fn entry_query_from_filters(
    mut query: EntryQuery, filters: Vec<StructuralFilter>, states: Vec<StructuralStateFilter>,
    structural: &StructuralSettings,
) -> Result<EntryQuery, CommandError> {
    for (field, matchers) in structural_matchers_by_field(filters, states, structural)? {
        for matcher in matchers {
            query = query.with_structural_matcher(field.clone(), matcher);
        }
    }
    Ok(query)
}

fn validate_query_columns(
    columns: QueryColumns, meta: &MetaRegistry, structural: &StructuralSettings,
) -> Result<QueryColumns, CommandError> {
    for field in columns.fields() {
        if !meta.contains_intrinsic_field(field) && !structural.contains_field(field) {
            return Err(CommandError::UndefinedQueryColumn(field.to_owned()));
        }
    }
    Ok(columns)
}

fn query_column_options(meta: &MetaRegistry, structural: &StructuralSettings) -> QueryColumns {
    let mut columns = vec![QueryColumn::Id, QueryColumn::Path];
    columns.extend(
        meta.intrinsic_fields().map(|(field, _)| QueryColumn::Field { field: field.to_owned() }),
    );
    columns.extend(
        structural.fields().map(|(field, _)| QueryColumn::Field { field: field.to_owned() }),
    );
    QueryColumns::new(columns)
}

fn structural_matchers_by_field(
    filters: Vec<StructuralFilter>, states: Vec<StructuralStateFilter>,
    structural: &StructuralSettings,
) -> Result<IndexMap<String, Vec<EntryStructuralMatcher>>, CommandError> {
    let mut matchers_by_field = IndexMap::<String, Vec<EntryStructuralMatcher>>::new();
    for filter in filters {
        if !structural.contains_field(&filter.field) {
            return Err(CommandError::UndefinedStructuralField(filter.field));
        }
        matchers_by_field
            .entry(filter.field)
            .or_default()
            .push(EntryStructuralMatcher::Targets(filter.targets));
    }
    for state in states {
        if !structural.contains_field(&state.field) {
            return Err(CommandError::UndefinedStructuralField(state.field));
        }
        matchers_by_field.entry(state.field).or_default().push(state.state.into());
    }
    Ok(matchers_by_field)
}

fn structural_targets_by_target(
    targets: Vec<StructuralTarget>, structural: &StructuralSettings,
) -> Result<IndexMap<String, Vec<EntryAddress>>, CommandError> {
    let mut targets_by_field = IndexMap::<String, Vec<EntryAddress>>::new();
    for target in targets {
        if !structural.contains_field(&target.field) {
            return Err(CommandError::UndefinedStructuralField(target.field));
        }
        targets_by_field.entry(target.field).or_default().push(target.target);
    }
    Ok(targets_by_field)
}

fn entry_read_metadata(entry: &Entry) -> EntryReadMetadataFields {
    let mut metadata = EntryReadMetadataFields::new();
    for (field, value) in &entry.metadata.intrinsic {
        metadata.insert(field.clone(), EntryReadMetadataValue::Text(value.clone()));
    }
    for (field, targets) in &entry.metadata.structural {
        metadata.insert(field.clone(), EntryReadMetadataValue::Targets(targets.clone()));
    }
    metadata
}

fn intrinsic_fields_from_request(
    requested: EntryIntrinsicFields, meta: &MetaRegistry,
) -> Result<EntryIntrinsicFields, CommandError> {
    for field in requested.keys() {
        if !meta.contains_intrinsic_field(field) {
            return Err(CommandError::UndefinedIntrinsicField(field.clone()));
        }
    }

    let mut intrinsic = EntryIntrinsicFields::new();
    for (field, _) in meta.intrinsic_fields() {
        let value = requested
            .get(field)
            .cloned()
            .ok_or_else(|| EntryParseError::MissingField(field.to_owned()))?;
        intrinsic.insert(field.to_owned(), value);
    }
    Ok(intrinsic)
}

fn plural<'a>(count: usize, singular: &'a str, plural: &'a str) -> &'a str {
    if count == 1 { singular } else { plural }
}

fn tide_selection_matches(request: &TideResolveRequest, status: &TideStatus) -> bool {
    request.neighbors.iter().any(|id| &status.workitem.neighbor == id)
        || request.workitems.iter().any(|workitem| &status.workitem == workitem)
}

fn tide_selection_request_matches(request: &TideSelectionRequest, status: &TideStatus) -> bool {
    request.neighbors.iter().any(|id| &status.workitem.neighbor == id)
        || request.workitems.iter().any(|workitem| &status.workitem == workitem)
}

fn tide_statuses_for_output(tide: &Tide, all: bool) -> Vec<TideStatus> {
    tide.statuses().iter().filter(|status| all || !status.resolved).cloned().collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::surface::QueryValue;

    fn initialized_context() -> (tempfile::TempDir, SurfaceContext) {
        let temp = tempfile::tempdir().unwrap();
        let config_path = temp.path().join(CONFIG_FILE_NAME);
        let context = SurfaceContext::new(&config_path);
        context.lake_init(LakeInitRequest { lake: Some(temp.path().join("lake")) }).unwrap();
        (temp, context)
    }

    fn create_alpha_entry(context: &SurfaceContext) {
        context
            .entry_new(EntryNewRequest {
                id: EntryAddress::new("alpha").unwrap(),
                intrinsic: intrinsic_fields([("name", "Alpha"), ("desc", "Alpha entry.")]),
                structural: Vec::new(),
                body: Some("Body.\n".to_owned()),
            })
            .unwrap();
    }

    fn query_request(columns: QueryColumnSelection) -> QueryRequest {
        QueryRequest { terms: vec!["alpha".to_owned()], columns, ..QueryRequest::default() }
    }

    fn create_charm_entry(
        temp: &tempfile::TempDir, context: &SurfaceContext, id: &EntryAddress, manifest: &str,
        artifacts: &[(&str, &str)],
    ) {
        context
            .entry_new(EntryNewRequest {
                id: id.clone(),
                intrinsic: intrinsic_fields([("name", id.as_str()), ("desc", "test charm")]),
                structural: Vec::new(),
                body: None,
            })
            .unwrap();

        let manifest_source = temp.path().join(format!("{id}.manifest.toml"));
        fs::write(&manifest_source, manifest).unwrap();
        context
            .entry_artifact_add(ArtifactAddRequest {
                id: id.clone(),
                source: manifest_source,
                artifact_path: Some(PathBuf::from(CHARM_MANIFEST_FILE_NAME)),
            })
            .unwrap();

        for (path, content) in artifacts {
            let source = temp.path().join(format!("{}.{}", id, path.replace('/', ".")));
            fs::write(&source, content).unwrap();
            context
                .entry_artifact_add(ArtifactAddRequest {
                    id: id.clone(),
                    source,
                    artifact_path: Some(PathBuf::from(path)),
                })
                .unwrap();
        }
    }

    fn intrinsic_fields<'a>(
        fields: impl IntoIterator<Item = (&'a str, &'a str)>,
    ) -> EntryIntrinsicFields {
        fields.into_iter().map(|(field, value)| (field.to_owned(), value.to_owned())).collect()
    }

    // sirno:witness:agent-skills:begin
    #[test]
    fn skill_wrapper_installation_preserves_roster_and_excludes_resource_only_skills() {
        let temp = tempfile::tempdir().unwrap();
        let context = SurfaceContext::new(temp.path().join(CONFIG_FILE_NAME));

        let installed = context.skill_wrappers_init().unwrap();

        assert_eq!(
            installed.records.iter().map(|record| record.name.as_str()).collect::<Vec<_>>(),
            [
                "sirno-editor",
                "sirno-actualizer",
                "sirno-internalizer",
                "sirno-narrative-session",
                "sirno-skill-synthesizer",
                "sirno-curator",
                "sirno-finalizer",
            ]
        );
        for record in &installed.records {
            assert_eq!(record.status, "wrote");
            assert!(record.changed);
            assert!(!record.wrapper_path.contains('\\'));
            assert!(!record.full_path.contains('\\'));
            assert!(!record.target_path.contains('\\'));
            let artifact = Path::new(env!("CARGO_MANIFEST_DIR")).join(&record.wrapper_path);
            assert_eq!(
                fs::read(temp.path().join(&record.target_path)).unwrap(),
                fs::read(artifact).unwrap()
            );
        }
        assert!(!temp.path().join(".agents/skills/design-doc-writer").exists());
        assert!(!temp.path().join(".claude").exists());
        assert!(context.skill_wrappers_check().unwrap().ok);
        let repeated = context.skill_wrappers_init().unwrap();
        assert!(
            repeated.records.iter().all(|record| record.status == "unchanged" && !record.changed)
        );
    }

    #[test]
    fn skill_wrapper_installation_repairs_drift_and_missing_packages() {
        let temp = tempfile::tempdir().unwrap();
        let context = SurfaceContext::new(temp.path().join(CONFIG_FILE_NAME));
        let installed = context.skill_wrappers_init().unwrap();
        let editor = temp.path().join(".agents/skills/sirno-editor/SKILL.md");
        let expected_editor = fs::read(&editor).unwrap();
        fs::write(&editor, "drifted wrapper").unwrap();
        fs::remove_file(temp.path().join(".agents/skills/sirno-actualizer/SKILL.md")).unwrap();

        let checked = context.skill_wrappers_check().unwrap();
        assert!(!checked.ok);
        assert_eq!(checked.records[0].status, "drifted");
        assert_eq!(checked.records[1].status, "missing");

        let repaired = context.skill_wrappers_init().unwrap();
        assert!(repaired.ok);
        assert_eq!(
            repaired
                .records
                .iter()
                .filter(|record| record.changed)
                .map(|record| record.name.as_str())
                .collect::<Vec<_>>(),
            ["sirno-editor", "sirno-actualizer"]
        );
        assert_eq!(fs::read(editor).unwrap(), expected_editor);
        assert!(context.skill_wrappers_check().unwrap().ok);
        assert_eq!(repaired.records.len(), installed.records.len());
    }

    #[cfg(unix)]
    #[test]
    fn skill_wrapper_claude_links_follow_installed_packages_and_repair_drift() {
        let temp = tempfile::tempdir().unwrap();
        let context = SurfaceContext::new(temp.path().join(CONFIG_FILE_NAME));
        let installed = context.skill_wrappers_init_with_claude(true).unwrap();
        assert_eq!(installed.records.len(), 14);
        for record in &installed.records[7..] {
            let link = temp.path().join(&record.target_path);
            assert_eq!(
                fs::read_link(&link).unwrap(),
                Path::new("../../.agents/skills").join(&record.name)
            );
            assert_eq!(
                fs::read(link.join("SKILL.md")).unwrap(),
                fs::read(temp.path().join(&record.wrapper_path).join("SKILL.md")).unwrap()
            );
        }
        assert!(!temp.path().join(".claude/skills/design-doc-writer").exists());
        assert!(context.skill_wrappers_check_with_claude(true).unwrap().ok);

        let editor_link = temp.path().join(".claude/skills/sirno-editor");
        fs::remove_file(&editor_link).unwrap();
        std::os::unix::fs::symlink("wrong-target", &editor_link).unwrap();
        assert!(!context.skill_wrappers_check_with_claude(true).unwrap().ok);
        let repaired = context.skill_wrappers_init_with_claude(true).unwrap();
        assert_eq!(repaired.records.iter().filter(|record| record.changed).count(), 1);
        assert!(context.skill_wrappers_check_with_claude(true).unwrap().ok);
    }
    // sirno:witness:agent-skills:end

    #[test]
    fn entry_new_requires_discovered_intrinsic_fields() {
        let (_temp, context) = initialized_context();

        let error = context
            .entry_new(EntryNewRequest {
                id: EntryAddress::new("alpha").unwrap(),
                intrinsic: intrinsic_fields([("desc", "Alpha entry.")]),
                structural: Vec::new(),
                body: None,
            })
            .unwrap_err();

        assert!(matches!(
            error,
            CommandError::EntryParse(EntryParseError::MissingField(field)) if field == "name"
        ));
    }

    #[test]
    fn entry_new_rejects_undiscovered_intrinsic_fields() {
        let (_temp, context) = initialized_context();

        let error = context
            .entry_new(EntryNewRequest {
                id: EntryAddress::new("alpha").unwrap(),
                intrinsic: intrinsic_fields([
                    ("name", "Alpha"),
                    ("desc", "Alpha entry."),
                    ("title", "Alpha"),
                ]),
                structural: Vec::new(),
                body: None,
            })
            .unwrap_err();

        assert!(matches!(error, CommandError::UndefinedIntrinsicField(field) if field == "title"));
    }

    #[test]
    fn query_defaults_to_id_and_path_columns() {
        let (_temp, context) = initialized_context();
        create_alpha_entry(&context);

        let response = context.entry_query(query_request(QueryColumnSelection::Default)).unwrap();

        assert_eq!(response.columns, vec!["id".to_owned(), "path".to_owned()]);
        assert_eq!(response.records.len(), 1);
        assert_eq!(
            response.records[0].get("id"),
            Some(&QueryValue::Text(Some("alpha".to_owned())))
        );
        assert!(
            matches!(response.records[0].get("path"), Some(QueryValue::Text(Some(path))) if path.ends_with("alpha.md"))
        );
    }

    #[test]
    fn query_column_options_include_discovered_intrinsic_fields() {
        let (_temp, context) = initialized_context();

        let response = context
            .entry_query(QueryRequest {
                columns: QueryColumnSelection::Options,
                ..QueryRequest::default()
            })
            .unwrap();

        assert!(response.columns.starts_with(&["id".to_owned(), "path".to_owned()]));
        assert!(response.columns.contains(&"desc".to_owned()));
        assert!(response.columns.contains(&"name".to_owned()));
    }

    #[test]
    fn query_selected_intrinsic_field_renders_as_scalar_value() {
        let (_temp, context) = initialized_context();
        create_alpha_entry(&context);

        let response = context
            .entry_query(query_request(QueryColumnSelection::Selected(QueryColumns::new(vec![
                QueryColumn::Id,
                QueryColumn::Field { field: "desc".to_owned() },
            ]))))
            .unwrap();

        assert_eq!(response.columns, vec!["id".to_owned(), "desc".to_owned()]);
        assert_eq!(
            response.records[0].get("desc"),
            Some(&QueryValue::Text(Some("Alpha entry.".to_owned())))
        );
    }

    #[test]
    fn query_rejects_unknown_column_fields() {
        let (_temp, context) = initialized_context();
        create_alpha_entry(&context);

        let error = context
            .query_entries(query_request(QueryColumnSelection::Selected(QueryColumns::new(vec![
                QueryColumn::Field { field: "unknown".to_owned() },
            ]))))
            .unwrap_err();

        assert!(matches!(error, CommandError::UndefinedQueryColumn(field) if field == "unknown"));
    }

    #[test]
    fn status_carries_check_diagnostics_outside_full_mode() {
        use crate::surface::dto::StatusMode;

        let (temp, context) = initialized_context();
        fs::write(temp.path().join("lake").join("bad.md"), "no frontmatter\n").unwrap();

        let result =
            context.status(StatusRequest::new(StatusMode::Normal, CheckMode::Review)).unwrap();

        assert!(!result.ok);
        let check = result.check.as_ref().expect("a failed status carries its check result");
        assert!(!check.ok);
        assert!(!check.diagnostics.is_empty());
    }

    #[test]
    fn status_rendering_mutes_diagnostics_only_when_quiet() {
        use crate::surface::dto::StatusMode;
        use crate::surface::output::format_status_result;

        let (temp, context) = initialized_context();
        fs::write(temp.path().join("lake").join("bad.md"), "no frontmatter\n").unwrap();

        let result =
            context.status(StatusRequest::new(StatusMode::Normal, CheckMode::Review)).unwrap();
        let message = result.check.as_ref().unwrap().diagnostics[0].message.clone();

        let loud = format_status_result(&result, false);
        assert!(loud.contains("lake check"));
        assert!(loud.contains(&message));

        let quiet = format_status_result(&result, true);
        assert!(quiet.contains("lake check"));
        assert!(!quiet.contains(&message));
        assert!(quiet.contains(&result.message));
        assert!(quiet.contains("next: inspect diagnostics"));
    }

    // sirno:witness:project-status-commands:begin
    #[test]
    fn status_explains_missing_and_changed_baselines_at_every_detail_level() {
        use crate::surface::dto::StatusMode;
        use crate::surface::output::format_status_result;

        let (_temp, context) = initialized_context();
        for show in [StatusMode::Summary, StatusMode::Normal, StatusMode::Full] {
            let result = context.status(StatusRequest::new(show, CheckMode::Review)).unwrap();
            assert!(!result.ok);
            for quiet in [false, true] {
                let rendered = format_status_result(&result, quiet);
                assert!(rendered.contains("status has blockers: anchor missing"), "{rendered}");
                assert!(rendered.contains("`sirno anchor update`"), "{rendered}");
            }
        }

        context.mist_render(None, false).unwrap();
        context.anchor_update().unwrap();
        let clean = context.status(StatusRequest::summary()).unwrap();
        assert!(clean.ok);
        let rendered = format_status_result(&clean, false);
        assert!(rendered.contains("status clean:"), "{rendered}");
        assert!(!rendered.contains("next:"), "{rendered}");

        create_alpha_entry(&context);
        context.mist_render(None, false).unwrap();
        let changed = context.status(StatusRequest::summary()).unwrap();
        assert!(!changed.ok);
        let rendered = format_status_result(&changed, true);
        assert!(rendered.contains("1 anchor ripple"), "{rendered}");
        assert!(rendered.contains("`sirno anchor update`"), "{rendered}");
    }

    #[test]
    fn summary_status_directs_pending_review_to_tide_before_anchor() {
        use crate::surface::output::format_status_result;

        let (temp, context) = initialized_context();
        fs::write(
            temp.path().join("lake/depends.md"),
            "---\nname: Depends\ndesc: Review dependencies.\nmeta.type: structural\nmeta.ripple.lake: [from]\n---\n\nReview dependents when their dependency changes.\n",
        )
        .unwrap();
        create_alpha_entry(&context);
        context
            .entry_new(EntryNewRequest {
                id: EntryAddress::new("beta").unwrap(),
                intrinsic: intrinsic_fields([("name", "Beta"), ("desc", "Uses alpha.")]),
                structural: vec![StructuralTarget {
                    field: "depends".to_owned(),
                    target: EntryAddress::new("alpha").unwrap(),
                }],
                body: None,
            })
            .unwrap();
        context.mist_render(None, false).unwrap();
        context.anchor_update().unwrap();
        let alpha = temp.path().join("lake/alpha.md");
        let source = fs::read_to_string(&alpha).unwrap();
        fs::write(alpha, format!("{source}\nChanged design.\n")).unwrap();
        context.mist_render(None, false).unwrap();

        let result = context.status(StatusRequest::summary()).unwrap();
        assert!(!result.ok);
        assert_eq!(result.blockers.tide_open_workitems, Some(1));
        assert!(result.tide.is_none());
        let rendered = format_status_result(&result, true);
        assert!(rendered.contains("1 tide workitem"), "{rendered}");
        assert!(rendered.contains("1 anchor ripple"), "{rendered}");
        assert!(rendered.contains("`sirno tide status --show full`"), "{rendered}");
        assert!(!rendered.contains("`sirno anchor update`"), "{rendered}");
    }

    #[test]
    fn summary_status_explains_projection_changes() {
        use crate::surface::output::format_status_result;

        let (temp, context) = initialized_context();
        context.mist_render(None, false).unwrap();
        context.anchor_update().unwrap();
        let projected = temp.path().join(crate::DEFAULT_MIST_PROJECTION_PATH).join("name.md");
        let source = fs::read_to_string(&projected).unwrap();
        fs::write(projected, format!("{source}\nChanged design.\n")).unwrap();

        let result = context.status(StatusRequest::summary()).unwrap();
        assert!(!result.ok);
        assert_eq!(result.blockers.mist_changed_entries, Some(1));
        assert!(result.mist.is_none());
        let rendered = format_status_result(&result, true);
        assert!(rendered.contains("1 mist blocker"), "{rendered}");
        assert!(rendered.contains("`sirno mist status`"), "{rendered}");
        assert!(!rendered.contains("`sirno anchor update`"), "{rendered}");
    }
    // sirno:witness:project-status-commands:end

    #[test]
    fn mist_render_status_and_intake_round_trip_entry_edits() {
        let (temp, context) = initialized_context();

        let render = context.mist_render(None, false).unwrap();
        assert!(render.ok);

        let projected = temp.path().join(crate::DEFAULT_MIST_PROJECTION_PATH).join("name.md");
        let source = fs::read_to_string(&projected).unwrap();
        fs::write(&projected, source.replace("required `name`", "accepted `name`")).unwrap();

        let status = context.mist_status(None).unwrap();
        assert!(!status.ok);
        assert_eq!(status.changed_entries, vec!["name"]);

        let intake = context.mist_intake(None).unwrap();
        assert_eq!(intake.updated_entries, vec!["name"]);

        let reservoir = temp.path().join("lake").join("name.md");
        assert!(fs::read_to_string(reservoir).unwrap().contains("accepted `name`"));
        assert!(context.mist_status(None).unwrap().ok);
    }

    #[test]
    fn mist_render_uses_reservoir_context_for_narrow_projection() {
        let (temp, context) = initialized_context();
        let mist_dir = temp.path().join(SIRNO_CONTROL_DIR_NAME).join(crate::MIST_SPEC_DIR_NAME);
        fs::create_dir_all(&mist_dir).unwrap();
        fs::write(
            mist_dir.join("default.toml"),
            r#"
[projection]
path = "narrow"
editable = true

[select]
exact_terms = ["compresses"]
"#,
        )
        .unwrap();
        let artifact_source = temp.path().join("concept.note.md");
        fs::write(&artifact_source, "artifact").unwrap();
        context
            .entry_artifact_add(ArtifactAddRequest {
                id: EntryAddress::new("concept").unwrap(),
                source: artifact_source,
                artifact_path: Some(PathBuf::from("note.md")),
            })
            .unwrap();

        let render = context.mist_render(None, false).unwrap();
        assert!(render.ok);
        assert!(render.changed_paths.len() > 1);
        assert!(render.message.contains("Total changes: 1/1"), "{}", render.message);

        let projection = temp.path().join("narrow");
        assert!(projection.join("concept.md").exists());
        assert!(projection.join(".artifacts/concept/note.md").exists());
        assert!(!projection.join("name.md").exists());
        assert!(
            fs::read_to_string(projection.join("concept.md"))
                .unwrap()
                .contains(crate::render::BEGIN_LINKS_GUARD)
        );
        assert!(context.mist_status(None).unwrap().ok);

        let delete = context.mist_render_delete(None).unwrap();
        assert!(delete.ok);
        assert!(
            !fs::read_to_string(projection.join("concept.md"))
                .unwrap()
                .contains(crate::render::BEGIN_LINKS_GUARD)
        );
    }

    #[cfg(unix)]
    #[test]
    fn direct_charm_requires_enablement_and_runs_spell() {
        let (temp, context) = initialized_context();
        let id = EntryAddress::new("direct-charm").unwrap();
        create_charm_entry(
            &temp,
            &context,
            &id,
            r#"
[spell]
command = ["sh", "hello.sh"]
"#,
            &[("hello.sh", r#"printf 'spell:%s' "$SIRNO_CHARM""#)],
        );

        let error = context.spell_run(id.clone()).unwrap_err();
        assert!(matches!(error, CommandError::CharmNotEnabled(blocked) if blocked == id));

        assert!(!context.charm_list().unwrap().charms[0].enabled);
        assert!(context.charm_enable(id.clone()).unwrap().changed);
        assert_eq!(context.spell_list().unwrap().spells.len(), 1);

        let result = context.spell_run(id).unwrap();
        assert!(result.ok);
        assert_eq!(result.phase, "spell");
        assert_eq!(result.stdout, "spell:direct-charm");
    }

    #[cfg(unix)]
    #[test]
    fn source_charm_builds_cache_and_runs_spell() {
        let (temp, context) = initialized_context();
        let id = EntryAddress::new("source-charm").unwrap();
        create_charm_entry(
            &temp,
            &context,
            &id,
            r#"
[spell]
command = ["built-spell"]

[charm.build]
command = ["sh", "build.sh"]
output = "built-spell"
"#,
            &[(
                "build.sh",
                r#"cat > "$SIRNO_SPELL_DIR/built-spell" <<'SCRIPT'
#!/bin/sh
printf 'built:%s' "$SIRNO_CHARM"
SCRIPT
chmod +x "$SIRNO_SPELL_DIR/built-spell"
"#,
            )],
        );

        context.charm_enable(id.clone()).unwrap();
        let result = context.spell_run(id.clone()).unwrap();
        assert!(result.ok);
        assert_eq!(result.stdout, "built:source-charm");

        let show = context.charm_show(id).unwrap();
        assert!(PathBuf::from(show.spell_cache_path).join("built-spell").exists());
    }
}
