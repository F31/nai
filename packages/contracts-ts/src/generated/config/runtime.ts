/* Generated from JSON Schema. Do not edit. */

export interface RuntimeConfig {
  schema_version: 1;
  edition: "lite" | "standard" | "pro";
  deployment: "local" | "hybrid" | "distributed";
  workspace_id: string;
  runtime: {
    max_active_sessions: number;
    plugin_catalog: string;
    control_listener: "named_pipe" | "unix_socket";
    http_enabled: boolean;
  };
  bindings: {
    [k: string]: string;
  };
  session_defaults: {
    mode: "rehearsal" | "assisted" | "auto_supervised";
    allow_auto_without_platform_grant: false;
    viewer_memory_enabled: boolean;
  };
  scheduler: {
    max_pending_utterances: number;
    max_pending_speech_ms: number;
    qa_ttl_ms: number;
    welcome_ttl_ms: number;
    thanks_ttl_ms: number;
    max_ingress_events?: number;
    max_state_slots?: number;
    max_dedup_entries?: number;
    viewer_cooldown_ms?: number;
    fairness_window?: number;
    max_viewer_share_percent?: number;
    starvation_ms?: number;
  };
  media: {
    lease_renew_ms: number;
    lease_expire_ms: number;
    output_sample_rate: 16000 | 24000 | 44100 | 48000;
    model_audio_format: "from_model_manifest";
    startup_buffer_ms: number;
    max_buffer_ms: number;
  };
  knowledge: {
    mode: "local" | "remote" | "hybrid";
    fts_enabled: boolean;
    semantic_enabled: boolean;
    embedding_profile_ref?: string;
    remote_source_refs: string[];
    index_during_live: "disabled" | "throttled";
  };
  budgets: {
    soft_threshold_ratio: number;
    hard_stop_new_paid_requests: true;
    currency: string;
    per_session_limit_minor: string;
    global_limit_minor?: string;
    max_ledger_entries?: number;
    global_usage_limits?: Counters;
    session_usage_limits?: Counters;
    global_concurrent_limits?: Counters;
    session_concurrent_limits?: Counters;
  };
  cache: {
    tts_max_bytes: number;
    invalidate_on_fact_change: true;
  };
  cloud: {
    control_enabled: boolean;
    relay_enabled: boolean;
    render_enabled: boolean;
  };
}
export interface Counters {
  [k: string]: string;
}
