"""Bounded in-process clock probe; no hardware playback or live audio."""
import queue
import threading
import time


def run(generate_batch, seconds, fps=25, capacity=8):
    if not (0 < seconds <= 600 and 1 <= fps <= 60 and 1 <= capacity <= 16):
        raise ValueError("probe bounds")
    frames = queue.Queue(maxsize=capacity)
    stop = threading.Event()
    data = {"consumed": 0, "underflow": 0, "produced": 0, "queue_peak": 0, "max_lateness_ms": 0.0}
    started = time.monotonic()
    end = started + seconds
    def consumer():
        tick = 0
        while True:
            due = started + tick / fps
            if due >= end or stop.is_set(): break
            delay = due - time.monotonic()
            if delay > 0 and stop.wait(delay): break
            data["max_lateness_ms"] = max(data["max_lateness_ms"], max(0.0, time.monotonic()-due)*1000)
            try:
                frames.get_nowait()
                data["consumed"] += 1
            except queue.Empty:
                data["underflow"] += 1
            tick += 1
        stop.set()
    thread = threading.Thread(target=consumer, daemon=False)
    thread.start()
    try:
        index = 0
        while not stop.is_set():
            batch = generate_batch(index)
            if not batch or len(batch) > 4: raise RuntimeError("invalid generated batch")
            for frame in batch:
                while not stop.is_set():
                    try:
                        frames.put(frame, timeout=0.05)
                        data["produced"] += 1
                        data["queue_peak"] = max(data["queue_peak"], frames.qsize())
                        index += 1
                        break
                    except queue.Full:
                        pass
    finally:
        stop.set()
        thread.join(timeout=2)
        if thread.is_alive(): raise RuntimeError("consumer failed to stop")
    data.update(elapsed_seconds=round(time.monotonic()-started, 3), remaining=frames.qsize(),
                max_lateness_ms=round(data["max_lateness_ms"], 3), capacity=capacity, fps=fps)
    return data


def _batch(generate_batch, index):
    batch = generate_batch(index)
    if not batch or len(batch) > 4: raise RuntimeError("invalid generated batch")
    return batch


def run_timestamped(generate_batch, seconds, fps=25, capacity=8, prefill=0):
    """Simulated audio ticks; missing frames are never replaced by old frames.

    Prefill offers the first frames before the clock starts. Those frames and
    their wait stay out of the timed window so startup lag is not counted as
    sustained loss. This clock is still not a hardware audio device.
    """
    from musetalk_timestamp_queue import TimestampFrames
    if not (0 < seconds <= 600 and 1 <= fps <= 60 and 1 <= capacity <= 16 and 0 <= prefill <= capacity):
        raise ValueError("probe bounds")
    epoch = 1
    frames = TimestampFrames(epoch, capacity)
    stop = threading.Event()
    data = {"presented": 0, "missing": 0, "expired": 0, "late_on_offer": 0,
            "skipped_generation": 0, "produced": 0, "queue_peak": 0, "max_lateness_ms": 0.0,
            "prefill_frames": 0, "prefill_seconds": 0.0}
    index = 0
    prefill_started = time.monotonic()
    while index < prefill:
        for frame in _batch(generate_batch, index):
            if index >= prefill: break
            status = frames.offer(epoch, index, frame)
            if status != "accepted": raise RuntimeError("prefill rejected frame: " + status)
            data["queue_peak"] = max(data["queue_peak"], frames.size())
            index += 1
    data["prefill_frames"] = index
    data["prefill_seconds"] = round(time.monotonic() - prefill_started, 3)
    started = time.monotonic()
    end = started + seconds
    def consumer():
        tick = 0
        while True:
            due = started + tick / fps
            if due >= end or stop.is_set(): break
            delay = due - time.monotonic()
            if delay > 0 and stop.wait(delay): break
            data["max_lateness_ms"] = max(data["max_lateness_ms"], max(0.0, time.monotonic()-due)*1000)
            status, _, expired = frames.take(epoch, tick)
            if status not in ("frame", "missing"): raise RuntimeError("timestamp gate failure")
            data["presented" if status == "frame" else "missing"] += 1
            data["expired"] += expired
            tick += 1
        stop.set()
    thread = threading.Thread(target=consumer, daemon=False)
    thread.start()
    try:
        while not stop.is_set():
            now_tick = max(0, int((time.monotonic() - started) * fps))
            if index < now_tick:
                data["skipped_generation"] += now_tick - index
                index = now_tick
            for frame in _batch(generate_batch, index):
                while not stop.is_set():
                    status = frames.offer(epoch, index, frame)
                    if status == "full":
                        stop.wait(0.005)
                        continue
                    if status == "late": data["late_on_offer"] += 1
                    elif status == "accepted":
                        data["produced"] += 1
                        data["queue_peak"] = max(data["queue_peak"], frames.size())
                    else: raise RuntimeError("invalid frame offer: " + status)
                    index += 1
                    break
    finally:
        stop.set()
        thread.join(timeout=2)
        if thread.is_alive(): raise RuntimeError("consumer failed to stop")
        remaining = frames.revoke()
    data.update(elapsed_seconds=round(time.monotonic()-started, 3), remaining_revoked=remaining,
                max_lateness_ms=round(data["max_lateness_ms"], 3), capacity=capacity, fps=fps,
                clock="simulated wall clock, not audio device")
    return data


def run_external_clock(generate_batch, start_clock, fps=25, capacity=8, prefill=8,
                       sample_rate=24000, clip_samples=96000, on_present=None, before_go=None,
                       deadline_s=None, pause_at_samples=None, pause_hold_s=None):
    """Present frames from an external played-sample clock.

    `start_clock` returns an object with `open` already done, plus `go`,
    `latest`, `wait_update`, and `close`. Prefill runs before `go`, so the
    device is not opened by this function. Tick selection uses played samples.
    Submitted samples are recorded and never select a frame. A disallowed or
    stopped clock presents nothing further. Each output tick is counted in
    `tick // fps`, so a 25 FPS clip splits into seconds of 25 ticks. Seconds
    that were never reached stay at zero.
    """
    from musetalk_timestamp_queue import TimestampFrames, playback_tick
    if (not (1 <= fps <= 60 and 1 <= capacity <= 16 and 0 <= prefill <= capacity)
            or type(sample_rate) is not int or type(clip_samples) is not int
            or not 0 < clip_samples <= 480_000 or sample_rate <= 0):
        raise ValueError("probe bounds")
    clip_ticks = playback_tick(clip_samples, sample_rate, fps)
    if clip_ticks < 1 or prefill > clip_ticks:
        raise ValueError("probe bounds")
    if (pause_at_samples is None) != (pause_hold_s is None):
        raise ValueError("probe bounds")
    if pause_at_samples is not None and (type(pause_at_samples) is not int
                                          or isinstance(pause_hold_s, bool)
                                          or type(pause_hold_s) not in (int, float)
                                          or not 0 < pause_at_samples < clip_samples
                                          or not 0 < pause_hold_s <= 2):
        raise ValueError("probe bounds")
    second_count = (clip_ticks + fps - 1) // fps
    presented_by_second = [0] * second_count
    missing_by_second = [0] * second_count
    epoch = 1
    frames = TimestampFrames(epoch, capacity)
    stop = threading.Event()
    data = {"presented": 0, "missing": 0, "expired": 0, "late_on_offer": 0,
            "skipped_generation": 0, "produced": 0, "queue_peak": 0,
            "prefill_frames": 0, "prefill_seconds": 0.0, "presented_after_disallow": 0,
            "selection_violations": 0, "played_peak": 0, "submitted_peak": 0,
            "allowed_seen": False, "timed_out": False, "max_presented_tick": -1}
    index = 0
    prefill_started = time.monotonic()
    try:
        while index < prefill:
            for frame in _batch(generate_batch, index):
                if index >= prefill:
                    break
                status = frames.offer(epoch, index, frame)
                if status != "accepted":
                    raise RuntimeError("prefill rejected frame: " + status)
                data["queue_peak"] = max(data["queue_peak"], frames.size())
                index += 1
        data["prefill_frames"] = index
        data["prefill_seconds"] = round(time.monotonic() - prefill_started, 3)
        clock = start_clock()
        error = []
        started = time.monotonic()
        deadline = started + (deadline_s if deadline_s is not None else clip_samples / sample_rate + 15)

        def consume():
            next_tick = 0
            seen = False
            try:
                while next_tick < clip_ticks and not stop.is_set():
                    if time.monotonic() > deadline:
                        data["timed_out"] = True
                        break
                    played, submitted, allowed, stopped = clock.wait_update(0.05)
                    data["played_peak"] = max(data["played_peak"], played)
                    data["submitted_peak"] = max(data["submitted_peak"], submitted)
                    if stopped or (seen and not allowed):
                        break
                    if not allowed:
                        continue
                    seen = True
                    data["allowed_seen"] = True
                    current = playback_tick(played, sample_rate, fps)
                    while next_tick <= current and next_tick < clip_ticks:
                        _, _, still, stopped_now = clock.latest()
                        if stopped_now or not still:
                            break
                        if next_tick > playback_tick(played, sample_rate, fps):
                            data["selection_violations"] += 1
                            break
                        status, frame, expired = frames.take(epoch, next_tick)
                        if status not in ("frame", "missing"):
                            raise RuntimeError("timestamp gate failure")
                        bucket = next_tick // fps
                        if status == "frame":
                            data["presented"] += 1
                            presented_by_second[bucket] += 1
                        else:
                            data["missing"] += 1
                            missing_by_second[bucket] += 1
                        data["expired"] += expired
                        data["max_presented_tick"] = next_tick
                        if on_present is not None:
                            on_present(next_tick, frame if status == "frame" else None)
                        next_tick += 1
                    if played >= clip_samples or next_tick >= clip_ticks:
                        break
            except Exception as exc:
                error.append(exc)
            finally:
                stop.set()

        consumer = threading.Thread(target=consume, daemon=False)
        consumer.start()
        producer_error = None
        bridge = {}
        pause_done = False
        try:
            if before_go is not None:
                before_go()
            clock.go()
            allowed_seen = False
            while not stop.is_set():
                played, _, allowed, stopped = clock.latest()
                if (pause_at_samples is not None and not pause_done and allowed_seen
                        and played >= pause_at_samples):
                    record = clock.pause(pause_hold_s)
                    record["requested_samples"] = pause_at_samples
                    data["pause"] = record
                    pause_done = True
                if stopped or (allowed_seen and not allowed):
                    break
                if not allowed:
                    if stop.wait(0.01):
                        break
                    continue
                allowed_seen = True
                current = min(playback_tick(played, sample_rate, fps), clip_ticks)
                if index < current:
                    data["skipped_generation"] += current - index
                    index = current
                if index >= clip_ticks:
                    if stop.wait(0.01):
                        break
                    continue
                for frame in _batch(generate_batch, index):
                    while not stop.is_set():
                        status = frames.offer(epoch, index, frame)
                        if status == "full":
                            if stop.wait(0.005):
                                break
                            continue
                        if status == "late":
                            data["late_on_offer"] += 1
                        elif status == "accepted":
                            data["produced"] += 1
                            data["queue_peak"] = max(data["queue_peak"], frames.size())
                        elif status != "revoked":
                            raise RuntimeError("invalid frame offer: " + status)
                        index += 1
                        break
        except Exception as exc:
            producer_error = exc
        finally:
            stop.set()
            bridge = clock.close() or {}
            consumer.join(timeout=2)
            if consumer.is_alive():
                raise RuntimeError("consumer failed to stop")
        if producer_error is not None:
            raise producer_error
        if error:
            raise error[0]
        data["elapsed_seconds"] = round(time.monotonic() - started, 3)
        data["bridge_reason"] = bridge.get("reason", "")
        data["played_final"] = bridge.get("played", data["played_peak"])
        data["submitted_final"] = bridge.get("submitted", data["submitted_peak"])
        data["progress_after_stop"] = bool(bridge.get("progress_after_stop", False))
        data["stderr_tail"] = bridge.get("stderr_tail", "")
        data["presented_by_second"] = presented_by_second
        data["missing_by_second"] = missing_by_second
        data["ratio_by_second"] = [round(presented / (presented + missing), 4) if presented + missing else None
                                   for presented, missing in zip(presented_by_second, missing_by_second)]
    finally:
        data["remaining_revoked"] = frames.revoke()
    data.update(clip_samples=clip_samples, clip_ticks=clip_ticks, fps=fps, sample_rate=sample_rate,
                capacity=capacity,
                clock="external played samples; submitted is recorded and not used to select frames")
    return data
