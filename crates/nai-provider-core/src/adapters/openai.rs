//! OpenAI-compatible adapter: chat-completions SSE, audio/speech binary and
//! embeddings JSON. Provider-specific quirks are mapped here, never in the
//! kernel. Media bytes are surfaced as buffer references, not JSON/Base64.
use crate::{
    client::{ProviderClient, ProviderError},
    gateway::{CallBudget, UsageGateway},
    sse::SseDecoder,
    usage::usage_charge,
};
use futures_util::StreamExt;
use nai_contracts::v1::{
    AudioFormat, BudgetCharge, ContractError, EmbeddingRequest, EmbeddingResult, ErrorCode,
    LlmChunk, LlmRequest, StreamEnd, TtsChunk, TtsRequest, Usage,
};
use nai_security::Operation;
use serde_json::{Value, json};

const MAX_STREAM_BYTES: usize = 64 * 1024 * 1024;

fn invalid(message: &str) -> ProviderError {
    ProviderError::new(ErrorCode::InvalidArgument, false, message)
}

pub struct OpenAiAdapter<'a> {
    client: &'a ProviderClient,
    siliconflow: bool,
}
impl<'a> OpenAiAdapter<'a> {
    pub fn new(client: &'a ProviderClient) -> Self {
        Self {
            client,
            siliconflow: false,
        }
    }
    /// SiliconFlow shares the speech envelope but requires an explicit rate.
    pub(crate) fn siliconflow(client: &'a ProviderClient) -> Self {
        Self {
            client,
            siliconflow: true,
        }
    }
    fn speech_rate(&self, req: &TtsRequest) -> Result<u32, ProviderError> {
        if !self.siliconflow {
            return Ok(24000);
        }
        let f = req
            .format
            .as_ref()
            .ok_or_else(|| invalid("SiliconFlow PCM format is required"))?;
        if f.codec != "pcm"
            || f.channels != 1
            || f.sample_format != "s16"
            || ![8000, 16000, 24000, 32000, 44100].contains(&f.sample_rate)
        {
            return Err(invalid("SiliconFlow requires supported mono s16 PCM"));
        }
        Ok(f.sample_rate)
    }
    fn model(&self) -> &str {
        &self.client.config().model
    }
    fn token_usage(&self, quantity: u64, estimated: bool) -> Usage {
        Usage {
            provider_ref: self.client.config().provider_ref.clone(),
            unit: "tokens".into(),
            quantity_decimal: quantity.to_string(),
            estimated,
            currency: self.client.config().currency.clone(),
            price_revision: self.client.config().price_revision.clone(),
        }
    }

    /// Streaming chat completion. Returns chunks in order; a StreamComplete
    /// chunk always terminates unless the server errored mid-stream.
    pub async fn llm_stream(
        &self,
        op: &Operation,
        req: &LlmRequest,
        gateway: &mut dyn UsageGateway,
        entry: nai_contracts::v1::UsageLedgerEntry,
    ) -> Result<Vec<LlmChunk>, ProviderError> {
        let budget = CallBudget::start(gateway, entry)?;
        if !budget.dispatched() {
            return Err(ProviderError::new(
                ErrorCode::Conflict,
                false,
                "llm request already dispatched",
            ));
        }
        if req.permitted_context.is_empty() || req.max_output_tokens == 0 {
            budget.finish(gateway, None).ok();
            return Err(invalid("llm request missing context or token cap"));
        }
        // The permitted context is the single user turn, sent verbatim: this
        // adapter adds no prompt text of its own and constructs no system turn.
        // A chat request with no user turn is not universally valid — a strict
        // gateway answers 400 `No user query found in messages` — so the role is
        // fixed here rather than left to the endpoint.
        let body = json!({
            "model": self.model(),
            "stream": true,
            "stream_options": {"include_usage": true},
            "messages": [{"role": "user", "content": req.permitted_context}],
            "max_tokens": req.max_output_tokens,
        });
        let stream = match self.client.stream(op, "/chat/completions", &body).await {
            Ok(stream) => stream,
            Err(error) => {
                budget.finish(gateway, None).ok();
                return Err(error);
            }
        };
        let result = self.collect_llm(op, stream.body, req).await;
        let (chunks, actual) = match result {
            Ok(value) => value,
            Err(error) => {
                budget.finish(gateway, None).ok();
                return Err(error);
            }
        };
        budget.finish(gateway, actual)?;
        Ok(chunks)
    }

    async fn collect_llm(
        &self,
        op: &Operation,
        response: reqwest::Response,
        req: &LlmRequest,
    ) -> Result<(Vec<LlmChunk>, Option<BudgetCharge>), ProviderError> {
        let mut decoder = SseDecoder::new();
        let mut chunks = Vec::new();
        let mut seq = 0u32;
        let mut usage: Option<Value> = None;
        let mut total = 0usize;
        let mut stream = response.bytes_stream();
        while let Some(block) = stream.next().await {
            op.check().map_err(|error| match error.code {
                ErrorCode::Cancelled => ProviderError::cancelled(),
                _ => ProviderError::deadline(),
            })?;
            let block = block
                .map_err(|error| ProviderError::unavailable(format!("provider stream: {error}")))?;
            total += block.len();
            if total > MAX_STREAM_BYTES {
                return Err(invalid("llm stream exceeds bound"));
            }
            for event in decoder.push(&block) {
                if event.done {
                    continue;
                }
                let data: Value = serde_json::from_str(&event.data)
                    .map_err(|_| invalid("provider returned non-JSON sse data"))?;
                if let Some(error) = data.get("error") {
                    chunks.push(LlmChunk {
                        seq,
                        end: StreamEnd::StreamError as i32,
                        error: Some(ContractError {
                            request_id: req
                                .context
                                .as_ref()
                                .map(|c| c.request_id.clone())
                                .unwrap_or_default(),
                            code: ErrorCode::ProviderUnavailable as i32,
                            category: "provider".into(),
                            message: error
                                .get("message")
                                .and_then(|m| m.as_str())
                                .unwrap_or("provider error")
                                .into(),
                            ..Default::default()
                        }),
                        ..Default::default()
                    });
                    return Ok((chunks, None));
                }
                if let Some(usage_value) = data.get("usage") {
                    usage = Some(usage_value.clone());
                }
                if let Some(delta) = data
                    .pointer("/choices/0/delta/content")
                    .and_then(|v| v.as_str())
                    .filter(|delta| !delta.is_empty())
                {
                    chunks.push(LlmChunk {
                        seq,
                        end: StreamEnd::StreamData as i32,
                        text_delta: delta.into(),
                        ..Default::default()
                    });
                    seq += 1;
                }
            }
        }
        for event in decoder.finish() {
            if event.done || event.data.trim() == "[DONE]" {
                break;
            }
        }
        chunks.push(LlmChunk {
            seq,
            end: StreamEnd::StreamComplete as i32,
            ..Default::default()
        });
        let actual = usage.as_ref().and_then(|value| {
            usage_charge(
                &self.client.config().currency,
                &self.client.config().price_revision,
                value,
                1,
            )
            .ok()
        });
        Ok((chunks, actual))
    }

    /// Text-to-speech binary stream. Sample counters are computed by the media
    /// decoder (D11); the adapter only sequences byte buffers. OpenAI does not
    /// return usage for TTS, so the ledger keeps an estimate for reconciliation.
    pub async fn tts_stream(
        &self,
        op: &Operation,
        req: &TtsRequest,
        gateway: &mut dyn UsageGateway,
        entry: nai_contracts::v1::UsageLedgerEntry,
        sink: &mut dyn FnMut(TtsChunk),
    ) -> Result<u64, ProviderError> {
        self.tts_core(op, req, gateway, entry, &mut |chunk, _bytes| sink(chunk))
            .await
    }

    /// The same metered request, returning the raw audio body instead of only
    /// counting it. The response format is bounded by the shared transport, and
    /// the caller must decode it against the declared format rather than guess.
    pub async fn tts_pcm(
        &self,
        op: &Operation,
        req: &TtsRequest,
        gateway: &mut dyn UsageGateway,
        entry: nai_contracts::v1::UsageLedgerEntry,
    ) -> Result<(Vec<u8>, AudioFormat), ProviderError> {
        let mut bytes = Vec::new();
        let total = self
            .tts_core(op, req, gateway, entry, &mut |_chunk, block| {
                bytes.extend_from_slice(block);
            })
            .await?;
        if bytes.len() as u64 != total {
            return Err(invalid("tts byte total disagrees with the body"));
        }
        let format = AudioFormat {
            codec: req
                .format
                .as_ref()
                .map(|format| format.codec.clone())
                .unwrap_or_else(|| "pcm".into()),
            // Speech PCM has a fixed wire format; the API has no rate parameter.
            sample_rate: self.speech_rate(req)?,
            channels: 1,
            sample_format: "s16".into(),
        };
        Ok((bytes, format))
    }

    async fn tts_core<E>(
        &self,
        op: &Operation,
        req: &TtsRequest,
        gateway: &mut dyn UsageGateway,
        entry: nai_contracts::v1::UsageLedgerEntry,
        emit: &mut E,
    ) -> Result<u64, ProviderError>
    where
        E: FnMut(TtsChunk, &[u8]),
    {
        let sample_rate = self.speech_rate(req)?;
        let budget = CallBudget::start(gateway, entry)?;
        if !budget.dispatched() {
            return Err(ProviderError::new(
                ErrorCode::Conflict,
                false,
                "tts request already dispatched",
            ));
        }
        if req.speech_text.is_empty()
            || req
                .voice
                .as_ref()
                .map(|v| v.voice_ref.is_empty())
                .unwrap_or(true)
        {
            budget.finish(gateway, None).ok();
            return Err(invalid("tts request missing text or voice"));
        }
        let format = req
            .format
            .as_ref()
            .map(|format| format.codec.clone())
            .unwrap_or_default();
        let response_format = match format.as_str() {
            "mp3" | "opus" | "aac" | "flac" | "wav" | "pcm" => format,
            _ => "pcm".to_string(),
        };
        let mut body = json!({
            "model": self.model(),
            "input": req.speech_text,
            "voice": req.voice.as_ref().map(|v| v.voice_ref.clone()).unwrap_or_default(),
            "response_format": response_format,
            "speed": req.speed,
        });
        if self.siliconflow {
            body["sample_rate"] = sample_rate.into();
            body["stream"] = false.into();
        }
        let stream = match self.client.stream(op, "/audio/speech", &body).await {
            Ok(stream) => stream,
            Err(error) => {
                budget.finish(gateway, None).ok();
                return Err(error);
            }
        };
        let mut seq = 0u32;
        let mut total = 0u64;
        let mut stream = stream.body.bytes_stream();
        let audio_format = AudioFormat {
            codec: response_format,
            sample_rate,
            channels: 1,
            sample_format: "s16".into(),
        };
        loop {
            op.check().map_err(|error| match error.code {
                ErrorCode::Cancelled => ProviderError::cancelled(),
                _ => ProviderError::deadline(),
            })?;
            let block = match stream.next().await {
                Some(block) => block.map_err(|error| {
                    ProviderError::unavailable(format!("provider audio stream: {error}"))
                })?,
                None => {
                    emit(
                        TtsChunk {
                            seq,
                            end: StreamEnd::StreamComplete as i32,
                            format: Some(audio_format.clone()),
                            ..Default::default()
                        },
                        &[],
                    );
                    break;
                }
            };
            total += block.len() as u64;
            if total > MAX_STREAM_BYTES as u64 {
                budget.finish(gateway, None).ok();
                return Err(invalid("tts stream exceeds bound"));
            }
            if !block.is_empty() {
                emit(
                    TtsChunk {
                        seq,
                        end: StreamEnd::StreamData as i32,
                        audio_buffer_ref: format!("provider://audio/{seq}"),
                        format: Some(audio_format.clone()),
                        sample_start: 0,
                        sample_count: 0,
                        ..Default::default()
                    },
                    &block,
                );
                seq += 1;
            }
        }
        budget.finish(gateway, None)?;
        Ok(total)
    }

    /// Embeddings JSON. Vector dimensions come from the response; normalization
    /// is assumed L2 for OpenAI-compatible providers until a manifest says otherwise.
    pub async fn embeddings(
        &self,
        op: &Operation,
        req: &EmbeddingRequest,
        gateway: &mut dyn UsageGateway,
        entry: nai_contracts::v1::UsageLedgerEntry,
    ) -> Result<EmbeddingResult, ProviderError> {
        let budget = CallBudget::start(gateway, entry)?;
        if !budget.dispatched() {
            return Err(ProviderError::new(
                ErrorCode::Conflict,
                false,
                "embedding request already dispatched",
            ));
        }
        if req.permitted_texts.is_empty() || req.permitted_texts.len() > 64 {
            budget.finish(gateway, None).ok();
            return Err(invalid("embedding request has no permitted texts"));
        }
        let body = json!({
            "model": self.model(),
            "input": req.permitted_texts,
            "encoding_format": "float",
        });
        let json = match self.client.post_json(op, "/embeddings", &body).await {
            Ok((_, json)) => json,
            Err(error) => {
                budget.finish(gateway, None).ok();
                return Err(error);
            }
        };
        let data = json
            .get("data")
            .and_then(|v| v.as_array())
            .ok_or_else(|| invalid("embedding response missing data"))?;
        let mut vectors = Vec::with_capacity(data.len());
        let mut dimension = 0u32;
        for (index, item) in data.iter().enumerate() {
            let embedding = item
                .get("embedding")
                .and_then(|v| v.as_array())
                .ok_or_else(|| invalid("embedding missing vector"))?;
            if index == 0 {
                dimension =
                    u32::try_from(embedding.len()).map_err(|_| invalid("embedding too large"))?;
            } else if embedding.len() as u32 != dimension {
                return Err(invalid("embedding dimension mismatch"));
            }
            let values = embedding
                .iter()
                .map(|value| value.as_f64().map(|v| v as f32))
                .collect::<Option<Vec<_>>>()
                .ok_or_else(|| invalid("embedding contains non-number"))?;
            vectors.push(nai_contracts::v1::EmbeddingVector { values });
        }
        let actual = json.get("usage").and_then(|value| {
            usage_charge(
                &self.client.config().currency,
                &self.client.config().price_revision,
                value,
                1,
            )
            .ok()
        });
        let tokens = actual
            .as_ref()
            .and_then(|c| c.units.get("tokens"))
            .copied()
            .unwrap_or(0);
        budget.finish(gateway, actual)?;
        Ok(EmbeddingResult {
            provider_model_revision: json
                .get("model")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .into(),
            dimension,
            normalization: "l2".into(),
            vectors,
            usage: vec![self.token_usage(tokens, true)],
        })
    }
}
