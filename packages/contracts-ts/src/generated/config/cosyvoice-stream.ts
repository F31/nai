/* Generated from JSON Schema. Do not edit. */

export interface CosyVoiceStreamConfig {
  endpoint: string;
  reference_file: string;
  prompt_text: string;
  deadline_ms: number;
  max_samples: number;
  lookahead?: {
    max_sentence_chars?: number;
    max_sentence_samples?: number;
    max_sentences?: number;
    idle_wait_ms?: number;
  };
}
