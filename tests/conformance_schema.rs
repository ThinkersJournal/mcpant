//! Conformance, part 1: messages are validated against the vendored per-revision JSON Schema
//! (`conformance/schemas/`, see SOURCE.md there). Nothing here depends on any SDK.

use serde_json::{Value, json};
use std::path::PathBuf;

pub const REVISIONS: [&str; 5] = [
    "2024-11-05",
    "2025-03-26",
    "2025-06-18",
    "2025-11-25",
    "2026-07-28",
];

fn schema_root(rev: &str) -> Value {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("conformance/schemas")
        .join(rev)
        .join("schema.json");
    let text = std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("read {}: {e}", p.display()));
    serde_json::from_str(&text).unwrap_or_else(|e| panic!("parse {}: {e}", p.display()))
}

/// Draft-07 revisions keep definitions under `definitions`, 2020-12 ones under `$defs`.
fn defs_key(root: &Value) -> &'static str {
    if root.get("$defs").is_some() {
        "$defs"
    } else {
        "definitions"
    }
}

/// Validate `instance` against the named definition of `rev`; Err carries every violation.
pub fn validate(rev: &str, def: &str, instance: &Value) -> Result<(), Vec<String>> {
    let mut root = schema_root(rev);
    let key = defs_key(&root);
    assert!(
        root[key].get(def).is_some(),
        "revision {rev} has no definition {def}"
    );
    root["$ref"] = Value::String(format!("#/{key}/{def}"));
    let validator = jsonschema::validator_for(&root).expect("schema compiles");
    let errs: Vec<String> = validator
        .iter_errors(instance)
        .map(|e| e.to_string())
        .collect();
    if errs.is_empty() { Ok(()) } else { Err(errs) }
}

#[test]
fn every_vendored_schema_compiles() {
    for rev in REVISIONS {
        let root = schema_root(rev);
        jsonschema::validator_for(&root).unwrap_or_else(|e| panic!("{rev}: {e}"));
    }
}

#[test]
fn call_tool_result_shape_holds_in_every_revision() {
    let ok = json!({"content": [{"type": "text", "text": "hi"}], "isError": false});
    for rev in REVISIONS {
        // 2026-07-28 makes `resultType` required; earlier revisions do not know it.
        let msg = if rev == "2026-07-28" {
            let mut m = ok.clone();
            m["resultType"] = json!("complete");
            m
        } else {
            ok.clone()
        };
        validate(rev, "CallToolResult", &msg).unwrap_or_else(|e| panic!("{rev}: {e:?}"));
    }
}

#[test]
fn negative_control_a_wrong_shape_is_rejected() {
    // Without this, the tests above could pass against a validator that accepts everything.
    let bad = json!({"content": "not an array"});
    for rev in REVISIONS {
        assert!(
            validate(rev, "CallToolResult", &bad).is_err(),
            "{rev} accepted a bad result"
        );
    }
}

#[test]
fn resulttype_is_required_only_from_2026_07_28() {
    let without = json!({"content": []});
    assert!(validate("2025-11-25", "CallToolResult", &without).is_ok());
    assert!(validate("2026-07-28", "CallToolResult", &without).is_err());
}
