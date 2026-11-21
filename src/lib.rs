//! MCPAnt: a Model Context Protocol host layer over a pinned `rmcp`. Each layer is an opt-in cargo feature; the
//! design and the evidence behind each one are in `docs/DESIGN.md` section 4b.
//!
//! Features: `truncate` (output budgets with an honest `truncated` flag), `channel` (push events to Claude Code), `backend` (a transport-free tool backend served over rmcp).

#![forbid(unsafe_code)]
#![warn(missing_docs)]

#[cfg(feature = "truncate")]
pub mod truncate;

#[cfg(feature = "channel")]
pub mod channel;

#[cfg(feature = "backend")]
pub mod backend;
