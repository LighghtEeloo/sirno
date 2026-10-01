---
name: Agent Skills
desc: The Sirno agent skill set and the handoffs between its skills.
category:
  - meta
belongs:
  - sirno
prerequisite:
  - methodology
  - semantic-locality
  - immutable-update
---

Sirno ships an agent skill set that renders the lake method into operational procedure.

This entry is the roster and handoff guide for that set.
The roster is part of this entry's local claim:
a reader should be able to see which skills ship,
what each one owns,
and where shared method applies.
The durable procedure for each skill lives in its own discipline entry.

| Skill | Discipline entry | Local role |
|---|---|---|
| `sirno-editor` | `repository-editing-discipline` | Repository-material edits that start in the lake. |
| `sirno-actualizer` | `actualization-discipline` | Lake-to-repository work. |
| `sirno-internalizer` | `internalization-discipline` | Repository-to-lake work. |
| `sirno-narrative-session` | `narrative-session-discipline` | Adaptive routes through lake knowledge. |
| `sirno-skill-synthesizer` | `skill-synthesis-discipline` | Rebuilds MCP resources and wrappers. |
| `sirno-curator` | `lake-curation-discipline` | Lake-shape audits and approved curation. |
| `sirno-finalizer` | `finalization-discipline` | Accepts, stages, and commits reviewed work. |

The roster orients readers; it is not the semantic contract of every skill.
A new packaged Sirno skill should update this roster and add its own discipline entry,
artifacts, package, and structural links.
It should not require existing discipline entries to rewrite their local meaning.

Each rostered discipline owns two lake artifacts.
`SKILL.full.template.md` is the static full-resource template bundled through `src/skills.rs`.
MCP renders it when an agent reads the matching `sirno://skills/sirno-*` resource.
The rendered resource injects active-project metadata,
including intrinsic fields and structural relations loaded from `.sirno/meta.toml`.
`SKILL.md` is the small wrapper copied into `.agents/skills/sirno-*/SKILL.md`.
The wrapper tells an agent to read the MCP resource before doing skill work.
When Claude skill integration is selected,
Sirno links installed `.agents/skills/sirno-*` package directories into `.claude/skills`.

One bundled inventory owns skill names, source entries, resource metadata, templates,
and optional wrappers.
MCP resource discovery and wrapper maintenance both use that inventory.
Resource-only methods remain available through MCP without becoming installed Sirno wrappers.
The inventory preserves the resource roster and packaged skill order.

Actualizer and internalizer load the editor procedure once per active project context,
then apply their own directional procedure.
The editor owns the shared repository-editing workflow and remains independently usable.

Packaged skills are portable.
They speak from the active project perspective:
use the configured lake path from `Sirno.toml`,
query and read the active project's entries,
and avoid assuming this source repository's `sirno-lake/` path or self-hosted entry set.
The `portable-agent-skill-language` entry states this rule directly.

All packaged skills share one lake-first rule.
Any edit to source, tests, generated artifacts, configuration, README files,
design documents outside the configured lake, or packaged skills begins by reading
the relevant active-project entries.
When a skill creates or revises an entry,
it applies semantic locality:
the entry body should carry the local meaning,
and any roster, route, index, or review front door should say what the list is for.
When a skill changes repository material,
it applies immutable update:
the old repository and lake are a snapshot,
and the next state should carry forward only the facts, prose, code, and layers that still belong.

The `design-doc-writer-skill` entry documents an adjacent design-document method.
It is a reusable method input for Sirno skill work,
not one of the packaged Sirno skills in this roster.
When a repository has its own design-document skill or documented prose method,
use that instead.

Repository skill maintenance is a local human CLI utility surface.
MCP serves skill resources for agents.
It does not expose `sirno util` commands for wrapper listing, checking, or installation.
Agents maintain skill artifacts and installed wrappers as explicit repository files.
Utility commands copy exact bundled wrapper constants.
When utility maintenance is needed,
an agent should report the needed human CLI action.
The editor skill may call CLI `sirno util config fix`
to canonicalize `Sirno.toml` comments during repository editing.

A full skill resource is a runtime rendering of lake method, not a separate authority.
When a resource template, rendered resource, wrapper, and the lake disagree,
the lake and `Sirno.toml` win,
and the artifact should be corrected.
Failure handling belongs in the full resource.
A wrapper body should be one sentence that directs the agent to the MCP resource,
so installed skills stay small while the full method remains source-controlled.
