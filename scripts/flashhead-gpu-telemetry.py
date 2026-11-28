"""Bounded, line-flushed nvidia-smi sampler for local diagnostics."""
import argparse
import csv
import json
import math
import os
import subprocess
import time
from pathlib import Path


def bounded_number(lower, upper):
    def parse(value):
        number = float(value)
        if not math.isfinite(number) or not lower <= number <= upper:
            raise argparse.ArgumentTypeError(f'value must be between {lower} and {upper}')
        return number
    return parse


def number_or_none(value):
    try:
        number = float(value.strip())
        return number if math.isfinite(number) else None
    except ValueError:
        return None


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--duration-seconds', type=bounded_number(1, 600), required=True)
    parser.add_argument('--interval-seconds', type=bounded_number(0.5, 30), default=1)
    parser.add_argument('--nvidia-smi', default='nvidia-smi')
    parser.add_argument('--stop-file', type=Path, help='optional new sentinel for graceful early completion')
    args = parser.parse_args()
    if args.stop_file and args.stop_file.exists():
        parser.error('stop file already exists')
    fields = ['timestamp', 'power.draw', 'power.limit', 'clocks.sm', 'utilization.gpu', 'temperature.gpu', 'memory.used']
    keys = ['power_w', 'power_limit_w', 'sm_clock_mhz', 'gpu_util_percent', 'temperature_c', 'memory_used_mib']
    start = time.monotonic()
    successes = failures = incomplete = 0
    missing_fields = set()
    with args.output.open('x', encoding='utf-8', buffering=1) as report:
        def write(event, **values):
            report.write(json.dumps({'event':event, 'unix_ms':int(time.time()*1000), 'elapsed_s':round(time.monotonic()-start,3), **values}, ensure_ascii=False)+'\n')
        write('start', duration_s=args.duration_seconds, interval_s=args.interval_seconds)
        while time.monotonic()-start < args.duration_seconds and not (args.stop_file and args.stop_file.exists()):
            tick = time.monotonic()
            try:
                result = subprocess.run([args.nvidia_smi, '--query-gpu='+','.join(fields), '--format=csv,noheader,nounits'], stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True, timeout=5, check=True, creationflags=getattr(subprocess,'CREATE_NO_WINDOW',0))
                rows = list(csv.reader(result.stdout.splitlines()))
                if len(rows) != 1 or len(rows[0]) != len(fields):
                    raise ValueError('expected exactly one GPU and seven fields')
                values = dict(zip(keys, (number_or_none(v) for v in rows[0][1:])))
                if all(value is None for value in values.values()):
                    raise ValueError('GPU sample contains no numeric fields')
                missing = [key for key, value in values.items() if value is None]
                if missing:
                    incomplete += 1
                    missing_fields.update(missing)
                write('sample', gpu_timestamp=rows[0][0].strip(), **values)
                successes += 1
            except (OSError, subprocess.SubprocessError, ValueError) as exc:
                failures += 1
                write('sample_error', error_type=type(exc).__name__, message=str(exc)[:300])
            remaining = min(args.interval_seconds-(time.monotonic()-tick), args.duration_seconds-(time.monotonic()-start))
            if remaining > 0:
                time.sleep(remaining)
        status = 'ok' if successes and not failures and not incomplete else 'partial' if successes else 'failed'
        write('complete', status=status, samples=successes, errors=failures, incomplete_samples=incomplete, missing_fields=sorted(missing_fields), stop_reason='sentinel' if args.stop_file and args.stop_file.exists() else 'duration')
    return 0 if status == 'ok' else 1


if __name__ == '__main__':
    raise SystemExit(main())
