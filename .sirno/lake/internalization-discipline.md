---
desc: The agent procedure for recording durable design facts from repository material into the lake.
name: Internalization Discipline
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
  - internalize
  - witness
refines:
  - internalize
---

Internalization records durable design facts from repository material into the configured lake.

This discipline owns the repository-to-lake direction.
Use it when source, tests, generated artifacts, configuration,
or non-lake documentation reveal a stable boundary, invariant, representation,
behavior, contract, or policy that future work should reason through.

`repository-editing-discipline` owns the shared edit method:
orientation, structural relation reading, witness inspection,
configuration alignment, documentation sync, validation, Tide handling, and staging.
The internalizer loads `sirno://skills/sirno-editor` once for the active project context
and applies that method to the requested scope.
Its resource adds directional decisions rather than repeating the shared procedure.
If the shared resource is unavailable, reuse a prior read from the same project context;
otherwise report the missing method and continue only grounded read-only inspection.
Internalization adds the directional rule:
the repository supplies evidence,
but the fact becomes canonical only when an entry names it.

Internalization rejects changelog material.
Commit history can carry the sequence of edits.
The lake should carry the design that survived the edit
and will shape later work.
If source material is unavailable, report the gap and internalize only facts grounded in available evidence.

Choose the closest existing entry when it already names the fact.
Create a new compact entry when the fact belongs to a new boundary.
Apply semantic locality:
the entry should state the fact directly,
remain meaningful without its neighbors,
and use structural links only for navigation, review, or accountability.

Connect evidence without manufacturing it.
When repository evidence already exists,
add or refine a precise witness block around the smallest stable region that supports the entry.
If the entry holds prose only or evidence is not yet in the repository,
leave it unwitnessed and let actualization wire evidence later.

The static full-resource template lives in
`.artifacts/internalization-discipline/SKILL.full.template.md`
and is rendered by `src/mcp.rs` as `sirno://skills/sirno-internalizer`.
The packaged wrapper lives in `.artifacts/internalization-discipline/SKILL.md`
and renders to `.agents/skills/sirno-internalizer/SKILL.md`.
Those artifacts operationalize this discipline and the shared editing method;
the lake entry remains the source of the durable directional claim.
