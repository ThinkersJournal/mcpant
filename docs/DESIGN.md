# MCPAnt design note

Status: draft 1, 2026-11-20 (lane: mcpant). Due 2026-11-23. Docs only; nothing here is built.
Inputs: pm-platform's `MCPANT-INPUTS.md` (lane-reported, 14 lanes). Anything marked **verified** I checked myself at
the stated ref; everything else is a lane's report and is labelled so.

## 1. The hello-world (the product, per CireSnave 6.24)

A stranger should be able to copy this and have a working stdio MCP server. This is the *intended* API, not yet real.

```rust
use mcpant::{Server, Tool, ToolError};
use serde_json::{json, Value};

#[tokio::main]
async fn main() -> Result<(), mcpant::Error> {
    Server::builder("greeter", env!("CARGO_PKG_VERSION"))
        .tool(
            Tool::new("greet", "Say hello to someone")
                .param("name", "string", "Who to greet")
                .handler(|args: Value| async move {
                    Ok::<_, ToolError>(json!(format!("Hello, {}!", args["name"].as_str().unwrap_or("world"))))
                }),
        )
        .serve_stdio()
        .await
}
```

What it must do with no further configuration: negotiate every supported spec revision, answer `server/discover` and
`initialize`, route `tools/list` and `tools/call`, return tool failures as `isError` results, send logs to stderr only
(stdout is protocol-only), and pass the conformance suite. Errors say what to do next. HTTP is one more line
(`.serve_http(httpant_server)`, see 5) and the same server value serves both.

## 2. Verified facts the decision rests on

| Fact | Evidence |
|---|---|
| The spec publishes five dated revisions plus a draft: 2024-11-05, 2025-03-26, 2025-06-18, 2025-11-25, 2026-07-28 | **verified** `modelcontextprotocol/modelcontextprotocol` `docs/specification/` and `schema/` directory listings, 2026-11-20 |
| 2026-07-28 is a different protocol, not an increment: no `initialize`, per-request `_meta` version/capabilities, `server/discover`, `subscriptions/listen`, no `ping`/`logging/setLevel`, required `resultType`, MRTR replaces server-initiated requests, no `Mcp-Session-Id`, no SSE resumability, `ttlMs`/`cacheScope` on list results, tasks become an extension, new reserved error-code range -32020..-32099 | **verified** `docs/specification/2026-07-28/changelog.mdx` |
| rmcp 3.11.0 (crates.io, Apache-2.0, MSRV 1.88) already has `V_2026_07_28` through `V_2024_11_05` and mentions `server/discover` in model, client, stdio framing and the Streamable HTTP server | **verified** read of `rmcp-3.11.0/src/model.rs:169-173,289-293`; `grep server/discover` hits 5 files |
| rmcp has **no** `claude/channel` support (0 hits; control: the same grep finds `server/discover`) | **verified** same tree |
| rmcp versions in use across our projects: 3.4.0 synapse, 3.4.1 agent-loop spike, 3.7 json-mcp-server (considered, not adopted), 3.10 sageant; current is 3.11.0 | lane-reported (inputs file) except 3.11.0 (**verified** `cargo info rmcp`) |
| MoCoPr: `mocopr` 0.1.0 on crates.io, MIT OR Apache-2.0 in the manifest, last push 2025-09-22, **GitHub reports no licence file** (`licenseInfo: null`), workspace of core/client/server/macros/rbac | **verified** `gh repo view`, `cargo info mocopr`, 2026-11-20 |
| The spec repo itself is mid-relicence MIT to Apache-2.0 (`NOASSERTION` on GitHub) | **verified** `LICENSE` text |

Consequence: the earlier assumption in ROADMAP that MCPAnt exists because rmcp lags the spec is **false as of
today**. The case for MCPAnt must rest on something else (section 4).

## 3. Build-or-adopt comparison

| Option | Gives | Costs | Verdict |
|---|---|---|---|
| **A. Adopt rmcp directly in every consumer** (status quo for synapse, sageant, agent-loop) | Complete, all five revisions, maintained by the spec's own org, macros | Four rmcp versions in the portfolio; each consumer re-writes the same layer above it (tool definition helper, grants/limits, truncation, namespacing, channel push); no per-version isolation we control; breaking 3.x releases hit us one by one; Apache-2.0 only | Fine for one consumer, which is not our situation |
| **B. MCPAnt as a thin layer over rmcp** | One pinned rmcp, one place for the shared layer above it; least code | We inherit rmcp's architecture and release cadence for the part CireSnave called "word for word", and cannot satisfy json-mcp-server / rust-analyzer-mcp-server (which hand-rolled on purpose and did not adopt it) | Strong runner-up |
| **C. MCPAnt own protocol core + the shared layer above it** (ROADMAP's plan) | Per-spec-version modules we control; conformance suite written from the spec text; builder shape shared with the other Ants; stdio+HTTP via HTTPAnt; one place for channel push | Most work; must keep pace with the spec; risk of being a worse rmcp | **Recommended, conditional on the gate in 4** |
| **D. Adopt MoCoPr** | Already written | Last push 2025-09, single 0.1.0, no licence file in the repo, predates 2025-11-25 and 2026-07-28; lightbulb carries it as an unused dependency and calls its scaffold CONTRADICTED (lane-reported) | Reject. Read for ideas only; EOL README pointer, no yank |
| **E. Keep hand-rolling per server** | Nothing | The duplication CireSnave wants gone | Reject |

fam's MCP adapter is TypeScript (decommissioning into Synapse): pattern only, nothing seeded.

### 4. Recommendation and the gate that could overturn it

Build C, in two layers (a distinction the inputs file asked the note to make explicit):

1. **`mcpant` core (protocol)**: types, framing, per-revision modules, negotiation, server and client halves,
   transports as traits. No Ant dependencies (ROADMAP row 2).
2. **`mcpant` host layer (opt-in features)**: what rmcp does not give *and* consumers asked for: tool registry with
   plain `(name, Value) -> Result<Value, ToolError>` backends testable without transport (sageant); collection grants
   and read-only default; response budget with an always-present `truncated` flag; untrusted-text envelope; uniform
   rejection envelope and batch/limit guards (editant); namespacing `<server>__<tool>` plus `mcpServers` config reader
   with approve-before-start, per-call limits, cancellation of process groups, `tools/list_changed` refresh
   (agent-loop); `claude/channel` push (synapse); wire probe/tee tooling (synapse).

**Gate (honest uncertainty):** because rmcp already covers all revisions, layer 1 is only worth owning if it beats
"wrap rmcp" on something measurable. Before layer 1 is built, the conformance suite (6) is run **against rmcp 3.11**
as a differential oracle. If rmcp passes everything and the per-version isolation buys nothing a consumer asked for,
fall back to B (layer 2 over a pinned rmcp, protocol core deferred). This is a CireSnave-level choice (it changes
the project's size by a large factor); I recommend C with the gate, and ask pm-platform to confirm.

## 5. Shape

- **Per-spec-version modules**: `mcpant::v2024_11_05`, `v2025_03_26`, `v2025_06_18`, `v2025_11_25`, `v2026_07_28`,
  each generated or checked against that revision's `schema.json`. A version-neutral facade (`mcpant::Request`, `Tool`)
  converts; 2026-07-28 is stateless and structurally different (no handshake), so it is a peer module, not a flag.
  Draft is not supported until it is dated.
- **Transports**: stdio (newline-delimited JSON-RPC, stdout protocol-only) in core. Streamable HTTP is a **HTTPAnt
  `Handler`/route set** mounted on its `Server` (httpant's request: body limits, shutdown, Origin, TLS for free; no
  listener or TLS of our own). SSE streams must not rely on activity-based idle timeouts (httpant
  `docs/LIMITS-BY-ADAPTER.md:19`, lane-reported). Under 2026-07-28 there is no session header or resumability, which
  simplifies the HTTP side; the legacy revisions still need `Mcp-Session-Id`.
- **Ants reused**: HTTPAnt (transport), ConfigurAnt (settings, builder shape), CapAnt (tool grants), RedactAnt
  (output/log redaction and the untrusted-text envelope), AuditAnt (tool-call audit trail). Each is an optional
  feature so the core stays dependency-free. I have **not** verified their current APIs; that is the first task of
  the host-layer work.
- **Consumers keep**: only project-specific tools and their argument structs (json-mcp-server's tools,
  rust-analyzer's `lsp.rs`, sageant's `Tools<S,I>`, Synapse's mailbox/ack/lease). Everything in 4(2) moves.
- **wasm-sandbox** needs a generator callable from Rust (tool spec in, server out) rather than string templates:
  satisfied by the builder in 1 being a plain crates.io API.

## 6. Conformance suite

Written from the **spec text and `schema.json` per revision**, not from memory and not from any SDK. Three parts:
(1) schema validation of every message we emit against that revision's JSON Schema; (2) behavioural cases per
revision (version negotiation and `UnsupportedProtocolVersionError`, `server/discover`, notifications get no
reply, unknown tool is a protocol error while tool failure is `isError`, `resultType`, `ttlMs`/`cacheScope`,
error-code ranges); (3) cross-implementation runs against rmcp 3.11 and the consumers' existing servers as oracles,
reported as differences, never silently reconciled. json-mcp-server's ~8 protocol tests and sageant's
`tests/wire.rs` harness are the seed; synapse's wire probe documents that a real client's init bytes **differ from
the spec**, so the suite also carries recorded real-client transcripts with the divergence named.
Suite results are published per revision; a revision we cannot fully pass is marked partial in the README.

## 7. Offered code and provenance (CireSnave: "I am not a plageurist")

| Source | Licence / origin as reported | Use |
|---|---|---|
| json-mcp-server `src/mcp/*`, tests | MIT OR Apache-2.0, Eric Evans 2025-06-29, lane rewrites 2026-10-25 | seed core + tests |
| rust-analyzer-mcp-server `src/protocol.rs`, stdio loop | MIT, (c) 2025 Eric Evans, single author; not checked against spec text | seed negotiation; re-derive against spec |
| sageant `crates/sageant-mcp` | MIT OR Apache-2.0; per-file trace **not redone**; credit sageant lane | seed host layer (PR offered on go) |
| synapse channel push + wire probe | MIT OR Apache-2.0, own code | seed channel feature and probe tooling |
| wasm-sandbox templates | MIT, Eric Evans (+ his AI agent); "none worth seeding" | JSON-RPC type shapes at most |
| MoCoPr | no licence file in repo (manifest says dual) | read only; copy nothing until the file exists |
| editant's md-mcp (`zekaizer/md-mcp`, Luke Lee) | **licence unresolved**; request awaits CireSnave | **copy nothing**; ADR-0011 envelope as a pattern only |
| rmcp | Apache-2.0 | dev-dependency oracle only; no code copied |

Rule for anything we do copy: verify our copy against its origin, keep its licence and notice, record it in
`NOTICE`/`PATENT-SWEEP.md`, credit the lane in the commit. The spec text is Apache-2.0/CC-BY; JSON schemas we
embed carry its notice.

## 8. Ease-of-use gate (6.24)

README opens with the section 1 example, compiled in CI. First non-placeholder release is blocked until a
fresh-eyes trial (an agent given only the README reaches the example's result) passes. 80% path: one obvious builder
call per tool, safe defaults (read-only, truncation on, stdout protocol-only), escape hatch to raw `Value` and to
the per-version modules. Errors name the fix.

## 9. Plan after this note merges

1. Conformance suite skeleton + differential run against rmcp 3.11 (decides the gate in 4).
2. Core: 2026-07-28 and 2025-11-25 first (json-mcp-server's dual era), older revisions after.
3. Host layer features seeded from the offers above.
4. Migrate json-mcp-server, rust-analyzer-mcp-server first (ROADMAP row 3), then sageant, synapse, agent-loop.
Release candidates only (6.30); the PM allocates versions; no publish from this lane.

## 10. Open questions for pm-platform

1. Confirm C-with-gate over B (section 4).
2. Owners of wasm-sandbox, fam, lsp-bridge are known to pm-ants; none has code to offer, so no answer is needed.
3. MoCoPr has no licence file: should the EOL pointer PR also add one? (Repo is CireSnave's.)
4. ConfigurAnt/CapAnt/RedactAnt/AuditAnt API fit is unverified.

— lane: mcpant
