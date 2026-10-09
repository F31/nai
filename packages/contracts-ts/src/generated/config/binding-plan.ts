/* Generated from JSON Schema. Do not edit. */

export interface BindingPlanConfig {
  schema_version: 1;
  session_id: string;
  mode: "rehearsal" | "assisted" | "auto_supervised";
  emits_output: boolean;
  /**
   * @minItems 1
   * @maxItems 64
   */
  requirements: [
    {
      capability_id: string;
      required: boolean;
    },
    ...{
      capability_id: string;
      required: boolean;
    }[]
  ];
  /**
   * @maxItems 64
   */
  selections: {
    capability_id: string;
    primary_plugin_id: string;
    /**
     * @maxItems 16
     */
    fallback_plugin_ids:
      | []
      | [string]
      | [string, string]
      | [string, string, string]
      | [string, string, string, string]
      | [string, string, string, string, string]
      | [string, string, string, string, string, string]
      | [string, string, string, string, string, string, string]
      | [string, string, string, string, string, string, string, string]
      | [string, string, string, string, string, string, string, string, string]
      | [string, string, string, string, string, string, string, string, string, string]
      | [string, string, string, string, string, string, string, string, string, string, string]
      | [string, string, string, string, string, string, string, string, string, string, string, string]
      | [string, string, string, string, string, string, string, string, string, string, string, string, string]
      | [string, string, string, string, string, string, string, string, string, string, string, string, string, string]
      | [
          string,
          string,
          string,
          string,
          string,
          string,
          string,
          string,
          string,
          string,
          string,
          string,
          string,
          string,
          string
        ]
      | [
          string,
          string,
          string,
          string,
          string,
          string,
          string,
          string,
          string,
          string,
          string,
          string,
          string,
          string,
          string,
          string
        ];
    policy_scope_ref: string;
  }[];
}
