//! MCPAnt: a Model Context Protocol host layer over a pinned `rmcp`. Each layer is an opt-in cargo feature; the
//! design and the evidence behind each one are in `docs/DESIGN.md` section 4b.
//!
//! Features: `truncate` (output budgets with an honest `truncated` flag), `channel` (push events to Claude Code), `backend` (a transport-free tool backend served over rmcp, stdio in one call; start here, see the README hello-world), `limits` (per-call timeout and concurrency cap for a backend), `namespace` (`<server>__<tool>` names for aggregating hosts).

#![forbid(unsafe_code)]
#![warn(missing_docs)]

#[cfg(feature = "truncate")]
pub mod truncate;

#[cfg(feature = "channel")]
pub mod channel;

#[cfg(feature = "backend")]
pub mod backend;

#[cfg(feature = "limits")]
pub mod limits;

#[cfg(feature = "namespace")]
pub mod namespace;
