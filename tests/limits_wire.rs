//! `mcpant::limits`: a hung tool is cancelled, and a call past the concurrency cap fails at once.
#![cfg(feature = "limits")]

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use mcpant::backend::{ToolBackend, ToolFailure};
use mcpant::limits::{Limited, Limits};
use rmcp::model::Tool;
use serde_json::{Value, json};

/// `hang` never returns and records when it is dropped; `quick` returns at once.
struct Probe(Arc<AtomicBool>);

struct SetOnDrop(Arc<AtomicBool>);
impl Drop for SetOnDrop {
    fn drop(&mut self) {
        self.0.store(true, Ordering::SeqCst);
    }
}

impl ToolBackend for Probe {
    fn definitions(&self) -> Vec<Tool> {
        Vec::new()
    }
    async fn call(&self, name: &str, _args: Value) -> Result<Value, ToolFailure> {
        match name {
            "hang" => {
                let _guard = SetOnDrop(Arc::clone(&self.0));
                std::future::pending::<()>().await;
                unreachable!()
            }
            _ => Ok(json!("ok")),
        }
    }
}

#[tokio::test]
async fn a_hung_call_is_cancelled_not_just_noticed() {
    let dropped = Arc::new(AtomicBool::new(false));
    let tools = Limited::new(
        Probe(Arc::clone(&dropped)),
        Limits {
            call_timeout: Some(Duration::from_millis(50)),
            max_concurrent: None,
        },
    );
    let e = tools.call("hang", json!({})).await.unwrap_err();
    assert_eq!(e.code, "timeout");
    assert!(
        dropped.load(Ordering::SeqCst),
        "the tool's future must be dropped (cancelled)"
    );
    assert_eq!(tools.call("quick", json!({})).await.unwrap(), json!("ok"));
}

#[tokio::test]
async fn a_call_past_the_cap_is_refused_busy_and_the_slot_frees_afterwards() {
    let tools = Arc::new(Limited::new(
        Probe(Arc::new(AtomicBool::new(false))),
        Limits {
            call_timeout: Some(Duration::from_millis(300)),
            max_concurrent: Some(1),
        },
    ));
    let t2 = Arc::clone(&tools);
    let first = tokio::spawn(async move { t2.call("hang", json!({})).await });
    tokio::time::sleep(Duration::from_millis(50)).await;
    let e = tools.call("quick", json!({})).await.unwrap_err();
    assert_eq!(e.code, "busy");
    assert_eq!(first.await.unwrap().unwrap_err().code, "timeout");
    assert_eq!(tools.call("quick", json!({})).await.unwrap(), json!("ok"));
}
