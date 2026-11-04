/* Generated from JSON Schema. Do not edit. */

export type PublicationArtifact =
  | {
      schema_version: 1;
      kind: "publication_impact";
      created_at_ms: number;
      created_by: string;
      role_revision: number;
      refs: {
        show: {
          object_id: string;
          revision: number;
          sha256: string;
        };
        product: {
          object_id: string;
          revision: number;
          sha256: string;
        };
        host: {
          object_id: string;
          revision: number;
          sha256: string;
        };
        series: {
          object_id: string;
          revision: number;
          sha256: string;
        };
      };
      previous_snapshot: {
        object_id: string;
        revision: number;
        sha256: string;
      } | null;
      graph_sha256: string;
      expires_at_ms: number;
      /**
       * @maxItems 128
       */
      changed_fields: string[];
      /**
       * @maxItems 32
       */
      explicit_dependencies: {
        cue_id: string;
        /**
         * @maxItems 32
         */
        fact_ids: string[];
      }[];
      /**
       * @maxItems 64
       */
      affected_sessions: string[];
      /**
       * @maxItems 64
       */
      affected_snapshots: string[];
      /**
       * @maxItems 16
       */
      unknown_items:
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
    }
  | {
      schema_version: 1;
      kind: "publication_snapshot";
      created_at_ms: number;
      created_by: string;
      refs: {
        show: {
          object_id: string;
          revision: number;
          sha256: string;
        };
        product: {
          object_id: string;
          revision: number;
          sha256: string;
        };
        host: {
          object_id: string;
          revision: number;
          sha256: string;
        };
        series: {
          object_id: string;
          revision: number;
          sha256: string;
        };
      };
      previous_snapshot: {
        object_id: string;
        revision: number;
        sha256: string;
      } | null;
      report: {
        object_id: string;
        revision: number;
        sha256: string;
      };
      scope: "private";
      presentation: "content_only";
      valid_until_ms: number | null;
      /**
       * @maxItems 32
       */
      explicit_dependencies: {
        cue_id: string;
        /**
         * @maxItems 32
         */
        fact_ids: string[];
      }[];
    }
  | {
      schema_version: 1;
      kind: "publication_revocation";
      created_at_ms: number;
      created_by: string;
      snapshot: {
        object_id: string;
        revision: number;
        sha256: string;
      };
      reason: string;
    }
  | {
      schema_version: 1;
      kind: "publication_private_preview";
      created_at_ms: number;
      created_by: string;
      snapshot: {
        object_id: string;
        revision: number;
        sha256: string;
      };
      session_id: string;
      epoch: number;
      cue_id: string;
      text: string;
      target: "private_workbench";
      expires_at_ms: number;
      role_revision: number;
      live_sentence?: {
        run_id: string;
        plan_sha256: string;
        sentence_cursor: number;
        sentence_index: number;
      };
      live_question?: {
        interruption_id: string;
        faq_task_id: string;
        faq_task_sha256: string;
        faq_id: string;
      };
    }
  | {
      schema_version: 1;
      kind: "publication_preview_receipt";
      created_at_ms: number;
      created_by: string;
      preview: {
        object_id: string;
        revision: number;
        sha256: string;
      };
      session_id: string;
      epoch: number;
    }
  | {
      schema_version: 1;
      kind: "publication_faq_task";
      created_at_ms: number;
      created_by: string;
      role_revision: number;
      snapshot: {
        object_id: string;
        revision: number;
        sha256: string;
      };
      refs: {
        show: {
          object_id: string;
          revision: number;
          sha256: string;
        };
        product: {
          object_id: string;
          revision: number;
          sha256: string;
        };
        host: {
          object_id: string;
          revision: number;
          sha256: string;
        };
        series: {
          object_id: string;
          revision: number;
          sha256: string;
        };
      };
      question: string;
      result: "exact" | "retrieved" | "no_match" | "ambiguous";
      faq_id: string;
      text: string;
      /**
       * @maxItems 32
       */
      fact_ids: string[];
      /**
       * @maxItems 32
       */
      evidence: {
        fact_id: string;
        label: string;
        value: string;
        unit: string;
        currency: string;
        source: string;
        conditions: string;
        valid_from_ms: number | null;
        valid_until_ms: number | null;
      }[];
      expires_at_ms: number;
      provider_calls: 0;
      api_cost_minor: 0;
      currency: "CNY";
      target: "private_workbench";
      retrieval?: {
        strategy: "faq-lexical-zh-v1";
        method: "exact" | "lexical";
        matched_question: string;
        score: number;
      };
      parent_task?: {
        context_sha256: string;
        child_id: string;
        parent_id: string;
      };
    }
  | {
      schema_version: 1;
      kind: "publication_faq_resolution";
      created_at_ms: number;
      created_by: string;
      task: {
        object_id: string;
        revision: number;
        sha256: string;
      };
      decision: "accept" | "edit" | "ignore" | "cancel";
      adopted_object_id: string;
      adopted_revision: number;
    }
  | {
      schema_version: 1;
      kind: "publication_llm_config";
      created_at_ms: number;
      created_by: string;
      name: string;
      base_url: string;
      model: string;
      pricing_unknown?: boolean;
      input_minor_per_million: number;
      output_minor_per_million: number;
      enabled: boolean;
      secret: {
        reference: string;
        nonce: string;
        ciphertext: string;
      } | null;
      removed?: boolean;
    }
  | {
      schema_version: 1;
      kind: "publication_llm_task";
      created_at_ms: number;
      created_by: string;
      role_revision: number;
      snapshot: {
        object_id: string;
        revision: number;
        sha256: string;
      };
      refs: {
        show: {
          object_id: string;
          revision: number;
          sha256: string;
        };
        product: {
          object_id: string;
          revision: number;
          sha256: string;
        };
        host: {
          object_id: string;
          revision: number;
          sha256: string;
        };
        series: {
          object_id: string;
          revision: number;
          sha256: string;
        };
      };
      model_ref: {
        object_id: string;
        revision: number;
        sha256: string;
      };
      cue_id: string;
      state: "queued" | "running" | "completed" | "cancelled" | "failed";
      reason: string;
      text: string;
      /**
       * @maxItems 32
       */
      fact_ids: string[];
      input: string;
      expires_at_ms: number;
      max_output_tokens: 256;
      reserved_minor: number;
      input_tokens: number;
      output_tokens: number;
      usage_known: boolean;
      target: "private_workbench";
      task_context?: {
        workspace_id: string;
        snapshot_id: string;
        snapshot_sha256: string;
        facts_sha256: string;
        context_sha256: string;
      };
    }
  | {
      schema_version: 1;
      kind: "publication_llm_resolution";
      created_at_ms: number;
      created_by: string;
      task: {
        object_id: string;
        revision: number;
        sha256: string;
      };
      decision: "accept" | "edit" | "ignore";
      adopted_object_id: string;
      adopted_revision: number;
    }
  | {
      schema_version: 1;
      kind: "publication_llm_test";
      created_at_ms: number;
      created_by: string;
      role_revision: number;
      model_ref: {
        object_id: string;
        revision: number;
        sha256: string;
      };
      state: "queued" | "running" | "completed" | "cancelled" | "failed";
      reason: string;
      text: string;
      input: string;
      expires_at_ms: number;
      max_output_tokens: 256;
      reserved_minor?: number;
      input_tokens: number;
      output_tokens: number;
      usage_known: boolean;
      target: "private_workbench";
      request_started?: boolean;
    }
  | {
      schema_version: 1;
      kind: "publication_tts_config";
      created_at_ms: number;
      created_by: string;
      name: string;
      base_url: string;
      model: string;
      enabled: boolean;
      secret: {
        reference: string;
        nonce: string;
        ciphertext: string;
      } | null;
      removed?: boolean;
      voice: string;
      protocol: "openai_speech" | "qwen_local";
      price_known: boolean;
      character_minor_per_million: number;
      language?:
        | ""
        | "auto"
        | "chinese"
        | "english"
        | "french"
        | "german"
        | "italian"
        | "japanese"
        | "korean"
        | "portuguese"
        | "russian"
        | "spanish";
    }
  | {
      schema_version: 1;
      kind: "publication_tts_test";
      created_at_ms: number;
      created_by: string;
      role_revision: number;
      model_ref: {
        object_id: string;
        revision: number;
        sha256: string;
      };
      state: "queued" | "running" | "completed" | "cancelled" | "failed";
      reason: string;
      expires_at_ms: number;
      reserved_minor?: number;
      target: "private_workbench";
      sample_count: number;
      audio_sha256: string;
      request_started?: boolean;
    }
  | {
      schema_version: 1;
      kind: "publication_tts_preview";
      created_at_ms: number;
      created_by: string;
      role_revision: number;
      model_ref: {
        object_id: string;
        revision: number;
        sha256: string;
      };
      state: "queued" | "running" | "completed" | "cancelled" | "failed";
      reason: string;
      expires_at_ms: number;
      reserved_minor: number;
      target: "private_workbench";
      sample_count: number;
      audio_sha256: string;
      source: {
        object_id: string;
        revision: number;
        sha256: string;
      };
      approval_sha256: string;
      max_cost_minor: number;
      task_context?: {
        workspace_id: string;
        snapshot_id: string;
        snapshot_sha256: string;
        facts_sha256: string;
        context_sha256: string;
      };
      auto_grant_id?: string;
      auto_run_id?: string;
    }
  | {
      schema_version: 1;
      kind: "publication_question_task";
      created_by: string;
      role_revision: number;
      created_at_ms: number;
      updated_at_ms: number;
      expires_at_ms: number;
      context: {
        workspace_id: string;
        snapshot_id: string;
        snapshot_sha256: string;
        facts_sha256: string;
        context_sha256: string;
      };
      question: string;
      dedup_key: string;
      state: "queued" | "running" | "waiting_human" | "cancelled";
      occurrences: number;
      child_task_id: "" | string;
      child_task_sha256: "" | string;
    }
  | {
      schema_version: 1;
      kind: "publication_question_index";
      created_by: string;
      context_sha256: string;
      bucket: number;
      /**
       * @maxItems 32
       */
      entries: string[];
    }
  | {
      schema_version: 1;
      created_by: string;
      role_revision: number;
      created_at_ms: number;
      updated_at_ms: number;
      context: {
        workspace_id: string;
        snapshot_id: string;
        snapshot_sha256: string;
        facts_sha256: string;
        context_sha256: string;
      };
      kind: "publication_live_run";
      name: string;
      session_epoch: number;
      /**
       * @minItems 1
       * @maxItems 32
       */
      cue_ids: [string, ...string[]];
      cursor: number;
      state: 1 | 2 | 3 | 4 | 5 | 6 | 7;
      submitted_count: number;
      skipped_count: number;
      active_approval: string;
      last_approval: string;
      active_bundle: string;
      last_bundle: string;
      sentence_mode?: boolean;
      sentence_cursor?: number;
      current_sentence_index?: number;
      submitted_sentences?: number;
      skipped_sentences?: number;
      sentence_total?: number;
      batch_approval?: {
        approval_id: string;
        plan_sha256: string;
        expires_at_ms: number;
        epoch: number;
        revoked: boolean;
        created_at_ms: number;
      };
      /**
       * @maxItems 32
       */
      batch_ids?: string[];
      /**
       * @maxItems 32
       */
      auto_ids?: string[];
      auto_grant?: {
        grant_id: string;
        model_id: string;
        model_revision: number;
        model_sha256: string;
        voice: string;
        plan_sha256: string;
        output_id: string;
        visual_sha256: string;
        license_sha256: string;
        stop_nonce: string;
        created_at_ms: number;
        expires_at_ms: number;
        epoch: number;
        max_cost_minor: number;
        reserved_minor: number;
        state: "active" | "stopped" | "failed";
        reason: string;
        /**
         * @maxItems 128
         */
        tasks: {
          index: number;
          preview_id: string;
          preview_sha256: string;
          task_id: string;
        }[];
        /**
         * @maxItems 16
         */
        question_ids?:
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
        question?: {
          interruption_id: string;
          faq_task_id: string;
          faq_task_sha256: string;
          state: "pending" | "preparing" | "playing" | "completed" | "cancelled";
          created_at_ms: number;
          expires_at_ms: number;
          return_cursor: number;
          task?: {
            index: number;
            preview_id: string;
            preview_sha256: string;
            task_id: string;
          };
        };
        /**
         * @maxItems 16
         */
        question_task_ids?:
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
      };
      takeover?: {
        takeover_id: string;
        active: boolean;
        partial_sentence: boolean;
        created_at_ms: number;
        epoch: number;
        resumed_at_ms: number;
      };
      /**
       * @maxItems 32
       */
      takeover_ids?: string[];
    }
  | {
      schema_version: 1;
      kind: "publication_review_index";
      created_by: string;
      created_at_ms: number;
      updated_at_ms: number;
      source_run_id: string;
      source_run_kind: "manual" | "live";
      /**
       * @minItems 0
       * @maxItems 3
       */
      notes: [] | [string] | [string, string] | [string, string, string];
      /**
       * @minItems 0
       * @maxItems 32
       */
      jobs: string[];
    }
  | {
      schema_version: 1;
      kind: "publication_review_note";
      created_by: string;
      created_at_ms: number;
      updated_at_ms: number;
      source: ReviewSource;
      title: string;
      proposal: ReviewFaq;
      question_source?: ReviewRef;
    }
  | {
      schema_version: 1;
      kind: "publication_review_draft_job";
      created_by: string;
      created_at_ms: number;
      updated_at_ms: number;
      source: ReviewSource;
      note: ReviewRef;
      base_product: ReviewRef;
      base_show: ReviewRef;
      targets: {
        product_id: string;
        show_id: string;
      };
      proposal: ReviewFaq;
      before: ReviewBefore;
      state: "preview" | "conflict" | "applied";
      binding_sha256: string;
      saved: null | {
        product: ReviewRef;
        show: ReviewRef;
      };
    }
  | {
      schema_version: 1;
      kind: "publication_review_question_index";
      created_by: string;
      created_at_ms: number;
      updated_at_ms: number;
      source_run_id: string;
      source_run_kind: "manual" | "live";
      /**
       * @maxItems 64
       */
      questions: string[];
    }
  | {
      schema_version: 1;
      kind: "publication_review_question";
      created_by: string;
      created_at_ms: number;
      updated_at_ms: number;
      source: ReviewSource;
      session_epoch: number;
      question_id: string;
      question: string;
      occurrences: number;
      expires_at_ms: number;
      processing: "queued" | "waiting_human" | "no_candidate" | "adopted" | "edited" | "ignored" | "cancelled";
      delivery: "unconfirmed" | "requested" | "submitted" | "stopped";
      candidate_id: string;
      candidate_sha256: string;
    };

export interface ReviewSource {
  run_id: string;
  run_kind: "manual" | "live";
  run_revision: number;
  snapshot_id: string;
  snapshot_sha256: string;
}
export interface ReviewFaq {
  id: string;
  question: string;
  text: string;
  /**
   * @minItems 1
   * @maxItems 16
   */
  fact_ids:
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
}
export interface ReviewRef {
  object_id: string;
  revision: number;
  sha256: string;
}
export interface ReviewBefore {
  id: string;
  question: string;
  text: string;
  /**
   * @minItems 0
   * @maxItems 32
   */
  fact_ids: string[];
}
