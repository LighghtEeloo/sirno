# Sirno

*Semantic Intermediate Representation of Nominal Objects*

<!-- sirno:witness:readme:begin -->
Sirno keeps project design in small, named Markdown entries and connects those entries to repository evidence.
Its Rust library, CLI, and MCP server let people and agents query the design,
inspect its implementation, and review related entries when it changes.

- A *lake* holds the project's design entries and structural links.
- A *witness* identifies code, tests, configuration, or assets that support an entry's claim.
- *Tide* lists related entries to review after a design change.
- *Anchor* records the accepted design baseline.

The entry states the claim. The witness locates evidence. People judge whether the evidence supports the claim.
<!-- sirno:witness:readme:end -->

<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="assets/sirno-20260401.png" width="40%">
    <source media="(prefers-color-scheme: light)" srcset="assets/sirno-nb-20260401.png" width="40%">
    <img src="assets/sirno-nb-20260401.png" width="40%" alt="Sirno mascot">
  </picture>
</p>

<!-- sirno:witness:readme:begin -->
## Quick Start

Install Sirno with Rust's package manager:

```sh
cargo install sirno
```

<!-- sirno:quickstart:begin -->
The example below creates a small project, witnesses its greeting implementation,
and completes a design change through review and baseline acceptance.
Run the steps in order in a new directory.

### 1. Create the design

```sh
mkdir sirno-demo
cd sirno-demo
sirno init --all
mkdir src
printf '\n[repo]\nmembers = ["src"]\n' >> Sirno.toml

sirno entry new greeting \
  --intrinsic name="Greeting" \
  --intrinsic desc="The command's greeting"
printf '\nThe command prints a greeting to standard output.\n' >> .sirno/lake/greeting.md
```

Initialization creates `Sirno.toml`, six seed entries in `.sirno/lake/`, and agent skill wrappers.
The `[repo]` section selects source files for witness lookup.

Each project defines its structural relations.
Add a `prerequisite` relation whose `from` policy asks dependent entries to be reviewed when their prerequisite changes:

```sh
cat > .sirno/lake/prerequisite.md <<'ENTRY'
---
name: Prerequisite
desc: Knowledge an entry depends on.
meta.type: "structural"
meta.ripple.lake: ["from"]
meta.ripple.anchor: ["from"]
---

When a prerequisite changes, review entries that depend on it.
ENTRY

sirno entry new usage \
  --intrinsic name="Usage" \
  --intrinsic desc="Running the greeting command" \
  --structural prerequisite=greeting
printf '\nRun `node src/greeting.js` to print the greeting.\n' >> .sirno/lake/usage.md
```

`usage` now depends on `greeting`.
The lake contains nine entries: the six seeds and the three just created.

### 2. Witness the implementation and accept a baseline

Write a JavaScript source file with a witness block, then inspect it:

```sh
printf '%s\n' \
  '// sirno:witness:greeting:begin' \
  'console.log("Hello, world.");' \
  '// sirno:witness:greeting:end' > src/greeting.js
sirno entry witness greeting --full
sirno mist render
sirno anchor update
```

Witness output shows the marked region in `src/greeting.js`.
Rendering creates the browsable `sirno-lake/` projection.
The first Anchor update accepts the initial design and reports:

```text
anchored 9 entries in .sirno/anchor.toml
```

### 3. Change the design and inspect the review work

Require the greeting to include the project name, and update the source:

```sh
printf '\nThe greeting includes the project name.\n' >> .sirno/lake/greeting.md
printf '%s\n' \
  '// sirno:witness:greeting:begin' \
  'console.log("Hello, Sirno.");' \
  '// sirno:witness:greeting:end' > src/greeting.js
sirno tide status
```

Tide lists `usage` with reason `greeting` and reports one open workitem.
`sirno tide status` exits with code 1 while that review is pending.
Read the dependent entry and inspect the updated evidence:

```sh
cat .sirno/lake/usage.md
sirno entry witness greeting --full
```

The instruction in `usage` still runs the same command,
and the witness now prints `Hello, Sirno.`.
The dependent entry needs no change.

### 4. Complete review and accept the change

```sh
sirno tide resolve usage
sirno mist render
sirno anchor update
sirno status --mode review
```

The final command reports:

```text
status clean: 9 entries in .sirno/lake
```

Resolving records the review; Anchor accepts the current design and clears the completed Tide state.
If the design changes again before acceptance, its affected review obligations reopen.
<!-- sirno:quickstart:end -->
<!-- sirno:witness:readme:end -->

<!-- sirno:witness:readme:begin -->
## Where to Edit

`.sirno/lake/` is the canonical design source.
The quick start edits those files directly and runs `sirno mist render` to refresh `sirno-lake/`.

`sirno-lake/` is an editable projection with generated navigation links.
To work there, render it first, edit its entry prose or metadata,
then run `sirno mist status` and `sirno mist intake` to bring the edits into the reservoir.
Intake also refreshes the projection.
Choose one editing surface for a change and keep Sirno-owned footer regions intact.

`sirno status --mode edit` checks work in progress; `--mode review` applies the review boundary.
Status reports pending review and projection work as well as structural errors,
and suggests the next command when work remains.
Use `sirno status --show normal` for separate lake, Tide, Anchor, and mist details.
Use `sirno --help` for the generated command inventory,
and `sirno COMMAND --help` for a command's flags and subcommands.
<!-- sirno:witness:readme:end -->

<!-- sirno:witness:readme:begin -->
## Use Sirno with an Agent

`sirno init --all` installs wrappers in `.agents/skills/` that load project-aware instructions from Sirno MCP.
Use `sirno init` for interactive setup,
or add `--claude-skills` to link wrappers into `.claude/skills/`.

Register the stdio server from the project root with your agent:

```sh
# Codex
codex mcp add sirno -- sirno util mcp

# Claude Code
claude mcp add sirno -- sirno util mcp
```

Restart the agent to load the connection.
For other clients, use `sirno` as the stdio command with arguments `["util", "mcp"]`.
Ask the agent to use `$sirno-editor` for project work;
the skill starts from the configured lake and follows the relevant design entries and witnesses.

<details>
<summary>Explore Sirno's own design with an agent</summary>

Clone this repository and start an agent session inside it.
First install Sirno and register its MCP server:

```text
Use $sirno-bootstrap to install Sirno and set up MCP in this repository if needed.
```

Restart the agent, then begin the guided tour:

```text
Use $sirno-narrative-session based on .sirno/lake/introduction.md.
I am new to Sirno. Ask about my background and goals, then guide me through the relevant entries.
```

</details>
<!-- sirno:witness:readme:end -->

<!-- sirno:witness:readme:begin -->
## Design Principles

Sirno addresses the design knowledge that otherwise scatters across source, documents, and review conversations.
Named entries give that knowledge a durable home while keeping implementation evidence in its original files.

- **Concept-driven documentation:** small entries preserve local meaning; structural links make them queryable.
- **Repository witnesses:** a shared entry id connects a design claim to evidence that a reviewer can inspect.
- **Project-owned method:** `meta` entries describe the project's vocabulary, documentation rules, and review habits.
- **Narrative routes:** ordered entry selections explain the design for a reader's task and background.

Read [.sirno/lake/introduction.md](.sirno/lake/introduction.md) for the first route through this project's design,
then [.sirno/lake/methodology.md](.sirno/lake/methodology.md) for its working method.
<!-- sirno:witness:readme:end -->

<!-- sirno:witness:readme:begin -->
## Status

Sirno provides a Rust library, CLI, MCP tools, and terminal interfaces.
Its implemented core covers Markdown entry storage, querying, structural checks,
witness lookup through `mosaika`, editable mist projections, and Anchor/Tide review tracking.
The lake contains the detailed contracts for those operations and the broader command surface.
Future interfaces may add a lightweight GUI or Obsidian integration.

To check the published walkthrough against a local build, run
`python3 scripts/smoke-readme.py target/debug/sirno` from this repository.
<!-- sirno:witness:readme:end -->

## License

Sirno is distributed under either the MIT license or the Apache License, Version 2.0:

- [MIT license](LICENSE-MIT)
- [Apache License, Version 2.0](LICENSE-APACHE)
