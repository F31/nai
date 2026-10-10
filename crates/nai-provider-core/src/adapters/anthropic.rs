//! Anthropic Messages adapter: the **second** LLM wire protocol, deliberately
//! unlike OpenAI's chat-completions so the kernel and the orchestration never
//! encode one vendor's shape.
//!
//! Differences this adapter carries, none of which the shared transport knows
//! about: the path (`/messages`, not `/chat/completions`), the credential
//! header (`x-api-key`, not `Authorization`), a version header, a request body
//! with a top-level `system`/`messages` split, SSE frames typed by a JSON `type`
//! field rather than `choices[0].delta`, and usage reported as
//! `input_tokens`/`output_tokens`.
//!
//! The canonical usage vocabulary stays closed ([`usage_charge`] rejects any
//! unit it does not know), so the vendor's field names are mapped **here**, at
//! the adapter boundary, instead of widening the ledger's vocabulary.
use crate::{
    client::{ProviderClient, ProviderError},
    gateway::{CallBudget, UsageGateway},
    sse::SseDecoder,
    usage::usage_charge,
};
use futures_util::StreamExt;
use nai_contracts::v1::{BudgetCharge, ContractError, ErrorCode, LlmChunk, LlmRequest, StreamEnd};
use nai_security::Operation;
use serde_json::{Value, json};

const MAX_STREAM_BYTES: usize = 64 * 1024 * 1024;

/// The minimum `anthropic-version` this adapter speaks. It is a header value,
/// not a wire path, so it belongs to the binding configuration.
pub const ANTHROPIC_VERSION: &str = "2023-06-01";

fn invalid(message: &str) -> ProviderError {
    ProviderError::invalid_argument(message)
}

pub struct AnthropicAdapter<'a> {
    client: &'a ProviderClient,
}

impl<'a> AnthropicAdapter<'a> {
    pub fn new(client: &'a ProviderClient) -> Self {
        Self { client }
    }

    /// Streaming Messages response. Returns chunks in order; a `StreamComplete`
    /// chunk always terminates unless the server reported an error event.
    pub async fn messages_stream(
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
        // The Messages API requires at least one turn, and this adapter adds no
        // prompt text of its own: the permitted context is the single user turn
        // verbatim. Nothing outside `permitted_context` is ever sent.
        let body = json!({
            "model": self.client.config().model,
            "max_tokens": req.max_output_tokens,
            "stream": true,
            "messages": [{"role": "user", "content": req.permitted_context}],
        });
        let stream = match self.client.stream(op, "/messages", &body).await {
            Ok(stream) => stream,
            Err(error) => {
                budget.finish(gateway, None).ok();
                return Err(error);
            }
        };
        let result = self.collect(op, stream.body, req).await;
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

    async fn collect(
        &self,
        op: &Operation,
        response: reqwest::Response,
        req: &LlmRequest,
    ) -> Result<(Vec<LlmChunk>, Option<BudgetCharge>), ProviderError> {
        let mut decoder = SseDecoder::new();
        let mut chunks = Vec::new();
        let mut seq = 0u32;
        let mut total = 0usize;
        let mut input_tokens = 0u64;
        let mut output_tokens = 0u64;
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
                // The Messages stream types every frame in the payload; the SSE
                // event name is a duplicate signal this decoder does not need.
                match data.get("type").and_then(Value::as_str).unwrap_or_default() {
                    "error" => {
                        let message = data
                            .pointer("/error/message")
                            .and_then(Value::as_str)
                            .unwrap_or("provider error");
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
                                message: message.into(),
                                ..Default::default()
                            }),
                            ..Default::default()
                        });
                        return Ok((chunks, None));
                    }
                    // `message_start` opens the response and carries the input
                    // token count; `message_delta` carries the (cumulative)
                    // output count. Both are folded into the canonical units
                    // below.
                    "message_start" => {
                        input_tokens = data
                            .pointer("/message/usage/input_tokens")
                            .and_then(Value::as_u64)
                            .unwrap_or(input_tokens);
                        output_tokens = data
                            .pointer("/message/usage/output_tokens")
                            .and_then(Value::as_u64)
                            .unwrap_or(output_tokens);
                    }
                    "message_delta" => {
                        output_tokens = data
                            .pointer("/usage/output_tokens")
                            .and_then(Value::as_u64)
                            .unwrap_or(output_tokens);
                    }
                    "content_block_delta" => {
                        if let Some(delta) = data
                            .pointer("/delta/text")
                            .and_then(Value::as_str)
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
                    // ping / content_block_start / content_block_stop /
                    // message_stop carry no text and no usage.
                    _ => {}
                }
            }
        }
        for event in decoder.finish() {
            if event.done {
                break;
            }
        }
        chunks.push(LlmChunk {
            seq,
            end: StreamEnd::StreamComplete as i32,
            ..Default::default()
        });
        let actual = canonical_usage(input_tokens, output_tokens).and_then(|usage| {
            usage_charge(
                &self.client.config().currency,
                &self.client.config().price_revision,
                &usage,
                1,
            )
            .ok()
        });
        Ok((chunks, actual))
    }
}

/// Maps the vendor's `input_tokens`/`output_tokens` into the canonical token
/// units. Zero fields are omitted rather than sent as zero, because a zero
/// unit is not a usable billing signal and [`usage_charge`] refuses it.
fn canonical_usage(input_tokens: u64, output_tokens: u64) -> Option<Value> {
    if input_tokens == 0 && output_tokens == 0 {
        return None;
    }
    let mut usage = serde_json::Map::new();
    if input_tokens > 0 {
        usage.insert("prompt_tokens".into(), json!(input_tokens));
    }
    if output_tokens > 0 {
        usage.insert("completion_tokens".into(), json!(output_tokens));
    }
    let total = input_tokens + output_tokens;
    if total > 0 {
        usage.insert("total_tokens".into(), json!(total));
    }
    Some(Value::Object(usage))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vendor_usage_is_mapped_into_the_canonical_units() {
        let mapped = canonical_usage(12, 34).unwrap();
        assert_eq!(mapped["prompt_tokens"], json!(12));
        assert_eq!(mapped["completion_tokens"], json!(34));
        assert_eq!(mapped["total_tokens"], json!(46));
        // A response that reported nothing yields no charge at all rather than
        // a zero-unit one the ledger would have to reject.
        assert!(canonical_usage(0, 0).is_none());
        let partial = canonical_usage(7, 0).unwrap();
        assert_eq!(partial["prompt_tokens"], json!(7));
        assert!(partial.get("completion_tokens").is_none());
    }

    #[test]
    fn mapped_units_survive_the_closed_usage_vocabulary() {
        let mapped = canonical_usage(9, 11).unwrap();
        let charge = usage_charge("CNY", "price-1", &mapped, 1).unwrap();
        assert_eq!(charge.units.get("prompt_tokens"), Some(&9));
        assert_eq!(charge.units.get("total_tokens"), Some(&20));
        assert_eq!(charge.units.get("requests"), Some(&1));
    }
}
