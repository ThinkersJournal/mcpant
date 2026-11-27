# Changelog

## [0.0.6] - 2026-11-27

- `backend`: `BackendServer::serve_stdio()`, `BackendServer::with_server_info(name, version)`, `tool()` helper and `ToolDefinition` re-export, so a hello-world needs no direct rmcp dependency (found by timing it from a clean project). `backend` now pulls tokio (`io-std`). Minor, additive.
- `examples/hello.rs`, the README hello-world, built by CI. README and ROADMAP now say what exists and what is deferred.
- rmcp 3.11 to 3.12 (lockfile; the `3.11` requirement already allowed it); `rust-version = "1.88"` stated; package description updated.
- CodeRipper portfolio pin to 467fd259 (0.48.1).

## [0.0.5] - 2026-11-24

- Docs and CI only: `docs/DESIGN.md` update; the CodeRipper portfolio pin moves to dc45e36 (0.47.15). Allocated by pm-assist.

## [0.0.4] - 2026-11-23

- Docs and tests only: the rate-limit pointer in `src/limits.rs` now names a keyed limiter with a worked example in `tests/http_rate_limit.rs`; HTTP stream edge tests; `docs/DESIGN.md`; `LICENSE-APACHE` replaced with the canonical Apache-2.0 text (11358 bytes, sha256 cfc7749b). Allocated by pm-assist 2026-11-23.

## [0.0.3] - 2026-11-22

- Host layer over a pinned rmcp 3.11, each layer an opt-in cargo feature, none on by default (DESIGN s4b): `truncate` (sageant's `within_budget`/`fit` lifted with credit, plus reported variants), `channel` (`claude/channel` capability, `escape`, `notify`), `backend` (transport-free `ToolBackend` served as an rmcp server; shape lifted from sageant-mcp), `limits` (per-call timeout that cancels, concurrency cap), `namespace` (`<server>__<tool>`).
- Spikes as tests: rmcp's `StreamableHttpService` mounts on HTTPAnt types; rmcp 3.11 pushes a custom-method notification.

## [0.0.2] - 2026-11-21

- Conformance schemas for 5 spec revisions + SPEC-LICENSE, 2 test files, DESIGN prior-art table; dev-dependencies only, shipped crate unchanged. Allocated by pm-assist.

## [0.0.1] - 2026-11-20

- Docs: `docs/DESIGN.md`, the MCPAnt design note (no code).

## [0.0.0] - 2026-10-28

- Name reserved with an empty placeholder crate.
