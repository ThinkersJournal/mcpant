# Roadmap

Nothing is built. Order, set by the PM on CireSnave's proposal (2026-10-28):

1. Design note with a build-or-adopt comparison (the official Rust MCP SDK, MoCoPr, our two hand-rolled servers).
2. Protocol core with no Ant dependencies; per-spec-version modules; conformance suite.
3. First consumers: json-mcp-server and rust-analyzer-mcp-server. MoCoPr gets an end-of-life README pointer (no yank).
4. 2026-11-21: drop the `RUSTSEC-2026-0543` (chrono unmaintained) ignore in `deny.toml` when rmcp and schemars stop pulling chrono; re-check at each rmcp bump; the ignore expires 2026-12-17 (CIRESNAVE-EXPECTATIONS 6.15b).
