/* Generated from JSON Schema. Do not edit. */

export interface AvatarManifest {
  schema_version: 1;
  avatar_id: string;
  version: string;
  kind: "2d" | "realistic_onnx" | "3d" | "remote";
  license_ref: string;
  /**
   * @minItems 1
   */
  assets: [
    {
      id: string;
      path: string;
      sha256: string;
      license_ref: string;
    },
    ...{
      id: string;
      path: string;
      sha256: string;
      license_ref: string;
    }[]
  ];
  model_binding: string;
  resource_profile_ref: string;
  /**
   * @minItems 1
   */
  supported_actions: [string, ...string[]];
  /**
   * @minItems 1
   */
  supported_emotions: [string, ...string[]];
  voice_binding: string;
  native_width: number;
  native_height: number;
  output_width: number;
  output_height: number;
  fps_num: number;
  fps_den: number;
  base_duration_samples: string;
  switch_points: string[];
  personalization_refs: {
    [k: string]: string;
  };
}
