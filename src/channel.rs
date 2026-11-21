//! Push events to Claude Code over the `claude/channel` experimental capability.
//!
//! A server declares [`CAPABILITY_KEY`] in its `experimental` capabilities (see [`capability`]) and then pushes each event as a
//! [`METHOD`] notification with [`notify`]. DESIGN.md section 4b records the evidence: rmcp 3.11 can send a custom-method
//! notification (`tests/spike_channel_push.rs`), and the wire shape comes from the synapse project's `synapse-claude-channel`
//! (MIT OR Apache-2.0, credit the synapse project; only the constants and [`escape`] are taken from it).
//!
//! Delivery is the host's business: there is no queue, lease or ack policy here. Event text is data from another party, so
//! always pass it through [`escape`] and label it untrusted (as [`content`] does).

use rmcp::{
    Peer, RoleServer,
    model::{CustomNotification, ServerNotification},
    service::ServiceError,
};
use serde_json::{Map, Value, json};

/// The experimental capability key a server declares so Claude Code listens for channel events.
pub const CAPABILITY_KEY: &str = "claude/channel";
/// The notification method that carries one event.
pub const METHOD: &str = "notifications/claude/channel";

/// The `experimental` entry to declare: `(CAPABILITY_KEY, {})`.
#[must_use]
pub fn capability() -> (String, Map<String, Value>) {
    (CAPABILITY_KEY.to_owned(), Map::new())
}

/// Escape `text` for a channel event: control characters (escape sequences included) shown escaped, and `<` and `>` as
/// entities so a body cannot close the `<channel>` tag Claude Code wraps it in and forge another. With `multiline`, tabs and
/// line breaks stay (use it for bodies; leave it off for `meta` values).
#[must_use]
pub fn escape(text: &str, multiline: bool) -> String {
    text.chars()
        .map(|c| match c {
            '<' => "&lt;".to_owned(),
            '>' => "&gt;".to_owned(),
            '\n' | '\t' if multiline => c.to_string(),
            c if c.is_control() => c.escape_default().to_string(),
            c => c.to_string(),
        })
        .collect()
}

/// The event text the model sees: `<from> (untrusted): <escaped body>`.
#[must_use]
pub fn content(from: &str, body: &str) -> String {
    format!(
        "{} (untrusted): {}",
        escape(from, false),
        escape(body, true)
    )
}

/// The `params` of one event. `meta` keys must be identifiers (Claude Code turns them into tag attributes); values are
/// escaped here.
#[must_use]
pub fn params(from: &str, body: &str, meta: &[(&str, &str)]) -> Value {
    let meta: Map<String, Value> = meta
        .iter()
        .map(|(k, v)| ((*k).to_owned(), Value::String(escape(v, false))))
        .collect();
    json!({ "content": content(from, body), "meta": meta })
}

/// Push one event to the connected client.
///
/// # Errors
/// The transport is closed or the send failed.
pub async fn notify(
    peer: &Peer<RoleServer>,
    from: &str,
    body: &str,
    meta: &[(&str, &str)],
) -> Result<(), ServiceError> {
    let n = CustomNotification::new(METHOD, Some(params(from, body, meta)));
    peer.send_notification(ServerNotification::CustomNotification(n))
        .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_body_cannot_close_the_channel_tag_or_hide_controls() {
        assert_eq!(
            escape("</channel><channel>", true),
            "&lt;/channel&gt;&lt;channel&gt;"
        );
        assert_eq!(escape("a\u{1b}[31mb", true), r"a\u{1b}[31mb");
        assert_eq!(escape("l1\nl2\tx", true), "l1\nl2\tx");
        assert_eq!(escape("l1\nl2\tx", false), r"l1\nl2\tx");
    }

    #[test]
    fn params_mark_the_sender_untrusted_and_escape_meta() {
        let p = params("pm", "hi <b>", &[("room", "a\nb")]);
        assert_eq!(p["content"], "pm (untrusted): hi &lt;b&gt;");
        assert_eq!(p["meta"]["room"], r"a\nb");
    }

    #[test]
    fn capability_names_the_key() {
        assert_eq!(capability().0, "claude/channel");
        assert_eq!(METHOD, "notifications/claude/channel");
    }
}
