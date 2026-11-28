/* Generated from JSON Schema. Do not edit. */

export interface LiveSessionConfig {
  avatar_config: string;
  tts_config: string;
  llm?: {
    base_url: string;
    model: string;
    api_key_ref: string;
    deadline_ms: number;
    max_answer_chars: number;
    max_output_tokens?: number;
  };
  worker: string;
  obs?: {
    ffmpeg: string;
    port: number;
    ready: string;
    repetitions?: number;
    extended_session?: boolean;
  };
  prefill_samples: number;
  tts_warmup_text: string;
  prepared_cache: {
    max_bytes: number;
    ttl_ms: number;
    max_entries: number;
    max_total_bytes: number;
    prefetch_max_bytes: number;
  };
}
