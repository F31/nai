//! End-to-end media timeline: 16k TTS -> 48k output mapping -> water-level
//! commit -> actual playback ACK -> continuous recording.
use nai_contracts::v1::OutputAck;
use nai_media_core::timeline::{
    ack::{AckTracker, PlaybackState},
    audio::{AudioBlock, SampleFormat},
    clock::{duration_in_output, map_cross_rate},
    record::{MemoryRecorder, RecordingSink},
    resample::resample_linear_i16,
    {CommitState, DEFAULT_VIDEO_WATER_FRAMES, UtteranceTimeline},
};

const MODEL_RATE: u32 = 16_000;
const OUTPUT_RATE: u32 = 48_000;

fn block(seq: u32, start: u64, count: u32, eos: bool) -> AudioBlock {
    AudioBlock {
        session_id: "s".into(),
        utterance_id: "utt".into(),
        generation_epoch: 7,
        seq,
        sample_rate: MODEL_RATE,
        channels: 1,
        format: SampleFormat::I16,
        sample_start: start,
        sample_count: count,
        end_of_stream: eos,
    }
}

#[test]
fn tts_to_output_timeline_commits_then_ack_and_records_continuously() {
    // Model audio: 4000 frames at 16k -> 12000 output frames at 48k (>= 200ms water).
    let model_pcm: Vec<i16> = (0..4000).map(|frame| (frame % 300) as i16).collect();
    let output = resample_linear_i16(&model_pcm, MODEL_RATE, OUTPUT_RATE, 40_000).unwrap();
    assert_eq!(output.len(), 12_000);
    assert_eq!(
        duration_in_output(4000, MODEL_RATE, OUTPUT_RATE),
        Some(12_000)
    );
    assert_eq!(map_cross_rate(MODEL_RATE, 4000, OUTPUT_RATE), 12_000);

    let mut timeline = UtteranceTimeline::new("utt", MODEL_RATE, OUTPUT_RATE, 200).unwrap();
    timeline.push_audio(&block(0, 0, 2000, false)).unwrap(); // 6000 output frames < 200ms audio water
    assert_eq!(timeline.state(), CommitState::NeedsAudio);
    timeline.push_audio(&block(1, 2000, 2000, true)).unwrap(); // 250ms model = 750ms output
    for frame in 1..=DEFAULT_VIDEO_WATER_FRAMES {
        timeline
            .push_video(frame, DEFAULT_VIDEO_WATER_FRAMES)
            .unwrap();
    }
    assert_eq!(timeline.commit(500, 0).unwrap(), CommitState::Committed);
    // After commit, more data is finished.
    assert_eq!(timeline.total_model_samples(), 4000);

    // Ack completion only after real playback.
    let mut tracker = AckTracker::new("out", &block(0, 0, 4000, true), 12_000).unwrap();
    assert_eq!(tracker.state(), PlaybackState::Pending);
    tracker.commit().unwrap();
    assert_eq!(
        tracker.acknowledge(&ack(false, 0)).unwrap(),
        PlaybackState::Committed
    );
    assert_eq!(
        tracker.acknowledge(&ack(true, 6_000)).unwrap(),
        PlaybackState::Playing
    );
    assert_eq!(
        tracker.acknowledge(&completed_ack(12_000)).unwrap(),
        PlaybackState::Completed
    );
    assert_eq!(tracker.state(), PlaybackState::Completed);

    // Record with the media clock position, resampled to output rate.
    let mut recorder = MemoryRecorder::new(OUTPUT_RATE);
    let first_samples: Vec<u8> = output[..6_000]
        .iter()
        .flat_map(|frame| frame.to_le_bytes())
        .collect();
    recorder
        .push_audio(&block_out(6_000), 0, &first_samples)
        .unwrap();
    let second: Vec<u8> = output[6_000..]
        .iter()
        .flat_map(|frame| frame.to_le_bytes())
        .collect();
    recorder
        .push_audio(&block_out(6_000), 6_000, &second)
        .unwrap();
    assert_eq!(recorder.samples.len(), 12_000);
    assert_eq!(recorder.position, 12_000);
    assert_eq!(recorder.gaps_written, 0);
    recorder.close();
}

fn block_out(count: u32) -> AudioBlock {
    AudioBlock {
        session_id: "s".into(),
        utterance_id: "utt".into(),
        generation_epoch: 7,
        seq: 0,
        sample_rate: OUTPUT_RATE,
        channels: 1,
        format: SampleFormat::I16,
        sample_start: 0,
        sample_count: count,
        end_of_stream: false,
    }
}

fn ack(committed: bool, played: u64) -> OutputAck {
    OutputAck {
        output_id: "out".into(),
        utterance_id: "utt".into(),
        committed,
        actual_local_playback_completed: false,
        played_sample_count: played,
        ..Default::default()
    }
}
fn completed_ack(played: u64) -> OutputAck {
    OutputAck {
        output_id: "out".into(),
        utterance_id: "utt".into(),
        committed: true,
        actual_local_playback_completed: true,
        played_sample_count: played,
        media_state: nai_contracts::v1::MediaState::Playing as i32,
        ..Default::default()
    }
}
