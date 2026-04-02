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

In the Sirno data representation, an entry is stored as one Markdown file in the data directory. The header contains the entry id, the optional human-readable name, the concise description, and the graph metadata owned by the entry. The body contains the full explanation.

### Edge

Edges connect entries. Every edge is one of two kinds:

- A *dependency* is a directed edge X → Y asserting that Y's validity is contingent on X's content. If X changes, Y must be re-examined. Dependencies encode causal structure.

- An *affinity* is a directed edge between entries that share conceptual relevance. Affinities exist for navigation and epistemic context. They carry no causal force and generate no obligations.

Either kind of edge may optionally refer to an additional entry that explains what the relation means. The attached entry is descriptive metadata. The edge kind and endpoints still determine the operational behavior.

Affinity direction is a data-representation and traversal choice. It determines which entry owns the edge in the Sirno data representation and which adjacency list exposes it first. It does not introduce dependency propagation or any notion of logical consequence.

### Grounding

A grounding maps an entry to locations in the codebase. Groundings are the interpretation function from the abstract graph into concrete syntax. An entry may have zero or more groundings.

Sirno provides two grounding mechanisms:

- *Grep* provides a set of search patterns (regular expressions, literal strings, glob patterns) that locate relevant code regions heuristically. Grep groundings are approximate: they may overapproximate or underapproximate the true set of relevant locations. They are useful for broad exploration and for entries whose relevance is diffuse across the codebase.

- *Telescope* is an anchor-based mechanism that embeds entry identifiers directly into code comments (e.g., `// @sirno:entry-id`). A telescope grounding establishes a nominal binding between an entry and a precise code location. Telescope anchors survive refactoring as long as the comment moves with the code.

Telescope anchors support derived views:

- A *span* is the region between two anchors, or from an anchor to a scope boundary, providing block-level code views without fragile line references.

- A *witness* is a telescope-grounded code region that serves as evidence for an entry's claim. During a rewrite session, an agent verifies that witnesses still substantiate their entries.

Grounding validation is stratified. Structural validation checks invariants expressible in the graph itself, such as anchor ownership. Repository validation checks supported grounding kinds against the codebase. A validator may report unsupported heuristic checks as warnings rather than commit blockers.

### Lifting

Lifting is the inverse of grounding: it constructs or updates an entry from observed code. Where grounding interprets the abstract graph into concrete syntax, lifting abstracts concrete code back into the knowledge graph, creating new entries, revising descriptions, or adjusting dependencies based on what the codebase contains.

Lifting is the primary operation during reflection. An agent examines code (located via grep or telescope), determines what knowledge it embodies, and lifts that knowledge into the graph.

### Obligation

An obligation is a proof burden generated when an entry is mutated. If entry X changes and a dependency X → Y exists, an obligation is created on Y: the claim that Y must be re-examined for consistency with the new X.

An obligation remains pending until an agent discharges it through confirmation, update, or justified deferral.

### Coherence

A graph state is coherent when every obligation has been discharged, every locked-entry mutation has received approval, and every grounding has been validated under the commit-time grounding validator. Coherence is the well-formedness invariant of the knowledge graph: the analogue of well-typedness for the system as a whole.

---

## Sirno Data Representation

A Sirno-managed project has a project root containing `Sirno.toml`. Together with the configured data directory, these files form the Sirno data representation. If the setting is absent, the data directory defaults to `.sirno` relative to the project root.

The Sirno data representation is the durable form of the current Sirno graph. The data directory is a flat directory of Markdown files. Each file represents one entry. The file stem is the entry id, so entry ids are serialized as portable single path segments rather than hierarchical paths. The layout is flat because entry ids are nominal: directory structure does not carry graph meaning.

Each entry file in the Sirno data representation begins with a machine-readable JSON header followed by a Markdown body. The header stores only state with a unique ownership rule. Entry-local fields, groundings, and lock state are owned by the entry itself. A dependency `X → Y` is owned by `X`. An affinity `X ↝ Y` is also owned by `X`. The body stores the entry's explanation text.

The Sirno data representation is the ground truth for a Sirno project. The in-memory graph, sessions, patches, and obligations are operational views derived from it. If Sirno keeps caches or indexes, they are derived data and may be rebuilt from the Sirno data representation.

---

## Operational Model

### Polarity

When an agent works on an entry, it adopts a polarity:

- In *actualization*, the graph is treated as authoritative. The agent rewrites code to match the entry's content.

- In *reflection*, the codebase is treated as authoritative. The agent updates the entry to match observed code.

Polarity is per-entry guidance to the agent, chosen based on the task at hand. Both polarities may coexist within a single session: an agent may reflect some entries while actualizing others. The system does not enforce polarity; it is a convention that structures the agent's reasoning about direction of truth.

### Lock

A lock is a write capability guard on an entry. A locked entry can be read and its obligations can be examined, but mutation requires external approval.

Locks encode trust boundaries. They protect entries whose content has system-wide consequences (core invariants, architectural decisions, stability guarantees) from unreviewed modification.

### Justification

A justification is a record produced when an agent proposes a mutation to a locked entry. It contains the deferred mutation and an argument entry describing why the change is necessary.

A justification is submitted to a reviewer, who grants or withholds approval. The deferred mutation materializes only upon approval.

### Checkpoint

A checkpoint is an immutable snapshot of the entire graph at a moment of coherence. Every checkpoint satisfies the coherence invariant. In a repository-backed deployment, a checkpoint is realized by a coherent snapshot of the Sirno data representation, typically through the host version-control history.

### Patch

A patch is the accumulated record of all proposed mutations during a session. It captures entry edits, entry creation, dependency and affinity changes, and grounding updates. A patch is a pending transaction: it describes the difference between the current Sirno data representation and the intended next checkpoint.

### Session

A session is the working interval between two checkpoints. It loads the current Sirno data representation into a mutable working graph. All mutations during the interval flow through the session, which applies each mutation to the working copy, records it in the patch, and generates obligations when entry content changes. The working state is visible only to the active session; other observers see the last checkpoint.

Only entry-content mutations (updates to an entry's name, description, or explanation, and entry removal) generate obligations. Structural mutations (edge changes, grounding attachments, lock state) do not. When an entry is removed, its dependents are captured before the removal deletes the associated edges.

The session tracks which entries have been examined during obligation discharge as a *visited set*. When a discharge generates obligations on an already-visited entry, the obligation is still created — the entry may need re-examination in a cycle — but the visited set allows the agent to detect re-entry and adjust its strategy. The session provides this state; it does not impose traversal order. The agent drives the iteration.

### Discharge

An agent discharges an obligation through one of four operations:

- *Confirm*: the target entry remains valid. The obligation is marked discharged; the target is added to the visited set.

- *Resolve*: the target entry requires an update. The update is applied through the normal mutation path, which may generate further obligations on its dependents. The obligation is marked discharged; the target is added to the visited set.

- *Justify*: the target entry requires an update but is locked. The agent submits a justification. The obligation transitions to awaiting approval.

- *Approve*: an external reviewer grants approval for a justified mutation. The deferred mutation is applied, generating further obligations as usual. The obligation is marked discharged; the target is added to the visited set. The lock is not rechecked — the approval is the authorization.

### Commit

A patch is promoted to a new checkpoint (committed) when the resulting graph state is coherent.

In practice this requires that all obligations induced by the patch's mutations have been discharged, every mutation to a locked entry has received reviewer approval, and every grounding in the resulting graph has been validated. The commit writes the resulting graph back to the Sirno data representation.

The grounding validator is part of the commit context. A structural validator checks graph-internal invariants only. A repository validator additionally checks supported grounding kinds against the codebase.

---

## Propagation Semantics

When an entry X is mutated within a session:

1. For each dependency edge X → Y, an obligation is generated on Y.
2. The agent examines Y in context of the new X.
3. If Y requires no change, the obligation is discharged.
4. If Y is updated, the obligation is discharged and step 1 recurs with Y.
5. If Y is locked, the agent produces a justification and the obligation remains pending until approval is granted and the update is applied.

Propagation follows dependency edges in their declared direction in both polarities. Reflection changes the source of truth: the agent starts from code observations, lifts them into entries, and then propagates obligations to downstream dependents in the same way as actualization.

For cyclic dependencies, the entries in a strongly connected component must be re-examined collectively. Obligations within a cycle are discharged as a group once the component reaches a consistent fixed point.

---

## Summary of Concepts

| Concept       | Role                                                   |
| ------------- | ------------------------------------------------------ |
| Entry         | Primitive knowledge unit with nominal identity         |
| Dependency    | Directed causal edge; validity contingency             |
| Affinity      | Directed navigational edge; epistemic context          |
| Grounding     | Entry-to-code mapping (grep or telescope)              |
| Lifting       | Code-to-entry abstraction; inverse of grounding        |
| Witness       | Telescope-grounded evidence for an entry's claim       |
| Obligation    | Proof burden from mutation, propagated along edges     |
| Coherence     | Well-formedness invariant on the graph state           |
| Polarity      | Per-entry direction-of-authority guidance              |
| Lock          | Write capability guard requiring reviewer approval     |
| Justification | Deferred locked-entry mutation plus its argument entry |
| Checkpoint    | Immutable coherent snapshot of the full graph          |
| Patch         | Pending transaction accumulating session mutations     |
| Session       | Working interval between checkpoints                   |
| Commit        | Promotion of a patch to a new checkpoint               |
