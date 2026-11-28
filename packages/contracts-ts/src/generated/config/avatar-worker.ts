/* Generated from JSON Schema. Do not edit. */

export interface AvatarWorkerConfig {
  sample_steps?: 2 | 3 | 4;
  safetensors_backend?: "mmap" | "pread";
  python: string;
  upstream_dir: string;
  /**
   * @minItems 1
   * @maxItems 8
   */
  dependency_dirs:
    | [string]
    | [string, string]
    | [string, string, string]
    | [string, string, string, string]
    | [string, string, string, string, string]
    | [string, string, string, string, string, string]
    | [string, string, string, string, string, string, string]
    | [string, string, string, string, string, string, string, string];
  model_dir: string;
  wav2vec_dir: string;
  portrait: string;
  portrait_sha256: string;
  portrait_authorized: true;
  mapping_reader: string;
  temp_dir: string;
  width: number;
  height: number;
  frame_num: number;
  seed: number;
  face_ratio: number;
  timeout_ms: number;
  maintenance_timeout_ms: number;
  warmup_samples: number;
}
