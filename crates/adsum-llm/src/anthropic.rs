//! Anthropic Messages API streaming provider.

use crate::ProviderError;
use adsum_state::Message;
use futures_util::Stream;
use reqwest::Client;

pub async fn stream(
    _client: &Client,
    _key: &str,
    _model: &str,
    _messages: &[Message],
    _system: &str,
) -> Result<impl Stream<Item = Result<String, ProviderError>>, ProviderError> {
    // Replaced in Task 10 with the real Anthropic SSE provider.
    Ok(futures_util::stream::iter(Vec::<Result<String, ProviderError>>::new()))
}
