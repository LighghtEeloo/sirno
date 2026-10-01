//! Terminal UI for human config maintenance.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use ratatui::Frame;
use ratatui::layout::Constraint;
use ratatui::widgets::{Cell, Row, Table};

use crate::surface::cli::tui::{
    TuiApp, TuiFlow, TuiKey, TuiSelection, handle_table_key, header_style, key_help, panel_block,
    render_key_footer, render_selectable_table, run_terminal_ui, run_tui_app, table_footer_areas,
};
use crate::surface::error::CommandError;
use crate::{
    CharmSettings, CheckSettings, ConfigError, RepoSettings, SirnoConfig, TutorialSettings,
};

const SECTIONS: [ConfigSection; 6] = [
    ConfigSection::Lake,
    ConfigSection::Repo,
    ConfigSection::Witness,
    ConfigSection::Check,
    ConfigSection::Tutorial,
    ConfigSection::Charm,
];

/// Run the interactive config maintenance UI.
pub(crate) fn run(config_path: &Path) -> Result<ExitCode, CommandError> {
    run_terminal_ui(|terminal| {
        let mut app = ConfigTui::load(config_path.to_path_buf())?;
        run_tui_app(terminal, &mut app)
    })
}

#[derive(Debug)]
struct ConfigTui {
    config_path: PathBuf,
    source: String,
    config: SirnoConfig,
    rows: Vec<ConfigSectionRow>,
    selection: TuiSelection,
    message: String,
}

impl ConfigTui {
    fn load(config_path: PathBuf) -> Result<Self, CommandError> {
        let source = read_config_source(&config_path)?;
        let config = SirnoConfig::from_file(&config_path)?;
        let rows = config_section_rows(&source, &config)?;
        Ok(Self {
            config_path,
            source,
            config,
            rows,
            selection: TuiSelection::default(),
            message: key_help(&["i inserts a section", "f fixes comments"]),
        })
    }

    fn reload(&mut self, message: String) -> Result<(), CommandError> {
        let selected_section = self.selected_section();
        self.source = read_config_source(&self.config_path)?;
        self.config = SirnoConfig::from_file(&self.config_path)?;
        self.rows = config_section_rows(&self.source, &self.config)?;
        self.selection
            .set(self.rows.iter().position(|row| row.section == selected_section).unwrap_or(0));
        self.message = message;
        Ok(())
    }

    fn selected_section(&self) -> ConfigSection {
        self.rows[self.selection.selected()].section
    }

    fn insert_selected(&mut self) -> Result<(), CommandError> {
        let section = self.selected_section();
        let mut config = self.config.clone();
        materialize_section(&mut config, section);
        let Some(section_source) = canonical_section_source(&config, section)? else {
            self.message = format!("{} has no canonical body to insert", section.label());
            return Ok(());
        };
        let source = replace_or_insert_section(&self.source, section, &section_source);
        if source == self.source {
            self.message = format!("{} is already inserted", section.label());
            return Ok(());
        }
        write_config_source(&self.config_path, &source)?;
        self.reload(format!("inserted {}", section.label()))
    }

    fn fix_selected(&mut self) -> Result<(), CommandError> {
        let row = self.rows[self.selection.selected()].clone();
        if !row.present {
            self.message = format!("{} is absent", row.section.label());
            return Ok(());
        }
        if row.comments == CommentStatus::Empty {
            self.message = format!("{} is empty", row.section.label());
            return Ok(());
        }
        let Some(section_source) = canonical_section_source(&self.config, row.section)? else {
            self.message = format!("{} is empty", row.section.label());
            return Ok(());
        };
        let source = replace_or_insert_section(&self.source, row.section, &section_source);
        if source == self.source {
            self.message = format!("{} comments are already complete", row.section.label());
            return Ok(());
        }
        write_config_source(&self.config_path, &source)?;
        self.reload(format!("fixed comments for {}", row.section.label()))
    }

    fn render(&self, frame: &mut Frame<'_>) {
        let areas = table_footer_areas(frame, 3);

        let header = Row::new(["Section", "Present", "Comments"]).style(header_style());
        let rows = self.rows.iter().map(|row| {
            Row::new([
                Cell::from(row.section.label()),
                Cell::from(if row.present { "yes" } else { "no" }),
                Cell::from(row.comments.label()),
            ])
        });
        let table =
            Table::new(rows, [Constraint::Length(18), Constraint::Length(10), Constraint::Min(16)])
                .header(header)
                .block(panel_block("Sirno.toml"));
        render_selectable_table(frame, areas.table, table, self.selection);
        render_key_footer(frame, areas.footer, self.message.as_str(), false);
    }
}

impl TuiApp for ConfigTui {
    fn render(&self, frame: &mut Frame<'_>) {
        ConfigTui::render(self, frame);
    }

    fn handle_key(&mut self, key: TuiKey) -> Result<TuiFlow, CommandError> {
        if let Some(flow) = handle_table_key(&mut self.selection, self.rows.len(), key) {
            return Ok(flow);
        }
        match key {
            // sirno:witness:utility-commands:begin
            | TuiKey::Char('i') => {
                self.insert_selected()?;
                Ok(TuiFlow::Continue)
            }
            | TuiKey::Char('f') => {
                self.fix_selected()?;
                Ok(TuiFlow::Continue)
            }
            // sirno:witness:utility-commands:end
            | TuiKey::Quit
            | TuiKey::Next
            | TuiKey::Prev
            | TuiKey::Tab
            | TuiKey::Char(_)
            | TuiKey::Other => Ok(TuiFlow::Continue),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct ConfigSectionRow {
    section: ConfigSection,
    present: bool,
    comments: CommentStatus,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum ConfigSection {
    Lake,
    Repo,
    Witness,
    Check,
    Tutorial,
    Charm,
}

impl ConfigSection {
    fn from_name(name: &str) -> Option<Self> {
        match name {
            | "lake" => Some(Self::Lake),
            | "repo" => Some(Self::Repo),
            | "witness" => Some(Self::Witness),
            | "check" => Some(Self::Check),
            | "tutorial" => Some(Self::Tutorial),
            | "charm" => Some(Self::Charm),
            | _ => None,
        }
    }

    fn label(self) -> &'static str {
        match self {
            | Self::Lake => "[lake]",
            | Self::Repo => "[repo]",
            | Self::Witness => "[witness]",
            | Self::Check => "[check]",
            | Self::Tutorial => "[tutorial]",
            | Self::Charm => "[charm]",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CommentStatus {
    Absent,
    Empty,
    Complete,
    Missing(usize),
}

impl CommentStatus {
    fn label(self) -> String {
        match self {
            | Self::Absent => "n/a".to_owned(),
            | Self::Empty => "empty".to_owned(),
            | Self::Complete => "complete".to_owned(),
            | Self::Missing(count) => format!("missing {count}"),
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct SectionBlock {
    start: usize,
    end: usize,
}

fn config_section_rows(
    source: &str, config: &SirnoConfig,
) -> Result<Vec<ConfigSectionRow>, CommandError> {
    let blocks = section_blocks(source);
    let canonical = config.to_commented_toml()?;
    let canonical_blocks = section_blocks(&canonical);
    Ok(SECTIONS
        .into_iter()
        .map(|section| {
            let Some(block) = blocks.get(&section).copied() else {
                return ConfigSectionRow {
                    section,
                    present: false,
                    comments: CommentStatus::Absent,
                };
            };
            let comments = if !section_has_content(source, block, section) {
                CommentStatus::Empty
            } else {
                let expected = canonical_blocks
                    .get(&section)
                    .map(|block| section_comments(&canonical, *block))
                    .unwrap_or_default();
                let present = section_comments(source, block);
                let missing = expected
                    .iter()
                    .filter(|comment| !present.iter().any(|current| current == *comment))
                    .count();
                if missing == 0 { CommentStatus::Complete } else { CommentStatus::Missing(missing) }
            };
            ConfigSectionRow { section, present: true, comments }
        })
        .collect())
}

fn materialize_section(config: &mut SirnoConfig, section: ConfigSection) {
    match section {
        | ConfigSection::Lake | ConfigSection::Witness => {}
        | ConfigSection::Repo => {
            config.repo.get_or_insert_with(|| RepoSettings { members: Vec::new() });
        }
        | ConfigSection::Check => {
            config.check = CheckSettings { render: Some(config.check.render_enabled()) };
        }
        | ConfigSection::Tutorial => {
            config.tutorial.get_or_insert_with(TutorialSettings::all);
        }
        | ConfigSection::Charm => {
            config.charm = CharmSettings::default();
        }
    }
}

fn canonical_section_source(
    config: &SirnoConfig, section: ConfigSection,
) -> Result<Option<String>, CommandError> {
    let canonical = config.to_commented_toml()?;
    Ok(section_text(&canonical, section))
}

fn section_text(source: &str, section: ConfigSection) -> Option<String> {
    let blocks = section_blocks(source);
    let block = blocks.get(&section)?;
    let lines = lines_without_endings(source);
    Some(lines[block.start..block.end].join("\n") + "\n")
}

fn replace_or_insert_section(source: &str, section: ConfigSection, section_source: &str) -> String {
    let blocks = section_blocks(source);
    let mut lines = lines_without_endings(source);
    let mut replacement = lines_without_endings(section_source);
    if let Some(block) = blocks.get(&section) {
        lines.splice(block.start..block.end, replacement);
    } else {
        let index = insertion_index(&blocks, lines.len(), section);
        if index > 0 && !lines.get(index.saturating_sub(1)).is_some_and(|line| line.is_empty()) {
            replacement.insert(0, String::new());
        }
        if index < lines.len() && !replacement.last().is_some_and(|line| line.is_empty()) {
            replacement.push(String::new());
        }
        lines.splice(index..index, replacement);
    }
    let mut output = lines.join("\n");
    output.push('\n');
    output
}

fn insertion_index(
    blocks: &BTreeMap<ConfigSection, SectionBlock>, fallback: usize, section: ConfigSection,
) -> usize {
    let position = SECTIONS.iter().position(|candidate| *candidate == section).unwrap();
    for later in SECTIONS.iter().skip(position + 1) {
        if let Some(block) = blocks.get(later) {
            return block.start;
        }
    }
    fallback
}

fn section_blocks(source: &str) -> BTreeMap<ConfigSection, SectionBlock> {
    let lines = lines_without_endings(source);
    let mut starts = Vec::<(usize, ConfigSection)>::new();
    let mut current = None;
    for (index, line) in lines.iter().enumerate() {
        if let Some((section, _)) = table_header(line)
            && current != Some(section)
        {
            starts.push((index, section));
            current = Some(section);
        }
    }

    let mut blocks = BTreeMap::new();
    for (index, (start, section)) in starts.iter().copied().enumerate() {
        let end = starts.get(index + 1).map(|(start, _)| *start).unwrap_or(lines.len());
        blocks.entry(section).or_insert(SectionBlock { start, end });
    }
    blocks
}

fn section_comments(source: &str, block: SectionBlock) -> Vec<String> {
    lines_without_endings(source)[block.start..block.end]
        .iter()
        .filter_map(|line| line.trim_start().strip_prefix("# ").map(str::to_owned))
        .collect()
}

fn section_has_content(source: &str, block: SectionBlock, section: ConfigSection) -> bool {
    lines_without_endings(source)[block.start..block.end].iter().any(|line| {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            return false;
        }
        if let Some((header_section, exact_top_level)) = table_header(trimmed) {
            return header_section == section && !exact_top_level;
        }
        true
    })
}

fn table_header(line: &str) -> Option<(ConfigSection, bool)> {
    let trimmed = line.trim();
    let (inner, array) = if let Some(rest) = trimmed.strip_prefix("[[") {
        (rest.split_once("]]")?.0, true)
    } else {
        let rest = trimmed.strip_prefix('[')?;
        (rest.split_once(']')?.0, false)
    };
    let top = inner.split('.').next()?.trim().trim_matches('"').trim_matches('\'');
    let section = ConfigSection::from_name(top)?;
    Some((section, !array && !inner.contains('.')))
}

fn read_config_source(path: &Path) -> Result<String, CommandError> {
    fs::read_to_string(path)
        .map_err(|source| ConfigError::Read { path: path.to_path_buf(), source }.into())
}

fn write_config_source(path: &Path, source: &str) -> Result<(), CommandError> {
    fs::write(path, source)
        .map_err(|source| ConfigError::Write { path: path.to_path_buf(), source }.into())
}

fn lines_without_endings(source: &str) -> Vec<String> {
    source.lines().map(str::to_owned).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{LakeSettings, WitnessSettings};

    fn config() -> SirnoConfig {
        SirnoConfig {
            lake: LakeSettings::new("docs"),
            upstreams: Default::default(),
            repo: None,
            witness: WitnessSettings { delimiters: Vec::new() },
            check: CheckSettings::default(),
            tutorial: None,
            charm: CharmSettings::default(),
        }
    }

    #[test]
    fn rows_report_present_and_comment_status() {
        let source = "\
[lake]
path = \"docs\"

[witness]
";

        let rows = config_section_rows(source, &config()).unwrap();

        let lake = rows.iter().find(|row| row.section == ConfigSection::Lake).unwrap();
        let witness = rows.iter().find(|row| row.section == ConfigSection::Witness).unwrap();
        let check = rows.iter().find(|row| row.section == ConfigSection::Check).unwrap();
        assert!(lake.present);
        assert_eq!(lake.comments, CommentStatus::Missing(1));
        assert!(witness.present);
        assert_eq!(witness.comments, CommentStatus::Empty);
        assert!(!check.present);
        assert_eq!(check.comments, CommentStatus::Absent);
    }

    #[test]
    fn insert_places_section_in_canonical_order() {
        let source = "\
[lake]
path = \"docs\"

[witness]
";
        let inserted = replace_or_insert_section(
            source,
            ConfigSection::Check,
            "[check]\n# Require generated footers to match current metadata during checks.\nrender = true\n",
        );

        assert!(inserted.find("[check]").unwrap() > inserted.find("[witness]").unwrap());
    }

    #[test]
    fn replace_keeps_other_sections_untouched() {
        let source = "\
[lake]
path = \"docs\"

[check]
render = true

[witness]
";
        let replaced = replace_or_insert_section(
            source,
            ConfigSection::Check,
            "[check]\n# Require generated footers to match current metadata during checks.\nrender = true\n",
        );

        assert!(replaced.contains("[lake]\npath = \"docs\""));
        assert!(replaced.contains("# Require generated footers"));
        assert!(replaced.contains("[witness]"));
    }
}
