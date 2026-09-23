//! Anthropic Messages API adapter for the provider-neutral Round Table port.
//!
//! The adapter reads its API key only from `ANTHROPIC_API_KEY` at construction.
//! It never logs that value, prompts, response bodies or authorization headers.
#![forbid(unsafe_code)]

use maia_roundtable::{
    ModelProvider, ModelRef, Participant, ParticipantDescriptor, ParticipantFailure,
    ParticipantFailureKind, ParticipantId, ParticipantRequest, ParticipantResponse,
    UsageCostMetadata,
};
use serde::{Deserialize, Serialize};
use std::{env, time::Duration};

const API_VERSION: &str = "2023-06-01";
const DEFAULT_ENDPOINT: &str = "https://api.anthropic.com/v1/messages";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnthropicConfig {
    pub model: String,
    pub endpoint: String,
    pub timeout: Duration,
    pub transient_retries: u8,
}
impl AnthropicConfig {
    pub fn from_environment() -> Result<Self, AnthropicConfigError> {
        let model = env::var("MAIA_ANTHROPIC_MODEL")
            .map_err(|_| AnthropicConfigError::MissingEnvironment("MAIA_ANTHROPIC_MODEL"))?;
        if model.trim().is_empty() {
            return Err(AnthropicConfigError::MissingEnvironment(
                "MAIA_ANTHROPIC_MODEL",
            ));
        }
        let endpoint = env::var("MAIA_ANTHROPIC_MESSAGES_ENDPOINT")
            .unwrap_or_else(|_| DEFAULT_ENDPOINT.into());
        let timeout_seconds = env::var("MAIA_ANTHROPIC_TIMEOUT_SECONDS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(30);
        Ok(Self {
            model,
            endpoint,
            timeout: Duration::from_secs(timeout_seconds),
            transient_retries: 1,
        })
    }
}
fn environment_api_key() -> Result<String, AnthropicConfigError> {
    let api_key = env::var("ANTHROPIC_API_KEY")
        .map_err(|_| AnthropicConfigError::MissingEnvironment("ANTHROPIC_API_KEY"))?;
    if api_key.trim().is_empty() {
        return Err(AnthropicConfigError::MissingEnvironment(
            "ANTHROPIC_API_KEY",
        ));
    }
    Ok(api_key)
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AnthropicConfigError {
    MissingEnvironment(&'static str),
    InvalidConfiguration,
}
impl std::fmt::Display for AnthropicConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for AnthropicConfigError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpReply {
    pub status: u16,
    pub request_id: Option<String>,
    pub body: String,
}
pub trait AnthropicTransport: Send + Sync {
    fn post_messages(
        &self,
        endpoint: &str,
        api_key: &str,
        payload: &str,
        timeout: Duration,
    ) -> Result<HttpReply, TransportError>;
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransportError {
    Timeout,
    Connect,
    Other,
}

pub struct ReqwestTransport {
    client: reqwest::blocking::Client,
}
impl ReqwestTransport {
    pub fn new(timeout: Duration) -> Result<Self, AnthropicConfigError> {
        Ok(Self {
            client: reqwest::blocking::Client::builder()
                .timeout(timeout)
                .build()
                .map_err(|_| AnthropicConfigError::InvalidConfiguration)?,
        })
    }
}
impl AnthropicTransport for ReqwestTransport {
    fn post_messages(
        &self,
        endpoint: &str,
        api_key: &str,
        payload: &str,
        timeout: Duration,
    ) -> Result<HttpReply, TransportError> {
        let response = self
            .client
            .post(endpoint)
            .timeout(timeout)
            .header("content-type", "application/json")
            .header("x-api-key", api_key)
            .header("anthropic-version", API_VERSION)
            .body(payload.to_owned())
            .send()
            .map_err(|e| {
                if e.is_timeout() {
                    TransportError::Timeout
                } else if e.is_connect() {
                    TransportError::Connect
                } else {
                    TransportError::Other
                }
            })?;
        let status = response.status().as_u16();
        let request_id = response
            .headers()
            .get("request-id")
            .and_then(|v| v.to_str().ok())
            .map(str::to_owned);
        let body = response.text().map_err(|_| TransportError::Other)?;
        Ok(HttpReply {
            status,
            request_id,
            body,
        })
    }
}

pub struct AnthropicParticipant<T: AnthropicTransport> {
    descriptor: ParticipantDescriptor,
    config: AnthropicConfig,
    api_key: String,
    transport: T,
}
impl AnthropicParticipant<ReqwestTransport> {
    pub fn from_environment() -> Result<Self, AnthropicConfigError> {
        let config = AnthropicConfig::from_environment()?;
        let api_key = environment_api_key()?;
        let transport = ReqwestTransport::new(config.timeout)?;
        Self::new(config, api_key, transport)
    }
}
impl<T: AnthropicTransport> AnthropicParticipant<T> {
    pub fn new(
        config: AnthropicConfig,
        api_key: String,
        transport: T,
    ) -> Result<Self, AnthropicConfigError> {
        if config.model.trim().is_empty()
            || config.endpoint.trim().is_empty()
            || api_key.trim().is_empty()
        {
            return Err(AnthropicConfigError::InvalidConfiguration);
        }
        Ok(Self {
            descriptor: ParticipantDescriptor {
                id: ParticipantId::new("anthropic-primary")
                    .map_err(|_| AnthropicConfigError::InvalidConfiguration)?,
                provider: ModelProvider::new("anthropic")
                    .map_err(|_| AnthropicConfigError::InvalidConfiguration)?,
                model: ModelRef::new(config.model.clone())
                    .map_err(|_| AnthropicConfigError::InvalidConfiguration)?,
            },
            config,
            api_key,
            transport,
        })
    }
}
impl<T: AnthropicTransport> Participant for AnthropicParticipant<T> {
    fn descriptor(&self) -> ParticipantDescriptor {
        self.descriptor.clone()
    }
    fn invoke(
        &self,
        request: ParticipantRequest,
    ) -> Result<ParticipantResponse, ParticipantFailure> {
        let payload = serde_json::to_string(&MessagesRequest {
            model: &self.config.model,
            max_tokens: request.max_output_tokens,
            messages: vec![Message {
                role: "user",
                content: &request.decision.prompt,
            }],
        })
        .map_err(|_| failure(ParticipantFailureKind::Permanent, None))?;
        let mut attempt = 0;
        loop {
            let reply = self.transport.post_messages(
                &self.config.endpoint,
                &self.api_key,
                &payload,
                self.config.timeout,
            );
            match reply {
                Ok(reply) if (200..300).contains(&reply.status) => {
                    return parse_success(self.descriptor(), reply);
                }
                Ok(reply)
                    if safe_transient_status(reply.status)
                        && attempt < self.config.transient_retries =>
                {
                    attempt += 1;
                    continue;
                }
                Ok(reply) => {
                    return Err(failure(
                        if safe_transient_status(reply.status) {
                            ParticipantFailureKind::Transient
                        } else {
                            ParticipantFailureKind::Permanent
                        },
                        reply.request_id,
                    ));
                }
                Err(error)
                    if matches!(error, TransportError::Timeout | TransportError::Connect)
                        && attempt < self.config.transient_retries =>
                {
                    attempt += 1;
                    continue;
                }
                Err(error) => {
                    return Err(failure(
                        if matches!(error, TransportError::Timeout | TransportError::Connect) {
                            ParticipantFailureKind::Transient
                        } else {
                            ParticipantFailureKind::Permanent
                        },
                        None,
                    ));
                }
            }
        }
    }
}
fn safe_transient_status(status: u16) -> bool {
    matches!(status, 408 | 429 | 500 | 502 | 503 | 504)
}
fn failure(
    kind: ParticipantFailureKind,
    provider_request_id: Option<String>,
) -> ParticipantFailure {
    ParticipantFailure {
        kind,
        provider_request_id,
    }
}
#[derive(Serialize)]
struct MessagesRequest<'a> {
    model: &'a str,
    max_tokens: u32,
    messages: Vec<Message<'a>>,
}
#[derive(Serialize)]
struct Message<'a> {
    role: &'a str,
    content: &'a str,
}
#[derive(Deserialize)]
struct MessagesResponse {
    content: Vec<ContentBlock>,
    usage: Option<Usage>,
}
#[derive(Deserialize)]
struct ContentBlock {
    #[serde(rename = "type")]
    kind: String,
    text: Option<String>,
}
#[derive(Deserialize)]
struct Usage {
    input_tokens: Option<u64>,
    output_tokens: Option<u64>,
}
fn parse_success(
    descriptor: ParticipantDescriptor,
    reply: HttpReply,
) -> Result<ParticipantResponse, ParticipantFailure> {
    let parsed: MessagesResponse = serde_json::from_str(&reply.body)
        .map_err(|_| failure(ParticipantFailureKind::Permanent, reply.request_id.clone()))?;
    let response_text = parsed
        .content
        .into_iter()
        .filter(|block| block.kind == "text")
        .filter_map(|block| block.text)
        .collect::<Vec<_>>()
        .join("\n");
    if response_text.is_empty() {
        return Err(failure(ParticipantFailureKind::Permanent, reply.request_id));
    }
    let usage = parsed.usage.map(|usage| UsageCostMetadata {
        input_tokens: usage.input_tokens,
        output_tokens: usage.output_tokens,
        cost_known: false,
        cost_minor: None,
        currency: None,
    });
    Ok(ParticipantResponse {
        participant: descriptor,
        response_text,
        evidence: vec![],
        provider_request_id: reply.request_id,
        // The Messages response does not report which model served the request
        // in a form this adapter captures, so it stays unknown rather than being
        // backfilled from the configured model.
        model_ref_used: None,
        usage,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use maia_roundtable::{DecisionRequest, ParticipantRequest};
    use std::sync::Mutex;
    struct FakeTransport {
        replies: Mutex<Vec<Result<HttpReply, TransportError>>>,
    }
    impl AnthropicTransport for FakeTransport {
        fn post_messages(
            &self,
            _: &str,
            _: &str,
            _: &str,
            _: Duration,
        ) -> Result<HttpReply, TransportError> {
            self.replies.lock().unwrap().remove(0)
        }
    }
    fn participant(
        replies: Vec<Result<HttpReply, TransportError>>,
    ) -> AnthropicParticipant<FakeTransport> {
        AnthropicParticipant::new(
            AnthropicConfig {
                model: "configured-claude-model".into(),
                endpoint: "https://example.invalid/v1/messages".into(),
                timeout: Duration::from_secs(1),
                transient_retries: 1,
            },
            "fixture-value".into(),
            FakeTransport {
                replies: Mutex::new(replies),
            },
        )
        .unwrap()
    }
    fn request() -> ParticipantRequest {
        ParticipantRequest {
            session_id: "s".into(),
            decision: DecisionRequest {
                id: "d".into(),
                subject: "synthetic".into(),
                prompt: "synthetic safe smoke prompt".into(),
                evidence: vec![],
            },
            max_output_tokens: 32,
        }
    }
    #[test]
    fn captures_request_id_and_usage_without_cost_invention() {
        let response = participant(vec![Ok(HttpReply { status: 200, request_id: Some("req_test".into()), body: r#"{"content":[{"type":"text","text":"answer"}],"usage":{"input_tokens":3,"output_tokens":5}}"#.into() })]).invoke(request()).unwrap();
        assert_eq!(response.provider_request_id.as_deref(), Some("req_test"));
        assert_eq!(response.usage.unwrap().output_tokens, Some(5));
    }
    #[test]
    fn retries_only_safe_transient_failure_once() {
        let response = participant(vec![
            Err(TransportError::Timeout),
            Ok(HttpReply {
                status: 200,
                request_id: None,
                body: r#"{"content":[{"type":"text","text":"answer"}]}"#.into(),
            }),
        ])
        .invoke(request());
        assert!(response.is_ok());
    }
    #[test]
    fn no_hidden_fallback_on_permanent_failure() {
        let error = participant(vec![Ok(HttpReply {
            status: 401,
            request_id: Some("req_auth".into()),
            body: "{}".into(),
        })])
        .invoke(request())
        .unwrap_err();
        assert_eq!(error.kind, ParticipantFailureKind::Permanent);
        assert_eq!(error.provider_request_id.as_deref(), Some("req_auth"));
    }
}
