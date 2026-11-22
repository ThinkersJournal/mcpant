//! Per-call limits for a [`ToolBackend`]: a timeout that cancels, and a cap on concurrent calls.
//!
//! DESIGN.md section 4b, "Limits": MoCoPr's middleware only compared elapsed time after a call returned, so a hung call was never
//! cancelled; this wraps the call in [`tokio::time::timeout`], which drops the future. Nothing is lifted from MoCoPr. A
//! call over the timeout, or one that cannot get a slot, answers as an error result with code `timeout` or `busy`.
//!
//! Rate limiting (calls per window, per client) is not here: it needs a client identity, which belongs to the HTTP mount
//! (`tower::limit::ConcurrencyLimit` for a cap; a keyed limiter for per-client budgets, worked example in `tests/http_rate_limit.rs`) rather than to a backend.

use std::sync::Arc;
use std::time::Duration;

use rmcp::model::Tool;
use serde_json::Value;
use tokio::sync::Semaphore;

use crate::backend::{ToolBackend, ToolFailure};

/// What to enforce. `None` means no limit.
#[derive(Debug, Clone, Copy, Default)]
pub struct Limits {
    /// Longest a single call may run before it is cancelled.
    pub call_timeout: Option<Duration>,
    /// Most calls running at once; a call that arrives when all slots are taken fails at once with `busy`.
    pub max_concurrent: Option<usize>,
}

/// A [`ToolBackend`] with [`Limits`] applied to every call.
pub struct Limited<B> {
    inner: B,
    limits: Limits,
    slots: Option<Arc<Semaphore>>,
}

impl<B: ToolBackend> Limited<B> {
    /// Apply `limits` to `inner`.
    #[must_use]
    pub fn new(inner: B, limits: Limits) -> Self {
        Self {
            slots: limits.max_concurrent.map(|n| Arc::new(Semaphore::new(n))),
            inner,
            limits,
        }
    }
}

impl<B: ToolBackend> ToolBackend for Limited<B> {
    fn definitions(&self) -> Vec<Tool> {
        self.inner.definitions()
    }

    async fn call(&self, name: &str, arguments: Value) -> Result<Value, ToolFailure> {
        let _slot =
            match &self.slots {
                Some(s) => Some(Arc::clone(s).try_acquire_owned().map_err(|_| {
                    ToolFailure::new("busy", "too many calls in flight; retry later")
                })?),
                None => None,
            };
        match self.limits.call_timeout {
            Some(t) => tokio::time::timeout(t, self.inner.call(name, arguments))
                .await
                .unwrap_or_else(|_| {
                    Err(ToolFailure::new(
                        "timeout",
                        format!(
                            "the call ran longer than {} ms and was cancelled",
                            t.as_millis()
                        ),
                    ))
                }),
            None => self.inner.call(name, arguments).await,
        }
    }
}
