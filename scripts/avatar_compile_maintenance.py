"""Bounded explicit maintenance experiment, not a production Ready grant."""
import json
import os
import threading


class Maintenance:
    def __init__(self, status, seconds=240):
        if not 1 <= seconds <= 240:
            raise ValueError('MAINTENANCE_LIMIT')
        self.status, self.seconds = status, seconds
        self.timer = threading.Timer(seconds, self.expired)
        self.timer.daemon = True

    def write(self, state):
        self.status.write_text(json.dumps({'state': state, 'limit_seconds': self.seconds}), encoding='utf-8')

    def expired(self):
        try:
            self.write('maintenance_timeout')
        finally:
            os._exit(124)

    def __enter__(self):
        self.write('compiling_and_warming')
        self.timer.start()
        return self

    def __exit__(self, kind, *_):
        self.timer.cancel()
        self.timer.join()
        self.write('maintenance_failed' if kind else 'maintenance_ready_not_production')
