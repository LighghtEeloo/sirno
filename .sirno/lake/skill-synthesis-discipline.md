---
name: Skill Synthesis Discipline
desc: The agent procedure for rebuilding MCP skill resources and installed skill wrappers.
category:
  - meta
belongs:
  - agent-skills
prerequisite:
  - agent-skills
  - portable-agent-skill-language
  - semantic-locality
  - immutable-update
---

Skill synthesis rebuilds the packaged Sirno skill wrappers and MCP skill resource templates
from the active project's `meta`-categorized entries.
Its static full-resource template lives in the `SKILL.full.template.md` entry artifact
and is rendered by `src/mcp.rs` as `sirno://skills/sirno-skill-synthesizer`.
Its packaged wrapper lives in the `SKILL.md` entry artifact
and renders to `.agents/skills/sirno-skill-synthesizer/SKILL.md`.
Each rostered discipline entry owns the same pair:
`SKILL.full.template.md` for the MCP resource template
and `SKILL.md` for the installed wrapper.

Read the sources first.
Read `Sirno.toml` for the lake path,
then `agent-skills` for the skill roster and the handoffs between skills,
then every `meta`-categorized entry through `sirno_entry_query`.
The lake is authoritative.
The full resource template, rendered resource, and wrapper are reproducible surfaces.
Any synthesis change is a repository edit.
Read and update the governing lake entries before changing skill artifacts,
installed packages, MCP resources, or Rust bundle lists.
Discover those surfaces in the active project.
Do not assume this source repository's file layout unless the active project's lake says to use it.

Separate disciplines from shared method.
A `meta` entry named by the active project's Sirno skill roster is a skill source.
The other `meta` entries carry vocabulary, principles, perspective, and design authority.
They are cross-cutting method that every skill must respect,
not skills in their own right.
Semantic locality and immutable update are cross-cutting methods.
When a generated skill creates, edits, audits, or materializes entries,
its procedure should preserve the entry's local meaning
and preserve useful lists without confusing them for semantic contracts.
A route, index, roster, or review front door may enumerate children
when that list is part of that entry's local claim.
When a generated skill changes repository material,
its procedure should treat the current repository and lake as a snapshot,
carry forward the facts that still hold,
and prefer replacing stale local shape over layering around it.
Repository-specific design-document skills or documented prose methods are the first method input
when Sirno skill work touches design prose.
If a repository has none,
default to `sirno://skills/design-doc-writer` from `design-doc-writer-skill`.
Their reusable content is reader evaluation,
conceptual ordering,
declarative precision,
reader-aware bullets and diagrams,
and whole-document coherence.
They do not become skill sources unless the Sirno skill roster adds them.
They may own full MCP resource template artifacts without rendering installed Sirno wrappers.

Bind each discipline to one MCP resource and one wrapper package.
A skill discipline owns exactly one `SKILL.full.template.md` resource template artifact,
one `SKILL.md` wrapper artifact,
and one `.agents/skills/sirno-<role>/SKILL.md` installed wrapper package.
The target package path is written in the discipline body until the project defines
a structural link relation for skill packages.
Keep the existing skill directory name and do not invent a new role
unless the active roster adds one.
Every rostered Sirno discipline should have both artifacts and a package,
and every `sirno-*` package should trace back to a discipline.

Split full procedure from wrapper.
The `SKILL.full.template.md` artifact operationalizes its discipline plus the shared `meta` method
it depends on.
Its runtime metadata slot lets MCP inject the active project's discovered intrinsic fields,
structural relations,
configured lake path,
default query columns,
and generated meta registry path.
The field records come from `.sirno/meta.toml`,
not from a fresh raw lake scan.
Skill-template frontmatter `name` is the skill directory id.
`description` states when to use the skill and the triggers that should invoke it.
The template body turns durable procedure into concrete steps and current MCP tools.
Shared procedure may be inherited through an explicit MCP resource reference.
Actualizer and internalizer resources load the editor once per active project context
and keep only their directional decisions and local failure paths.
Preserve that delegation during synthesis instead of copying the editor's checklist.
The editor remains usable without loading either directional supplement.
Entry-writing principles and shared failure handling apply through the loaded editor resource.
If a shared resource is unavailable, a prior read from the same project context may supply the method;
otherwise report the missing source and continue only grounded read-only inspection.
The resource and its declared shared method must cover the discipline's failure paths:
missing sources, unavailable tools, blocked validation,
absent evidence, and design changes that must be internalized into the lake.
The wrapper `SKILL.md` artifact keeps the same frontmatter,
then uses a one-sentence body that points to the matching `sirno://skills/sirno-*` resource.
Do not duplicate the full procedure in the wrapper.
Project configuration maintenance lives in the editor skill.
Other full resources should point back to the repository editing workflow
instead of copying the `Sirno.toml` schema checklist.

Routine project initialization installs wrappers by default.
Skill synthesis edits the lake-owned artifacts and installed wrappers directly.
Use ordinary file comparison at review boundaries
to fail when an installed wrapper has drifted from its artifact.
The utility command family remains human CLI operator maintenance;
do not make it part of agent skill procedure or MCP workflow.
When utility maintenance is needed,
report the human CLI action rather than turning it into an agent step.
The editor skill may call CLI `sirno util config fix`
for deterministic `Sirno.toml` comment repair.

Inspect the current Sirno MCP tools before writing tool names into a skill.
A full skill resource that names a missing tool is worse than one that only names the procedure.

Keep the lake the source of truth.
When a skill resource template, rendered resource, wrapper, and the lake disagree,
correct the artifact or wrapper, never the lake.
This discipline is itself a skill source;
the synthesizer rebuilds its own template and wrapper the same way it rebuilds the others.

Validate after writing.
Run render maintenance if lake metadata changed,
then the edit-mode and review-mode structural checks.
Confirm each `SKILL.md` has valid frontmatter,
confirm each `SKILL.full.template.md` has valid frontmatter,
and that the disciplines, resources, wrappers, and packages still correspond one to one.
If a package exists without a discipline,
either add the missing discipline to the lake or report the package as outside the reproducible set.
If a discipline exists without a package,
create the package only when the roster says the skill should ship.
If a full skill template would need behavior the lake does not commit,
leave that behavior out and report the missing design instead of inventing it.
