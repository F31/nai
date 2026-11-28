/* Generated from JSON Schema. Do not edit. */

export interface PreparedUtterance {
  version: 1;
  scope: "private_session_prepared_asset_not_current_model_cache";
  identity_sha256: string;
  pcm_sha256: string;
  /**
   * @minItems 1
   * @maxItems 150
   */
  frame_sha256: [string, ...string[]];
  width: number;
  height: number;
  samples: number;
  frames: number;
  ttl_ms: number;
  model_api_calls: 1;
}
