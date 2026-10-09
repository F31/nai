/* Generated from JSON Schema. Do not edit. */

export type ProviderConfig = {
  schema_version: 1;
  id: string;
  protocol: "openai_compatible" | "http_stream" | "http_json" | "websocket";
  endpoint: string;
  model: string;
  credential_ref: string;
  region: string;
  data_policy_ref: string;
  timeout_ms: number;
  max_retries: number;
  max_concurrency: number;
  price_revision: string;
  quota?: {
    currency: string;
    limit_minor: string;
    usage_limits: Counters;
    concurrent_limits: Counters;
  };
};

export interface Counters {
  [k: string]: string;
}
