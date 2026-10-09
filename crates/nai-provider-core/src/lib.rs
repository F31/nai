//! Provider transport: bounded HTTP client, deadline/cancellation, retry,
//! SSE/binary stream handling, usage mapping and budget wiring. Media bytes are
//! returned by buffer reference; PCM/audio never travels over the control bus.
pub mod adapters;
pub mod client;
pub mod gateway;
pub mod sse;
pub mod text;
pub mod usage;

pub use adapters::OpenAiAdapter;
pub use client::{HttpConfig, ProviderClient, ProviderError, StreamResponse};
pub use gateway::{CallBudget, DispatchDecision, InMemoryGateway, UsageGateway};
pub use sse::{SseDecoder, SseEvent};
pub use text::{
    MAX_OUTPUT_CHARS, MAX_PROMPT_CHARS, RehearsalTextGenerator, TextGenerator, TextRequest,
    TextResponse, generate_with_budget,
};
pub use usage::usage_charge;
