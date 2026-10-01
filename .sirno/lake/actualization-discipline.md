---
desc: The agent procedure for shaping repository material from the configured lake.
name: Actualization Discipline
category:
  - meta
belongs:
  - agent-skills
prerequisite:
  - agent-skills
  - repository-editing-discipline
  - portable-agent-skill-language
  - methodology
  - semantic-locality
  - actualize
  - witness
  - project-config
refines:
  - actualize
---

Actualization shapes repository material from the configured lake.

This discipline owns the lake-to-repository direction.
Use it when the governing entry is known,
or when a compact entry can be named before the repository change settles.
The repository change may be source, tests, generated artifacts, configuration,
skill artifacts, README prose, or another non-lake document.

`repository-editing-discipline` owns the shared edit method:
orientation, structural relation reading, witness inspection,
configuration alignment, documentation sync, validation, Tide handling, and staging.
The actualizer loads `sirno://skills/sirno-editor` once for the active project context
and applies that method to the requested scope.
Its resource adds directional decisions rather than repeating the shared procedure.
If the shared resource is unavailable, reuse a prior read from the same project context;
otherwise report the missing method and continue only grounded read-only inspection.
Actualization adds the directional rule:
the entry is the design source,
and the repository material is shaped to match it.

When repository work reveals a durable design fact that the lake does not yet name,
hand off to `internalization-discipline`.
Return to actualization after the entry exists.
If the governing design remains unresolved, report the missing claim and defer dependent edits.
When the request mixes directions or the direction is unclear,
use `repository-editing-discipline` as the front door.

Actualization should keep the repository edit narrow.
The entry holds the claim.
The implementation, test, configuration, document, artifact, or witness block supplies evidence
or material form for that claim.
Add or refine witness blocks only around stable evidence.
Do not add placeholder witnesses.

The static full-resource template lives in
`.artifacts/actualization-discipline/SKILL.full.template.md`
and is rendered by `src/mcp.rs` as `sirno://skills/sirno-actualizer`.
The packaged wrapper lives in `.artifacts/actualization-discipline/SKILL.md`
and renders to `.agents/skills/sirno-actualizer/SKILL.md`.
Those artifacts operationalize this discipline and the shared editing method;
the lake entry remains the source of the durable directional claim.
