---
desc: The proposed policy that keeps every entry grounded and removes entries whose grounds are gone.
name: Entry Retirement
category:
  - concept
  - proposal
belongs:
  - entry
  - lake-review
prerequisite:
  - observer
  - entry-lifecycle
  - witness
  - deprecated
  - proposal
---

*Entry retirement* is the proposed policy that ties every *entry* to an observable reason to exist.
That reason is the entry's *ground*.
The built-in grounding *observer* watches grounds,
and declared observers watch the transitional categories.
Together they decide when an *entry* needs notice, needs an update, or leaves the *lake*.

The *lake* states the current design, and Git keeps every earlier state.
Retired knowledge stays recoverable without staying in a reader's context.
Agents tend to keep text that is already present,
and text that stays in context steers what they write next.
Entry retirement makes removal a check result
rather than a judgment an agent must volunteer.

## Commitments

These rules hold across every option in the open design space:

- Every *entry* participates.
  No category or field opts out.
- Grounding always runs.
  A project declares what counts as a ground, not whether grounding applies.
- Every trigger is a change in *lake* or *repository* state,
  so each trigger names what changed.
- Retirement ends in deletion.
  The *lake* has no archive tier; Git is the archive.

## Grounds

A *ground* is an observable reason for an *entry* to exist.
Candidate ground kinds:

| Ground | The *entry* is grounded when |
|---|---|
| Evidence | A *repository witness* block names it. |
| Dependent | A grounded *entry* links to it through a grounding relation. |
| Schema use | *Entries* use it as a metadata field, as with relation and intrinsic entries. |
| Route | It is a declared reading route. |
| Owned output | It renders *repository* material, such as a packaged skill. |

Grounding spreads from roots along grounding relations,
the way a tracing collector marks live objects.
Evidence, schema use, routes, and owned output are roots.
An *entry* that grounding never reaches gets a retire finding.

A trial with only evidence, dependents, and routes
left relation entries and skill-discipline entries ungrounded,
although both carry live design.
Schema use and owned output close those two gaps.
The *entries* still ungrounded after that had neither incoming links nor *witnesses*.

## Transitional Design

Transitional *entries* describe design in motion.
Each kind is grounded by something that ends when the motion ends,
and a declared observer watches for that end:

- A proposal is grounded by the *entries* it amends.
  It gets a notice when an amended *entry* changes,
  an update finding when it gains a *witness*,
  and a retire finding when every amended *entry* is gone.
- A superseded design is grounded by its remaining *witnesses*.
  Once its successor is witnessed, those *witnesses* contradict the design direction.
  Removing the legacy material removes the last ground, so the *entry* retires.
  Legacy material that must stay is a current requirement and gets a standing *entry*.
- A future-work reservation becomes a small proposal with amended *entries*.

Standing categories can carry declared rules too.
An `implemented` *entry* that loses its last *witness* is a candidate for an update finding.

Entry retirement acts on whole *entries*.
Transitional statements therefore live in transitional *entries*.
A standing *entry* describes the present
and reaches transitional *entries* only through structural links,
so retiring a transitional *entry* surfaces every reference to it.

## Open Design Space

Each question has more than one defensible answer.
The options are candidates, not a ranking.

Grounding:

- How is a ground declared?
  - Derived from existing structure, with no new field.
  - An explicit per-entry field.
  - Derived, with explicit overrides.
- Which relations carry grounding?
  - Every structural relation.
  - Only `prerequisite` and `refines`.
  - A per-relation policy on the relation entry, beside `meta.ripple`.
- How are routes declared?
  - The `narrative` category.
  - A list in `Sirno.toml`.
  - A per-entry marker.
  - A *query* selector, which can ground every *entry* at once.

Transitions:

- How is supersession expressed?
  - A `supersedes` relation.
  - The `deprecated` category plus a successor link.
  - No lasting state: removal is a pending edit that blocks review.
- How severe is a landed supersession that still has legacy *witnesses*?
  - An update finding.
  - A notice.
- How does a proposal name what it amends?
  - A new `amends` relation with a `from` ripple policy.
  - The existing `prerequisite` relation, whose `from` policy already produces notices.
  - The existing `refines` relation.
- Are lifecycle categories authored or derived?
  - Authored, as today.
  - Derived from grounds, so `implemented` means witnessed.

Presentation and scope:

- What does the default mist show?
  - Every *entry*.
  - Current design only, with proposals in a planning mist.
  - Every *entry* except ungrounded ones.
- How is stale prose inside a standing *entry* caught?
  - Curation discipline only.
  - A warning when a retired *entry*'s name still appears in live prose.
- How do glacier *entries* retire?
  - Upstream owns retirement, and local checks only report.
  - Local checks block like any other *entry*.

## Self-Application

This *entry* is transitional.
It amends `entry-lifecycle`, `deprecated`, and `proposal`,
and it builds on `observer` for findings and outcomes.
It lists them as prerequisites,
so a change to any of them gives this *entry* a Tide notice today.
It should leave the *lake* once its design lands in the *entries* it amends.
