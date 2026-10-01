//! Human and JSON rendering helpers for command results.

use std::env;
use std::error::Error;
use std::path::{Path, PathBuf};

use anstyle::{AnsiColor, Style};
use comfy_table::{Cell, Color as TableColor, ContentArrangement, Table, presets::UTF8_FULL};
use indexmap::IndexMap;
use serde::Serialize;
use unicode_width::UnicodeWidthStr;

use crate::surface::dto::{
    AnchorRippleKind, AnchorRippleRecord, AnchorUpdateResult, ConfigCommentResult,
    DiagnosticRecord, MistIntakeResult, MistStatusResult, PathRecord, QueryColumn, QueryColumns,
    QueryOutputFormat, QueryResults, QueryValue, RenderResult, SkillWrapperRecord, StatusResult,
};
use crate::surface::error::CommandError;
use crate::{
    CheckMode, Entry, EntryDirectoryError, EntryDirectoryReport, EntryIntrinsicFields,
    UpstreamCrystallizeReport, UpstreamStatusReport, UpstreamStatusState, WitnessRecord,
};

/// Render any serializable value as pretty JSON.
pub fn format_json<T: Serialize + ?Sized>(value: &T) -> Result<String, CommandError> {
    Ok(serde_json::to_string_pretty(value)?)
}

pub(crate) fn print_json<T: Serialize + ?Sized>(value: &T) -> Result<(), CommandError> {
    println!("{}", format_json(value)?);
    Ok(())
}

// sirno:witness:cli-interface:begin
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum OutputStyle {
    Plain,
    Styled,
    #[cfg(test)]
    Forced,
}

impl OutputStyle {
    fn colors(self) -> bool {
        self != Self::Plain
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SemanticStyle {
    Changed,
    Error,
    Muted,
    Success,
    Warning,
}

impl SemanticStyle {
    fn text_style(self) -> Style {
        let color = match self {
            | Self::Changed => AnsiColor::Cyan,
            | Self::Error => AnsiColor::Red,
            | Self::Muted => AnsiColor::BrightBlack,
            | Self::Success => AnsiColor::Green,
            | Self::Warning => AnsiColor::Yellow,
        };
        Style::new().fg_color(Some(color.into()))
    }

    fn table_color(self) -> TableColor {
        match self {
            | Self::Changed => TableColor::Cyan,
            | Self::Error => TableColor::Red,
            | Self::Muted => TableColor::DarkGrey,
            | Self::Success => TableColor::Green,
            | Self::Warning => TableColor::Yellow,
        }
    }
}

pub(crate) fn print_cli_error(error: &CommandError) {
    anstream::eprint!("{}", format_cli_error(error, OutputStyle::Styled));
}

pub(crate) fn format_command_error(error: &CommandError) -> String {
    let mut output = format!("{error}\n");
    append_error_sources(&mut output, error);
    append_error_help(&mut output, error);
    output.trim_end().to_owned()
}

fn format_cli_error(error: &CommandError, style: OutputStyle) -> String {
    let mut output = format!("{} {error}\n", style_text("sirno:", SemanticStyle::Error, style));
    append_error_sources(&mut output, error);
    append_error_help(&mut output, error);
    output
}

fn append_error_sources(output: &mut String, error: &CommandError) {
    let mut rendered = error.to_string();
    let mut source = error.source();
    while let Some(error) = source {
        let source_text = error.to_string();
        if !source_text.is_empty() && !rendered.contains(&source_text) {
            output.push_str(&format!("  caused by: {source_text}\n"));
            rendered.push_str(&source_text);
        }
        source = error.source();
    }
}

fn append_error_help(output: &mut String, error: &CommandError) {
    if let Some(help) = error.help() {
        output.push_str(&format!("  help: {help}\n"));
    }
}

pub(crate) fn format_success_text(value: &str, style: OutputStyle) -> String {
    style_text(value, SemanticStyle::Success, style)
}

pub(crate) fn format_muted_text(value: &str, style: OutputStyle) -> String {
    style_text(value, SemanticStyle::Muted, style)
}

pub(crate) fn format_intrinsic_summary(fields: &EntryIntrinsicFields) -> String {
    if fields.is_empty() {
        return "-".to_owned();
    }
    fields.iter().map(|(field, value)| format!("{field}={value}")).collect::<Vec<_>>().join("; ")
}

pub(crate) fn format_warning_text(value: &str, style: OutputStyle) -> String {
    style_text(value, SemanticStyle::Warning, style)
}
// sirno:witness:cli-interface:end

pub(crate) fn print_witness_records(records: &[WitnessRecord], full: bool) {
    print!("{}", format_witness_records(records, full));
}

pub(crate) fn format_witness_records(records: &[WitnessRecord], full: bool) -> String {
    let mut out = String::new();
    for (index, record) in records.iter().enumerate() {
        if full && index > 0 {
            out.push_str("---\n\n");
        }
        out.push_str(&format_witness_record(record, full));
    }
    out
}

pub(crate) fn format_witness_record(record: &WitnessRecord, full: bool) -> String {
    let range = format_witness_summary(record);
    if !full {
        let marker =
            record.body.lines().next().map(str::to_owned).unwrap_or_else(|| record.marker.clone());
        return format!("{range}\t{marker}\n");
    }

    let mut out = format!("{range}\n\n");
    out.push_str(&record.body);
    if !record.body.ends_with('\n') {
        out.push('\n');
    }
    out.push('\n');
    out
}

fn format_witness_summary(record: &WitnessRecord) -> String {
    format!(
        "{}:{}:{}-{} :: {}:{}-{}",
        record.path.display(),
        record.opening.start_line,
        record.opening.start_column,
        record.opening.end_column,
        record.closing.start_line,
        record.closing.start_column,
        record.closing.end_column
    )
}

pub(crate) fn display_path(path: &Path) -> String {
    path.display().to_string()
}

pub(crate) fn display_paths(paths: &[PathBuf]) -> Vec<String> {
    paths.iter().map(|path| display_path(path)).collect()
}

// sirno:witness:diagnostics:begin
pub(crate) fn diagnostics_from_entry_report(
    report: &EntryDirectoryReport,
) -> Vec<DiagnosticRecord> {
    let mut diagnostics = Vec::new();
    for diagnostic in report.file_diagnostics() {
        diagnostics.push(DiagnosticRecord {
            severity: diagnostic.severity.label().to_owned(),
            code: diagnostic.code.to_owned(),
            path: Some(display_path(&diagnostic.path)),
            line: diagnostic.line,
            column: diagnostic.column,
            entry: None,
            field: None,
            target: None,
            message: diagnostic.message.clone(),
            help: diagnostic.help.clone(),
        });
    }
    for diagnostic in report.structural_report().diagnostics() {
        diagnostics.push(DiagnosticRecord {
            severity: diagnostic.severity.label().to_owned(),
            code: diagnostic.code().to_owned(),
            path: diagnostic
                .entry
                .as_ref()
                .and_then(|entry| report.entry_file_path(entry))
                .map(display_path),
            line: None,
            column: None,
            entry: diagnostic.entry.as_ref().map(ToString::to_string),
            field: Some(diagnostic.field.clone()),
            target: diagnostic.target.as_ref().map(ToString::to_string),
            message: diagnostic.message(),
            help: diagnostic.help(),
        });
    }
    diagnostics
}
// sirno:witness:diagnostics:end

pub(crate) fn print_status_result(result: &StatusResult, quiet: bool) {
    anstream::print!("{}", format_status_result_with_style(result, quiet, OutputStyle::Styled));
}

pub(crate) fn print_upstream_crystallize_report(result: &UpstreamCrystallizeReport) {
    anstream::println!("{}", result.message);
    for path in &result.changed_paths {
        anstream::println!("{path}");
    }
}

pub(crate) fn print_upstream_status_report(result: &UpstreamStatusReport) {
    anstream::print!("{}", format_upstream_status_report_with_style(result, OutputStyle::Styled));
}

pub(crate) fn print_anchor_update_result(result: &AnchorUpdateResult) {
    anstream::println!("{}", result.message);
}

fn format_anchor_ripple_table(ripples: &[AnchorRippleRecord]) -> String {
    let rows = ripples
        .iter()
        .map(|record| vec![record.id.to_string(), anchor_ripple_kind_label(record.kind).to_owned()])
        .collect::<Vec<_>>();
    format_human_table_with_width_and_style(
        vec!["entry".to_owned(), "ripple".to_owned()],
        rows,
        None,
        OutputStyle::Plain,
    )
}

fn anchor_ripple_kind_label(kind: AnchorRippleKind) -> &'static str {
    match kind {
        | AnchorRippleKind::Added => "added",
        | AnchorRippleKind::Changed => "changed",
        | AnchorRippleKind::Deleted => "deleted",
    }
}

fn format_upstream_status_report_with_style(
    result: &UpstreamStatusReport, style: OutputStyle,
) -> String {
    if result.upstreams.is_empty() {
        return format!("{}\n", result.message);
    }

    let rows = result
        .upstreams
        .iter()
        .map(|upstream| {
            vec![
                upstream.domain.clone(),
                upstream_status_state_label(upstream.state).to_owned(),
                upstream.commit.clone().unwrap_or_default(),
                upstream.git.clone(),
            ]
        })
        .collect::<Vec<_>>();
    let mut output = format_human_table_with_width_and_style(
        vec!["domain".to_owned(), "state".to_owned(), "commit".to_owned(), "git".to_owned()],
        rows,
        None,
        style,
    );
    output.push_str(&result.message);
    output.push('\n');
    output
}

fn upstream_status_state_label(state: UpstreamStatusState) -> &'static str {
    match state {
        | UpstreamStatusState::Ok => "ok",
        | UpstreamStatusState::MissingLock => "missing-lock",
        | UpstreamStatusState::StaleLock => "stale-lock",
        | UpstreamStatusState::MissingCache => "missing-cache",
        | UpstreamStatusState::MissingGlacier => "missing-glacier",
        | UpstreamStatusState::GlacierDrift => "glacier-drift",
    }
}

#[cfg(test)]
pub(crate) fn format_status_result(result: &StatusResult, quiet: bool) -> String {
    format_status_result_with_style(result, quiet, OutputStyle::Plain)
}

// sirno:witness:project-status-commands:begin
fn format_status_result_with_style(
    result: &StatusResult, quiet: bool, style: OutputStyle,
) -> String {
    let mut output = String::new();
    output.push_str(&format!("config: {}\n", result.config_path));
    output.push_str(&format!("lake: {} ({} entries)\n", result.lake_path, result.entry_count));
    let structural_field_count = if result.structural_fields.is_empty() {
        result.structural_field_count
    } else {
        result.structural_fields.len()
    };
    output.push_str(&format!(
        "structure: {} discovered {}\n",
        structural_field_count,
        plural(structural_field_count, "relation", "relations")
    ));
    let render = result
        .check_policy
        .as_ref()
        .map(|policy| if policy.render { "render links checked" } else { "render links skipped" });
    let mode = result.check_policy.as_ref().map(|policy| check_mode_label(policy.mode));
    if !quiet && let Some(check) = result.check.as_ref().filter(|check| !check.ok) {
        output.push_str(&format_diagnostics_with_style(&check.diagnostics, style));
    }
    let check_ok =
        result.check.as_ref().map_or(result.blockers.check_errors == 0, |check| check.ok);
    if check_ok {
        if let (Some(mode), Some(render)) = (mode, render) {
            output.push_str(&format!(
                "lake check: {} ({mode}; {render})\n",
                style_text("ok", SemanticStyle::Success, style)
            ));
        } else {
            output.push_str(&format!(
                "lake check: {}\n",
                style_text("ok", SemanticStyle::Success, style)
            ));
        }
    } else if let (Some(mode), Some(render)) = (mode, render) {
        output.push_str(&format!(
            "lake check: {} ({mode}; {render})\n",
            style_text("failed", SemanticStyle::Error, style)
        ));
    } else {
        output.push_str(&format!(
            "lake check: {}\n",
            style_text("failed", SemanticStyle::Error, style)
        ));
    }
    if let Some(tide) = &result.tide {
        if tide.clear {
            output.push_str(&format!(
                "tide: {}\n",
                style_text("clear", SemanticStyle::Success, style)
            ));
        } else {
            output.push_str(&format!(
                "tide: {} open {} in {} {}, {} review {}\n",
                tide.open_workitems,
                plural(tide.open_workitems, "workitem", "workitems"),
                tide.open_waves,
                plural(tide.open_waves, "wave", "waves"),
                tide.review_entries,
                plural(tide.review_entries, "entry", "entries")
            ));
        }
    }
    if let Some(anchor) = &result.anchor {
        if anchor.ok {
            output.push_str(&format!(
                "anchor: {} ({})\n",
                style_text("current", SemanticStyle::Success, style),
                anchor.anchor_path
            ));
        } else if !anchor.initialized {
            output.push_str(&format!(
                "anchor: {} ({})\n",
                style_text("missing", SemanticStyle::Warning, style),
                anchor.anchor_path
            ));
        } else {
            output.push_str(&format!(
                "anchor: {} {} ({})\n",
                anchor.ripple_count,
                plural(anchor.ripple_count, "ripple", "ripples"),
                anchor.anchor_path
            ));
        }
        if !anchor.ripples.is_empty() {
            output.push_str(&format_anchor_ripple_table(&anchor.ripples));
        }
    }
    if let Some(mist) = &result.mist {
        if mist.ok {
            output.push_str(&format!(
                "mist: {} ({})\n",
                style_text("clean", SemanticStyle::Success, style),
                mist.projection_path
            ));
        } else {
            output.push_str(&format!(
                "mist: {} ({})\n",
                style_text("pending", SemanticStyle::Warning, style),
                mist.message
            ));
        }
    }
    let summary_style = if result.ok { SemanticStyle::Success } else { SemanticStyle::Warning };
    output.push_str(&format!("{}\n", style_text(&result.message, summary_style, style)));
    if !result.ok {
        let next = if result.blockers.check_errors > 0 {
            "inspect diagnostics with `sirno status --show full`"
        } else if result.blockers.mist_blocker_count() > 0 {
            "inspect projection changes with `sirno mist status`"
        } else if result.blockers.tide_open_workitems.unwrap_or(0) > 0 {
            "review affected entries with `sirno tide status --show full`"
        } else if result.blockers.has_anchor_blocker() {
            "review the lake, run `sirno mist render`, then accept it with `sirno anchor update`"
        } else {
            "inspect project details with `sirno status --show full`"
        };
        output.push_str(&format!("next: {next}\n"));
    }
    output
}
// sirno:witness:project-status-commands:end

fn check_mode_label(mode: CheckMode) -> &'static str {
    match mode {
        | CheckMode::Edit => "edit",
        | CheckMode::Review => "review",
    }
}

fn plural<'a>(count: usize, singular: &'a str, plural: &'a str) -> &'a str {
    if count == 1 { singular } else { plural }
}

pub(crate) fn print_render_result(result: &RenderResult) {
    anstream::print!("{}", format_render_result_with_style(result, OutputStyle::Styled));
}

pub(crate) fn print_mist_status_result(result: &MistStatusResult) {
    anstream::print!("{}", format_mist_status_result_with_style(result, OutputStyle::Styled));
}

pub(crate) fn print_mist_intake_result(result: &MistIntakeResult) {
    anstream::println!("{}", result.message);
    for path in &result.changed_paths {
        anstream::println!("{path}");
    }
}

#[cfg(test)]
#[allow(dead_code)]
pub(crate) fn format_render_result(result: &RenderResult) -> String {
    format_render_result_with_style(result, OutputStyle::Plain)
}

fn format_render_result_with_style(result: &RenderResult, style: OutputStyle) -> String {
    if result.diagnostics.is_empty() {
        return format!("{}\n", result.message);
    }

    let mut output = format_diagnostics_with_style(&result.diagnostics, style);
    output.push_str(&result.message);
    output.push('\n');
    output
}

fn format_mist_status_result_with_style(result: &MistStatusResult, style: OutputStyle) -> String {
    let mut output = String::new();
    output.push_str(&format!("mist: {}\n", result.mist));
    output.push_str(&format!("reservoir: {}\n", result.reservoir_path));
    output.push_str(&format!("projection: {}\n", result.projection_path));
    append_mist_status_rows(&mut output, "changed", &result.changed_entries, style);
    append_mist_status_rows(&mut output, "stale", &result.stale_entries, style);
    append_mist_status_rows(&mut output, "missing", &result.missing_entries, style);
    append_mist_status_rows(&mut output, "staged", &result.staged_paths, style);
    output.push_str(&result.message);
    output.push('\n');
    output
}

fn append_mist_status_rows(
    output: &mut String, label: &str, values: &[String], style: OutputStyle,
) {
    if values.is_empty() {
        return;
    }
    let label = style_text(label, SemanticStyle::Warning, style);
    for value in values {
        output.push_str(&format!("{label}: {value}\n"));
    }
}

pub(crate) fn print_config_comment_result(result: &ConfigCommentResult) {
    anstream::print!("{}", format_config_comment_result_with_style(result, OutputStyle::Styled));
}

#[cfg(test)]
#[allow(dead_code)]
pub(crate) fn format_config_comment_result(result: &ConfigCommentResult) -> String {
    format_config_comment_result_with_style(result, OutputStyle::Plain)
}

fn format_config_comment_result_with_style(
    result: &ConfigCommentResult, style: OutputStyle,
) -> String {
    let mut output = String::new();
    if !result.changed {
        for comment in &result.missing_comments {
            output.push_str(&style_text("missing:", SemanticStyle::Warning, style));
            output.push(' ');
            output.push_str(comment);
            output.push('\n');
        }
    }
    output.push_str(&result.message);
    output.push('\n');
    output
}

// sirno:witness:diagnostics:begin
fn format_diagnostics_with_style(diagnostics: &[DiagnosticRecord], style: OutputStyle) -> String {
    let mut output = String::new();
    for diagnostic in diagnostics {
        let severity = styled_diagnostic_severity(&diagnostic.severity, style);
        let location = format_diagnostic_location(diagnostic);
        match location {
            | Some(location) => {
                output.push_str(&format!(
                    "{severity}[{}]: {location}: {}\n",
                    diagnostic.code, diagnostic.message
                ));
            }
            | None => {
                output.push_str(&format!(
                    "{severity}[{}]: {}\n",
                    diagnostic.code, diagnostic.message
                ));
            }
        }
        if let Some(help) = &diagnostic.help {
            output.push_str(&format!("  help: {help}\n"));
        }
    }
    output
}

fn format_diagnostic_location(diagnostic: &DiagnosticRecord) -> Option<String> {
    let path = diagnostic.path.as_ref()?;
    let Some(line) = diagnostic.line else {
        return Some(path.clone());
    };
    let Some(column) = diagnostic.column else {
        return Some(format!("{path}:{line}"));
    };
    Some(format!("{path}:{line}:{column}"))
}
// sirno:witness:diagnostics:end

// sirno:witness:cli-interface:begin
fn styled_diagnostic_severity(severity: &str, style: OutputStyle) -> String {
    match severity {
        | "error" => style_text(severity, SemanticStyle::Error, style),
        | "warning" => style_text(severity, SemanticStyle::Warning, style),
        | _ => severity.to_owned(),
    }
}

fn style_text(value: &str, semantic: SemanticStyle, style: OutputStyle) -> String {
    if !style.colors() {
        return value.to_owned();
    }
    let text_style = semantic.text_style();
    format!("{text_style}{value}{text_style:#}")
}
// sirno:witness:cli-interface:end

pub(crate) fn format_gen_link_report(
    root: &Path, entry_count: usize, changed_entry_count: usize, changed_paths: &[PathBuf],
) -> String {
    if changed_paths.is_empty() {
        return format!("No changes in {}", root.display());
    }

    let mut report = String::new();
    for path in changed_paths {
        report.push_str("- ");
        report.push_str(&path.display().to_string());
        report.push('\n');
    }
    report.push_str("Total changes: ");
    report.push_str(&changed_entry_count.to_string());
    report.push('/');
    report.push_str(&entry_count.to_string());
    report.push_str(" in ");
    report.push_str(&root.display().to_string());
    report
}

pub(crate) fn print_query_results(
    results: &QueryResults, format: QueryOutputFormat,
) -> Result<(), CommandError> {
    match format {
        | QueryOutputFormat::Json => {
            println!("{}", results.to_json()?);
        }
        | QueryOutputFormat::Human => {
            print!("{}", format_query_table(&results.columns, &results.rows));
        }
    }
    Ok(())
}

pub(crate) fn print_query_column_options(
    columns: &QueryColumns, format: QueryOutputFormat,
) -> Result<(), CommandError> {
    match format {
        | QueryOutputFormat::Json => {
            println!("{}", format_json(&columns.labels())?);
        }
        | QueryOutputFormat::Human => {
            let labels = columns.labels();
            print!(
                "{}",
                format_human_table(["column"], labels.iter().map(|label| [label.as_str()]))
            );
        }
    }
    Ok(())
}

pub(crate) fn output_path(path: PathBuf, absolute: bool) -> Result<PathBuf, CommandError> {
    if !absolute || path.is_absolute() {
        return Ok(path);
    }
    Ok(env::current_dir().map_err(CommandError::CurrentDirectory)?.join(path))
}

pub(crate) fn format_path_table(records: &[PathRecord]) -> String {
    let headers = ["kind", "path"];
    let rows = records.iter().map(|record| [record.kind, record.path.as_str()]);
    format_human_table(headers, rows)
}

#[cfg(test)]
#[allow(dead_code)]
pub(crate) fn format_skill_wrapper_table(records: &[SkillWrapperRecord]) -> String {
    format_skill_wrapper_table_with_style(records, OutputStyle::Plain)
}

pub(crate) fn format_skill_wrapper_table_for_terminal(records: &[SkillWrapperRecord]) -> String {
    format_skill_wrapper_table_with_style(records, OutputStyle::Styled)
}

fn format_skill_wrapper_table_with_style(
    records: &[SkillWrapperRecord], style: OutputStyle,
) -> String {
    let headers = ["status", "name", "target"];
    let rows = records
        .iter()
        .map(|record| [record.status.as_str(), record.name.as_str(), record.target_path.as_str()]);
    format_human_table_with_style(headers, rows, style)
}

pub(crate) fn query_result_rows(
    report: &EntryDirectoryReport, entries: &[&Entry], columns: &QueryColumns,
) -> Result<Vec<Vec<QueryValue>>, CommandError> {
    entries
        .iter()
        .map(|entry| {
            columns
                .columns
                .iter()
                .map(|column| format_query_column(report, entry, column))
                .collect::<Result<Vec<_>, _>>()
        })
        .collect()
}

fn format_query_column(
    report: &EntryDirectoryReport, entry: &Entry, column: &QueryColumn,
) -> Result<QueryValue, CommandError> {
    match column {
        | QueryColumn::Id => Ok(QueryValue::text(entry.id.to_string())),
        | QueryColumn::Path => {
            let path = report
                .entry_file_path(&entry.id)
                .ok_or_else(|| EntryDirectoryError::MissingEntryFilePath(entry.id.clone()))?;
            Ok(QueryValue::text(path.display().to_string()))
        }
        | QueryColumn::Field { field } if report.meta().contains_intrinsic_field(field) => {
            Ok(QueryValue::optional_text(entry.metadata.intrinsic_field(field)))
        }
        | QueryColumn::Field { field } if report.structural().contains_field(field) => {
            Ok(QueryValue::targets(entry.metadata.structural_field(field)))
        }
        | QueryColumn::Field { field } => {
            panic!("query column `{field}` was not validated before rendering")
        }
    }
}

pub(crate) fn format_query_json(
    columns: &QueryColumns, rows: &[Vec<QueryValue>],
) -> Result<String, CommandError> {
    format_json(&query_result_records(columns, rows))
}

pub(crate) fn query_result_records(
    columns: &QueryColumns, rows: &[Vec<QueryValue>],
) -> Vec<IndexMap<String, QueryValue>> {
    rows.iter()
        .map(|row| {
            columns
                .columns
                .iter()
                .zip(row)
                .map(|(column, value)| (column.label().to_owned(), value.clone()))
                .collect()
        })
        .collect()
}

pub(crate) fn format_query_table(columns: &QueryColumns, rows: &[Vec<QueryValue>]) -> String {
    let headers = columns.columns.iter().map(|column| column.label()).collect::<Vec<_>>();
    let display_rows = rows
        .iter()
        .map(|row| row.iter().map(QueryValue::display).collect::<Vec<_>>())
        .collect::<Vec<_>>();
    format_human_table(headers, display_rows.iter().map(|row| row.iter().map(String::as_str)))
}

fn format_human_table<'a>(
    headers: impl IntoIterator<Item = &'a str>,
    rows: impl IntoIterator<Item = impl IntoIterator<Item = &'a str>>,
) -> String {
    format_human_table_with_style(headers, rows, OutputStyle::Plain)
}

fn format_human_table_with_style<'a>(
    headers: impl IntoIterator<Item = &'a str>,
    rows: impl IntoIterator<Item = impl IntoIterator<Item = &'a str>>, style: OutputStyle,
) -> String {
    let headers = headers.into_iter().map(str::to_owned).collect::<Vec<_>>();
    let rows = rows
        .into_iter()
        .map(|row| row.into_iter().map(str::to_owned).collect::<Vec<_>>())
        .collect::<Vec<_>>();
    format_human_table_with_width_and_style(headers, rows, None, style)
}

#[cfg(test)]
#[allow(dead_code)]
pub(crate) fn format_human_table_with_width(
    headers: Vec<String>, rows: Vec<Vec<String>>, width: Option<u16>,
) -> String {
    format_human_table_with_width_and_style(headers, rows, width, OutputStyle::Plain)
}

pub(crate) fn format_human_table_semantic_with_width(
    headers: Vec<String>, rows: Vec<Vec<String>>, width: Option<u16>, style: OutputStyle,
) -> String {
    format_human_table_with_width_and_style(headers, rows, width, style)
}

fn format_human_table_with_width_and_style(
    headers: Vec<String>, rows: Vec<Vec<String>>, width: Option<u16>, style: OutputStyle,
) -> String {
    let mut table = Table::new();
    table.load_preset(UTF8_FULL);
    table.set_content_arrangement(ContentArrangement::Dynamic);
    if let Some(width) = width {
        table.set_width(width);
    }
    #[cfg(test)]
    table.force_no_tty();
    #[cfg(test)]
    if style == OutputStyle::Forced {
        table.enforce_styling();
    }
    let (headers, rows) = elide_human_table_columns(headers, rows, table.width());
    let styled_rows = rows
        .into_iter()
        .map(|row| {
            row.into_iter()
                .enumerate()
                .map(|(index, cell)| {
                    let header = headers.get(index).map(String::as_str).unwrap_or_default();
                    semantic_cell(header, cell, style)
                })
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    table.set_header(headers);
    table.add_rows(styled_rows);
    let mut output = table.to_string();
    output.push('\n');
    output
}

fn semantic_cell(header: &str, value: String, style: OutputStyle) -> Cell {
    let cell = Cell::new(value.clone());
    if let Some(semantic) = semantic_table_cell_style(header, &value).filter(|_| style.colors()) {
        cell.fg(semantic.table_color())
    } else {
        cell
    }
}

// sirno:witness:cli-interface:begin
fn semantic_table_cell_style(header: &str, value: &str) -> Option<SemanticStyle> {
    match header {
        | "state" | "status" => semantic_status_style(value),
        | _ => None,
    }
}

fn semantic_status_style(value: &str) -> Option<SemanticStyle> {
    match value {
        | "ok" | "resolved" | "unchanged" => Some(SemanticStyle::Success),
        | "drifted" | "glacier-drift" | "missing" | "missing-cache" | "missing-glacier"
        | "missing-lock" | "open" | "stale-lock" => Some(SemanticStyle::Warning),
        | "linked" | "wrote" => Some(SemanticStyle::Changed),
        | _ => None,
    }
}
// sirno:witness:cli-interface:end

fn elide_human_table_columns(
    headers: Vec<String>, rows: Vec<Vec<String>>, width: Option<u16>,
) -> (Vec<String>, Vec<Vec<String>>) {
    let Some(width) = width.map(usize::from) else {
        return (headers, rows);
    };
    if headers.len() <= 2 || min_table_width(&headers) <= width {
        return (headers, rows);
    }

    for visible in (1..headers.len()).rev() {
        let mut candidate_headers = headers.iter().take(visible).cloned().collect::<Vec<_>>();
        candidate_headers.push("...".to_owned());
        if min_table_width(&candidate_headers) <= width {
            let candidate_rows = rows
                .into_iter()
                .map(|row| {
                    let mut cells = row.into_iter().take(visible).collect::<Vec<_>>();
                    cells.push("...".to_owned());
                    cells
                })
                .collect();
            return (candidate_headers, candidate_rows);
        }
    }

    (
        headers.into_iter().take(1).collect(),
        rows.into_iter().map(|row| row.into_iter().take(1).collect()).collect(),
    )
}

fn min_table_width(headers: &[String]) -> usize {
    headers.iter().map(|header| UnicodeWidthStr::width(header.as_str()).max(1)).sum::<usize>()
        + headers.len() * 3
        + 1
}

pub(crate) fn print_entry_directory_report(report: &EntryDirectoryReport) {
    if report.is_clean() {
        print_ok_path(report.root());
        return;
    }

    anstream::print!("{}", format_entry_directory_report_with_style(report, OutputStyle::Styled));
}

fn format_entry_directory_report_with_style(
    report: &EntryDirectoryReport, style: OutputStyle,
) -> String {
    let diagnostics = diagnostics_from_entry_report(report);
    let mut output = format_diagnostics_with_style(&diagnostics, style);
    output.push_str(&format!("{}\n", entry_directory_report_summary(report, style)));
    output
}

fn entry_directory_report_summary(report: &EntryDirectoryReport, style: OutputStyle) -> String {
    if report.has_errors() {
        format!(
            "check: {} in {}",
            style_text("failed", SemanticStyle::Error, style),
            report.root().display()
        )
    } else {
        format!(
            "check: {} in {}",
            style_text("warnings", SemanticStyle::Warning, style),
            report.root().display()
        )
    }
}

pub(crate) fn print_ok_path(path: &Path) {
    anstream::println!("{}", format_ok_line(&path.display().to_string(), OutputStyle::Styled));
}

fn format_ok_line(location: &str, style: OutputStyle) -> String {
    format!("{}: {location}", style_text("ok", SemanticStyle::Success, style))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cli_error_renders_help_line_for_actionable_error() {
        let error = CommandError::RepoMembersNotConfigured;
        let rendered = format_cli_error(&error, OutputStyle::Plain);
        assert_eq!(
            rendered,
            "sirno: repo members are not configured\n  help: Add `[repo].members` to Sirno.toml.\n"
        );
    }

    #[test]
    fn command_error_renders_help_line_for_mcp_output() {
        let error = CommandError::CharmNotEnabled("charm".parse().unwrap());
        let rendered = format_command_error(&error);
        assert_eq!(
            rendered,
            "charm `charm` is not enabled\n  help: Enable the charm with `sirno charm enable charm`."
        );
    }

    #[test]
    fn command_error_delegates_help_to_missing_config() {
        let error = CommandError::Config(crate::ConfigError::Read {
            path: PathBuf::from("Sirno.toml"),
            source: std::io::Error::from(std::io::ErrorKind::NotFound),
        });
        let rendered = format_command_error(&error);
        assert!(rendered.contains("  help: Run `sirno init` to create Sirno.toml."), "{rendered}");
    }

    #[test]
    fn command_error_omits_help_line_when_no_hint_is_known() {
        let error = CommandError::TerminalUi(std::io::Error::from(std::io::ErrorKind::BrokenPipe));
        let rendered = format_command_error(&error);
        assert!(!rendered.contains("help:"), "{rendered}");
    }
}
