# Roadmap

Order set by the PM on CireSnave's proposal (2026-10-28); ruling 2026-11-20 (CireSnave, board 473, option B): a host layer
over a pinned rmcp, own protocol core deferred. State at 2026-11-26:

1. **Done:** design note with a build-or-adopt comparison (`docs/DESIGN.md`).
2. **Done as the host layer; deferred as the own core.** Built: five opt-in features (`backend`, `truncate`, `channel`,
   `limits`, `namespace`), a stdio hello-world, a conformance suite that runs rmcp against each spec revision's schema,
   spikes mounting rmcp on HTTPAnt. Not built: own per-spec-version protocol core (DESIGN 4b), client half, `serve_http`.
3. **Next: first publish (PM) and first consumers.** Publish readiness is in the PR of 2026-11-27. Consumer migrations are
   rows 3a and 3b below; neither starts until the PM says. MoCoPr gets an end-of-life README pointer (no yank) after the
   first consumer ships.
   - **3a. rust-analyzer-mcp-server** (0.1.13, MSRV 1.85.1; read at origin/main 557318e). Hand-rolls the protocol: `src/protocol.rs`
     (135 lines) and the `initialize`/`tools/list`/`tools/call` dispatch in `src/server.rs` (615 lines). Deletes: those and its
     own stdio loop. Keeps: `src/lsp.rs` (261), the rust-analyzer driver and file watcher, the tool bodies, which become a
     `ToolBackend`. Cost: MSRV 1.85.1 to 1.88 (permitted, CireSnave 2026-10-08), plus rmcp's dependency tree.
   - **3b. json-mcp-server** (0.3.8, MSRV 1.89, no MSRV cost; read at origin/main 2e985d8). Hand-rolls the protocol: `src/mcp/protocol.rs`
     (105) and `src/mcp/server.rs` (211, including `add_modern_fields` for the 2026-07-28 hints). Deletes: those two. Keeps:
     `src/json_tools/*` and `main.rs`; its truncation notice may move to `truncate` (not read). **No change until the PM
     says** (it is going into an index listing this week).
4. 2026-11-21: drop the `RUSTSEC-2026-0543` (chrono unmaintained) ignore in `deny.toml` when rmcp and schemars stop pulling
   chrono; re-checked at rmcp 3.12.0 on 2026-11-26 (still pulled); re-check at each rmcp bump; the ignore expires
   2026-12-17 (CIRESNAVE-EXPECTATIONS 6.15b).
