//! `<server>__<tool>` names for a host that aggregates several MCP servers.
//!
//! The rule is the one recorded in agent-loop's `docs/TOOLS.md` (attributed there to rig's MCP client): names use the charset
//! model providers accept (`A-Z a-z 0-9 _ -`), and neither part contains `__` or ends in `_`, so [`split`] is unambiguous. DESIGN.md
//! section 4b, "Namespacing": no code was taken from rig; this is written from that rule.
//!
//! Sanitising can make two different names equal (`a.b` and `a_b`), so a [`Table`] refuses a second tool that maps to a
//! qualified name already taken, rather than silently shadowing the first.

use std::collections::HashMap;

/// Separator between the server part and the tool part.
pub const SEPARATOR: &str = "__";

/// `part` reduced to the accepted charset, with no `__` and no trailing `_`. Empty if nothing usable remains.
#[must_use]
pub fn sanitize(part: &str) -> String {
    let mut out = String::with_capacity(part.len());
    for c in part.chars() {
        let c = if c.is_ascii_alphanumeric() || c == '-' {
            c
        } else {
            '_'
        };
        if c == '_' && out.ends_with('_') {
            continue;
        }
        out.push(c);
    }
    while out.ends_with('_') {
        out.pop();
    }
    out
}

/// The name offered to the model for `tool` on `server`, or `None` if either part sanitises to nothing.
#[must_use]
pub fn qualify(server: &str, tool: &str) -> Option<String> {
    let (s, t) = (sanitize(server), sanitize(tool));
    if s.is_empty() || t.is_empty() {
        return None;
    }
    Some(format!("{s}{SEPARATOR}{t}"))
}

/// Split a qualified name at its one separator. `None` if it has none or has more than one.
#[must_use]
pub fn split(qualified: &str) -> Option<(&str, &str)> {
    let (s, t) = qualified.split_once(SEPARATOR)?;
    (!s.is_empty() && !t.is_empty() && !t.contains(SEPARATOR)).then_some((s, t))
}

/// Why a tool could not be registered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RegisterError {
    /// A part sanitised to nothing.
    Unusable,
    /// Another tool already has this qualified name.
    Collision(String),
}

/// Qualified name to the original `(server, tool)` pair, so a call goes to the wire with the server's own name.
#[derive(Debug, Default)]
pub struct Table {
    by_qualified: HashMap<String, (String, String)>,
}

impl Table {
    /// An empty table.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Register `tool` of `server`; returns the name to offer.
    ///
    /// # Errors
    /// [`RegisterError::Unusable`] or [`RegisterError::Collision`].
    pub fn register(&mut self, server: &str, tool: &str) -> Result<String, RegisterError> {
        let q = qualify(server, tool).ok_or(RegisterError::Unusable)?;
        if self.by_qualified.contains_key(&q) {
            return Err(RegisterError::Collision(q));
        }
        self.by_qualified
            .insert(q.clone(), (server.to_owned(), tool.to_owned()));
        Ok(q)
    }

    /// The original `(server, tool)` for a qualified name.
    #[must_use]
    pub fn resolve(&self, qualified: &str) -> Option<(&str, &str)> {
        self.by_qualified
            .get(qualified)
            .map(|(s, t)| (s.as_str(), t.as_str()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitising_keeps_the_charset_and_never_leaves_a_double_or_trailing_underscore() {
        assert_eq!(sanitize("my.server/v2"), "my_server_v2");
        assert_eq!(sanitize("a__b"), "a_b");
        assert_eq!(sanitize("tool_"), "tool");
        assert_eq!(sanitize("..."), "");
        assert_eq!(sanitize("ok-name_1"), "ok-name_1");
    }

    #[test]
    fn qualify_and_split_round_trip() {
        let q = qualify("serena", "find_symbol").unwrap();
        assert_eq!(q, "serena__find_symbol");
        assert_eq!(split(&q), Some(("serena", "find_symbol")));
        assert_eq!(qualify("", "x"), None);
        assert_eq!(split("nosep"), None);
        assert_eq!(split("a__b__c"), None);
    }

    #[test]
    fn a_collision_after_sanitising_is_refused_and_resolve_returns_the_original() {
        let mut t = Table::new();
        assert_eq!(t.register("a.b", "go").unwrap(), "a_b__go");
        assert_eq!(
            t.register("a_b", "go"),
            Err(RegisterError::Collision("a_b__go".to_owned()))
        );
        assert_eq!(t.register("!!", "go"), Err(RegisterError::Unusable));
        assert_eq!(t.resolve("a_b__go"), Some(("a.b", "go")));
        assert_eq!(t.resolve("zzz__go"), None);
    }
}
