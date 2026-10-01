---
desc: The command surface for summarizing the current Sirno project state.
name: Project Status Commands
category:
  - concept
  - implemented
belongs:
  - project-config
prerequisite:
  - project-config
  - cli-interface
  - mcp-interface
  - lake
  - anchor
  - tide
  - mist
refines:
  - command-families
---

Project status commands summarize the configured Sirno project as an operational dashboard.
Status is the go-to read-only health command for human and agent callers.

## Command Surface

| Surface | Operation | Behavior |
|---|---|---|
| CLI | `sirno status` | Prints the current project status. |
| CLI | `sirno st` | Short alias for `sirno status`. |
| MCP | `sirno_status` | Returns typed project status for agent-facing callers. |

Status reports:

- config path;
- reservoir path;
- entry count;
- structural link relation count;
- selected-mode lake check;
- active Tide summary;
- Anchor baseline state;
- default mist projection state.

CLI status keeps structural policy collapsed.
CLI and MCP status accept `show: summary | normal | full`.
Omitting `show` selects `summary`.
CLI status accepts `-m, --mode edit | review`.
MCP status accepts `mode: edit | review`.
Omitting `mode` selects `review`.
Summary status returns `ok`, project paths, entry and structural-field counts,
blocker counts, and a short message.
Normal status adds check policy, Tide summary, Anchor summary, and default mist status.
Full status adds structural link policy, selected-mode check output, and Anchor ripples.

A failed lake check carries its check output at every detail level,
so a failing status can always explain itself.
CLI status prints the check diagnostic block unless `--quiet` is set.
Every human detail level prints the overall status message and compact blocker counts,
including when `--quiet` suppresses check diagnostics.
A blocked status also suggests the next step:
inspect check diagnostics, inspect mist changes, review Tide workitems,
or review the lake, render its projection, and accept it with Anchor, in that order.
The suggestion guides the operator; status remains read-only.

`project-status-commands` owns project status command spelling and behavior.
`project-config` owns project configuration.
`lake`, `anchor`, `tide`, and `mist` own the status domains being summarized.

The *repository witnesses* for this entry should show CLI and MCP status dispatch,
the shared surface-context operation that assembles status data,
and human output that explains blockers at every detail level.
