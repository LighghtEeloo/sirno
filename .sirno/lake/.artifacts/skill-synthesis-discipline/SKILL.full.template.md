---
name: sirno-skill-synthesizer
description: >-
  Rebuild Sirno skill templates, MCP resources, wrappers, and installed packages from the active
  project's lake.
---

# Sirno Skill Synthesizer

## Purpose

Use this skill when the active repository owns packaged Sirno skills or MCP skill resources
and those packages should be rebuilt from Sirno entries.
The active project's lake is the source of truth.
Generated skill files are reproducible surfaces.

If the active repository defines no skill roster, no skill artifacts, or no package locations,
say so instead of inventing a layout.

This full skill text is served as `sirno://skills/sirno-skill-synthesizer`.
It follows the project's skill-synthesis discipline.

{{SIRNO_ACTIVE_PROJECT_METADATA}}

## Project Binding

Bind the MCP server to the repository before calling project tools.
Call `sirno_cwd` with the repository root when the server may not already be there.
Project tools resolve `Sirno.toml` from the server current working directory on each call.
Call `sirno_cwd` again before switching projects in the same server process.

## Lake-First Rule

Skill synthesis is a repository edit.
Read and update the governing entries before changing artifacts,
installed packages, MCP resource lists, bundle constants, or tests.
Discover those surfaces from the active project.
Do not assume a particular repository layout unless the active project's entries say to use it.
When a skill resource, wrapper, and the lake disagree,
correct the artifact or wrapper, never the lake.

## Synthesis Workflow

1. Read the roster.
   Read `Sirno.toml` and query the active lake for the skill roster.
   A project may use `agent-skills` as the front door and rostered `*-discipline` entries
   as skill sources.
   Follow the active project's roster instead of assuming those ids exist.
   Query and read its meta entries for shared vocabulary, principles, and method.
   Treat other method entries as shared method unless the roster says they ship as skills.

2. Discover the package surface.
   Read the roster and the discipline entries for artifact paths,
   installed package paths, and MCP resource names.
   If the project has code that embeds or serves those resources, inspect that code before editing.
   Keep one source entry per package.

3. Inspect the current MCP tools.
   List the Sirno MCP tools available in the active server before writing tool names into a skill.
   A full resource that names a missing tool is worse than one that only names the procedure.

4. Render full-resource templates.
   The template operationalizes its discipline plus the shared method it depends on.
   Include the runtime metadata slot when a skill depends on active project fields.
   Add nothing the lake does not commit.
   Preserve explicit shared-resource delegation instead of expanding duplicated checklists.
   Actualizer and internalizer load the editor once per active project context
   and retain their directional decisions and local failure paths.
   The editor remains usable without either supplement.
   Shared principles and failure handling may be supplied by the loaded resource.
   If it is unavailable, reuse a prior read from the same project context;
   otherwise report the missing source and continue only grounded read-only inspection.
   Apply semantic locality when the rendered resource creates, edits, audits, or materializes entries:
   entries should preserve their own local meaning,
   and useful lists should not be confused for semantic contracts.
   A route, index, roster, or review front door may enumerate children
   when that list is part of that entry's local claim.
   Apply immutable update when the rendered resource changes repository material:
   treat the current repository and lake as a snapshot,
   carry forward the facts that still hold,
   and prefer replacing stale local shape over layering around it.
   Preserve shared documentation habits such as reader-aware bullets, diagrams,
   conceptual ordering, and declarative precision when the rendered resource touches prose.
   Cover failure paths for missing sources, unavailable tools, blocked validation,
   absent evidence, and design changes that must be internalized into the lake.
   Defer project configuration maintenance to the editor skill rather than copying its checklist.

5. Render wrappers.
   The wrapper keeps the same frontmatter as the full resource.
   Its body is one sentence that points to the matching MCP resource.
   Copy the wrapper artifact exactly into the installed package.

6. Update exposed surfaces.
   Update any MCP resource list, bundled wrapper list, tests, docs, or install metadata
   that the active project uses.
   Keep human CLI utility maintenance as a CLI surface, not an MCP procedure.
   When utility maintenance is needed, report the human CLI action rather than turning it into an
   agent step.

## Validation

Run `sirno_mist_render` after reservoir metadata changes.
Run `sirno_status` in edit mode and review mode.
Confirm each generated `SKILL.md` and `SKILL.full.template.md` has valid frontmatter.
Compare installed wrappers against their artifacts byte for byte.
Confirm every rostered discipline has the artifacts, resources, and packages
that the active project expects.
Confirm every installed `sirno-*` package traces back to a rostered discipline.

Run the formatter, tests, and checks that fit the active repository.

If validation is blocked, report the blocker and the remaining risk.
If a resource would need behavior the lake does not commit,
leave it out and report the missing design instead of inventing it.
