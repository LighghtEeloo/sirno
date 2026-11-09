---
desc: The proposed first-class mechanism that watches lake and repository state and reports findings about entries.
name: Observer
category:
  - concept
  - proposal
belongs:
  - lake-review
prerequisite:
  - anchor-file
  - ripple
  - tide
  - tide-workitem
  - tide-resolution
  - structural-check
  - structural-edge-policy
  - meta-type
  - query
  - witness
  - transform
  - spell-invocation
---

An *observer* is a declared rule that watches part of the *lake* or *repository*
and reports findings about *entries*.
This proposal makes observation the one mechanism behind review obligations, structural checks,
and *entry retirement*.
Tide becomes the first declared observer on top of a built-in review core.

## Change And State

Observation reads one of two inputs:

| Kind | Reads | Findings | Example today |
|---|---|---|---|
| Change observation | The accepted baseline and the current state | Notices | *Tide workitems* |
| State observation | The current state only | Updates and retirements | Dangling structural link targets |

A change finding is tied to the fingerprint of the change that caused it.
A reviewer can acknowledge it, and it reopens when that change moves again.
A state finding has no change to tie an acknowledgment to, so only an edit clears it.

## Outcomes

| Outcome | Meaning | Edit mode | Review mode | Exit |
|---|---|---|---|---|
| Notice | A watched subject changed. | Open item | Blocks acceptance | Acknowledgment or an edit. |
| Update | A watched subject contradicts the *entry*. | Warning | Error | The *entry* or *repository* changes. |
| Retire | The *entry* has no reason to exist. | Warning | Error | Deletion, or the reason returns. |

A notice asks whether a claim still holds, which takes a reviewer's judgment.
A notice whose subject also changed in the same edit may be inferred as reviewed,
as `sirno resolve --infer` does today.
Update and retire findings are facts Sirno can observe, so only an edit clears them.
No outcome depends on a deadline or a renewal.

## Built-In Core

The core holds what every observer must agree on and what keeps observation binding:

1. **Well-formedness.**
   Parsing, *entry addresses*, metadata shape, and *witness* delimiters stay hard errors.
   Observation needs a valid view of the *lake* before it can start.
2. **One view and one baseline.**
   The view covers *entries*, *entry artifacts*, and *witness* regions.
   Anchor fingerprints every subject an observer reads.
3. **Fixed outcome rules.**
   An observer chooses an outcome, not the rules of that outcome.
   A declared softer outcome would let retained content leave review.
4. **One review record and one gate.**
   Acknowledged notices from every observer share one record.
   Acceptance requires no open notice and no update or retire finding.
5. **Grounding.**
   The grounding observer always runs.
   *Entry retirement* defines what it watches.

The gate gives every accepted baseline one invariant:
it has no open notice, no contradiction, and no ungrounded *entry*.

## Declared Observers

A declared observer is an *entry*.
One candidate shape gives it three parts:

- a selector that names the watched *entries*, using the *query* selector;
- a predicate from a small built-in set,
  such as `changed`, `neighbor-changed`, `evidence-changed`, `witnessed`, `unwitnessed`, and `target-missing`;
- an outcome: notice, update, or retire.

Lifecycle rules are declared rather than built in.
Categories such as `proposal`, `implemented`, and `deprecated` are project vocabulary,
so a rule such as "a witnessed proposal needs an update" belongs to the project.
Observers are *entries*, so changing an observer is a *ripple* that Tide reviews.

Predicates outside the built-in set can come from spells.
An observe hook would pass the view and the change set to a spell and read findings back.
The core applies the same outcome rules to spell findings.
This depends on the hook contract in *spell invocation*.

## Tide As An Observer

Tide splits along the core boundary:

| Tide part | Home |
|---|---|
| Anchor baseline | Core, widened to every observed subject. |
| *Ripple* detection and fingerprints | Core, widened from *entries* to *witness* regions. |
| Propagation along relation edges | The structural observer, configured by `meta.ripple` on relation entries. |
| *Tide resolutions* and inference | Core review record, shared by every observer. |
| The clear-Tide gate on Anchor update | Core gate, shared by every finding. |
| `sirno tide` commands, TUI, and MCP tools | A review surface over all findings. |

The *tide workitem* tuple `(ripple, field, direction, neighbor)` shows the seam.
`field` and `direction` are settings of the structural observer.
A general finding is identified by `(observer, cause, subject)`
and tied to the fingerprint of its cause.

Structural checks split the same way.
Dangling targets and unmarked category targets are state observations with an update outcome,
reported as *diagnostics*.
Parse and address failures stay in the well-formedness layer.

## Evidence Observation

Anchor fingerprints *entries* and their artifact trees, but not *witness* regions.
A code change inside a *witness* region reaches no *entry* today,
so an *entry* can keep describing behavior its code no longer has.

With *witness* regions in the baseline,
evidence observation can run in both *transform* directions:

- A changed region gives its *entry* a notice, which is the internalize direction.
- A changed *entry* gives its *witness* regions a notice, which is the actualize direction.

The actualize direction needs findings whose subject is a *repository* region rather than an *entry*.

## Open Design Space

Each question has more than one defensible answer.
The options are candidates, not a ranking.

Declaration:

- How is an observer declared?
  - An *entry* with `meta.type: "observer"` and selector, predicate, and outcome fields.
  - Policy fields on the *entries* it concerns, as `meta.ripple` sits on relation entries.
  - Both: policy fields for common rules and observer *entries* for the rest.
- How large is the built-in predicate set?
  - Minimal, with spells for everything else.
  - Broad enough that projects rarely need spells.
- How does review present a changed observer?
  - Like any changed *entry*.
  - With the findings the change adds and removes.

Outcomes:

- Is there a hint level that never blocks?
  - No: every finding is a notice, an update, or a retirement.
  - Yes, for advice such as list-valued metadata without a relation *entry*.
- Can a state finding be waived?
  - No: only an edit clears it.
  - Yes, against the subject's fingerprint, so the waiver reopens when the subject changes.

Change observation:

- Which side does a change observer read?
  - Each observer declares the waterline, Anchor, or both, as `meta.ripple.lake` and `meta.ripple.anchor` do.
  - The core always reads both sides.
- Do waves stay in the model?
  - Yes, as the unit of propagation from one cause.
  - No, only as a display grouping by cause.

Evidence:

- What identifies a *witness* region in Anchor?
  - *Entry address* and content.
  - *Entry address*, file path, and content.
- How precise is the evidence fingerprint?
  - Exact bytes.
  - Whitespace-normalized text.
- Which *entries* receive an internalize notice?
  - The owning *entry* only.
  - The owning *entry* and its structural neighbors.
- Which directions run by default?
  - Internalize only.
  - Both directions.

Naming:

- What does the name Tide cover?
  - The whole review record.
  - Only the structural observer.

## Self-Application

This *entry* is transitional.
It amends `anchor-file`, `ripple`, `tide`, `tide-workitem`, `tide-resolution`,
`structural-check`, `structural-edge-policy`, and `meta-type`.
It lists them as prerequisites,
so a change to any of them gives this *entry* a Tide notice today.
It should leave the *lake* once its design lands in the *entries* it amends.
