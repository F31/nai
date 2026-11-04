//! Explicit wire fixtures, not runtime readiness evidence.
use nai_contracts::workbench::*;
use prost::Message;
use std::{fs, path::Path};
fn config() -> WorkbenchConfig {
    WorkbenchConfig {
        edition: Edition::Pro as i32,
        deployment: Deployment::Local as i32,
        presentation: Presentation::ContentOnly as i32,
        behavior: Behavior::Assisted as i32,
        delivery_scope: DeliveryScope::Private as i32,
        queue_limit: 2,
        task_deadline_ms: 3000,
        shadow: false,
    }
}
fn roundtrip<M: Message + Default + PartialEq + std::fmt::Debug>(
    dir: &Path,
    name: &str,
    m: M,
    write: bool,
) {
    let p = dir.join(name);
    if write {
        fs::write(p, m.encode_to_vec()).unwrap();
    } else {
        let b = fs::read(p).unwrap();
        assert!(b.len() <= 65536);
        assert_eq!(M::decode(b.as_slice()).unwrap(), m);
    }
}
fn main() {
    let args: Vec<_> = std::env::args().collect();
    assert_eq!(args.len(), 3);
    let write = args[1] == "write";
    assert!(write || args[1] == "verify");
    let dir = Path::new(&args[2]);
    let context = AgentTaskContext {
        workspace_id: "00000000-0000-4000-8000-000000000001".into(),
        snapshot_id: "00000000-0000-4000-8000-000000000002".into(),
        snapshot_sha256: "a".repeat(64),
        facts_sha256: "b".repeat(64),
        context_sha256: "c".repeat(64),
    };
    roundtrip(
        dir,
        "model-context-response.bin",
        FoundationResponse {
            outcome: Some(foundation_response::Outcome::Result(FoundationResult {
                agent_context: Some(context.clone()),
                ..Default::default()
            })),
            ..Default::default()
        },
        write,
    );
    for (name, operation) in [
        (
            "live-create",
            foundation_command::Operation::CreateLiveRun(LiveRunCreate {
                scope: Some(QuestionPoolRead {
                    workspace_id: context.workspace_id.clone(),
                    snapshot_id: context.snapshot_id.clone(),
                    snapshot_sha256: context.snapshot_sha256.clone(),
                }),
                run_id: context.workspace_id.clone(),
            }),
        ),
        (
            "live-read",
            foundation_command::Operation::ReadLiveRun(LiveRunRead {
                workspace_id: context.workspace_id.clone(),
                run_id: context.workspace_id.clone(),
            }),
        ),
        (
            "live-control",
            foundation_command::Operation::ControlLiveRun(LiveRunControl {
                workspace_id: context.workspace_id.clone(),
                run_id: context.workspace_id.clone(),
                expected_revision: u64::MAX,
                expected_epoch: u64::MAX,
                action: LiveRunAction::Pause as i32,
            }),
        ),
    ] {
        roundtrip(
            dir,
            &format!("{name}.bin"),
            FoundationCommand {
                idempotency_key: context.workspace_id.clone(),
                operation: Some(operation),
            },
            write,
        );
    }
    roundtrip(
        dir,
        "live-context.bin",
        LiveContext {
            source: Some(context.clone()),
            run_id: context.workspace_id.clone(),
            revision: u64::MAX,
            session_id: context.workspace_id.clone(),
            epoch: u64::MAX,
            controller_id: context.workspace_id.clone(),
            cue_ids: vec!["intro".into(), "details".into()],
            cursor: 1,
            current_cue_id: "details".into(),
            next_cue_id: String::new(),
            state: LiveRunState::Paused as i32,
            submitted_count: 1,
            skipped_count: 0,
            reason: "PAUSED".into(),
            sentence_mode: true,
            sentence_total: 2,
            sentence_cursor: 1,
            current_sentence_index: 0,
            submitted_sentences: 1,
            skipped_sentences: 0,
            human_takeover: true,
            takeover_id: "handoff-wire".into(),
            partial_sentence: true,
        },
        write,
    );
    roundtrip(
        dir,
        "live-speech-read.bin",
        FoundationCommand {
            idempotency_key: context.workspace_id.clone(),
            operation: Some(foundation_command::Operation::ReadLiveSpeechPlan(
                LiveRunRead {
                    workspace_id: context.workspace_id.clone(),
                    run_id: context.workspace_id.clone(),
                },
            )),
        },
        write,
    );
    let mut text_plan = LiveSpeechPlan {
        source: Some(context.clone()),
        run_id: context.workspace_id.clone(),
        revision: u64::MAX,
        epoch: u64::MAX,
        sentences: vec![
            LiveSentence {
                cue_id: "intro".into(),
                sentence_index: 0,
                text: "容量500.5毫升。".into(),
            },
            LiveSentence {
                cue_id: "details".into(),
                sentence_index: 0,
                text: "重量1.25千克。".into(),
            },
        ],
        current_start: 1,
        prefetch_limit: 2,
        max_sentence_chars: 120,
        plan_sha256: "c".repeat(64),
        batch_approval: None,
    };
    text_plan.plan_sha256 = nai_contracts::live_speech_plan_hash(&text_plan);
    roundtrip(dir, "live-speech-plan.bin", text_plan, write);
    for (name, operation) in [
        (
            "live-sentence-preview",
            foundation_command::Operation::PreviewLiveSentence(LiveSentencePreview {
                run: Some(LiveRunRead {
                    workspace_id: context.workspace_id.clone(),
                    run_id: context.workspace_id.clone(),
                }),
                preview_id: context.workspace_id.clone(),
                sentence_cursor: 1,
            }),
        ),
        (
            "live-batch-control",
            foundation_command::Operation::ControlLiveBatch(LiveBatchControl {
                run: Some(LiveRunRead {
                    workspace_id: context.workspace_id.clone(),
                    run_id: context.workspace_id.clone(),
                }),
                expected_revision: u64::MAX,
                expected_epoch: u64::MAX,
                approval_id: context.workspace_id.clone(),
                plan_sha256: "c".repeat(64),
                acknowledge_rights: true,
                revoke: false,
            }),
        ),
    ] {
        roundtrip(
            dir,
            &format!("{name}.bin"),
            FoundationCommand {
                idempotency_key: context.workspace_id.clone(),
                operation: Some(operation),
            },
            write,
        );
    }
    roundtrip(
        dir,
        "live-batch-view.bin",
        LiveBatchApproval {
            approval_id: context.workspace_id.clone(),
            plan_sha256: "c".repeat(64),
            expires_at_ms: u64::MAX,
            approved: false,
            revoked: true,
        },
        write,
    );
    roundtrip(dir, "question-context.bin", context.clone(), write);
    let pool_scope = QuestionPoolRead {
        workspace_id: context.workspace_id.clone(),
        snapshot_id: context.snapshot_id.clone(),
        snapshot_sha256: context.snapshot_sha256.clone(),
    };
    for (name, operation) in [
        (
            "question-read",
            foundation_command::Operation::ReadQuestionPool(pool_scope.clone()),
        ),
        (
            "question-enqueue",
            foundation_command::Operation::EnqueueQuestion(QuestionPoolEnqueue {
                scope: Some(pool_scope),
                question_id: context.workspace_id.clone(),
                question: "容量多大？".into(),
            }),
        ),
        (
            "question-run",
            foundation_command::Operation::RunQuestion(QuestionPoolRun {
                workspace_id: context.workspace_id.clone(),
                question_id: context.workspace_id.clone(),
                expected_revision: u64::MAX,
                task_id: context.snapshot_id.clone(),
            }),
        ),
        (
            "question-cancel",
            foundation_command::Operation::CancelQuestion(QuestionPoolCancel {
                workspace_id: context.workspace_id.clone(),
                question_id: context.workspace_id.clone(),
                expected_revision: u64::MAX,
            }),
        ),
    ] {
        roundtrip(
            dir,
            &format!("{name}.bin"),
            FoundationCommand {
                idempotency_key: context.workspace_id.clone(),
                operation: Some(operation),
            },
            write,
        );
    }
    roundtrip(
        dir,
        "question-view.bin",
        QuestionPoolView {
            context: Some(context.clone()),
            capacity: 32,
            ttl_ms: 120000,
            entries: vec![QuestionPoolEntry {
                id: context.workspace_id.clone(),
                revision: u64::MAX,
                question: "容量多大？".into(),
                state: AgentTaskState::WaitingHuman as i32,
                suggestion_state: SuggestionState::Proposed as i32,
                created_at_ms: u64::MAX - 120000,
                expires_at_ms: u64::MAX,
                occurrences: 99,
                task_id: context.snapshot_id.clone(),
                task_sha256: "d".repeat(64),
                context_sha256: context.context_sha256.clone(),
                reason: "CANDIDATE_WAITING_HUMAN".into(),
            }],
        },
        write,
    );
    roundtrip(
        dir,
        "caption-presentation.bin",
        PublicPresentationStatus {
            preparation_id: "00000000-0000-4000-8000-000000000001".into(),
            frame_index: u64::MAX,
            sample_start: u64::MAX,
            caption_visible: true,
            caption_sha256: "a".repeat(64),
            composited_sha256: "b".repeat(64),
            presentation_count: u64::MAX,
            submission_clock: true,
        },
        write,
    );
    roundtrip(
        dir,
        "caption-unavailable.bin",
        PublicWindowPrepared {
            caption_state: PublicCaptionState::MissingGlyph as i32,
            reason: WorkbenchError::CapabilityUnavailable.as_str_name().into(),
            ..Default::default()
        },
        write,
    );
    roundtrip(
        dir,
        "execution-lease.bin",
        PublicExecutionLease {
            authorization: "f".repeat(64),
            output_id: "00000000-0000-4000-8000-000000000001".into(),
            bundle_sha256: "a".repeat(64),
            prepared_media_sha256: "b".repeat(64),
            session_id: "00000000-0000-4000-8000-000000000002".into(),
            epoch: u64::MAX,
            challenge: u64::MAX,
            seq: u64::MAX,
            lease_ms: 500,
            purpose: "nai-public-execution-v1".into(),
            preparation_id: "00000000-0000-4000-8000-000000000003".into(),
        },
        write,
    );

    let execution = PublicExecutionLease {
        authorization: "f".repeat(64),
        output_id: "00000000-0000-4000-8000-000000000001".into(),
        bundle_sha256: "a".repeat(64),
        prepared_media_sha256: "b".repeat(64),
        session_id: "00000000-0000-4000-8000-000000000002".into(),
        epoch: u64::MAX,
        seq: u64::MAX,
        challenge: u64::MAX,
        lease_ms: 500,
        purpose: "nai-public-execution-v1".into(),
        preparation_id: "00000000-0000-4000-8000-000000000003".into(),
    };
    for (name, operation) in [
        (
            "execution-start.bin",
            foundation_command::Operation::StartPublicExecution(FoundationPublicWindowPrepare {
                candidate: Some(Default::default()),
                bundle_sha256: "a".repeat(64),
            }),
        ),
        (
            "execution-renew.bin",
            foundation_command::Operation::RenewPublicExecution(FoundationPublicWindowPrepare {
                candidate: Some(Default::default()),
                bundle_sha256: "a".repeat(64),
            }),
        ),
        (
            "execution-stop.bin",
            foundation_command::Operation::StopPublicExecution(PublicApprovalRevoke {
                workspace_id: "00000000-0000-4000-8000-000000000001".into(),
                approval_id: "00000000-0000-4000-8000-000000000002".into(),
            }),
        ),
    ] {
        roundtrip(
            dir,
            name,
            FoundationCommand {
                operation: Some(operation),
                ..Default::default()
            },
            write,
        );
    }
    roundtrip(
        dir,
        "execution-private.bin",
        FoundationResponse {
            outcome: Some(foundation_response::Outcome::Result(FoundationResult {
                private_execution_lease: Some(Box::new(execution.clone())),
                ..Default::default()
            })),
            ..Default::default()
        },
        write,
    );
    roundtrip(
        dir,
        "execution-transport.bin",
        PublicOutputLease {
            execution: Some(Box::new(execution)),
            refresh_execution_challenge: true,
            ..Default::default()
        },
        write,
    );
    roundtrip(
        dir,
        "window-preparation.bin",
        PublicWindowPreparation {
            preparation_id: "00000000-0000-4000-8000-000000000003".into(),
            execution_key: "f".repeat(64),
            bundle: Some(Default::default()),
            caption_text: "容量为500毫升。".into(),
            data_path: "/fixture/public-preload-synthetic.pcm".into(),
            output_id: "00000000-0000-4000-8000-000000000001".into(),
            bundle_sha256: "a".repeat(64),
            ..Default::default()
        },
        write,
    );
    roundtrip(
        dir,
        "window-prepared.bin",
        PublicWindowPrepared {
            device_ready: true,
            device_name: "测试默认设备".into(),
            preparation_id: "00000000-0000-4000-8000-000000000003".into(),
            execution_challenge: u64::MAX,
            execution_seq: u64::MAX,
            reason: "DEVICE_INITIALIZED_MUTED".into(),
            submitted_samples: 0,
            ..Default::default()
        },
        write,
    );
    roundtrip(
        dir,
        "window-prepare-command.bin",
        FoundationCommand {
            operation: Some(foundation_command::Operation::PreparePublicWindow(
                FoundationPublicWindowPrepare {
                    candidate: Some(Default::default()),
                    bundle_sha256: "a".repeat(64),
                },
            )),
            ..Default::default()
        },
        write,
    );
    roundtrip(
        dir,
        "window-clear.bin",
        PublicOutputLease {
            authorization: "f".repeat(64),
            seq: u64::MAX,
            challenge: u64::MAX,
            lease_ms: 500,
            clear_prepared: true,
            ..Default::default()
        },
        write,
    );

    roundtrip(
        dir,
        "public-approval-draft.bin",
        PublicApprovalDraft {
            bundle: Some(PublicMediaBundle {
                approval_id: "00000000-0000-4000-8000-000000000003".into(),
                approved_sha256: "a".repeat(64),
                ..Default::default()
            }),
            text: "已核对规格".into(),
            expires_at_ms: u64::MAX,
        },
        write,
    );
    roundtrip(
        dir,
        "public-approval-confirm.bin",
        FoundationCommand {
            operation: Some(foundation_command::Operation::ConfirmPublicApproval(
                FoundationPublicApproval {
                    candidate: Some(FoundationPublicCandidate {
                        task_id: "00000000-0000-4000-8000-000000000002".into(),
                        ..Default::default()
                    }),
                    bundle_sha256: "a".repeat(64),
                    acknowledge_public_rights: true,
                },
            )),
            ..Default::default()
        },
        write,
    );
    roundtrip(
        dir,
        "public-approval-revoke.bin",
        FoundationCommand {
            operation: Some(foundation_command::Operation::RevokePublicApproval(
                PublicApprovalRevoke {
                    workspace_id: "00000000-0000-4000-8000-000000000001".into(),
                    approval_id: "00000000-0000-4000-8000-000000000003".into(),
                },
            )),
            ..Default::default()
        },
        write,
    );
    roundtrip(
        dir,
        "public-approval-view.bin",
        PublicApprovalView {
            approved: true,
            playback_available: false,
            expires_at_ms: u64::MAX,
            ..Default::default()
        },
        write,
    );

    roundtrip(
        dir,
        "prepared-receipt.bin",
        PreparedMediaReceipt {
            protocol_major: 1,
            descriptor_sha256: "a".repeat(64),
            consumer_instance_id: "00000000-0000-4000-8000-000000000003".into(),
            pcm_sha256: "b".repeat(64),
            sample_count: 480000,
            expected_frames: 600,
            first_frame_sha256: "c".repeat(64),
            tail_frame: Some(PublicMediaFrameInfo {
                frame_index: 600,
                sample_start: 479200,
                sample_count: 800,
                sample_rate: 24000,
                width: 512,
                height: 288,
                pixels_sha256: "d".repeat(64),
                openness_q16: 65535,
            }),
        },
        write,
    );
    roundtrip(
        dir,
        "prepared-media.bin",
        PreparedMediaDescriptor {
            protocol_major: 1,
            workspace_id: "00000000-0000-4000-8000-000000000001".into(),
            task_id: "00000000-0000-4000-8000-000000000002".into(),
            task_sha256: "a".repeat(64),
            expected_frames: 600,
            samples_per_frame: 800,
            first_frame_sha256: "b".repeat(64),
            pcm_format: "s16le-mono-24000".into(),
            pixel_format: "rgb32le".into(),
            renderer_id: "rms-sample-v1".into(),
            ..Default::default()
        },
        write,
    );
    roundtrip(
        dir,
        "public-media-frame.bin",
        PublicMediaFrameInfo {
            frame_index: 600,
            sample_start: 479200,
            sample_count: 800,
            sample_rate: 24000,
            width: 512,
            height: 288,
            pixels_sha256: "c".repeat(64),
            openness_q16: 65535,
        },
        write,
    );
    if write {
        fs::create_dir_all(dir).unwrap();
    }
    let avatar = PublicMediaVisual {
        artifact_id: "d074f2a8-a618-4b43-9e24-679bf43a0001".into(),
        package_sha256: "c".repeat(64),
        license_sha256: "d".repeat(64),
        width: 512,
        height: 288,
        fps: 30,
    };
    let output_id = "00000000-0000-4000-8000-000000000020";
    roundtrip(
        dir,
        "output-binding-challenge.bin",
        PublicOutputBindingChallenge {
            challenge: u64::MAX,
            ttl_ms: 250,
        },
        write,
    );
    roundtrip(
        dir,
        "output-binding-observation.bin",
        PublicOutputObservation {
            challenge: u64::MAX,
            status: Some(PublicOutputStatus {
                output_id: output_id.into(),
                visual: Some(avatar.clone()),
                ..Default::default()
            }),
            stop_nonce: "00000000-0000-4000-8000-000000000021".into(),
        },
        write,
    );
    roundtrip(
        dir,
        "output-binding-prepare.bin",
        FoundationRequest {
            command: Some(FoundationCommand {
                operation: Some(foundation_command::Operation::PreparePublicOutputBinding(
                    true,
                )),
                ..Default::default()
            }),
            ..Default::default()
        },
        write,
    );
    let guard = PublicGuardObject {
        protocol_major: 1,
        policy_id: "nai-public-exact-spec-v2".into(),
        workspace_id: "workspace".into(),
        session_id: "session".into(),
        epoch: u64::MAX,
        preview_id: "preview".into(),
        preview_sha256: "a".repeat(64),
        task_id: "task".into(),
        task_sha256: "b".repeat(64),
        snapshot_id: "snapshot".into(),
        snapshot_sha256: "c".repeat(64),
        facts_sha256: "d".repeat(64),
        caption_sha256: nai_contracts::public_caption_hash("本句夹具，非公共批准"),
        output_target_id: "903fb418-ef18-4c27-99f6-173a018e0001".into(),
        audio: Some(PublicMediaAudio {
            artifact_id: "audio".into(),
            pcm_sha256: "e".repeat(64),
            sample_count: u64::MAX,
            sample_rate: 24000,
            channels: 1,
        }),
        visual: Some(avatar.clone()),
    };
    let review = PublicGuardReview {
        object_sha256: nai_contracts::public_guard_hash(&guard),
        object: Some(guard.clone()),
        policy_passed: false,
        reason: PublicGuardReason::UnsupportedSpeech as i32,
        checked_at_ms: 1,
        valid_until_ms: 30001,
    };
    roundtrip(dir, "public-guard-object.bin", guard, write);
    roundtrip(dir, "public-guard-review.bin", review.clone(), write);
    roundtrip(
        dir,
        "public-output-avatar.bin",
        PublicOutputStatus {
            output_id: output_id.into(),
            protocol_major: 1,
            visual: Some(avatar.clone()),
            visual_name: "内置简洁2D形象".into(),
            visual_license: "MIT".into(),
            standby: true,
            standby_frames: u64::MAX,
            ..Default::default()
        },
        write,
    );
    roundtrip(
        dir,
        "public-candidate.bin",
        FoundationRequest {
            command: Some(FoundationCommand {
                operation: Some(foundation_command::Operation::CheckPublicCandidate(
                    FoundationPublicCandidate {
                        workspace_id: "workspace".into(),
                        preview_id: "preview".into(),
                        preview_sha256: "a".repeat(64),
                        task_id: "task".into(),
                        task_sha256: "b".repeat(64),
                        output_observation: None,
                    },
                )),
                ..Default::default()
            }),
            ..Default::default()
        },
        write,
    );
    roundtrip(
        dir,
        "public-candidate-response.bin",
        FoundationResponse {
            outcome: Some(foundation_response::Outcome::Result(FoundationResult {
                public_candidate_check: Some(Box::new(PublicCandidateCheck {
                    text: "本句夹具，非公共批准".into(),
                    epoch: u64::MAX,
                    checked_at_ms: u64::MAX,
                    approval_available: false,
                    guard_review: Some(review),
                    visual: Some(avatar),
                    visual_name: "内置简洁2D形象".into(),
                    visual_license: "MIT".into(),
                    axes: vec![PublicCandidateAxis {
                        axis: "output".into(),
                        state: PublicCandidateAxisState::NotReady as i32,
                        reason: "PUBLIC_MEDIA_OUTPUT_NOT_INSTALLED".into(),
                    }],
                    ..Default::default()
                })),
                ..Default::default()
            })),
            ..Default::default()
        },
        write,
    );
    let selection = OutputDeviceSelection {
        output_session_id: "c".repeat(64),
        device_id: 2,
    };
    roundtrip(
        dir,
        "device-select.bin",
        FoundationRequest {
            command: Some(FoundationCommand {
                operation: Some(foundation_command::Operation::SelectOutputDevice(
                    FoundationOutputDeviceSelect {
                        workspace_id: "workspace".into(),
                        task_id: "task".into(),
                        selection: Some(selection.clone()),
                    },
                )),
                ..Default::default()
            }),
            ..Default::default()
        },
        write,
    );
    roundtrip(
        dir,
        "device-select-worker.bin",
        AuditionControl {
            operation: Some(audition_control::Operation::SelectDevice(selection)),
        },
        write,
    );
    roundtrip(
        dir,
        "device-choices.bin",
        FoundationResponse {
            outcome: Some(foundation_response::Outcome::Result(FoundationResult {
                audition: Some(AuditionView {
                    state: AuditionState::Choosing as i32,
                    output_session_id: "c".repeat(64),
                    devices: vec![
                        OutputDeviceView {
                            device_id: 1,
                            name: "扬声器".into(),
                            is_default: true,
                        },
                        OutputDeviceView {
                            device_id: 2,
                            name: "耳机".into(),
                            is_default: false,
                        },
                    ],
                    ..Default::default()
                }),
                ..Default::default()
            })),
            ..Default::default()
        },
        write,
    );
    let plan = FoundationSpeechPlan {
        workspace_id: "workspace".into(),
        preview_id: "preview".into(),
        preview_sha256: "a".repeat(64),
        model_id: "model".into(),
        model_revision: u64::MAX,
        max_cost_minor: u64::MAX,
        task_id: "one-task".into(),
    };
    for (name, operation) in [
        (
            "speech-plan",
            foundation_command::Operation::CheckPreviewSpeech(plan.clone()),
        ),
        (
            "speech-start",
            foundation_command::Operation::SynthesizePreviewSpeech(FoundationSpeechStart {
                plan: Some(plan),
                task_id: "one-task".into(),
                approval_sha256: "b".repeat(64),
            }),
        ),
    ] {
        roundtrip(
            dir,
            &format!("{name}.bin"),
            FoundationRequest {
                command: Some(FoundationCommand {
                    operation: Some(operation),
                    ..Default::default()
                }),
                ..Default::default()
            },
            write,
        );
    }
    roundtrip(
        dir,
        "speech-plan-response.bin",
        FoundationResponse {
            outcome: Some(foundation_response::Outcome::Result(FoundationResult {
                speech_plan: Some(SpeechPlanView {
                    approval_sha256: "b".repeat(64),
                    text: "已发布话术".into(),
                    characters: 5,
                    reserve_minor: u64::MAX,
                    expires_at_ms: u64::MAX,
                    target: "private_native_audio".into(),
                    voice: "voice:alex".into(),
                    estimate_known: true,
                    agent_context: Some(context.clone()),
                }),
                ..Default::default()
            })),
            ..Default::default()
        },
        write,
    );
    for (name, operation) in [
        (
            "audition-start",
            foundation_command::Operation::StartTtsAudition(FoundationAuditionStart {
                audio: Some(FoundationTtsAudio {
                    workspace_id: "workspace".into(),
                    task_id: "task".into(),
                    task_sha256: "a".repeat(64),
                }),
                expected_stop_nonce: "nonce".into(),
            }),
        ),
        (
            "audition-renew",
            foundation_command::Operation::RenewTtsAudition(FoundationLlmTaskRead {
                workspace_id: "workspace".into(),
                task_id: "task".into(),
            }),
        ),
        (
            "audition-stop",
            foundation_command::Operation::StopTtsAudition(FoundationLlmTaskRead {
                workspace_id: "workspace".into(),
                task_id: "task".into(),
            }),
        ),
    ] {
        roundtrip(
            dir,
            &format!("{name}.bin"),
            FoundationRequest {
                command: Some(FoundationCommand {
                    operation: Some(operation),
                    ..Default::default()
                }),
                ..Default::default()
            },
            write,
        );
    }
    roundtrip(
        dir,
        "audition-nonce.bin",
        AuditionStopNonce {
            nonce: "nonce".into(),
        },
        write,
    );
    roundtrip(
        dir,
        "audition-control.bin",
        AuditionControl {
            operation: Some(audition_control::Operation::Lease(AuditionLease {
                challenge: u64::MAX,
                epoch: u64::MAX,
                ttl_ms: 500,
            })),
        },
        write,
    );
    roundtrip(
        dir,
        "audition-status.bin",
        AuditionStatus {
            view: Some(AuditionView {
                state: AuditionState::Submitted as i32,
                reason: "SUBMITTED_NOT_DEVICE_ACK".into(),
                submitted_samples: 480000,
                ..Default::default()
            }),
            challenge: u64::MAX,
        },
        write,
    );
    roundtrip(
        dir,
        "audition-response.bin",
        FoundationResponse {
            outcome: Some(foundation_response::Outcome::Result(FoundationResult {
                audition: Some(AuditionView {
                    state: AuditionState::Stopped as i32,
                    reason: "STOPPED".into(),
                    submitted_samples: 42,
                    ..Default::default()
                }),
                ..Default::default()
            })),
            ..Default::default()
        },
        write,
    );
    for speech in [false, true] {
        roundtrip(
            dir,
            if speech {
                "catalog-delete-tts.bin"
            } else {
                "catalog-delete-llm.bin"
            },
            FoundationRequest {
                protocol_major: 2,
                authorization: vec![0; 64],
                request_id: "catalog-delete".into(),
                seq: u64::MAX,
                ttl_ms: 4000,
                command: Some(FoundationCommand {
                    idempotency_key: "00000000-0000-0000-0000-000000000002".into(),
                    operation: Some(if speech {
                        foundation_command::Operation::DeleteTtsModel(FoundationDelete {
                            workspace_id: "00000000-0000-0000-0000-000000000003".into(),
                            object_id: "00000000-0000-0000-0000-000000000004".into(),
                            expected_revision: u64::MAX,
                        })
                    } else {
                        foundation_command::Operation::DeleteLlmModel(FoundationDelete {
                            workspace_id: "00000000-0000-0000-0000-000000000003".into(),
                            object_id: "00000000-0000-0000-0000-000000000004".into(),
                            expected_revision: u64::MAX,
                        })
                    }),
                }),
            },
            write,
        );
    }
    for (name, operation) in [
        (
            "tts-authorize",
            foundation_command::Operation::AuthorizeTtsAudio(FoundationTtsAudio {
                workspace_id: "00000000-0000-0000-0000-000000000003".into(),
                task_id: "00000000-0000-0000-0000-000000000004".into(),
                task_sha256: "aa".repeat(32),
            }),
        ),
        (
            "tts-revoke",
            foundation_command::Operation::RevokeTtsAudio(FoundationTtsAudio {
                workspace_id: "00000000-0000-0000-0000-000000000003".into(),
                task_id: "00000000-0000-0000-0000-000000000004".into(),
                task_sha256: "aa".repeat(32),
            }),
        ),
    ] {
        roundtrip(
            dir,
            &format!("{name}.bin"),
            FoundationRequest {
                protocol_major: 2,
                authorization: vec![0; 64],
                request_id: name.into(),
                seq: u64::MAX,
                ttl_ms: 4000,
                command: Some(FoundationCommand {
                    idempotency_key: "00000000-0000-0000-0000-000000000002".into(),
                    operation: Some(operation),
                }),
            },
            write,
        );
    }
    roundtrip(
        dir,
        "tts-audio-response.bin",
        FoundationResponse {
            request_id: "tts-audio-response".into(),
            seq: u64::MAX,
            outcome: Some(foundation_response::Outcome::Result(FoundationResult {
                tts_audio: Some(TtsAudioAuthorization {
                    artifact_id: "00000000-0000-0000-0000-000000000004".into(),
                    sample_count: u64::MAX,
                    sample_rate: 24000,
                    pcm_sha256: "bb".repeat(32),
                    lease_epoch: u64::MAX,
                    ttl_ms: 500,
                }),
                ..Default::default()
            })),
        },
        write,
    );
    for (name, operation) in [
        (
            "tts-test",
            foundation_command::Operation::TestTts(FoundationTtsTest {
                workspace_id: "00000000-0000-0000-0000-000000000003".into(),
                task_id: "00000000-0000-0000-0000-000000000004".into(),
                model_id: "00000000-0000-0000-0000-000000000005".into(),
                model_revision: u64::MAX,
            }),
        ),
        (
            "tts-read",
            foundation_command::Operation::ReadTtsTask(FoundationLlmTaskRead {
                workspace_id: "00000000-0000-0000-0000-000000000003".into(),
                task_id: "00000000-0000-0000-0000-000000000004".into(),
            }),
        ),
        (
            "tts-cancel",
            foundation_command::Operation::CancelTts(FoundationLlmTaskRead {
                workspace_id: "00000000-0000-0000-0000-000000000003".into(),
                task_id: "00000000-0000-0000-0000-000000000004".into(),
            }),
        ),
    ] {
        roundtrip(
            dir,
            &format!("{name}.bin"),
            FoundationRequest {
                protocol_major: 2,
                authorization: vec![0; 64],
                request_id: name.into(),
                seq: u64::MAX,
                ttl_ms: 4000,
                command: Some(FoundationCommand {
                    idempotency_key: "00000000-0000-0000-0000-000000000002".into(),
                    operation: Some(operation),
                }),
            },
            write,
        );
    }
    roundtrip(
        dir,
        "catalog-tts.bin",
        FoundationRequest {
            protocol_major: 2,
            authorization: vec![0; 64],
            request_id: "catalog-tts".into(),
            seq: u64::MAX,
            ttl_ms: 4000,
            command: Some(FoundationCommand {
                idempotency_key: "00000000-0000-0000-0000-000000000002".into(),
                operation: Some(foundation_command::Operation::SaveTtsModel(
                    FoundationTtsModel {
                        protocol: "openai_speech".into(),
                        language: String::new(),
                        workspace_id: "00000000-0000-0000-0000-000000000003".into(),
                        model_id: "00000000-0000-0000-0000-000000000004".into(),
                        expected_revision: u64::MAX,
                        name: "语音夹具".into(),
                        base_url: "https://example.invalid/v1".into(),
                        model: "speech".into(),
                        voice: "voice:alex".into(),
                        price_known: true,
                        character_minor_per_million: u64::MAX,
                        ..Default::default()
                    },
                )),
            }),
        },
        write,
    );
    roundtrip(
        dir,
        "catalog-qwen.bin",
        FoundationRequest {
            protocol_major: 2,
            authorization: vec![0; 64],
            request_id: "catalog-qwen".into(),
            seq: u64::MAX,
            ttl_ms: 4000,
            command: Some(FoundationCommand {
                idempotency_key: "00000000-0000-0000-0000-000000000002".into(),
                operation: Some(foundation_command::Operation::SaveTtsModel(
                    FoundationTtsModel {
                        workspace_id: "00000000-0000-0000-0000-000000000003".into(),
                        model_id: "00000000-0000-0000-0000-000000000004".into(),
                        name: "本地Qwen3".into(),
                        base_url: "http://127.0.0.1:8000/v1".into(),
                        model: "qwen3-tts-1.7b-customvoice".into(),
                        voice: "serena".into(),
                        protocol: "qwen_local".into(),
                        language: "chinese".into(),
                        enabled: true,
                        ..Default::default()
                    },
                )),
            }),
        },
        write,
    );
    roundtrip(
        dir,
        "catalog-response.bin",
        FoundationResponse {
            request_id: "catalog-response".into(),
            seq: u64::MAX,
            outcome: Some(foundation_response::Outcome::Result(FoundationResult {
                tts_models: vec![TtsModelView {
                    protocol: "openai_speech".into(),
                    language: String::new(),
                    id: "00000000-0000-0000-0000-000000000004".into(),
                    revision: u64::MAX,
                    name: "语音夹具".into(),
                    base_url: "https://example.invalid/v1".into(),
                    model: "speech".into(),
                    voice: "voice:alex".into(),
                    enabled: true,
                    credential_saved: true,
                    price_known: false,
                    character_minor_per_million: 0,
                }],
                ..Default::default()
            })),
        },
        write,
    );
    roundtrip(
        dir,
        "llm-test.bin",
        FoundationRequest {
            protocol_major: 2,
            authorization: vec![0; 64],
            request_id: "model-test-fixture".into(),
            seq: u64::MAX,
            ttl_ms: 4000,
            command: Some(FoundationCommand {
                idempotency_key: "00000000-0000-0000-0000-000000000002".into(),
                operation: Some(foundation_command::Operation::TestLlm(
                    FoundationModelTest {
                        workspace_id: "00000000-0000-0000-0000-000000000003".into(),
                        task_id: "00000000-0000-0000-0000-000000000004".into(),
                        model_id: "00000000-0000-0000-0000-000000000005".into(),
                        model_revision: u64::MAX,
                    },
                )),
            }),
        },
        write,
    );
    roundtrip(
        dir,
        "llm-worker.bin",
        LlmWorkerRequest {
            task_id: "test-worker-fixture".into(),
            connection_test: true,
            max_output_tokens: 256,
            deadline_ms: 15000,
            ..Default::default()
        },
        write,
    );
    roundtrip(
        dir,
        "llm-check.bin",
        FoundationRequest {
            protocol_major: 2,
            authorization: vec![0; 64],
            request_id: "local-check-fixture".into(),
            seq: u64::MAX,
            ttl_ms: 4000,
            command: Some(FoundationCommand {
                idempotency_key: "fixture-only".into(),
                operation: Some(foundation_command::Operation::CheckLlm(FoundationLlmPlan {
                    workspace_id: "fixture-only".into(),
                    snapshot_id: "fixture-only".into(),
                    snapshot_sha256: "a".repeat(64),
                    cue_id: "intro".into(),
                    model_id: "fixture-only".into(),
                    model_revision: u64::MAX,
                    max_cost_minor: u64::MAX,
                })),
            }),
        },
        write,
    );
    roundtrip(
        dir,
        "llm-check-response.bin",
        FoundationResponse {
            request_id: "local-check-fixture".into(),
            seq: u64::MAX,
            outcome: Some(foundation_response::Outcome::Result(FoundationResult {
                llm_check: Some(LlmLocalCheck {
                    reason: LlmLocalReason::BudgetDisabled as i32,
                    estimate_known: true,
                    estimated_reserve_minor: u64::MAX,
                    available_minor: 0,
                    input_bytes: 4096,
                    max_output_tokens: 256,
                    deadline_ms: 15000,
                    credential_checked: true,
                    currency: "CNY".into(),
                }),
                ..Default::default()
            })),
        },
        write,
    );
    for (name, operation) in [
        (
            "llm-config.bin",
            foundation_command::Operation::SaveLlmModel(FoundationLlmModel {
                workspace_id: "fixture-only".into(),
                model_id: "fixture-only".into(),
                expected_revision: u64::MAX,
                name: "fixture model".into(),
                base_url: "https://example.invalid/v1".into(),
                model: "test-model".into(),
                input_minor_per_million: u64::MAX,
                output_minor_per_million: u64::MAX,
                enabled: true,
                api_key: vec![],
                prices_confirmed: true,
            }),
        ),
        (
            "llm-start.bin",
            foundation_command::Operation::StartLlm(FoundationLlmStart {
                workspace_id: "fixture-only".into(),
                task_id: "fixture-only".into(),
                snapshot_id: "fixture-only".into(),
                snapshot_sha256: "a".repeat(64),
                cue_id: "intro".into(),
                model_id: "fixture-only".into(),
                model_revision: u64::MAX,
                max_cost_minor: u64::MAX,
            }),
        ),
    ] {
        roundtrip(
            dir,
            name,
            FoundationRequest {
                protocol_major: 2,
                authorization: vec![0; 64],
                request_id: "llm-fixture".into(),
                seq: u64::MAX,
                ttl_ms: 4000,
                command: Some(FoundationCommand {
                    idempotency_key: "fixture-only".into(),
                    operation: Some(operation),
                }),
            },
            write,
        );
    }
    roundtrip(
        dir,
        "llm-response.bin",
        FoundationResponse {
            request_id: "llm-fixture".into(),
            seq: u64::MAX,
            outcome: Some(foundation_response::Outcome::Result(FoundationResult {
                resource_id: "fixture-only".into(),
                task_state: AgentTaskState::Cancelled as i32,
                charge_status: "unknown".into(),
                reserved_minor: u64::MAX,
                llm_budget: Some(LlmBudgetView {
                    mode: WorkspaceBudgetMode::Limited as i32,
                    revision: u64::MAX,
                    limit_minor: u64::MAX,
                    used_minor: u64::MAX,
                    unknown_minor: u64::MAX,
                    reserved_minor: 0,
                    currency: "CNY".into(),
                }),
                ..Default::default()
            })),
        },
        write,
    );
    roundtrip(
        dir,
        "faq.bin",
        FoundationRequest {
            protocol_major: 2,
            authorization: vec![0; 64],
            request_id: "wire-fixture".into(),
            seq: u64::MAX,
            ttl_ms: 4000,
            command: Some(FoundationCommand {
                idempotency_key: "fixture".into(),
                operation: Some(foundation_command::Operation::AskFaq(
                    FoundationFaqRequest {
                        workspace_id: "workspace-fixture".into(),
                        task_id: "task-fixture".into(),
                        snapshot_id: "snapshot-fixture".into(),
                        snapshot_sha256: "a".repeat(64),
                        question: "容量多大？".into(),
                    },
                )),
            }),
        },
        write,
    );
    roundtrip(
        dir,
        "faq-resolve.bin",
        FoundationRequest {
            protocol_major: 2,
            authorization: vec![0; 64],
            request_id: "wire-fixture".into(),
            seq: u64::MAX,
            ttl_ms: 4000,
            command: Some(FoundationCommand {
                idempotency_key: "fixture".into(),
                operation: Some(foundation_command::Operation::ResolveFaq(
                    FoundationFaqResolve {
                        workspace_id: "workspace-fixture".into(),
                        task_id: "task-fixture".into(),
                        task_sha256: "a".repeat(64),
                        decision: FaqDecision::Edit as i32,
                        edited_text: "标准款500毫升。".into(),
                        acknowledge_edit: true,
                    },
                )),
            }),
        },
        write,
    );
    roundtrip(dir,"faq-retrieved.bin",FoundationResponse {
        request_id:"faq-retrieved".into(),seq:u64::MAX,
        outcome:Some(foundation_response::Outcome::Result(FoundationResult {
            faq_reason:FaqReason::Retrieved as i32,
            task_state:AgentTaskState::Completed as i32,
            suggestion_state:SuggestionState::Proposed as i32,
            body:serde_json::json!({"result":"retrieved","retrieval":{"strategy":"faq-lexical-zh-v1","method":"lexical","matched_question":"容量多大？","score":1000}}).to_string(),
            ..Default::default()
        })),
    },write);
    roundtrip(
        dir,
        "faq-response.bin",
        FoundationResponse {
            request_id: "wire-fixture".into(),
            seq: u64::MAX,
            outcome: Some(foundation_response::Outcome::Result(FoundationResult {
                task_state: AgentTaskState::Completed as i32,
                suggestion_state: SuggestionState::Edited as i32,
                faq_reason: FaqReason::Resolved as i32,
                adopted_object_id: "product-fixture".into(),
                adopted_revision: u64::MAX,
                ..Default::default()
            })),
        },
        write,
    );
    for (name, operation) in [
        (
            "preview.bin",
            foundation_command::Operation::Preview(FoundationPreview {
                workspace_id: "workspace-fixture".into(),
                preview_id: "preview-fixture".into(),
                snapshot_id: "snapshot-fixture".into(),
                snapshot_sha256: "a".repeat(64),
                cue_id: "intro".into(),
                session_id: "session-fixture".into(),
                expected_epoch: u64::MAX,
            }),
        ),
        (
            "browse.bin",
            foundation_command::Operation::ListPreparation(FoundationBrowse {
                workspace_id: "workspace-fixture".into(),
                kind: "product_draft".into(),
                after_id: "cursor-fixture".into(),
                limit: 32,
            }),
        ),
    ] {
        roundtrip(
            dir,
            name,
            FoundationRequest {
                protocol_major: 2,
                authorization: vec![0; 64],
                request_id: "wire-fixture".into(),
                seq: u64::MAX,
                ttl_ms: 4000,
                command: Some(FoundationCommand {
                    idempotency_key: "fixture".into(),
                    operation: Some(operation),
                }),
            },
            write,
        );
    }
    roundtrip(
        dir,
        "preview-response.bin",
        FoundationResponse {
            request_id: "wire-fixture".into(),
            seq: u64::MAX,
            outcome: Some(foundation_response::Outcome::Result(FoundationResult {
                evidence: EvidenceKind::FrontendPresented as i32,
                state: DeliveryState::Delivering as i32,
                resources: vec![FoundationResource {
                    id: "product-fixture".into(),
                    name: "产品".into(),
                    kind: "product_draft".into(),
                    revision: u64::MAX,
                    ..Default::default()
                }],
                next_cursor: "cursor-fixture".into(),
                ..Default::default()
            })),
        },
        write,
    );
    roundtrip(
        dir,
        "request.bin",
        WorkbenchRequest {
            protocol_major: 2,
            authorization: vec![0; 64],
            request_id: "wire-fixture".into(),
            seq: u64::MAX,
            ttl_ms: 4000,
            operation: Some(workbench_request::Operation::Inspect(WorkbenchInspect {
                config: Some(config()),
            })),
        },
        write,
    );
    roundtrip(
        dir,
        "response.bin",
        WorkbenchResponse {
            request_id: "wire-fixture".into(),
            seq: u64::MAX,
            handshake: Some(WorkbenchHandshake {
                protocol_major: 2,
                max_frame_bytes: 65536,
                build: "fixture".into(),
                clock: "monotonic-process".into(),
                lease_ms: 0,
            }),
            outcome: Some(workbench_response::Outcome::Problem(WorkbenchProblem {
                code: WorkbenchError::CapabilityUnavailable as i32,
                safe_message: "fixture-only".into(),
                request_id: "wire-fixture".into(),
                retryable: false,
                field_errors: vec![],
            })),
        },
        write,
    );
    roundtrip(
        dir,
        "delivery.bin",
        WorkbenchDeliveryUnit {
            delivery_id: "unit-fixture".into(),
            workspace_id: "workspace-fixture".into(),
            session_id: "session-fixture".into(),
            epoch: u64::MAX,
            snapshot_ref: "snapshot-fixture".into(),
            target_id: "private-fixture".into(),
            scope: DeliveryScope::Private as i32,
            bundle_sha256: "a".repeat(64),
            revision: u64::MAX,
            ttl_ms: 3000,
            state: DeliveryState::Selected as i32,
            evidence: EvidenceKind::Unspecified as i32,
        },
        write,
    );
    roundtrip(
        dir,
        "foundation.bin",
        FoundationRequest {
            protocol_major: 2,
            authorization: vec![0; 64],
            request_id: "00000000-0000-0000-0000-000000000001".into(),
            seq: 1,
            ttl_ms: 4000,
            command: Some(FoundationCommand {
                idempotency_key: "00000000-0000-0000-0000-000000000002".into(),
                operation: Some(foundation_command::Operation::PutPreparation(
                    FoundationObject {
                        workspace_id: "00000000-0000-0000-0000-000000000003".into(),
                        object_id: "00000000-0000-0000-0000-000000000004".into(),
                        expected_revision: u64::MAX,
                        body: include_str!("../../../tests/fixtures/contracts/preparation.json")
                            .into(),
                    },
                )),
            }),
        },
        write,
    );
    roundtrip(
        dir,
        "publication.bin",
        FoundationRequest {
            protocol_major: 2,
            authorization: vec![0; 64],
            request_id: "publication-fixture".into(),
            seq: u64::MAX,
            ttl_ms: 4000,
            command: Some(FoundationCommand {
                idempotency_key: "00000000-0000-0000-0000-000000000002".into(),
                operation: Some(foundation_command::Operation::Publish(FoundationPublish {
                    workspace_id: "00000000-0000-0000-0000-000000000003".into(),
                    snapshot_id: "00000000-0000-0000-0000-000000000004".into(),
                    report_id: "00000000-0000-0000-0000-000000000005".into(),
                    report_sha256: "a".repeat(64),
                    acknowledge_unknown: true,
                })),
            }),
        },
        write,
    );
    roundtrip(
        dir,
        "publication-response.bin",
        FoundationResponse {
            request_id: "publication-fixture".into(),
            seq: u64::MAX,
            outcome: Some(foundation_response::Outcome::Result(FoundationResult {
                resource_id: "00000000-0000-0000-0000-000000000004".into(),
                publication_status: PublicationStatus::Blocked as i32,
                body: "已撤销的契约夹具，非运行结果".into(),
                ..Default::default()
            })),
        },
        write,
    );
    let auto_run = LiveRunRead {
        workspace_id: context.workspace_id.clone(),
        run_id: "00000000-0000-4000-8000-000000000003".into(),
    };
    let auto_start = LiveAutoStart {
        run: Some(auto_run.clone()),
        grant_id: "grant-wire".into(),
        expected_revision: u64::MAX,
        expected_epoch: u64::MAX,
        plan_sha256: "a".repeat(64),
        model_id: "model-wire".into(),
        model_revision: u64::MAX,
        max_cost_minor: u64::MAX,
        output_id: "native-wire".into(),
        visual_sha256: "b".repeat(64),
        license_sha256: "c".repeat(64),
        stop_nonce: "nonce-wire".into(),
        acknowledge_rights: true,
    };
    let auto_prepare = LiveAutoPrepare {
        run: Some(auto_run.clone()),
        grant_id: "grant-wire".into(),
        sentence_cursor: 1,
        preview_id: "preview-wire".into(),
        task_id: "task-wire".into(),
    };
    let auto_stop = LiveAutoStop {
        run: Some(auto_run.clone()),
        grant_id: "grant-wire".into(),
        reason: "OPERATOR_STOP".into(),
    };
    let auto_confirm = LiveAutoConfirm {
        run: Some(auto_run.clone()),
        grant_id: "grant-wire".into(),
        candidate: Some(FoundationPublicCandidate {
            workspace_id: context.workspace_id.clone(),
            task_id: "task-wire".into(),
            ..Default::default()
        }),
        bundle_sha256: "d".repeat(64),
    };
    let auto_task = LiveAutoTask {
        sentence_cursor: 1,
        preview_id: "preview-wire".into(),
        preview_sha256: "a".repeat(64),
        task_id: "task-wire".into(),
        task_sha256: "b".repeat(64),
        task_state: AgentTaskState::Completed as i32,
        model_id: "model-wire".into(),
        model_revision: u64::MAX,
    };
    let auto_view = LiveAutoView {
        run_id: auto_run.run_id.clone(),
        grant_id: "grant-wire".into(),
        state: "playing".into(),
        reason: "AUTHORIZED".into(),
        model_id: "model-wire".into(),
        model_revision: u64::MAX,
        voice: "serena".into(),
        output_id: "native-wire".into(),
        max_cost_minor: u64::MAX,
        reserved_minor: 20,
        expires_at_ms: u64::MAX,
        tasks: vec![auto_task.clone()],
        sentence_cursor: 1,
        sentence_total: 3,
        interruption_id: "interruption-wire".into(),
        interruption_state: "playing".into(),
        question: "容量多大？".into(),
        answer: "标准款容量为500毫升。".into(),
        return_cursor: 1,
    };
    let question = LiveQuestionControl {
        run: Some(auto_run.clone()),
        grant_id: "grant-wire".into(),
        interruption_id: "interruption-wire".into(),
        faq_task_id: "faq-wire".into(),
        faq_task_sha256: "f".repeat(64),
        expected_revision: u64::MAX,
        expected_epoch: u64::MAX,
        acknowledge_answer: true,
        cancel: false,
    };
    roundtrip(dir, "live-question-control.bin", question.clone(), write);
    roundtrip(
        dir,
        "live-question-command.bin",
        FoundationCommand {
            idempotency_key: "wire-question".into(),
            operation: Some(foundation_command::Operation::ControlLiveQuestion(question)),
        },
        write,
    );
    roundtrip(dir, "auto-start.bin", auto_start.clone(), write);
    roundtrip(dir, "auto-prepare.bin", auto_prepare.clone(), write);
    roundtrip(dir, "auto-stop.bin", auto_stop.clone(), write);
    roundtrip(dir, "auto-confirm.bin", auto_confirm.clone(), write);
    roundtrip(dir, "auto-task.bin", auto_task, write);
    roundtrip(dir, "auto-view.bin", auto_view.clone(), write);
    for (name, operation) in [
        (
            "auto-start-command.bin",
            foundation_command::Operation::StartLiveAuto(auto_start),
        ),
        (
            "auto-prepare-command.bin",
            foundation_command::Operation::PrepareLiveAuto(auto_prepare),
        ),
        (
            "auto-read-command.bin",
            foundation_command::Operation::ReadLiveAuto(auto_run),
        ),
        (
            "auto-stop-command.bin",
            foundation_command::Operation::StopLiveAuto(auto_stop),
        ),
        (
            "auto-confirm-command.bin",
            foundation_command::Operation::ConfirmLiveAuto(auto_confirm),
        ),
    ] {
        roundtrip(
            dir,
            name,
            FoundationCommand {
                idempotency_key: "wire".into(),
                operation: Some(operation),
            },
            write,
        );
    }
    roundtrip(
        dir,
        "auto-response.bin",
        FoundationResponse {
            request_id: "auto-wire".into(),
            seq: u64::MAX,
            outcome: Some(foundation_response::Outcome::Result(FoundationResult {
                live_auto: Some(auto_view),
                ..Default::default()
            })),
        },
        write,
    );
    roundtrip(
        dir,
        "mic-control.bin",
        MicControl {
            protocol_major: 1,
            seq: u64::MAX,
            challenge: u64::MAX,
            action: 3,
            instance_id: "mic-instance".into(),
            ..Default::default()
        },
        write,
    );
    roundtrip(
        dir,
        "mic-status.bin",
        MicStatus {
            protocol_major: 1,
            seq: u64::MAX,
            captured_frames: u64::MAX,
            active: false,
            reason: "STOPPED".into(),
            ..Default::default()
        },
        write,
    );
    roundtrip(
        dir,
        "takeover-command.bin",
        FoundationCommand {
            idempotency_key: "takeover-wire".into(),
            operation: Some(foundation_command::Operation::ControlLiveTakeover(
                LiveTakeoverControl {
                    expected_epoch: u64::MAX,
                    resume: true,
                    acknowledge: true,
                    skip_partial_sentence: true,
                    ..Default::default()
                },
            )),
        },
        write,
    );
    let review_source = ReviewSource {
        workspace_id: "workspace".into(),
        run_id: "ended-run".into(),
        run_kind: ReviewRunKind::Manual as i32,
    };
    let review_ref = ReviewObjectRef {
        object_id: "source".into(),
        revision: u64::MAX,
        sha256: "a".repeat(64),
    };
    let review_faq = ReviewFaq {
        id: "capacity".into(),
        question: "容量？".into(),
        text: "500毫升。".into(),
        fact_ids: vec!["capacity".into()],
    };
    for (name, operation) in [
        (
            "review-read",
            foundation_command::Operation::ReadReview(review_source.clone()),
        ),
        (
            "review-note",
            foundation_command::Operation::SaveReviewNote(ReviewNoteSave {
                question_source: Some(review_ref.clone()),
                source: Some(review_source.clone()),
                note_id: "note".into(),
                expected_revision: u64::MAX,
                title: "人工重点".into(),
                proposal: Some(review_faq.clone()),
            }),
        ),
        (
            "review-plan",
            foundation_command::Operation::PlanReviewDraft(ReviewDraftPlan {
                source: Some(review_source.clone()),
                note_id: "note".into(),
                note_revision: u64::MAX,
                job_id: "job".into(),
                target_product_id: "next-product".into(),
                target_show_id: "next-show".into(),
                expected_product_revision: u64::MAX,
                expected_show_revision: u64::MAX,
            }),
        ),
        (
            "review-job",
            foundation_command::Operation::ReadReviewDraft(ReviewDraftRead {
                workspace_id: "workspace".into(),
                job_id: "job".into(),
            }),
        ),
        (
            "review-apply",
            foundation_command::Operation::ApplyReviewDraft(ReviewDraftApply {
                workspace_id: "workspace".into(),
                job_id: "job".into(),
                expected_revision: u64::MAX,
                expected_sha256: "b".repeat(64),
                acknowledge: true,
            }),
        ),
    ] {
        roundtrip(
            dir,
            &format!("{name}.bin"),
            FoundationCommand {
                idempotency_key: "review-wire".into(),
                operation: Some(operation),
            },
            write,
        );
    }
    roundtrip(
        dir,
        "review-context.bin",
        FoundationResponse {
            request_id: "review".into(),
            seq: u64::MAX,
            outcome: Some(foundation_response::Outcome::Result(FoundationResult {
                review_context: Some(ReviewContext {
                    questions: vec![ReviewQuestion {
                        record: Some(review_ref.clone()),
                        question_id: "question".into(),
                        question: "容量多大？".into(),
                        occurrences: 99,
                        processing: "adopted".into(),
                        delivery: "unconfirmed".into(),
                        candidate_id: "faq".into(),
                        candidate_sha256: "e".repeat(64),
                        runtime_expired: true,
                        expires_at_ms: u64::MAX,
                        updated_at_ms: u64::MAX,
                    }],
                    question_capacity: 64,
                    source: Some(review_source.clone()),
                    run_name: "本场".into(),
                    run_revision: u64::MAX,
                    snapshot_id: "snapshot".into(),
                    snapshot_sha256: "c".repeat(64),
                    product: Some(review_ref.clone()),
                    show: Some(review_ref.clone()),
                    published_faq: vec![review_faq.clone()],
                    current_faq: vec![review_faq.clone()],
                    facts: vec![ReviewFact {
                        id: "capacity".into(),
                        label: "容量".into(),
                        value: "500毫升".into(),
                    }],
                    notes: vec![ReviewNoteView {
                        question_source: Some(review_ref.clone()),
                        note_id: "note".into(),
                        revision: u64::MAX,
                        title: "人工重点".into(),
                        proposal: Some(review_faq.clone()),
                        created_at_ms: i64::MAX as u64,
                        evidence: "self_reported".into(),
                    }],
                    facts_unchanged: true,
                    reason: "READY".into(),
                }),
                ..Default::default()
            })),
        },
        write,
    );
    roundtrip(
        dir,
        "review-draft.bin",
        FoundationResponse {
            request_id: "review".into(),
            seq: u64::MAX,
            outcome: Some(foundation_response::Outcome::Result(FoundationResult {
                review_draft: Some(ReviewDraftView {
                    source: Some(review_source),
                    job_id: "job".into(),
                    revision: u64::MAX,
                    sha256: "b".repeat(64),
                    state: "applied".into(),
                    reason: "READY".into(),
                    note_id: "note".into(),
                    base_product: Some(review_ref.clone()),
                    base_show: Some(review_ref.clone()),
                    before: Some(review_faq.clone()),
                    current: Some(review_faq.clone()),
                    proposal: Some(review_faq),
                    target_product_id: "next-product".into(),
                    target_show_id: "next-show".into(),
                    saved_product: Some(ReviewObjectRef {
                        object_id: "next-product".into(),
                        ..review_ref.clone()
                    }),
                    saved_show: Some(ReviewObjectRef {
                        object_id: "next-show".into(),
                        ..review_ref
                    }),
                }),
                ..Default::default()
            })),
        },
        write,
    );
    for (name, operation) in [
        (
            "review-question-enqueue",
            foundation_command::Operation::EnqueueRunQuestion(ReviewRunQuestion {
                source: Some(ReviewSource {
                    workspace_id: "workspace".into(),
                    run_id: "run".into(),
                    run_kind: ReviewRunKind::Live as i32,
                }),
                expected_epoch: u64::MAX,
                question: Some(QuestionPoolEnqueue {
                    scope: Some(QuestionPoolRead {
                        workspace_id: "workspace".into(),
                        snapshot_id: "snapshot".into(),
                        snapshot_sha256: "a".repeat(64),
                    }),
                    question_id: "question".into(),
                    question: "容量多大？".into(),
                }),
            }),
        ),
        (
            "review-question-read",
            foundation_command::Operation::ReadRunQuestionPool(RunQuestionPoolRead {
                source: Some(ReviewSource {
                    workspace_id: "workspace".into(),
                    run_id: "run".into(),
                    run_kind: ReviewRunKind::Live as i32,
                }),
                scope: Some(QuestionPoolRead {
                    workspace_id: "workspace".into(),
                    snapshot_id: "snapshot".into(),
                    snapshot_sha256: "a".repeat(64),
                }),
                expected_epoch: u64::MAX,
            }),
        ),
    ] {
        roundtrip(
            dir,
            &format!("{name}.bin"),
            FoundationCommand {
                idempotency_key: "key".into(),
                operation: Some(operation),
            },
            write,
        );
    }
    roundtrip(
        dir,
        "software-feature-guard.bin",
        PublicGuardReview {
            object: Some(PublicGuardObject {
                protocol_major: 1,
                policy_id: PublicGuardPolicy::ExactSpecsAndSoftwareV5
                    .as_str_name()
                    .into(),
                epoch: u64::MAX,
                ..Default::default()
            }),
            policy_passed: true,
            reason: PublicGuardReason::ExactFeaturePassed as i32,
            checked_at_ms: u64::MAX,
            valid_until_ms: u64::MAX,
            object_sha256: "a".repeat(64),
        },
        write,
    );
    roundtrip(
        dir,
        "software-window-rejected.bin",
        PublicGuardReview {
            object: Some(PublicGuardObject {
                protocol_major: 1,
                policy_id: PublicGuardPolicy::ExactSpecsAndSoftwareV5
                    .as_str_name()
                    .into(),
                epoch: u64::MAX,
                ..Default::default()
            }),
            policy_passed: false,
            reason: PublicGuardReason::PlaybackWindowTooShort as i32,
            checked_at_ms: u64::MAX,
            valid_until_ms: u64::MAX,
            object_sha256: "a".repeat(64),
        },
        write,
    );
    println!(
        "workbench wire fixture {0}: 115 messages, uint64 max, private target; not runtime evidence",
        args[1]
    );
}
