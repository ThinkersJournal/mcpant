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
| MoCoPr: `mocopr` 0.1.0 on crates.io, MIT OR Apache-2.0 in the manifest, last push 2025-09-22, GitHub reported no licence file on 2026-11-20 morning (`licenseInfo: null`); **LICENSE-MIT and LICENSE-APACHE were added to main later that day** (clone `98d0c31`), matching the manifest, workspace of core/client/server/macros/rbac | **verified** `gh repo view`, `cargo info mocopr`, 2026-11-20 |
| The spec repo itself is mid-relicence MIT to Apache-2.0 (`NOASSERTION` on GitHub) | **verified** `LICENSE` text |

Consequence: the earlier assumption in ROADMAP that MCPAnt exists because rmcp lags the spec is **false as of
today**. The case for MCPAnt must rest on something else (section 4).

## 3. Build-or-adopt comparison

| Option | Gives | Costs | Verdict |
|---|---|---|---|
| **A. Adopt rmcp directly in every consumer** (status quo for synapse, sageant, agent-loop) | Complete, all five revisions, maintained by the spec's own org, macros | Four rmcp versions in the portfolio; each consumer re-writes the same layer above it (tool definition helper, grants/limits, truncation, namespacing, channel push); no per-version isolation we control; breaking 3.x releases hit us one by one; Apache-2.0 only | Fine for one consumer, which is not our situation |
| **B. MCPAnt as a thin layer over rmcp** | One pinned rmcp, one place for the shared layer above it; least code | We inherit rmcp's architecture and release cadence for the part CireSnave called "word for word", and cannot satisfy json-mcp-server / rust-analyzer-mcp-server (which hand-rolled on purpose and did not adopt it) | Strong runner-up |
| **C. MCPAnt own protocol core + the shared layer above it** (ROADMAP's plan) | Per-spec-version modules we control; conformance suite written from the spec text; builder shape shared with the other Ants; stdio+HTTP via HTTPAnt; one place for channel push | Most work; must keep pace with the spec; risk of being a worse rmcp | **Recommended, conditional on the gate in 4** |
| **D. Adopt MoCoPr** | Already written | Last push 2025-09, single 0.1.0, predates 2025-11-25 and 2026-07-28; lightbulb carries it as an unused dependency and calls its scaffold CONTRADICTED (lane-reported) | Reject. Read for ideas only; EOL README pointer, no yank |
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
fall back to B (layer 2 over a pinned rmcp, protocol core deferred). The differential must cover 2026-07-28 **and** the four legacy revisions. The gate also records rmcp's MSRV (1.88, verified) against each consumer's `rust-version`; CireSnave 2026-10-08 permits raising MSRV, so a gap is a cost to note, not a blocker. This is a CireSnave-level choice (it changes
the project's size by a large factor); I recommend C with the gate, and ask pm-platform to confirm.

### 4a. Gate result, first run (2026-11-20, rmcp 3.11.0, `tests/conformance_rmcp_oracle.rs`)

rmcp served a one-tool calculator over an in-memory pipe; the test spoke raw JSON-RPC lines (not rmcp's client)
and validated what rmcp answered against each revision's vendored schema. Result: **all 15 checks passed**
(`InitializeResult`, `ListToolsResult`, `CallToolResult` for each of the four legacy revisions; `DiscoverResult`,
`ListToolsResult`, `CallToolResult` for 2026-07-28), negotiated the requested version each time, computed
`add(2,3) = 5`, and answered an unsupported 2026-07-28 version with code -32022 plus the supported list. Scope of
that claim: one tool, tools only, happy path plus one error; resources, prompts, subscriptions, MRTR, HTTP and
the `ttlMs`/`cacheScope` hints were not exercised, and the -32022 reply was read by eye, not schema-validated. **Update 2026-11-21:** the -32022 reply is now schema-validated against `UnsupportedProtocolVersionError` (2026-07-28) and passes, with a control (the same reply with code -32000 is rejected by that definition). Still unexercised: resources, prompts, subscriptions, MRTR, the `ttlMs`/`cacheScope` hints. The HTTP status the spec requires (400) is also met: over a running HTTPAnt `Server`, rmcp answers an unsupported `_meta` version with 400 and the -32022 body (`tests/spike_httpant_server.rs`); note it answers 400 with -32020 instead when the `MCP-Protocol-Version` header is missing.

What this means for the gate: rmcp is a **correct** protocol implementation for the tools path in all five
revisions, so "rmcp is wrong or missing something" is not a reason to own layer 1. The remaining reasons to own it
are the ones in 4: per-revision isolation under our control, one pinned place instead of four rmcp versions, and
the Claude channel push. That is a judgement for CireSnave (board 473), not something the suite settles. **The
cheapest sound reading is B first** (host layer over a pinned rmcp, which can ship soon), with the own core
following only if a consumer shows a need rmcp cannot meet; I now lean B over C and say so plainly, which
changes the recommendation in the table above.

MSRV (consumer `rust-version` at origin/main, 2026-11-20): rmcp 3.11.0 needs 1.88. json-mcp-server 1.89,
rust-analyzer-mcp-server **1.85.1**, synapse 1.98, sageant 1.94, agent-loop 1.97.1. Only rust-analyzer-mcp-server
is below rmcp's floor; CireSnave (2026-10-08) permits raising it.

### 4b. Ruling and prior-art survey of the layers over rmcp (2026-11-20)

**Ruling (CireSnave 2026-11-20, quoted by the PM, verbatim):** *"I'm fine with option B, but we should check to see
if anyone has already built the layers we would add on top of rmcp in one or more other projects."* So: B, one
shared host layer over one pinned rmcp, conformance suite kept; layer 1 (own protocol core) is deferred, and no
layer below is built before its row exists. Lift only code with a clear licence, keep credit, copy nothing with an
unresolved licence (md-mcp stands). MoCoPr now has LICENSE-MIT and LICENSE-APACHE on main (verified: both files
at clone `98d0c31`, 2026-11-20), so its code may be lifted with credit to Eric Evans.

Method: consumers by `git grep` at `origin/main` per repo (never a disk walk); published crates by crates.io
API/`cargo search` (10 queries across the layer names) and by unpacking the candidate `.crate` files. Licence =
crates.io `license` field plus the LICENSE files in the unpacked crate. Not read: crates whose names I only saw in a
search result. "Adopt" = depend on it; "lift" = copy code with credit; "build" = write it here.

| Layer | Who has it | Licence | Verdict | Evidence |
|---|---|---|---|---|
| **Tool registry + dispatch** | rmcp itself: `ToolRouter` and `#[tool]` macros (`handler/server/router/tool.rs:325`); sageant's plain-backend shim `(name, Value)->Result<Value, ToolError>` on top of it (sageant-mcp `server.rs`, 170 lines); MoCoPr `mocopr-server/src/registry.rs` (245 lines, own protocol types) | rmcp Apache-2.0; sageant MIT OR Apache-2.0 (own code, per-file third-party trace not redone); MoCoPr dual MIT/Apache | **Adopt** rmcp's router; **lift** sageant's transport-free backend shim as a thin helper. Do not lift MoCoPr (its types are not rmcp's) | rmcp 3.11 source; sageant `origin/main` 7395e92; MoCoPr clone 98d0c31 |
| **Limits (per-call timeout, rate, size)** | MoCoPr `middleware.rs` (320 lines: `TimeoutMiddleware`, `RateLimitMiddleware`, `AuthMiddleware`, Logging; its own `Middleware` trait); `mcp-server-core` 0.4.0 (rmcp ~3.9, hardening for an HTTP deployment: sign-in limits, body limits, host/origin checks); `mcp-proxy` 0.8.0 (Tower gateway; module names rbac, retry, failover, audit, cache, coalesce (contents not read)); agent-loop spec (per-call limits) but no code | MoCoPr MIT/Apache; mcp-server-core MIT OR Apache-2.0; mcp-proxy MIT OR Apache-2.0 | **Build** a small tower-style per-call limit layer over rmcp's service; **MoCoPr middleware read 2026-11-21 (clone `98d0c31`, `mocopr-server/src/middleware.rs`, 367 lines): do not lift.** Its `TimeoutMiddleware` only compares elapsed time in `after_response` and returns `Error::Timeout` then: a hung call is never cancelled, so it is not a per-call timeout. `RateLimitMiddleware` is one global bucket behind a `Mutex` (no per-client key) and calls `mocopr_core::utils::RateLimiter`, i.e. both depend on MoCoPr's own `JsonRpcRequest`/`Error` types, not rmcp's. Nothing portable, so no credit owed. **Build** on `tower`'s `timeout`, `limit::RateLimit`/`ConcurrencyLimit` for the HTTP mount (already a dev-dependency, `tower::service_fn` in the spike) and a `tokio::time::timeout` wrapper at tool dispatch for the per-call limit, since only that cancels a hung tool. Do not adopt mcp-server-core or mcp-proxy: both are whole servers/gateways, pinned to their own rmcp (3.9) and own HTTP stack, for a different job | `cargo info`/crates.io API; unpacked `.crate` files; MoCoPr `middleware.rs` |
| **Output truncation with a `truncated` flag** | sageant-mcp `lib.rs`: `within_budget`, `fit` (token estimate, always reports `truncated`); json-mcp-server has its own truncation notice (`tests/integration_tests.rs:775`, implementation not read). No published crate found | sageant MIT OR Apache-2.0; json-mcp-server MIT OR Apache-2.0 | **Lift** sageant's `within_budget`/`fit` (verify provenance per file first; sageant lane offered a PR) | sageant `origin/main:crates/sageant-mcp/src/lib.rs:16,225-252`; 0 crates.io hits for the query (control: the same search lists tool-registry crates) |
| **Namespacing `<server>__<tool>`** | agent-loop: decision recorded in `docs/TOOLS.md` (names sanitised to provider charset, rig's rule), **no code found** by `git grep` (0 hits for a namespacing implementation in `src/`; control: `ToolHost` found); `rig-rmcp` `with_name` rename (rig, MIT, per agent-loop's survey, not re-read); `mcp-proxy` uses a `/` separator in access-log code | rig MIT; mcp-proxy MIT OR Apache-2.0 | **Build** (small, and agent-loop's rule is the spec). **`rig-rmcp` 0.46.0 read 2026-11-21** (unpacked `.crate`, `Cargo.toml` `license = "MIT"`, no LICENSE file in the package): it has only a rename hook, `McpTool::with_name(ToolName)` (`src/native.rs:128`); a grep of its `src/` for `__`, `sanitiz` and `sanitis` finds nothing, so the `<server>__<tool>` rule and the charset sanitising are **not** in this crate (agent-loop attributes them to rig's `plugins/rig-mcp-client`, which I did not read). Nothing to lift from rig-rmcp; no credit owed for the rule unless the plugin is read and copied from. Scope note: namespacing is a *client/aggregator* concern (many servers, one model); a single mcpant server has no collision to prevent, so this row is a helper for hosts that aggregate, lowest priority of the five | agent-loop 9ca43cb `docs/TOOLS.md:23,64,86`; mcp-proxy 0.8.0 `access_log.rs:10-33` |
| **`claude/channel` push** | synapse `crates/synapse-claude-channel` (`CHANNEL_METHOD`, lib.rs:59/131, `examples/wire_probe.rs`, own code); `silta-claude` 0.6.0 (rmcp ^3.5; `src/mcp.rs:26,562` declares the capability and sends `notifications/claude/channel`; own daemon, a plugin not a library); `dione`, `kazoo-mcp`, `gate4agent-wake` (names only, **not read**) | synapse MIT OR Apache-2.0; silta-claude Apache-2.0 OR MIT | **Lift a small part** of synapse's (own code, tested against a real client, lane offered it). Read 2026-11-21 at synapse `origin/main` (`crates/synapse-claude-channel/src/lib.rs`): the lease/ledger/ack loop is generic but lives in `synapse_mcp_server::push` and is coupled to the Synapse daemon, so it stays in synapse. What is Claude-Code-specific, and so what mcpant would carry, is four things: the capability key `claude/channel`, the method `notifications/claude/channel`, `escape()` (control chars escaped, `<` `>` as entities so a body cannot close the `<channel>` tag), and the protocol cap (`supported_protocol_versions` returning `known_up_to(V_2025_11_25)` so Claude Code's `server/discover` gets `-32022` and falls back to `initialize`). Row decision: build `mcpant::channel` as a `notify(peer, content, meta)` helper + `escape` + the cap, no queue or ack policy (the host owns delivery). **Spiked 2026-11-21 (`tests/spike_channel_push.rs`, passes): yes.** With rmcp 3.11.0 a `ServerHandler` whose `get_info` sets `experimental["claude/channel"]` shows it in the `initialize` result, and `peer.send_notification(ServerNotification::CustomNotification(CustomNotification::new(method, params)))` (from a tool taking `Peer<RoleServer>`) puts `{"method":"notifications/claude/channel","params":{...}}` on the wire with no `id`. **Over HTTP (2026-11-21, `tests/channel_over_http.rs`, passes, on a running HTTPAnt `Server`):** after initialize, `notifications/initialized` (202) and a standalone `GET /mcp` SSE stream, a tool call that runs `channel::notify` returns its result on the POST's own stream and the `notifications/claude/channel` event arrives on the standalone GET stream, not on the POST. So a host that pushes over HTTP needs that GET stream open (a client that never opens it receives nothing; not tested). Not covered: a real Claude Code client (synapse's `wire_probe` does that). silta-claude is corroboration of the wire shape only. rmcp has 0 `claude/channel` hits (verified) | synapse `origin/main` 35f397bb; silta-claude unpacked 0.6.0 |
| **HTTP transport** | rmcp `StreamableHttpService` (`transport/streamable_http_server/tower.rs:1473`, a tower service); HTTPAnt has `BoxService = BoxCloneService<Request, Response, Infallible>` (`httpant-core/src/lib.rs:37`) and a `Handler` trait; `mcp-server-core`/`mcp-proxy`/`rust-mcp-sdk` 2.0.0 (own protocol stack, MIT, 350k downloads) bring their own listener | rmcp Apache-2.0; HTTPAnt ours | **Adopt** rmcp's service and mount it on HTTPAnt. **Spiked 2026-11-21 (`tests/spike_httpant_fit.rs`): fits.** HTTPAnt's `Request` is `http::Request<Body>` and `Body` implements `http_body::Body`, so rmcp's `StreamableHttpService` accepts it directly; mapping the response with `Body::new` gives HTTPAnt's `Response`. A real `initialize` POST through that adapter returned 200 `text/event-stream` with an `InitializeResult` (httpant-core 0.7.10 from crates.io, rmcp 3.11.0). **Widened 2026-11-21 (`tests/spike_httpant_server.rs`, passes): mounts on a running HTTPAnt `Server`** (httpant 0.7.10, hyper adapter): `Server::new().post("/mcp", handler)` where the handler takes HTTPAnt's `Request`, calls rmcp's service and returns `resp.map(Body::new)`; HTTPAnt's own `Client` POSTed `initialize` over a real loopback socket and got an `InitializeResult`. GET (SSE) and DELETE routes on the same path also work (`tests/channel_over_http.rs`; DELETE is mounted via `Server::route(Method::DELETE, ..)` but not exercised). **Idle stream (same test): a standalone GET stream left quiet for 4 s still delivered the pushed event**; httpant-server 0.7.10's public surface has no response-side idle timeout setter (grep of `lib.rs`, `adapter.rs`, `settings.rs`; the read-timeout limits in `LIMITS-BY-ADAPTER.md` are request-side and on `origin/main`, not in 0.7.10), so 4 s proves only that nothing short cuts it. **Still not covered:** minutes-long SSE streams, the unreleased request-side timeouts against HTTPAnt's idle/body limits, the 2026-07-28 HTTP path, Origin/host handling under a proxy | rmcp source; httpant `origin/main` 0e33dc8 |
| **Channel-row leads from pm-platform's search (2026-11-20)** | `dione` 0.87.0 (Discord channel server, MIT OR Apache-2.0; no GitHub licence flag); `gate4agent-wake` 0.5.21 (MIT; a wake decision, not a protocol layer); `kazoo-mcp` 0.3.4 (**AGPL-3.0-or-later**); unread code leads: BlueTeam-OU/InterWeave `claude/channel-core` (Apache-2.0), suzuke/AgEnD (Apache-2.0), teamctl `team-mcp` (MIT), tmt (MIT) | as stated | `kazoo-mcp`: **reject** (AGPL). Others: **read before lifting anything**; synapse stays the channel source until one proves better | pm-platform `cargo info` and `gh search code`, lane-reported, not re-run by me |
| **Also seen, rejected** | `bote` 0.92 (GPL-3.0-only: cannot be a dependency of an MIT OR Apache-2.0 crate); `mcptk` 0.1.0 (15 downloads, own protocol); `mcp-kit-gateway`, `mcp-shield` (gateways, own stacks); MoCoPr's protocol code (superseded by rmcp) | as stated | None lifted | crates.io API |

What this changes: nothing in the table is a ready crate we could simply depend on for limits, truncation or
namespacing; the usable prior art is our own code (sageant, synapse) plus two licence-clean references (MoCoPr
limits, rig's namespacing rule). The host layer is therefore mostly **lifts from our own projects plus a small
amount of new code**, which is the cheap outcome. Search limits (pm-platform's, lane-reported): top 15 per query, 4 queries, not a census; its truncation, namespacing and `list_changed` searches found only application servers, and **no limits query was run** there (mine was a `cargo search`). Gaps in this survey: crates.io search is keyword-based, so a
crate with an unhelpful name or description can be missed; only the candidates named here were unpacked; GitHub
code search for unpublished projects was not run.

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
  feature so the core stays dependency-free. **Checked against crates.io on 2026-11-21 (`cargo info`; descriptions only, APIs not read): three of these five do not do what this list assumed.** CapAnt 0.1.0 is a capability-*matching* engine (developers declare needs, providers declare abilities, it returns the providers that satisfy them), not a tool-grant or authorisation layer, so it is not the home for sageant-style write grants. RedactAnt is published as `redactant` 0.1.14: secret *values* with blank `Display`/`Debug`, an explicit `expose()` and zeroize on drop; it is not an output redactor or an untrusted-text envelope (the envelope stays a host-layer item, `channel::escape` is the only piece we have). AuditAnt is `auditant` **0.0.0, a placeholder**: nothing to build on yet. ConfigurAnt 0.7.2 does fit settings (schema-first, layered, secrets never printed) but needs Rust 1.95. HTTPAnt is verified (0.7.10, four spikes). Consequence: none of CapAnt, RedactAnt or AuditAnt is wired in; `redactant` is the only plausible one, for secret config values, if a layer ever holds one.
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
| sageant `crates/sageant-mcp` | MIT OR Apache-2.0; **per-file trace done 2026-11-21** at sageant `origin/main` e7c04ad: all 5 files of the crate (`src/lib.rs`, `src/server.rs`, `tests/tools.rs`, `tests/wire.rs`, `README.md`) were first committed in 7dc0f1f (2026-11-15, sageant#24) by the lane, SPDX `MIT OR Apache-2.0` in each source file, and a grep for copyright/adapted/derived/based-on/copied/URL markers found none naming a third party (control: the same grep finds the SPDX lines). Limit: this shows no *declared* third-party origin, not that none exists; `within_budget` and `fit` are generic and were not compared against other code. Credit the sageant lane | seed host layer (PR offered on go) |
| synapse channel push + wire probe | MIT OR Apache-2.0, own code | seed channel feature and probe tooling |
| wasm-sandbox templates | MIT, Eric Evans (+ his AI agent); "none worth seeding" | JSON-RPC type shapes at most |
| MoCoPr | MIT OR Apache-2.0: LICENSE-MIT and LICENSE-APACHE on main (`98d0c31`), matching its manifest; author Eric Evans | may be lifted (limits middleware); keep both licence texts and credit Eric Evans |
| OverMind `4662ef9` | only an optional Python `mcp>=2.2.0` and `examples/audit_mcp_tools.py` (pm-platform's read, lane-reported) | nothing to lift (a lint seed at most) |
| rockfish `290f75d`, fireline `6136520` | 0 code hits; fireline one doc line (`SEAM-INVENTORY.md:38`) (pm-platform's read) | nothing |
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
The first release is a plain 0.x (6.30 allows release candidates only for too-broken or pre-major polish, which does not apply); the PM allocates versions; no publish from this lane.

## 10. Open questions for pm-platform

1. Confirm C-with-gate over B (section 4).
2. Owners of wasm-sandbox, fam, lsp-bridge are known to pm-ants; none has code to offer, so no answer is needed.
3. Answered: MoCoPr now carries LICENSE-MIT and LICENSE-APACHE (no action for this lane).
4. Answered in part (2026-11-21, section 5): CapAnt is capability matching, `redactant` is secret values, AuditAnt is a 0.0.0 placeholder, ConfigurAnt fits but needs Rust 1.95. Still unread: their source APIs.

— lane: mcpant
