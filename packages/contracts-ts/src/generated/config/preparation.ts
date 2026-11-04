/* Generated from JSON Schema. Do not edit. */

export type PreparationDraft = Product | Host | Series | Show | ManualRun | AvatarProfile | AvatarPhoto | AvatarVideo;
export type Fact = {
  id: string;
  label: string;
  value_type: "text" | "decimal" | "money" | "boolean";
  value: string | boolean;
  currency?: "CNY";
  unit?: string;
  source: string;
  authority: "owner_confirmed" | "unverified";
  conditions: string;
  valid_from_ms: number | null;
  valid_until_ms: number | null;
};

export interface Product {
  schema_version: 1;
  kind: "product_draft";
  name: string;
  /**
   * @maxItems 64
   */
  facts: Fact[];
  /**
   * @maxItems 32
   */
  faq: Faq[];
  /**
   * @maxItems 32
   */
  talk_tracks: Binding[];
  /**
   * @maxItems 32
   */
  prohibitions: string[];
}
export interface Faq {
  id: string;
  text: string;
  /**
   * @maxItems 32
   */
  fact_ids: string[];
  question: string;
}
export interface Binding {
  id: string;
  text: string;
  /**
   * @maxItems 32
   */
  fact_ids: string[];
}
export interface Host {
  schema_version: 1;
  kind: "host_draft";
  name: string;
  style: "professional" | "friendly" | "concise";
  preferences: string;
}
export interface Series {
  schema_version: 1;
  kind: "series_draft";
  name: string;
  goal: string;
}
export interface Show {
  schema_version: 1;
  kind: "show_draft";
  name: string;
  product: Ref;
  host: Ref;
  series: Ref;
  /**
   * @maxItems 32
   */
  cues: Cue[];
}
export interface Ref {
  object_id: string;
  revision: number;
  sha256: string;
}
export interface Cue {
  id: string;
  title: string;
  talk_track_id: string;
  /**
   * @maxItems 32
   */
  mandatory_fact_ids: string[];
  allow_questions: boolean;
}
export interface ManualRun {
  schema_version: 1;
  kind: "manual_run_draft";
  name: string;
  show: Ref;
  snapshot: Ref;
  /**
   * @minItems 1
   * @maxItems 32
   */
  cue_ids: [string, ...string[]];
  cursor: number;
  status: "open" | "ended";
  mode: "private_manual";
  evidence: "self_reported";
  /**
   * @maxItems 8
   */
  marks:
    | []
    | [
        {
          id: string;
          cue_id: string;
          note: string;
        }
      ]
    | [
        {
          id: string;
          cue_id: string;
          note: string;
        },
        {
          id: string;
          cue_id: string;
          note: string;
        }
      ]
    | [
        {
          id: string;
          cue_id: string;
          note: string;
        },
        {
          id: string;
          cue_id: string;
          note: string;
        },
        {
          id: string;
          cue_id: string;
          note: string;
        }
      ]
    | [
        {
          id: string;
          cue_id: string;
          note: string;
        },
        {
          id: string;
          cue_id: string;
          note: string;
        },
        {
          id: string;
          cue_id: string;
          note: string;
        },
        {
          id: string;
          cue_id: string;
          note: string;
        }
      ]
    | [
        {
          id: string;
          cue_id: string;
          note: string;
        },
        {
          id: string;
          cue_id: string;
          note: string;
        },
        {
          id: string;
          cue_id: string;
          note: string;
        },
        {
          id: string;
          cue_id: string;
          note: string;
        },
        {
          id: string;
          cue_id: string;
          note: string;
        }
      ]
    | [
        {
          id: string;
          cue_id: string;
          note: string;
        },
        {
          id: string;
          cue_id: string;
          note: string;
        },
        {
          id: string;
          cue_id: string;
          note: string;
        },
        {
          id: string;
          cue_id: string;
          note: string;
        },
        {
          id: string;
          cue_id: string;
          note: string;
        },
        {
          id: string;
          cue_id: string;
          note: string;
        }
      ]
    | [
        {
          id: string;
          cue_id: string;
          note: string;
        },
        {
          id: string;
          cue_id: string;
          note: string;
        },
        {
          id: string;
          cue_id: string;
          note: string;
        },
        {
          id: string;
          cue_id: string;
          note: string;
        },
        {
          id: string;
          cue_id: string;
          note: string;
        },
        {
          id: string;
          cue_id: string;
          note: string;
        },
        {
          id: string;
          cue_id: string;
          note: string;
        }
      ]
    | [
        {
          id: string;
          cue_id: string;
          note: string;
        },
        {
          id: string;
          cue_id: string;
          note: string;
        },
        {
          id: string;
          cue_id: string;
          note: string;
        },
        {
          id: string;
          cue_id: string;
          note: string;
        },
        {
          id: string;
          cue_id: string;
          note: string;
        },
        {
          id: string;
          cue_id: string;
          note: string;
        },
        {
          id: string;
          cue_id: string;
          note: string;
        },
        {
          id: string;
          cue_id: string;
          note: string;
        }
      ];
}
export interface AvatarProfile {
  schema_version: 1;
  kind: "avatar_profile_draft";
  name: string;
  avatar_id: string;
  package_version: string;
  package_sha256: string;
  license_sha256: string;
  width: number;
  height: number;
  selected: boolean;
  revoked: boolean;
}
export interface AvatarPhoto {
  schema_version: 1;
  kind: "avatar_photo_draft";
  name: string;
  photo_sha256: string;
  width: number;
  height: number;
  revoked: boolean;
}
export interface AvatarVideo {
  schema_version: 1;
  kind: "avatar_video_draft";
  name: string;
  video_sha256: string;
  duration_ms: number;
  inspect_version: "mp4-ftyp-mvhd-v1";
  realistic_drive: "model_missing";
  revoked: boolean;
}
