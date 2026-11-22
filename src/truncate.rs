//! Output truncation that is always reported.
//!
//! A tool that returns a list or a long text should cap it by size and tell the model it did. [`within_budget`]
//! keeps the leading values of a list that fit a byte budget; [`fit`] keeps the longest prefix of a string whose
//! JSON encoding fits. [`estimate_tokens`] gives the four-bytes-to-the-token estimate the budgets are usually set
//! from. Neither cuts inside a UTF-8 character.
//!
//! The two functions are lifted from the `sageant-mcp` crate (sageant, MIT OR Apache-2.0, credit the sageant
//! project; DESIGN.md section 7 records the per-file provenance trace). `estimate_tokens` and [`Truncated`] are new.

use serde_json::Value;

/// A cut result: the kept part and whether anything was dropped.
#[derive(Debug, Clone, PartialEq)]
pub struct Truncated<T> {
    /// What fit.
    pub kept: T,
    /// True if anything was dropped; callers should surface this to the model.
    pub truncated: bool,
}

/// The estimated token count of `bytes` bytes of text (four bytes to the token, rounded up).
#[must_use]
pub const fn estimate_tokens(bytes: usize) -> usize {
    bytes.div_ceil(4)
}

/// The leading values whose serialised sizes (and one separating byte each) fit in `budget` bytes.
#[must_use]
pub fn within_budget(values: Vec<Value>, budget: usize) -> Vec<Value> {
    let mut used = 0;
    let mut kept = Vec::new();
    for v in values {
        used += serde_json::to_string(&v).map_or(0, |s| s.len()) + 1;
        if used > budget {
            break;
        }
        kept.push(v);
    }
    kept
}

/// Like [`within_budget`], but says whether any value was dropped.
#[must_use]
pub fn within_budget_reported(values: Vec<Value>, budget: usize) -> Truncated<Vec<Value>> {
    let total = values.len();
    let kept = within_budget(values, budget);
    Truncated {
        truncated: kept.len() < total,
        kept,
    }
}

/// The longest prefix of `text` whose JSON encoding fits in `budget` bytes.
#[must_use]
pub fn fit(text: &str, budget: usize) -> &str {
    if serde_json::to_string(text).map_or(usize::MAX, |s| s.len()) <= budget {
        return text;
    }
    let bounds: Vec<usize> = text
        .char_indices()
        .map(|(i, _)| i)
        .chain([text.len()])
        .collect();
    let (mut lo, mut hi) = (0, bounds.len() - 1); // bounds[lo] fits, bounds[hi] does not
    while hi - lo > 1 {
        let mid = usize::midpoint(lo, hi);
        if serde_json::to_string(&text[..bounds[mid]]).map_or(usize::MAX, |s| s.len()) <= budget {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    &text[..bounds[lo]]
}

/// Like [`fit`], but says whether the text was cut.
#[must_use]
pub fn fit_reported(text: &str, budget: usize) -> Truncated<&str> {
    let kept = fit(text, budget);
    Truncated {
        truncated: kept.len() < text.len(),
        kept,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn budget_keeps_a_prefix_and_reports_the_cut() {
        let v = vec![json!("aaaa"), json!("bbbb"), json!("cccc")]; // each serialises to 6 bytes, +1 separator
        assert_eq!(within_budget(v.clone(), 14).len(), 2);
        let r = within_budget_reported(v.clone(), 14);
        assert!(r.truncated && r.kept == v[..2]);
        let all = within_budget_reported(v.clone(), 100);
        assert!(!all.truncated && all.kept == v);
        assert!(within_budget_reported(v, 0).kept.is_empty());
    }

    #[test]
    fn fit_never_cuts_inside_a_character_and_counts_escapes() {
        let s = "héllo wörld";
        for b in 0..30 {
            let cut = fit(s, b);
            assert!(s.starts_with(cut));
            assert!(serde_json::to_string(cut).unwrap().len() <= b || cut.is_empty());
        }
        // A quote costs two bytes once encoded, so 3 quotes do not fit in 6 bytes (2 for the string quotes + 6).
        assert_eq!(fit("\"\"\"", 6), "\"\"");
        assert!(!fit_reported("short", 100).truncated);
        assert!(fit_reported("a long enough text", 8).truncated);
    }

    #[test]
    fn tokens_round_up() {
        assert_eq!(estimate_tokens(0), 0);
        assert_eq!(estimate_tokens(1), 1);
        assert_eq!(estimate_tokens(4), 1);
        assert_eq!(estimate_tokens(5), 2);
    }
}
