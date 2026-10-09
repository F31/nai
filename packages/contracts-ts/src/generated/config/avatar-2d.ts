/* Generated from JSON Schema. Do not edit. */

export interface Avatar2DConfig {
  schema_version: 1;
  pixel_format: "rgba8";
  sample_rate: number;
  max_buffer_samples: number;
  noise_gate: number;
  full_open_rms: number;
  attack_ms: number;
  release_ms: number;
  /**
   * @minItems 3
   * @maxItems 3
   */
  layers: [
    {
      asset_id: string;
      role: "base" | "mouth_closed" | "mouth_open";
      width: number;
      height: number;
      x: number;
      y: number;
    },
    {
      asset_id: string;
      role: "base" | "mouth_closed" | "mouth_open";
      width: number;
      height: number;
      x: number;
      y: number;
    },
    {
      asset_id: string;
      role: "base" | "mouth_closed" | "mouth_open";
      width: number;
      height: number;
      x: number;
      y: number;
    }
  ];
}
