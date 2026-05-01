//! OpenAI Chat Completions API streaming provider.
//!
//! Endpoint: POST https://api.openai.com/v1/chat/completions
//! Auth: Authorization: Bearer <key>
//! Streaming: SSE; each `data:` line is a JSON envelope. The terminator is
//! the literal `data: [DONE]` line.

use crate::ProviderError;
use adsum_state::{Message, Role};
use eventsource_stream::Eventsource;
use futures_util::{Stream, StreamExt};
use reqwest::Client;
use serde::{Deserialize, Serialize};

const ENDPOINT: &str = "https://api.openai.com/v1/chat/completions";

#[derive(Serialize)]
struct RequestBody<'a> {
    model: &'a str,
    messages: Vec<RequestMessage<'a>>,
    stream: bool,
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
    let mut req_messages = Vec::with_capacity(messages.len() + 1);
    req_messages.push(RequestMessage {
        role: "system",
        content: system,
    });
    for m in messages {
        req_messages.push(RequestMessage {
            role: match m.role {
                Role::User => "user",
                Role::Assistant => "assistant",
            },
            content: &m.content,
        });
    }

    let body = RequestBody {
        model,
        messages: req_messages,
        stream: true,
    };

    let response = client
        .post(ENDPOINT)
        .bearer_auth(key)
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

    Ok(parse_event_stream(response.bytes_stream().eventsource()))
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
                            message: format!("Failed to parse stream from OpenAI: {e}"),
                        }),
                        events,
                    ));
                }
                Some(Ok(event)) => {
                    if event.data.trim() == "[DONE]" {
                        return None;
                    }
                    if let Some(text) = parse_openai_data(&event.data) {
                        return Some((Ok(text), events));
                    }
                    // Non-text chunk (role-only delta, finish_reason, etc.) — loop.
                }
            }
        }
    })
}

#[derive(Deserialize)]
struct ChunkEnvelope {
    choices: Vec<ChunkChoice>,
}

#[derive(Deserialize)]
struct ChunkChoice {
    delta: ChunkDelta,
}

#[derive(Deserialize)]
struct ChunkDelta {
    #[serde(default)]
    content: Option<String>,
}

/// Parse the JSON `data:` payload of one OpenAI chat-completion stream event.
/// Returns the `choices[0].delta.content` string, or None if the chunk has no
/// content (e.g. role-only delta, tool-call delta, finish chunk).
pub(crate) fn parse_openai_data(data: &str) -> Option<String> {
    let envelope: ChunkEnvelope = serde_json::from_str(data).ok()?;
    let choice = envelope.choices.into_iter().next()?;
    choice.delta.content.filter(|s| !s.is_empty())
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
        429 => "Rate limited by OpenAI. Try again shortly.".into(),
        500..=599 => format!("OpenAI returned {code}: {body}"),
        _ => format!("HTTP {code}: {body}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_content_delta_yields_text() {
        let data = r#"{"id":"x","object":"chat.completion.chunk","choices":[{"index":0,"delta":{"content":"Hello"}}]}"#;
        let got = parse_openai_data(data);
        assert_eq!(got.as_deref(), Some("Hello"));
    }

    #[test]
    fn parse_role_only_delta_returns_none() {
        let data = r#"{"choices":[{"index":0,"delta":{"role":"assistant"}}]}"#;
        let got = parse_openai_data(data);
        assert_eq!(got, None);
    }

    #[test]
    fn parse_finish_chunk_returns_none() {
        let data = r#"{"choices":[{"index":0,"delta":{},"finish_reason":"stop"}]}"#;
        let got = parse_openai_data(data);
        assert_eq!(got, None);
    }

    #[test]
    fn parse_empty_content_returns_none() {
        let data = r#"{"choices":[{"index":0,"delta":{"content":""}}]}"#;
        let got = parse_openai_data(data);
        assert_eq!(got, None);
    }

    #[test]
    fn parse_malformed_returns_none() {
        assert_eq!(parse_openai_data("not json"), None);
        assert_eq!(parse_openai_data(r#"{"foo":"bar"}"#), None);
    }
}
