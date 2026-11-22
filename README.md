# MCPAnt

Reserved name. Planned: a Model Context Protocol library for Rust (server and client halves), designed around
per-spec-version modules and a conformance suite, replacing the earlier MoCoPr crate and the hand-rolled MCP code in
our own MCP servers. It will use the other Ants where they exist (HTTPAnt, CapAnt, RedactAnt, AuditAnt, ConfigurAnt).
Nothing is implemented; see ROADMAP.md.

## What exists today

A host layer over a pinned [`rmcp`](https://crates.io/crates/rmcp), as opt-in cargo features (none on by default):

| Feature | What |
|---|---|
| `truncate` | Budgeted output with an honest `truncated` flag (lifted from sageant, credited). |
| `channel` | Declare `claude/channel` and push escaped, untrusted-labelled events to Claude Code. |
| `backend` | Write tools as plain functions (`ToolBackend`), serve them over rmcp. |
| `limits` | Per-call timeout that cancels the call, and a concurrency cap. |
| `namespace` | `<server>__<tool>` names for hosts that aggregate servers. |

The own protocol core described above is deferred (`docs/DESIGN.md` section 4b); the conformance suite under `conformance/` and `tests/` is kept.
