//! Provider transport: bounded HTTP client, deadline/cancellation, retry,
//! SSE/binary stream handling, usage mapping and budget wiring. Media bytes are
//! returned by buffer reference; PCM/audio never travels over the control bus.
//!
//! `text` and `speech` are the metered seams a caller uses instead of talking to
//! an adapter directly: both wrap the call in reserve -> dispatch -> finish, so
//! neither can bypass the budget, cancellation or usage accounting. `speech`
//! returns PCM in a buffer, never over the control bus.
//!
//! `binding` is the composition seam: it turns a configured protocol name into
//! the matching wire adapter, so callers hold a capability client rather than a
//! vendor adapter and a provider swap stays a configuration edit.
pub mod adapters;
pub mod binding;
pub mod client;
pub mod gateway;
pub mod speech;
pub mod sse;
pub mod text;
pub mod usage;

pub use adapters::{AnthropicAdapter, ElevenLabsAdapter, OpenAiAdapter};
pub use binding::{Capability, LlmClient, Protocol, ProviderProfile, SpeechClient};
pub use client::{AuthScheme, HttpConfig, ProviderClient, ProviderError, StreamResponse};
pub use gateway::{CallBudget, DispatchDecision, InMemoryGateway, UsageGateway};
pub use speech::{
    MAX_SPEECH_CHARS, MAX_SPEECH_SAMPLES, MIN_SAMPLE_RATE, RehearsalSpeechSynthesizer,
    SpeechRequest, SpeechResponse, SpeechSynthesizer, synthesize_with_budget,
};
pub use sse::{SseDecoder, SseEvent};
pub use text::{
    MAX_OUTPUT_CHARS, MAX_PROMPT_CHARS, RehearsalTextGenerator, TextGenerator, TextRequest,
    TextResponse, generate_with_budget,
};
pub use usage::usage_charge;
