use elements::BlockHash;
use serde::Deserialize;

use crate::{descriptor::url_encode_descriptor, Error};

/// An update received from the Waterfalls descriptor subscription stream.
///
/// The event is a hint. `Tip` only means the chain tip changed, while `Mempool`,
/// `Block`, and `Reorg` indicate that subscribed wallet scripts may have changed
/// and callers should schedule a normal Waterfalls scan.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct WaterfallsSubscriptionEvent {
    /// The kind of subscription event.
    #[serde(rename = "type")]
    pub kind: WaterfallsSubscriptionEventKind,

    /// The Waterfalls server tip observed when the event was emitted.
    #[serde(default)]
    pub tip: Option<WaterfallsSubscriptionTip>,
}

/// The kind of Waterfalls subscription event.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum WaterfallsSubscriptionEventKind {
    /// A new block was indexed, but no watched script changed for this subscription.
    Tip,

    /// A watched script changed in the mempool.
    Mempool,

    /// A new block was indexed and a watched script changed for this subscription.
    Block,

    /// A chain reorganization happened.
    Reorg,
}

/// Chain tip metadata included in Waterfalls subscription events.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct WaterfallsSubscriptionTip {
    /// The tip block height.
    pub height: u32,

    /// The tip block hash.
    pub block_hash: BlockHash,

    /// The tip block timestamp.
    pub timestamp: u32,
}

pub(crate) fn waterfalls_subscribe_url(base_url: &str, descriptor: &str) -> String {
    format!(
        "{}/v1/subscribe?descriptor={}",
        base_url.trim_end_matches('/'),
        url_encode_descriptor(descriptor)
    )
}

#[derive(Default)]
#[allow(dead_code)] // Staged for the streaming subscription handle.
pub(crate) struct WaterfallsSseParser {
    buffer: String,
    event_name: Option<String>,
    data: String,
}

#[allow(dead_code)] // Staged for the streaming subscription handle.
impl WaterfallsSseParser {
    pub(crate) fn push_str(
        &mut self,
        chunk: &str,
    ) -> Result<Vec<WaterfallsSubscriptionEvent>, Error> {
        self.buffer.push_str(chunk);
        let mut events = Vec::new();

        while let Some(line_end) = self.buffer.find('\n') {
            let mut line = self.buffer.drain(..=line_end).collect::<String>();
            if line.ends_with('\n') {
                line.pop();
            }
            if line.ends_with('\r') {
                line.pop();
            }

            if let Some(event) = self.process_line(&line)? {
                events.push(event);
            }
        }

        Ok(events)
    }

    fn process_line(&mut self, line: &str) -> Result<Option<WaterfallsSubscriptionEvent>, Error> {
        if line.is_empty() {
            return self.dispatch_event();
        }

        if line.starts_with(':') {
            return Ok(None);
        }

        let (field, value) = line.split_once(':').unwrap_or((line, ""));
        let value = value.strip_prefix(' ').unwrap_or(value);
        match field {
            "event" => self.event_name = Some(value.to_string()),
            "data" => {
                if !self.data.is_empty() {
                    self.data.push('\n');
                }
                self.data.push_str(value);
            }
            _ => {}
        }

        Ok(None)
    }

    fn dispatch_event(&mut self) -> Result<Option<WaterfallsSubscriptionEvent>, Error> {
        let event_name = self.event_name.take();
        let data = std::mem::take(&mut self.data);

        if data.is_empty() || event_name.as_deref() != Some("update") {
            return Ok(None);
        }

        Ok(Some(serde_json::from_str(&data)?))
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use elements::BlockHash;

    use super::{
        waterfalls_subscribe_url, WaterfallsSseParser, WaterfallsSubscriptionEventKind,
        WaterfallsSubscriptionTip,
    };

    fn test_tip() -> WaterfallsSubscriptionTip {
        WaterfallsSubscriptionTip {
            height: 42,
            block_hash: BlockHash::from_str(
                "0000000000000000000000000000000000000000000000000000000000000001",
            )
            .unwrap(),
            timestamp: 123,
        }
    }

    #[test]
    fn subscribe_url_encodes_descriptor() {
        let url = waterfalls_subscribe_url("https://example.test/api/", "ct(foo/<0;1>/*)#abcd1234");
        assert_eq!(
            url,
            "https://example.test/api/v1/subscribe?descriptor=ct%28foo%2F%3C0%3B1%3E%2F%2A%29%23abcd1234"
        );
    }

    #[test]
    fn sse_parser_ignores_ready_comment() {
        let mut parser = WaterfallsSseParser::default();
        let events = parser.push_str(": ready\n\n").unwrap();
        assert!(events.is_empty());
    }

    #[test]
    fn sse_parser_handles_chunk_boundaries() {
        let mut parser = WaterfallsSseParser::default();
        let first = parser
            .push_str("event: update\ndata: {\"type\":\"ti")
            .unwrap();
        assert!(first.is_empty());

        let second = parser
            .push_str(
                "p\",\"tip\":{\"height\":42,\"block_hash\":\"0000000000000000000000000000000000000000000000000000000000000001\",\"timestamp\":123}}\n\n",
            )
            .unwrap();

        assert_eq!(second.len(), 1);
        assert_eq!(second[0].kind, WaterfallsSubscriptionEventKind::Tip);
        assert_eq!(second[0].tip, Some(test_tip()));
    }

    #[test]
    fn sse_parser_handles_multiple_events() {
        let mut parser = WaterfallsSseParser::default();
        let events = parser
            .push_str(
                "event: update\ndata: {\"type\":\"mempool\"}\n\nevent: update\ndata: {\"type\":\"block\"}\n\n",
            )
            .unwrap();

        assert_eq!(events.len(), 2);
        assert_eq!(events[0].kind, WaterfallsSubscriptionEventKind::Mempool);
        assert_eq!(events[1].kind, WaterfallsSubscriptionEventKind::Block);
    }

    #[test]
    fn sse_parser_ignores_non_update_events() {
        let mut parser = WaterfallsSseParser::default();
        let events = parser
            .push_str("event: message\ndata: {\"type\":\"tip\"}\n\n")
            .unwrap();
        assert!(events.is_empty());
    }

    #[test]
    fn sse_parser_rejects_malformed_json() {
        let mut parser = WaterfallsSseParser::default();
        let err = parser
            .push_str("event: update\ndata: {\"type\":\n\n")
            .unwrap_err();
        assert!(err.to_string().contains("EOF"));
    }

    #[test]
    fn sse_parser_waits_for_blank_line() {
        let mut parser = WaterfallsSseParser::default();
        let events = parser
            .push_str("event: update\ndata: {\"type\":\"reorg\"}\n")
            .unwrap();
        assert!(events.is_empty());
    }
}
