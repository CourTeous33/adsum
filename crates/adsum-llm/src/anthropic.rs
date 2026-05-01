//! Anthropic Messages API streaming provider.
//!
//! Endpoint: POST https://api.anthropic.com/v1/messages
//! Auth: x-api-key header
//! Streaming: SSE; we care about content_block_delta (text) and message_stop
//! (terminator). All other event types are ignored.

use crate::ProviderError;
use adsum_state::{Message, Role};
use eventsource_stream::Eventsource;
use futures_util::{Stream, StreamExt};
use reqwest::Client;
use serde::{Deserialize, Serialize};

const ENDPOINT: &str = "https://api.anthropic.com/v1/messages";
const ANTHROPIC_VERSION: &str = "2023-06-01";
const MAX_TOKENS: u32 = 4096;

#[derive(Serialize)]
struct RequestBody<'a> {
    model: &'a str,
    system: &'a str,
    messages: Vec<RequestMessage<'a>>,
    stream: bool,
    max_tokens: u32,
}

#[derive(Serialize)]
struct RequestMessage<'a> {
    role: &'static str,
    content: &'a str,
}

pub async fn stream(
    client: &Client,
    key: &str,
    model: &str,
    messages: &[Message],
    system: &str,
) -> Result<impl Stream<Item = Result<String, ProviderError>>, ProviderError> {
    let body = RequestBody {
        model,
        system,
        messages: messages
            .iter()
            .map(|m| RequestMessage {
                role: match m.role {
                    Role::User => "user",
                    Role::Assistant => "assistant",
                },
                content: &m.content,
            })
            .collect(),
        stream: true,
        max_tokens: MAX_TOKENS,
    };

    let response = client
        .post(ENDPOINT)
        .header("x-api-key", key)
        .header("anthropic-version", ANTHROPIC_VERSION)
        .header("content-type", "application/json")
        .json(&body)
        .send()
        .await
        .map_err(|e| ProviderError {
            code: classify_reqwest_error(&e),
            message: e.to_string(),
        })?;

    let status = response.status();
    if !status.is_success() {
        let text = response.text().await.unwrap_or_default();
        return Err(ProviderError {
            code: classify_status(status.as_u16()),
            message: friendly_message(status.as_u16(), &text),
        });
    }

    let byte_stream = response.bytes_stream();
    let event_stream = byte_stream.eventsource();
    Ok(parse_event_stream(event_stream))
}

fn parse_event_stream<S>(events: S) -> impl Stream<Item = Result<String, ProviderError>>
where
    S: Stream<Item = Result<eventsource_stream::Event, eventsource_stream::EventStreamError<reqwest::Error>>>
        + Unpin,
{
    use futures_util::stream::unfold;
    unfold(events, |mut events| async move {
        loop {
            match events.next().await {
                None => return None,
                Some(Err(e)) => {
                    return Some((
                        Err(ProviderError {
                            code: "decode".into(),
                            message: format!("Failed to parse stream from Anthropic: {e}"),
                        }),
                        events,
                    ));
                }
                Some(Ok(event)) => {
                    if let Some(text) = parse_anthropic_event(&event.event, &event.data) {
                        return Some((Ok(text), events));
                    }
                    // Non-text event (ping, message_start, content_block_start/stop,
                    // message_delta, message_stop) — keep looping.
                }
            }
        }
    })
}

#[derive(Deserialize)]
struct ContentBlockDeltaEnvelope {
    delta: ContentBlockDelta,
}

#[derive(Deserialize)]
#[serde(tag = "type")]
enum ContentBlockDelta {
    #[serde(rename = "text_delta")]
    TextDelta { text: String },
    #[serde(other)]
    Other,
}

/// Returns the text payload of a `content_block_delta` event whose delta is a
/// `text_delta`. Returns None for any other event type (ping, message_*, etc.)
/// or any non-text delta.
pub(crate) fn parse_anthropic_event(event_name: &str, data: &str) -> Option<String> {
    if event_name != "content_block_delta" {
        return None;
    }
    let envelope: ContentBlockDeltaEnvelope = serde_json::from_str(data).ok()?;
    match envelope.delta {
        ContentBlockDelta::TextDelta { text } => Some(text),
        ContentBlockDelta::Other => None,
    }
}

fn classify_status(code: u16) -> String {
    match code {
        401 | 403 => code.to_string(),
        429 => "rate_limited".into(),
        500..=599 => "5xx".into(),
        _ => code.to_string(),
    }
}

fn classify_reqwest_error(e: &reqwest::Error) -> String {
    if e.is_timeout() || e.is_connect() {
        "network".into()
    } else if e.is_decode() {
        "decode".into()
    } else {
        "network".into()
    }
}

fn friendly_message(code: u16, body: &str) -> String {
    match code {
        401 | 403 => "Invalid API key — check Settings".into(),
        429 => "Rate limited by Anthropic. Try again shortly.".into(),
        500..=599 => format!("Anthropic returned {code}: {body}"),
        _ => format!("HTTP {code}: {body}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_text_delta_yields_text() {
        let data = r#"{"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"Hello"}}"#;
        let got = parse_anthropic_event("content_block_delta", data);
        assert_eq!(got.as_deref(), Some("Hello"));
    }

    #[test]
    fn parse_ping_returns_none() {
        let got = parse_anthropic_event("ping", r#"{"type":"ping"}"#);
        assert_eq!(got, None);
    }

    #[test]
    fn parse_message_stop_returns_none() {
        let got = parse_anthropic_event("message_stop", r#"{"type":"message_stop"}"#);
        assert_eq!(got, None);
    }

    #[test]
    fn parse_non_text_delta_returns_none() {
        // input_json_delta etc. — variants we don't surface as text.
        let data = r#"{"type":"content_block_delta","index":0,"delta":{"type":"input_json_delta","partial_json":"{}"}}"#;
        let got = parse_anthropic_event("content_block_delta", data);
        assert_eq!(got, None);
    }

    #[test]
    fn parse_malformed_data_returns_none() {
        let got = parse_anthropic_event("content_block_delta", "not json at all");
        assert_eq!(got, None);
    }

    #[test]
    fn classify_status_buckets_correctly() {
        assert_eq!(classify_status(401), "401");
        assert_eq!(classify_status(403), "403");
        assert_eq!(classify_status(429), "rate_limited");
        assert_eq!(classify_status(500), "5xx");
        assert_eq!(classify_status(503), "5xx");
    }
}
