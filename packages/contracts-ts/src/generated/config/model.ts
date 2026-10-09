/* Generated from JSON Schema. Do not edit. */

export interface ModelManifest {
  schema_version: 1;
  model_id: string;
  version: string;
  license_ref: string;
  /**
   * @minItems 1
   */
  graphs: [
    {
      id: string;
      file: File;
      external_weights: File[];
      /**
       * @minItems 1
       */
      inputs: [Tensor, ...Tensor[]];
      /**
       * @minItems 1
       */
      outputs: [Tensor, ...Tensor[]];
      opset: number;
    },
    ...{
      id: string;
      file: File;
      external_weights: File[];
      /**
       * @minItems 1
       */
      inputs: [Tensor, ...Tensor[]];
      /**
       * @minItems 1
       */
      outputs: [Tensor, ...Tensor[]];
      opset: number;
    }[]
  ];
  /**
   * @minItems 1
   */
  required_graph_ids: [string, ...string[]];
  preprocess_revision: string;
  postprocess_revision: string;
  sample_rate: number;
  window_samples: number;
  hop_samples: number;
  lookahead_samples: number;
  padding: string;
  normalization: string;
  frame_mapping: string;
  fps_num: number;
  fps_den: number;
  /**
   * @minItems 1
   */
  runtime_matrix: [
    {
      os: string;
      arch: string;
      ort: string;
      ort_crate: string;
      ep: "cpu" | "cuda" | "tensorrt" | "directml";
      precision: "fp32" | "fp16" | "int8";
      driver: string;
      cuda: string;
      cudnn: string;
      certification_ref: string;
    },
    ...{
      os: string;
      arch: string;
      ort: string;
      ort_crate: string;
      ep: "cpu" | "cuda" | "tensorrt" | "directml";
      precision: "fp32" | "fp16" | "int8";
      driver: string;
      cuda: string;
      cudnn: string;
      certification_ref: string;
    }[]
  ];
  bom_ref: string;
  golden_ref: string;
}
export interface File {
  path: string;
  sha256: string;
  license_ref: string;
}
export interface Tensor {
  name: string;
  dtype: "float32" | "float16" | "int64" | "int32" | "uint8" | "bool";
  /**
   * @minItems 1
   */
  shape: [number | string, ...(number | string)[]];
}
