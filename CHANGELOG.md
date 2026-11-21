# Changelog

## [Unreleased] (version allocated by pm-assist at gate time)

- Host layer over a pinned rmcp 3.11, each layer an opt-in cargo feature, none on by default (DESIGN s4b): `truncate` (sageant's `within_budget`/`fit` lifted with credit, plus reported variants), `channel` (`claude/channel` capability, `escape`, `notify`), `backend` (transport-free `ToolBackend` served as an rmcp server; shape lifted from sageant-mcp), `limits` (per-call timeout that cancels, concurrency cap), `namespace` (`<server>__<tool>`).
- Spikes as tests: rmcp's `StreamableHttpService` mounts on HTTPAnt types; rmcp 3.11 pushes a custom-method notification.

## [0.0.2] - 2026-11-21

- Conformance schemas for 5 spec revisions + SPEC-LICENSE, 2 test files, DESIGN prior-art table; dev-dependencies only, shipped crate unchanged. Allocated by pm-assist.

## [0.0.1] - 2026-11-20

- Docs: `docs/DESIGN.md`, the MCPAnt design note (no code).

## [0.0.0] - 2026-10-28

- Name reserved with an empty placeholder crate.
