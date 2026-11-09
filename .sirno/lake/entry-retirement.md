---
desc: The proposed mechanism that decides when an entry needs notice, an update, or removal.
name: Entry Retirement
category:
  - concept
  - proposal
belongs:
  - entry
  - lake-review
prerequisite:
  - entry-lifecycle
  - witness
  - ripple
  - tide
  - anchor-file
  - deprecated
  - proposal
---

*Entry retirement* is the proposed mechanism that ties every *entry* to an observable reason to exist.
Sirno watches that reason.
When it changes, the *entry* needs notice.
When it contradicts the *entry*, the *entry* needs an update.
When it is gone, the *entry* leaves the *lake*.

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
- Every trigger is a change in *lake* or *repository* state,
  so each trigger names what changed.
- Sirno evaluates triggers from metadata, structure, and *witnesses*.
  It does not parse prose.
- Retirement ends in deletion.
  The *lake* has no archive tier; Git is the archive.
- A reviewer may acknowledge a judgment, but not a contradiction.

## Outcomes

| Outcome | Trigger | Response | Exit |
|---|---|---|---|
| Notice | A *ground* changed. | Review obligation. | A reviewer accepts the change, or the *entry* is edited. |
| Update | A *ground* contradicts the *entry*. | Edit warning, review error. | The *entry* or *repository* changes. |
| Retire | No *ground* remains. | Edit warning, review error. | The *entry* is deleted or gains a *ground*. |

A notice asks whether a claim still holds, which takes a reviewer's judgment.
Its resolution covers only the exact change the reviewer saw, as *tide resolutions* do today.
Update and retire states are facts Sirno can observe, so only an edit clears them.
No outcome depends on a deadline or a renewal.

Candidate update triggers:

- a `proposal` *entry* gains a *witness*;
- an `implemented` *entry* loses its last *witness*;
- a structural link target is deleted;
- a superseded *entry* keeps its *witnesses* after its successor is witnessed.

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
An *entry* that grounding never reaches is ungrounded.

A trial with only evidence, dependents, and routes
left relation entries and skill-discipline entries ungrounded,
although both carry live design.
Schema use and owned output close those two gaps.
The *entries* still ungrounded after that had neither incoming links nor *witnesses*.

## Evidence Notices

Anchor fingerprints *entries* and their artifact trees, but not *witness* regions.
A code change inside a *witness* region reaches no *entry* today,
so an *entry* can keep describing behavior its code no longer has.

Entry retirement adds an evidence fingerprint to each Anchor *entry* record.
A changed evidence fingerprint is an evidence *ripple* on the owning *entry*,
and it produces a notice.

## Transitional Design

Transitional *entries* describe design in motion.
Each kind is grounded by something that ends when the motion ends:

- A proposal is grounded by the *entries* it amends.
  It gets a notice when an amended *entry* changes,
  needs an update when it gains a *witness*,
  and retires when every amended *entry* is gone.
- A superseded design is grounded by its remaining *witnesses*.
  Once its successor is witnessed, those *witnesses* contradict the design direction.
  Removing the legacy material removes the last ground, so the *entry* retires.
  Legacy material that must stay is a current requirement and gets a standing *entry*.
- A future-work reservation becomes a small proposal with amended *entries*.

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

Evidence:

- Which *entries* receive an evidence notice?
  - The owning *entry* only.
  - The owning *entry* and its Tide neighbors.
- How precise is the evidence fingerprint?
  - One fingerprint per *entry*, or one per region.
  - Exact bytes, or whitespace-normalized text.

Transitions:

- How is supersession expressed?
  - A `supersedes` relation.
  - The `deprecated` category plus a successor link.
  - No lasting state: removal is a pending edit that blocks review.
- How severe is a landed supersession that still has legacy *witnesses*?
  - Review error.
  - Warning.
  - Tide workitem.
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
It amends `entry-lifecycle`, `ripple`, `tide`, `anchor-file`, `deprecated`, and `proposal`.
It lists them as prerequisites,
so a change to any of them gives this *entry* a Tide notice today.
It should leave the *lake* once its design lands in the *entries* it amends.
