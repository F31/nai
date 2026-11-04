#![cfg(target_os = "windows")]
use nai_media_core::device_clock::DeviceClockCursor;
use nai_media_core::video_clock::VideoCursor;
use std::time::{Duration, Instant};
use wasapi::{DeviceEnumerator, Direction, SampleType, StreamMode, WaveFormat};

#[test]
#[ignore = "explicit Windows silent audio device admission"]
fn actual_wasapi_clock_advances_and_freezes_after_stop() {
    wasapi::initialize_mta().unwrap();
    let enumerator = DeviceEnumerator::new().unwrap();
    let device = enumerator.get_default_device(&Direction::Render).unwrap();
    let mut client = device.get_iaudioclient().unwrap();
    let format = WaveFormat::new(32, 32, &SampleType::Float, 48000, 2, None);
    client
        .initialize_client(
            &format,
            &Direction::Render,
            &StreamMode::EventsShared {
                autoconvert: true,
                buffer_duration_hns: 500_000,
            },
        )
        .unwrap();
    let event = client.set_get_eventhandle().unwrap();
    let render = client.get_audiorenderclient().unwrap();
    let clock = client.get_audioclock().unwrap();
    let frequency = clock.get_frequency().unwrap();
    let (initial, _) = clock.get_position().unwrap();
    assert_eq!(initial, 0, "new stream must have a known origin");
    let mut cursor = DeviceClockCursor::new(1, frequency, 48000).unwrap();
    let silence = [0u8; 4800 * 8];
    let prefill = client.get_available_space_in_frames().unwrap() as usize;
    assert!((1..=4800).contains(&prefill));
    render
        .write_to_device(prefill, &silence[..prefill * 8], None)
        .unwrap();
    cursor.commit(prefill as u64).unwrap();
    let mut submitted = prefill as u64;
    let mut video = VideoCursor::new(48000, 48000, 25, 1, 25).unwrap();
    // Public media uses 24 kHz source samples while this device stream uses 48 kHz.
    let mut source_video = VideoCursor::new(24000, 24000, 25, 1, 25).unwrap();
    let mut last_video_index = 0;
    let mut changes = 0;
    client.start_stream().unwrap();
    // Stop on every unwind path, before releasing COM interfaces.
    struct Stop<'a>(&'a wasapi::AudioClient);
    impl Drop for Stop<'_> {
        fn drop(&mut self) {
            let _ = self.0.stop_stream();
        }
    }
    let stop = Stop(&client);
    let deadline = Instant::now() + Duration::from_secs(3);
    let mut played = 0;
    let mut observations = 0;
    while Instant::now() < deadline && played < 24000 {
        event.wait_for_event(100).unwrap();
        let (position, _qpc) = clock.get_position().unwrap();
        played = cursor
            .observe(1, clock.get_frequency().unwrap(), position)
            .unwrap();
        observations += 1;
        // Hardware clock frames must not be confused with callback-submitted frames.
        let frame = video.select(played, submitted).unwrap();
        assert_eq!(frame.index(), ((played * 25 / 48000) as u32).min(24));
        assert!(frame.index() >= last_video_index);
        if frame.index() > last_video_index {
            changes += 1;
            last_video_index = frame.index();
        }
        video.submitted(frame).unwrap();
        let source_played = cursor
            .observe_source(1, frequency, position, 24000)
            .unwrap();
        if played < submitted {
            assert_eq!(
                cursor.observe_cue_source(1, frequency, position, 0, submitted, 24000),
                Ok(Some(source_played))
            );
        }
        let source_submitted = (submitted / 2).min(24000);
        let source_frame = source_video
            .select(source_played, source_submitted)
            .unwrap();
        assert_eq!(source_frame.index(), frame.index());
        source_video.submitted(source_frame).unwrap();
        let frames = client.get_available_space_in_frames().unwrap() as usize;
        assert!(frames <= 4800);
        render
            .write_to_device(frames, &silence[..frames * 8], None)
            .unwrap();
        cursor.commit(frames as u64).unwrap();
        submitted += frames as u64;
    }
    assert!(
        played >= 24000 && observations > 1 && changes > 1,
        "no actual device progress"
    );
    client.stop_stream().unwrap();
    let (stopped, _) = clock.get_position().unwrap();
    std::thread::sleep(Duration::from_millis(100));
    assert_eq!(clock.get_position().unwrap().0, stopped);
    let stopped_frames = cursor.observe(1, frequency, stopped).unwrap();
    let stopped_frame = video.select(stopped_frames, submitted).unwrap();
    let stopped_source_frame = source_video
        .select(
            cursor.observe_source(1, frequency, stopped, 24000).unwrap(),
            (submitted / 2).min(24000),
        )
        .unwrap();
    assert_eq!(stopped_source_frame.index(), stopped_frame.index());
    assert_eq!(
        stopped_frame.index(),
        ((stopped_frames * 25 / 48000) as u32).min(24)
    );
    assert_eq!(
        video.select(stopped_frames, submitted).unwrap().index(),
        stopped_frame.index()
    );
    println!(
        "IAudioClock frequency={frequency} played_frames={played} observations={observations} video_frame_changes={changes} stopped_position={stopped}; silent_only=true playback_ack=false"
    );
    drop(stop);
}
