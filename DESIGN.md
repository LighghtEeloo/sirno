# Sirno

*Semantic Intermediate Representation of Nominal Obligations*

Sirno is a graph-shaped knowledge database for codebases. It mediates between abstract design knowledge and concrete code through a structured graph of named, agent-maintained knowledge units. Agents consult and update the graph as part of any code-touching operation, keeping design and implementation in agreement.

---

## Core Concepts

### Entry

An entry is the primitive unit of knowledge in Sirno. Each entry carries:

- An *id* is a unique, agent-assigned nominal identifier.
- A *name* is an optional human-readable concept name.
- A *description* concisely summarizes the entry's claim.
- An *explanation* gives a full account of the entry's content, rationale, and context.

An entry represents a single claim about the codebase: an invariant, a design decision, a module's purpose, a data representation choice, or any other isolable piece of understanding. Entries are self-contained in the sense that their explanation is intelligible on its own, though their full significance may involve their position in the graph.

Entries are the only durable owner of dedicated explanatory text in the model. When another concept needs prose, it refers to an entry. Non-entry strings are reserved for operational syntax such as search patterns.

### Edge

Edges connect entries. Every edge is one of two kinds:

- A *dependency* is a directed edge X → Y asserting that Y's validity is contingent on X's content. If X changes, Y must be re-examined. Dependencies encode causal structure.

- An *affinity* is an undirected edge between entries that share conceptual relevance. Affinities exist for navigation and epistemic context. They carry no causal force and generate no obligations.

Either kind of edge may optionally refer to an additional entry that explains what the relation means. The attached entry is descriptive metadata. The edge kind and endpoints still determine the operational behavior.

### Grounding

A grounding maps an entry to locations in the codebase. Groundings are the interpretation function from the abstract graph into concrete syntax. An entry may have zero or more groundings.

Sirno provides two grounding mechanisms:

- *Grep* provides a set of search patterns (regular expressions, literal strings, glob patterns) that locate relevant code regions heuristically. Grep groundings are approximate: they may overapproximate or underapproximate the true set of relevant locations. They are useful for broad exploration and for entries whose relevance is diffuse across the codebase.

- *Telescope* is an anchor-based mechanism that embeds entry identifiers directly into code comments (e.g., `// @sirno:entry-id`). A telescope grounding establishes a nominal binding between an entry and a precise code location. Telescope anchors survive refactoring as long as the comment moves with the code.

Telescope anchors support derived views:

- A *span* is the region between two anchors, or from an anchor to a scope boundary, providing block-level code views without fragile line references.

- A *witness* is a telescope-grounded code region that serves as evidence for an entry's claim. During a rewrite session, an agent verifies that witnesses still substantiate their entries.

### Lifting

Lifting is the inverse of grounding: it constructs or updates an entry from observed code. Where grounding interprets the abstract graph into concrete syntax, lifting abstracts concrete code back into the knowledge graph, creating new entries, revising descriptions, or adjusting dependencies based on what the codebase contains.

Lifting is the primary operation during reflection. An agent examines code (located via grep or telescope), determines what knowledge it embodies, and lifts that knowledge into the graph.

### Obligation

An obligation is a proof burden generated when an entry is mutated. If entry X changes and a dependency X → Y exists, an obligation is created on Y: the claim that Y must be re-examined for consistency with the new X.

Obligations propagate along dependency edges. When an agent discharges an obligation on Y, possibly mutating Y in the process, new obligations may arise on entries downstream of Y, and so on until a fixed point is reached.

An obligation is discharged by an agent either confirming the downstream entry remains valid, or updating it and propagating further.

### Coherence

A graph state is coherent when every obligation has been discharged and every grounding accurately locates its code. Coherence is the well-formedness invariant of the knowledge graph: the analogue of well-typedness for the system as a whole.

---

## Operational Model

### Polarity

When an agent works on an entry, it adopts a polarity:

- In *actualization*, the graph is treated as authoritative. The agent rewrites code to match the entry's content.

- In *reflection*, the codebase is treated as authoritative. The agent updates the entry to match observed code.

Polarity is per-entry guidance to the agent, chosen based on the task at hand. Both polarities may coexist within a single session: an agent may reflect some entries while actualizing others. The system does not enforce polarity; it is a convention that structures the agent's reasoning about direction of truth.

### Lock

A lock is a write capability guard on an entry. A locked entry can be read and its obligations can be examined, but mutation requires external approval.

An agent that needs to mutate a locked entry produces a *justification*: a record containing the deferred mutation and an argument entry describing why the change is necessary. The justification is submitted to a reviewer, who grants or withholds approval. The mutation materializes only upon approval.

Locks encode trust boundaries. They protect entries whose content has system-wide consequences (core invariants, architectural decisions, stability guarantees) from unreviewed modification.

### Checkpoint

A checkpoint is an immutable snapshot of the entire graph at a moment of coherence. Every checkpoint satisfies the coherence invariant. Checkpoints are the durable states of the knowledge graph; all prior states remain accessible.

### Patch

A patch is the accumulated record of all proposed mutations during a session. It captures entry edits, entry creation, dependency and affinity changes, and grounding updates. A patch is a pending transaction: it describes the difference between the current checkpoint and the intended next checkpoint.

### Session

A session is the working interval between two checkpoints. It maintains a mutable working copy of the base graph. All mutations during the interval flow through the session, which applies each mutation to the working copy, records it in the patch, and generates obligations when entry content changes. The working state is visible only to the active session; other observers see the last checkpoint.

Only entry-content mutations (updates to an entry's name, description, or explanation, and entry removal) generate obligations. Structural mutations (edge changes, grounding attachments, lock state) do not. When an entry is removed, its dependents are captured before the removal deletes the associated edges.

The session tracks which entries have been examined during obligation discharge as a *visited set*. When a discharge generates obligations on an already-visited entry, the obligation is still created — the entry may need re-examination in a cycle — but the visited set allows the agent to detect re-entry and adjust its strategy. The session provides this state; it does not impose traversal order. The agent drives the iteration.

### Discharge

An agent discharges an obligation through one of four operations:

- *Confirm*: the target entry remains valid. The obligation is marked discharged; the target is added to the visited set.

- *Resolve*: the target entry requires an update. The update is applied through the normal mutation path, which may generate further obligations on its dependents. The obligation is marked discharged; the target is added to the visited set.

- *Justify*: the target entry requires an update but is locked. The agent submits a justification (the proposed mutation and an argument entry). The obligation transitions to awaiting approval.

- *Approve*: an external reviewer grants approval for a justified mutation. The deferred mutation is applied, generating further obligations as usual. The obligation is marked discharged; the target is added to the visited set. The lock is not rechecked — the approval is the authorization.

### Commit

A patch is promoted to a new checkpoint (committed) when it satisfies two conditions:

- *Obligation-completeness* requires that all obligations induced by the patch's mutations have been discharged. Every dependency chain has been followed to a fixed point.

- *Approval-completeness* requires that every mutation to a locked entry within the patch has received reviewer approval.

Both conditions must hold simultaneously.

---

## Propagation Semantics

When an entry X is mutated within a session:

1. For each dependency edge X → Y, an obligation is generated on Y.
2. The agent examines Y in context of the new X.
3. If Y requires no change, the obligation is discharged.
4. If Y is updated, the obligation is discharged and step 1 recurs with Y.
5. If Y is locked, the agent produces a justification and the obligation remains pending until approval is granted and the update is applied.

Propagation follows dependency edges in their declared direction during actualization. During reflection, when a grounding change is lifted into an entry, obligations may propagate against the dependency direction: entries that depend on the changed entry must also be checked. The dependency graph thus carries bidirectional operational meaning, with edges defining validity contingency in one direction and change-notification in both.

For cyclic dependencies, the entries in a strongly connected component must be re-examined collectively. Obligations within a cycle are discharged as a group once the component reaches a consistent fixed point.

---

## Summary of Concepts

| Concept       | Role                                                      |
|---------------|-----------------------------------------------------------|
| Entry         | Primitive knowledge unit with nominal identity            |
| Dependency    | Directed causal edge; validity contingency                |
| Affinity      | Undirected navigational edge; epistemic context           |
| Grounding     | Entry-to-code mapping (grep or telescope)                 |
| Lifting       | Code-to-entry abstraction; inverse of grounding           |
| Witness       | Telescope-grounded evidence for an entry's claim          |
| Obligation    | Proof burden from mutation, propagated along edges        |
| Coherence     | Well-formedness invariant on the graph state              |
| Polarity      | Per-entry direction-of-authority guidance                 |
| Lock          | Write capability guard requiring reviewer approval        |
| Justification | Deferred locked-entry mutation plus its argument entry    |
| Checkpoint    | Immutable coherent snapshot of the full graph             |
| Patch         | Pending transaction accumulating session mutations        |
| Session       | Working interval between checkpoints                      |
| Commit        | Promotion of a patch to a new checkpoint                  |
