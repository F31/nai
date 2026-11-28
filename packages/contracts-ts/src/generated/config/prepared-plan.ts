/* Generated from JSON Schema. Do not edit. */

export interface PreparedPlan {
  version: 1;
  scope: "private_session_prepared_plan";
  input_sha256: string;
  /**
   * @minItems 1
   * @maxItems 8
   */
  asset_identities:
    | [string]
    | [string, string]
    | [string, string, string]
    | [string, string, string, string]
    | [string, string, string, string, string]
    | [string, string, string, string, string, string]
    | [string, string, string, string, string, string, string]
    | [string, string, string, string, string, string, string, string];
  total_bytes: number;
  total_samples: number;
  model_api_calls: number;
}
