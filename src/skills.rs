//! Bundled skill inventory shared by MCP resources and wrapper installation.
//!
//! Each resource has one source entry and full template.
//! Wrapper packages are selected explicitly, so resource-only methods stay outside installation.

use std::path::{Path, PathBuf};

const SKILL_RESOURCE_URI_PREFIX: &str = "sirno://skills/";
const AGENT_SKILL_ROOT: &str = ".agents/skills";
const CLAUDE_SKILL_ROOT: &str = ".claude/skills";

/// One embedded skill resource and its optional installed wrapper.
#[derive(Clone, Copy, Debug)]
pub(crate) struct BundledSkill {
    /// Stable resource and package name.
    pub(crate) name: &'static str,
    /// Lake entry that owns the embedded artifacts.
    pub(crate) entry_id: &'static str,
    /// Human-readable MCP resource title.
    pub(crate) title: &'static str,
    /// MCP resource description.
    pub(crate) description: &'static str,
    /// Full procedure before active-project metadata is injected.
    pub(crate) template: &'static str,
    wrapper: Option<&'static str>,
}

macro_rules! bundled_skill {
    (resource, $name:literal, $entry:literal, $title:literal, $description:literal) => {
        BundledSkill {
            name: $name,
            entry_id: $entry,
            title: $title,
            description: $description,
            template: include_str!(concat!(
                "../.sirno/lake/.artifacts/",
                $entry,
                "/SKILL.full.template.md"
            )),
            wrapper: None,
        }
    };
    (wrapper, $name:literal, $entry:literal, $title:literal, $description:literal) => {
        BundledSkill {
            wrapper: Some(include_str!(concat!("../.sirno/lake/.artifacts/", $entry, "/SKILL.md"))),
            ..bundled_skill!(resource, $name, $entry, $title, $description)
        }
    };
}

// sirno:witness:agent-skills:begin
const BUNDLED_SKILLS: &[BundledSkill] = &[
    // sirno:witness:design-doc-writer-skill:begin
    bundled_skill!(
        resource,
        "design-doc-writer",
        "design-doc-writer-skill",
        "Design Doc Writer",
        "Full design-doc-writer skill text."
    ),
    // sirno:witness:design-doc-writer-skill:end
    bundled_skill!(
        wrapper,
        "sirno-editor",
        "repository-editing-discipline",
        "Sirno Editor",
        "Full Sirno editor skill text."
    ),
    bundled_skill!(
        wrapper,
        "sirno-actualizer",
        "actualization-discipline",
        "Sirno Actualizer",
        "Full Sirno actualizer skill text."
    ),
    bundled_skill!(
        wrapper,
        "sirno-internalizer",
        "internalization-discipline",
        "Sirno Internalizer",
        "Full Sirno internalizer skill text."
    ),
    bundled_skill!(
        wrapper,
        "sirno-narrative-session",
        "narrative-session-discipline",
        "Sirno Narrative Session",
        "Full Sirno narrative-session skill text."
    ),
    bundled_skill!(
        wrapper,
        "sirno-skill-synthesizer",
        "skill-synthesis-discipline",
        "Sirno Skill Synthesizer",
        "Full Sirno skill-synthesizer text."
    ),
    bundled_skill!(
        wrapper,
        "sirno-curator",
        "lake-curation-discipline",
        "Sirno Curator",
        "Full Sirno curator skill text."
    ),
    bundled_skill!(
        wrapper,
        "sirno-finalizer",
        "finalization-discipline",
        "Sirno Finalizer",
        "Full Sirno finalizer skill text."
    ),
];

impl BundledSkill {
    /// Return every MCP skill resource in its published order.
    pub(crate) fn all() -> &'static [Self] {
        BUNDLED_SKILLS
    }

    /// Find the bundled resource with an exact URI.
    pub(crate) fn for_uri(uri: &str) -> Option<&'static Self> {
        let name = uri.strip_prefix(SKILL_RESOURCE_URI_PREFIX)?;
        Self::all().iter().find(|skill| skill.name == name)
    }

    /// Select only skills that ship an installed wrapper package.
    pub(crate) fn wrapper_packages() -> impl Iterator<Item = BundledSkillWrapper<'static>> {
        Self::all()
            .iter()
            .filter_map(|skill| skill.wrapper.map(|content| BundledSkillWrapper { skill, content }))
    }

    /// Return the stable MCP resource URI.
    pub(crate) fn uri(&self) -> String {
        format!("{SKILL_RESOURCE_URI_PREFIX}{}", self.name)
    }

    /// Return the source template path with portable forward-slash spelling.
    pub(crate) fn template_path(&self) -> String {
        format!(".sirno/lake/.artifacts/{}/SKILL.full.template.md", self.entry_id)
    }
}

/// A skill selected for wrapper installation, carrying its exact embedded content.
#[derive(Clone, Copy, Debug)]
pub(crate) struct BundledSkillWrapper<'a> {
    /// Resource whose inventory entry explicitly selects wrapper installation.
    pub(crate) skill: &'a BundledSkill,
    /// Exact wrapper bytes embedded from the owning entry's artifact.
    pub(crate) content: &'static str,
}

impl BundledSkillWrapper<'_> {
    /// Return the source wrapper path with portable forward-slash spelling.
    pub(crate) fn artifact_path(&self) -> String {
        format!(".sirno/lake/.artifacts/{}/SKILL.md", self.skill.entry_id)
    }

    /// Return the installed wrapper path with portable forward-slash spelling.
    pub(crate) fn target_path(&self) -> String {
        format!("{AGENT_SKILL_ROOT}/{}/SKILL.md", self.skill.name)
    }

    /// Return the installed package directory relative to the project root.
    pub(crate) fn agent_skill_path(&self) -> PathBuf {
        Path::new(AGENT_SKILL_ROOT).join(self.skill.name)
    }

    /// Return the optional Claude link path relative to the project root.
    pub(crate) fn claude_skill_path(&self) -> PathBuf {
        Path::new(CLAUDE_SKILL_ROOT).join(self.skill.name)
    }

    /// Return the package directory relative to its Claude link's parent.
    pub(crate) fn claude_link_source(&self) -> PathBuf {
        Path::new("..").join("..").join(self.agent_skill_path())
    }
}
// sirno:witness:agent-skills:end
