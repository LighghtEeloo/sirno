---
name: sirno-internalizer
description: >-
  Internalize durable repository facts into the configured lake when material reveals design knowledge.
---

# Sirno Internalizer

## Purpose

Use this skill for the repository-to-lake direction of repository work.
Record durable design facts from source, tests, generated artifacts, configuration,
or non-lake documentation into the active project's Sirno Lake.
The repository supplies evidence; the fact becomes canonical when an entry names it.

When the work primarily shapes repository material from an existing entry,
use `sirno://skills/sirno-actualizer`.
When the direction is mixed or unclear, use `sirno://skills/sirno-editor`.

This full skill text is served as `sirno://skills/sirno-internalizer`.

{{SIRNO_ACTIVE_PROJECT_METADATA}}

## Shared Method

Read `sirno://skills/sirno-editor` once for the active project context before doing skill work.
Reuse a current read from that context.
Follow its project binding, orientation, entry-writing, evidence, configuration,
documentation, validation, Tide, frozen-entry, and staging procedure for the requested scope.
This resource supplies the directional decisions below.

If the shared resource is unavailable, reuse a prior read from the same project context.
Otherwise report the missing method and continue only grounded read-only inspection
until the shared source is available.

## Directional Decisions

1. Identify the durable fact in the source material.
   Keep boundaries, invariants, representations, behaviors, contracts, and policies
   that future work should reason through.
   Leave the sequence of edits to commit history.
   If source material is unavailable, report the gap and use only facts grounded in available evidence.

2. Choose the closest entry that names the fact, including frozen entries.
   Revise it when the claim has become sharper.
   Create an entry for a new boundary, or rename one whose address no longer fits,
   using the editor's entry procedure.

3. Connect existing evidence through the editor's witness procedure.
   If the entry holds prose only or its evidence is not yet in the repository,
   leave it unwitnessed until actualization supplies evidence.

4. When migrating claims from non-lake documents, internalize them first.
   Actualize the published form from those entries when that update is part of the request.

Finish with the editor's validation and scoped staging procedure.
