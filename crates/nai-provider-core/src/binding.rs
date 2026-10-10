//! Provider binding: the single place where a **configuration choice** becomes a
//! concrete wire adapter.
//!
//! The kernel and the pipeline never name a vendor. They hold one of the
//! capability clients below and call one method; which vendor answers is decided
//! entirely by [`ProviderProfile::protocol`], which is parsed from configuration
//! text. Adding a provider therefore means adding an adapter and one protocol
//! arm, and swapping providers means editing configuration — not the caller.
//!
//! Two properties are enforced here rather than documented as intentions:
//!
//! * The credential presentation and the provider-specific static headers are
//!   **derived from the protocol**, so configuration names a protocol instead of
//!   spelling out header names. A caller cannot half-describe a provider.
//! * A protocol is asked only for a capability it has. Selecting a speech-only
//!   provider for text generation is an explicit error, never a silent
//!   degradation to some default vendor.
use crate::adapters::{AnthropicAdapter, ElevenLabsAdapter, OpenAiAdapter};
use crate::client::{AuthScheme, HttpConfig, ProviderClient, ProviderError};
use crate::gateway::UsageGateway;
use nai_contracts::v1::{ErrorCode, LlmChunk, LlmRequest, TtsChunk, TtsRequest, UsageLedgerEntry};
use nai_security::{Operation, Secret};
use std::time::Duration;

/// Wire protocol of one provider endpoint, as named in configuration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Protocol {
    /// OpenAI-compatible: chat-completions SSE, `/audio/speech` binary,
    /// `/embeddings` JSON, `Authorization: Bearer`.
    OpenAiCompatible,
    /// Anthropic Messages SSE on `/messages`, `x-api-key` plus a version header.
    AnthropicMessages,
    /// ElevenLabs speech on `/v1/text-to-speech/{voice}`, `xi-api-key`.
    ElevenLabs,
}

/// What a protocol can be asked to do.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Capability {
    Llm,
    Speech,
}

impl Protocol {
    /// Parses the stable configuration name. An unknown name is refused: a
    /// typo must not silently select a default vendor.
    pub fn from_config(name: &str) -> Result<Self, ProviderError> {
        match name {
            "openai" | "openai-compatible" => Ok(Self::OpenAiCompatible),
            "anthropic" | "anthropic-messages" => Ok(Self::AnthropicMessages),
            "elevenlabs" => Ok(Self::ElevenLabs),
            _ => Err(ProviderError::invalid_argument("unknown provider protocol")),
        }
    }

    /// The configuration name this protocol is written as.
    pub fn as_config(self) -> &'static str {
        match self {
            Self::OpenAiCompatible => "openai",
            Self::AnthropicMessages => "anthropic",
            Self::ElevenLabs => "elevenlabs",
        }
    }

    pub fn capability(self) -> Capability {
        match self {
            Self::OpenAiCompatible => Capability::Llm,
            Self::AnthropicMessages => Capability::Llm,
            Self::ElevenLabs => Capability::Speech,
        }
    }

    /// Whether this protocol serves both text and speech through the same
    /// endpoint and credential.
    fn serves(self, capability: Capability) -> bool {
        match capability {
            Capability::Llm => matches!(self, Self::OpenAiCompatible | Self::AnthropicMessages),
            Capability::Speech => {
                matches!(self, Self::OpenAiCompatible | Self::ElevenLabs)
            }
        }
    }

    /// How the protocol expects its credential. Derived, never configured.
    fn auth(self) -> AuthScheme {
        match self {
            Self::OpenAiCompatible => AuthScheme::Bearer,
            Self::AnthropicMessages => AuthScheme::Header {
                name: "x-api-key".into(),
            },
            Self::ElevenLabs => AuthScheme::Header {
                name: "xi-api-key".into(),
            },
        }
    }

    /// Provider-mandated static headers. Derived, never configured.
    fn static_headers(self) -> Vec<(String, String)> {
        match self {
            Self::AnthropicMessages => vec![(
                "anthropic-version".into(),
                crate::adapters::anthropic::ANTHROPIC_VERSION.into(),
            )],
            _ => Vec::new(),
        }
    }
}

/// Everything configuration says about one provider endpoint. It carries no
/// credential: the composition root supplies that separately, so a profile can
/// be logged, diffed and compared in a test without exposing a secret.
#[derive(Debug, Clone, PartialEq)]
pub struct ProviderProfile {
    /// Stable identity recorded in usage charges. Two endpoints must not share
    /// one identity, or the ledger cannot tell them apart after a swap.
    pub provider_ref: String,
    pub endpoint: String,
    pub model: String,
    pub price_revision: String,
    pub currency: String,
    pub timeout: Duration,
    pub max_retries: u32,
    pub protocol: Protocol,
}

impl ProviderProfile {
    /// Builds the transport configuration for this profile. The credential
    /// presentation and the provider headers come from the protocol, so the
    /// caller only supplies the secret itself.
    pub fn http_config(&self, credential: Secret) -> HttpConfig {
        HttpConfig {
            provider_ref: self.provider_ref.clone(),
            endpoint: self.endpoint.clone(),
            api_key: Some(credential),
            auth: self.protocol.auth(),
            extra_headers: self.protocol.static_headers(),
            model: self.model.clone(),
            price_revision: self.price_revision.clone(),
            currency: self.currency.clone(),
            timeout: self.timeout,
            max_retries: self.max_retries,
        }
    }

    pub fn client(&self, credential: Secret) -> Result<ProviderClient, ProviderError> {
        ProviderClient::new(self.http_config(credential))
    }

    /// The text-generation client for this profile, or an explicit error when
    /// the configured protocol has no text capability.
    pub fn llm<'a>(&self, client: &'a ProviderClient) -> Result<LlmClient<'a>, ProviderError> {
        self.require(Capability::Llm)?;
        Ok(match self.protocol {
            Protocol::OpenAiCompatible => LlmClient::OpenAi(OpenAiAdapter::new(client)),
            Protocol::AnthropicMessages => LlmClient::Anthropic(AnthropicAdapter::new(client)),
            Protocol::ElevenLabs => unreachable!("capability is checked above"),
        })
    }

    /// The speech client for this profile, or an explicit error when the
    /// configured protocol has no speech capability.
    pub fn speech<'a>(
        &self,
        client: &'a ProviderClient,
    ) -> Result<SpeechClient<'a>, ProviderError> {
        self.require(Capability::Speech)?;
        Ok(match self.protocol {
            Protocol::OpenAiCompatible => SpeechClient::OpenAi(OpenAiAdapter::new(client)),
            Protocol::ElevenLabs => SpeechClient::ElevenLabs(ElevenLabsAdapter::new(client)),
            Protocol::AnthropicMessages => unreachable!("capability is checked above"),
        })
    }

    fn require(&self, capability: Capability) -> Result<(), ProviderError> {
        if self.protocol.serves(capability) {
            return Ok(());
        }
        Err(ProviderError::new(
            ErrorCode::InvalidArgument,
            false,
            match capability {
                Capability::Llm => "the configured provider has no text capability",
                Capability::Speech => "the configured provider has no speech capability",
            },
        ))
    }
}

/// Text generation through whichever protocol the profile selected. The caller
/// holds this, not a vendor adapter, so a provider swap is invisible to it.
pub enum LlmClient<'a> {
    OpenAi(OpenAiAdapter<'a>),
    Anthropic(AnthropicAdapter<'a>),
}

impl LlmClient<'_> {
    /// Streams one generation. Whatever the wire did — `choices[0].delta`,
    /// typed `content_block_delta`, different usage field names — the result is
    /// the same ordered chunk sequence with the same terminal chunk.
    pub async fn generate(
        &self,
        op: &Operation,
        request: &LlmRequest,
        gateway: &mut dyn UsageGateway,
        entry: UsageLedgerEntry,
    ) -> Result<Vec<LlmChunk>, ProviderError> {
        match self {
            Self::OpenAi(adapter) => adapter.llm_stream(op, request, gateway, entry).await,
            Self::Anthropic(adapter) => adapter.messages_stream(op, request, gateway, entry).await,
        }
    }
}

/// Speech synthesis through whichever protocol the profile selected. Each call
/// covers exactly one utterance: no sample counter, buffer reference or
/// sequence number is carried between calls, which is what keeps two sentences
/// from being stitched into one continuous buffer downstream.
pub enum SpeechClient<'a> {
    OpenAi(OpenAiAdapter<'a>),
    ElevenLabs(ElevenLabsAdapter<'a>),
}

impl SpeechClient<'_> {
    pub async fn tts_stream(
        &self,
        op: &Operation,
        request: &TtsRequest,
        gateway: &mut dyn UsageGateway,
        entry: UsageLedgerEntry,
        sink: &mut dyn FnMut(TtsChunk),
    ) -> Result<u64, ProviderError> {
        match self {
            Self::OpenAi(adapter) => adapter.tts_stream(op, request, gateway, entry, sink).await,
            Self::ElevenLabs(adapter) => {
                adapter.tts_stream(op, request, gateway, entry, sink).await
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The error code of a binding decision, without requiring the *success*
    /// type to be `Debug` (a client enum holds adapters, which are not).
    fn rejection<T>(result: Result<T, ProviderError>) -> ErrorCode {
        match result {
            Ok(_) => panic!("the binding should have refused this capability"),
            Err(error) => error.code,
        }
    }

    fn profile(protocol: Protocol) -> ProviderProfile {
        ProviderProfile {
            provider_ref: "vendor".into(),
            endpoint: "https://example.invalid/v1".into(),
            model: "model".into(),
            price_revision: "price-1".into(),
            currency: "CNY".into(),
            timeout: Duration::from_secs(30),
            max_retries: 0,
            protocol,
        }
    }

    #[test]
    fn configuration_names_are_stable_and_a_typo_never_selects_a_default() {
        assert_eq!(
            Protocol::from_config("openai").unwrap(),
            Protocol::OpenAiCompatible
        );
        assert_eq!(
            Protocol::from_config("anthropic").unwrap(),
            Protocol::AnthropicMessages
        );
        assert_eq!(
            Protocol::from_config("elevenlabs").unwrap(),
            Protocol::ElevenLabs
        );
        assert!(Protocol::from_config("OpenAI").is_err());
        assert!(Protocol::from_config("").is_err());
        for protocol in [
            Protocol::OpenAiCompatible,
            Protocol::AnthropicMessages,
            Protocol::ElevenLabs,
        ] {
            assert_eq!(
                Protocol::from_config(protocol.as_config()).unwrap(),
                protocol
            );
        }
    }

    #[test]
    fn credential_and_header_shape_is_derived_from_the_protocol() {
        let secret = || Secret::new(b"k".to_vec()).unwrap();
        let openai = profile(Protocol::OpenAiCompatible).http_config(secret());
        assert_eq!(openai.auth, AuthScheme::Bearer);
        assert!(openai.extra_headers.is_empty());

        let anthropic = profile(Protocol::AnthropicMessages).http_config(secret());
        assert_eq!(
            anthropic.auth,
            AuthScheme::Header {
                name: "x-api-key".into()
            }
        );
        assert_eq!(
            anthropic.extra_headers,
            vec![("anthropic-version".into(), "2023-06-01".into())]
        );

        let eleven = profile(Protocol::ElevenLabs).http_config(secret());
        assert_eq!(
            eleven.auth,
            AuthScheme::Header {
                name: "xi-api-key".into()
            }
        );
        assert!(eleven.extra_headers.is_empty());
    }

    #[test]
    fn a_protocol_is_never_asked_for_a_capability_it_lacks() {
        let secret = || Secret::new(b"k".to_vec()).unwrap();
        let speech_only = profile(Protocol::ElevenLabs);
        let client = speech_only.client(secret()).unwrap();
        assert_eq!(
            rejection(speech_only.llm(&client)),
            ErrorCode::InvalidArgument
        );
        assert!(speech_only.speech(&client).is_ok());

        let text_only = profile(Protocol::AnthropicMessages);
        let client = text_only.client(secret()).unwrap();
        assert!(text_only.llm(&client).is_ok());
        assert_eq!(
            rejection(text_only.speech(&client)),
            ErrorCode::InvalidArgument
        );

        // An OpenAI-compatible endpoint serves both through one credential.
        let both = profile(Protocol::OpenAiCompatible);
        let client = both.client(secret()).unwrap();
        assert!(both.llm(&client).is_ok());
        assert!(both.speech(&client).is_ok());
    }
}
