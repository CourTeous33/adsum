//! Anthropic Messages API streaming provider.
//!
//! Endpoint: POST https://api.anthropic.com/v1/messages
//! Auth: x-api-key header
//! Streaming: SSE; we care about content_block_delta (text) and message_stop
//! (terminator). All other event types are ignored.

use crate::ProviderError;
use adsum_state::Block;
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

/// Translate `&[Block]` into the existing single-shot wire format that
/// `stream()` already sends. Skips `Block::SkillInvocation`. Tool blocks
/// don't appear in this single-shot path; they trigger a `debug_assert!`.
fn blocks_to_v1_messages(blocks: &[Block]) -> Vec<RequestMessage<'_>> {
    blocks
        .iter()
        .filter_map(|b| match b {
            Block::UserText { text } => Some(RequestMessage {
                role: "user",
                content: text,
            }),
            Block::AssistantText { text } => Some(RequestMessage {
                role: "assistant",
                content: text,
            }),
            Block::SkillInvocation { .. } => None,
            Block::ToolUse { .. } | Block::ToolResult { .. } => {
                debug_assert!(false, "v1 single-shot path does not handle tool blocks");
                None
            }
        })
        .collect()
}

pub async fn stream(
    client: &Client,
    key: &str,
    model: &str,
    blocks: &[Block],
    system: &str,
) -> Result<impl Stream<Item = Result<String, ProviderError>>, ProviderError> {
    let body = RequestBody {
        model,
        system,
        messages: blocks_to_v1_messages(blocks),
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
    S: Stream<
            Item = Result<
                eventsource_stream::Event,
                eventsource_stream::EventStreamError<reqwest::Error>,
            >,
        > + Unpin,
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

use crate::{ProviderEvent, StopReason};
use adsum_tools::ToolSchema;

#[allow(dead_code)]
#[derive(Serialize)]
struct AgentRequestBody<'a> {
    model: &'a str,
    system: &'a str,
    messages: Vec<AgentMessage<'a>>,
    tools: &'a [AgentToolDef<'a>],
    stream: bool,
    max_tokens: u32,
}

#[derive(Serialize)]
pub(crate) struct AgentMessage<'a> {
    role: &'static str,
    content: Vec<AgentContent<'a>>,
}

#[allow(dead_code)]
#[derive(Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum AgentContent<'a> {
    Text { text: &'a str },
    ToolUse { id: &'a str, name: &'a str, input: &'a serde_json::Value },
    ToolResult { tool_use_id: &'a str, content: &'a str, is_error: bool },
}

#[allow(dead_code)]
#[derive(Serialize)]
struct AgentToolDef<'a> {
    name: &'a str,
    description: &'a str,
    input_schema: &'a serde_json::Value,
}

/// Translate `&[Block]` into Anthropic's wire-format messages. Consecutive
/// blocks of the same role are grouped into one message with a content
/// array. `Block::SkillInvocation` is metadata; skipped.
#[allow(dead_code)]
pub(crate) fn blocks_to_anthropic_messages(blocks: &[Block]) -> Vec<AgentMessage<'_>> {
    let mut out: Vec<AgentMessage> = Vec::new();
    for block in blocks {
        let (role, content) = match block {
            Block::UserText { text } => ("user", AgentContent::Text { text }),
            Block::AssistantText { text } => ("assistant", AgentContent::Text { text }),
            Block::ToolUse { id, name, input } => (
                "assistant",
                AgentContent::ToolUse { id, name, input },
            ),
            Block::ToolResult { tool_use_id, content, is_error } => (
                "user",
                AgentContent::ToolResult { tool_use_id, content, is_error: *is_error },
            ),
            Block::SkillInvocation { .. } => continue,
        };
        match out.last_mut() {
            Some(last) if last.role == role => last.content.push(content),
            _ => out.push(AgentMessage { role, content: vec![content] }),
        }
    }
    out
}

/// Open an SSE stream against Anthropic's Messages API with tool support.
#[allow(dead_code)]
pub async fn agent_stream(
    client: &Client,
    key: &str,
    model: &str,
    blocks: &[Block],
    system: &str,
    tools: &[ToolSchema],
) -> Result<impl Stream<Item = Result<ProviderEvent, ProviderError>>, ProviderError> {
    let messages = blocks_to_anthropic_messages(blocks);
    let tool_defs: Vec<AgentToolDef> = tools
        .iter()
        .map(|t| AgentToolDef {
            name: t.name,
            description: t.description,
            input_schema: &t.input_schema,
        })
        .collect();

    let body = AgentRequestBody {
        model,
        system,
        messages,
        tools: &tool_defs,
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

    let event_stream = response.bytes_stream().eventsource();
    Ok(parse_provider_event_stream(event_stream))
}

#[allow(dead_code)]
fn parse_provider_event_stream<S>(events: S) -> impl Stream<Item = Result<ProviderEvent, ProviderError>>
where
    S: Stream<
            Item = Result<
                eventsource_stream::Event,
                eventsource_stream::EventStreamError<reqwest::Error>,
            >,
        > + Unpin,
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
                    if let Some(provider_event) =
                        parse_anthropic_provider_event(&event.event, &event.data)
                    {
                        return Some((Ok(provider_event), events));
                    }
                    // Non-event-of-interest (ping, message_start, message_stop) — keep looping.
                }
            }
        }
    })
}

#[allow(dead_code)]
#[derive(Deserialize)]
struct ContentBlockStartEnvelope {
    content_block: ContentBlockStart,
}

#[allow(dead_code)]
#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum ContentBlockStart {
    Text { #[serde(default)] text: String },
    ToolUse { id: String, name: String },
    #[serde(other)]
    Other,
}

#[allow(dead_code)]
#[derive(Deserialize)]
struct MessageDeltaEnvelope {
    delta: MessageDeltaInner,
}

#[allow(dead_code)]
#[derive(Deserialize)]
struct MessageDeltaInner {
    #[serde(default)]
    stop_reason: Option<String>,
}

/// Parse one SSE event into a `ProviderEvent`. Returns None for non-meaningful
/// events (ping, message_start, message_stop).
#[allow(dead_code)]
pub(crate) fn parse_anthropic_provider_event(
    event_name: &str,
    data: &str,
) -> Option<ProviderEvent> {
    match event_name {
        "content_block_start" => {
            let env: ContentBlockStartEnvelope = serde_json::from_str(data).ok()?;
            match env.content_block {
                ContentBlockStart::ToolUse { id, name } => {
                    Some(ProviderEvent::ToolUseStart { id, name })
                }
                _ => None,
            }
        }
        "content_block_delta" => {
            let env: ContentBlockDeltaEnvelope = serde_json::from_str(data).ok()?;
            match env.delta {
                ContentBlockDelta::TextDelta { text } => {
                    Some(ProviderEvent::AssistantTextDelta(text))
                }
                ContentBlockDelta::Other => {
                    // Try parsing as input_json_delta.
                    let raw: serde_json::Value = serde_json::from_str(data).ok()?;
                    let partial = raw["delta"]["partial_json"].as_str()?.to_string();
                    Some(ProviderEvent::ToolUseInputDelta(partial))
                }
            }
        }
        "content_block_stop" => {
            // Anthropic doesn't tell us in `content_block_stop` whether the
            // closed block was a tool_use. The agent loop tracks pending
            // tool-use IDs separately; emit a generic close that the loop
            // matches against the most recent ToolUseStart by order.
            let raw: serde_json::Value = serde_json::from_str(data).ok()?;
            let _index = raw.get("index")?;
            Some(ProviderEvent::ToolUseClose { id: String::new() })
        }
        "message_delta" => {
            let env: MessageDeltaEnvelope = serde_json::from_str(data).ok()?;
            let stop = env.delta.stop_reason?;
            let reason = match stop.as_str() {
                "end_turn" => StopReason::EndTurn,
                "tool_use" => StopReason::ToolUse,
                "max_tokens" => StopReason::MaxTokens,
                other => StopReason::Other(other.to_string()),
            };
            Some(ProviderEvent::StopTurn { reason })
        }
        _ => None,
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

    #[test]
    fn parse_tool_use_start_event_yields_tool_use_start() {
        let data = r#"{"type":"content_block_start","index":1,"content_block":{"type":"tool_use","id":"toolu_x","name":"wiki_read","input":{}}}"#;
        let ev = parse_anthropic_provider_event("content_block_start", data);
        match ev {
            Some(crate::ProviderEvent::ToolUseStart { id, name }) => {
                assert_eq!(id, "toolu_x");
                assert_eq!(name, "wiki_read");
            }
            other => panic!("expected ToolUseStart, got {other:?}"),
        }
    }

    #[test]
    fn parse_input_json_delta_yields_tool_use_input_delta() {
        let data = r#"{"type":"content_block_delta","index":1,"delta":{"type":"input_json_delta","partial_json":"{\"slug\":\"foo\"}"}}"#;
        let ev = parse_anthropic_provider_event("content_block_delta", data);
        match ev {
            Some(crate::ProviderEvent::ToolUseInputDelta(json)) => {
                assert_eq!(json, "{\"slug\":\"foo\"}");
            }
            other => panic!("expected ToolUseInputDelta, got {other:?}"),
        }
    }

    #[test]
    fn parse_message_delta_with_tool_use_stop_yields_stop_turn_tool_use() {
        let data = r#"{"type":"message_delta","delta":{"stop_reason":"tool_use","stop_sequence":null},"usage":{}}"#;
        let ev = parse_anthropic_provider_event("message_delta", data);
        match ev {
            Some(crate::ProviderEvent::StopTurn { reason }) => {
                assert_eq!(reason, crate::StopReason::ToolUse);
            }
            other => panic!("expected StopTurn(ToolUse), got {other:?}"),
        }
    }

    #[test]
    fn parse_message_delta_with_end_turn_yields_stop_turn_end_turn() {
        let data = r#"{"type":"message_delta","delta":{"stop_reason":"end_turn"},"usage":{}}"#;
        let ev = parse_anthropic_provider_event("message_delta", data);
        match ev {
            Some(crate::ProviderEvent::StopTurn { reason }) => {
                assert_eq!(reason, crate::StopReason::EndTurn);
            }
            other => panic!("expected StopTurn(EndTurn), got {other:?}"),
        }
    }

    #[test]
    fn blocks_to_anthropic_groups_consecutive_user_blocks() {
        use adsum_state::Block;
        let blocks = vec![
            Block::UserText { text: "hi".into() },
            Block::AssistantText { text: "hello".into() },
            Block::AssistantText { text: " more".into() },
            Block::ToolResult { tool_use_id: "t".into(), content: "ok".into(), is_error: false },
            Block::UserText { text: "next".into() },
        ];
        let msgs = blocks_to_anthropic_messages(&blocks);
        // user(hi) → assistant(hello + more) → user(tool_result + next)
        assert_eq!(msgs.len(), 3);
        assert_eq!(msgs[0].role, "user");
        assert_eq!(msgs[0].content.len(), 1);
        assert_eq!(msgs[1].role, "assistant");
        assert_eq!(msgs[1].content.len(), 2);
        assert_eq!(msgs[2].role, "user");
        assert_eq!(msgs[2].content.len(), 2);
    }

    #[test]
    fn blocks_to_anthropic_skips_skill_invocation() {
        use adsum_state::Block;
        let blocks = vec![
            Block::SkillInvocation { name: "query".into(), args: "x".into() },
            Block::UserText { text: "hi".into() },
        ];
        let msgs = blocks_to_anthropic_messages(&blocks);
        assert_eq!(msgs.len(), 1);
        assert_eq!(msgs[0].role, "user");
    }
}
