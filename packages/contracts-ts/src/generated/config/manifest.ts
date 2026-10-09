/* Generated from JSON Schema. Do not edit. */

export interface PluginManifest {
  manifest_schema: 1;
  plugin_id: string;
  plugin_version: string;
  kernel_range: string;
  protocol_major: 1;
  /**
   * @minItems 1
   */
  contracts: [string, ...string[]];
  /**
   * @minItems 1
   */
  supported_hosts: ["inproc" | "subprocess" | "remote" | "wasm", ...("inproc" | "subprocess" | "remote" | "wasm")[]];
  activation: "on_session_start" | "on_demand" | "manual";
  idle_policy: "keep_warm_during_session" | "suspend_after_ttl" | "stop_after_job";
  requires: string[];
  /**
   * @minItems 1
   */
  os: ["windows" | "linux" | "macos" | "android" | "ios", ...("windows" | "linux" | "macos" | "android" | "ios")[]];
  /**
   * @minItems 1
   */
  architectures: ["x86_64" | "aarch64", ...("x86_64" | "aarch64")[]];
  artifact_hash: string;
  license_ref: string;
  install_bytes: string;
  config_schema_ref: string;
  data_schema_ref: string;
  health_operation: string;
  migration_ref: string;
  rollback_range: string;
  price_policy_ref: string;
  resources: {
    profile_id: string;
    certification_ref: string;
  };
  permissions: {
    network: boolean;
    media_output: boolean;
    secrets: string[];
  };
}
