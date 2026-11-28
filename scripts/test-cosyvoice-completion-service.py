"""ASGI completion tests with tiny CPU-only PCM generators."""
import asyncio
import hashlib
import sys
import unittest
from fastapi import FastAPI
import httpx
import cosyvoice_request_gate as gate_module
from cosyvoice_completion_service import install
sys.modules['nai_request_gate'] = gate_module


class Tests(unittest.TestCase):
    def test_success_and_partial_failure_receipts(self):
        for fails in (False, True):
            app = FastAPI()
            response = install(app, gate_module.RequestGate(2), lambda data: data, lambda: 24000)
            def data():
                yield b'\1\0\2\0'
                if fails:
                    raise RuntimeError('private failure')
            @app.post('/inference_zero_shot')
            async def post():
                return response(data(), True)
            async def check():
                async with httpx.AsyncClient(transport=httpx.ASGITransport(app=app, raise_app_exceptions=False), base_url='http://test') as client:
                    audio = await client.post('/inference_zero_shot')
                    key = audio.headers['x-nai-request']
                    result = (await client.get('/nai/tts/status/' + key)).json()
                    self.assertEqual(result['instance_id'], audio.headers['x-nai-instance'])
                    self.assertEqual(result['state'], 'failed' if fails else 'completed')
                    if not fails:
                        self.assertEqual(result['samples'], 2)
                        self.assertEqual(result['sha256'], hashlib.sha256(audio.content).hexdigest())
                    self.assertEqual((await client.get('/nai/tts/status/' + '0' * 32)).status_code, 404)
            asyncio.run(check())


if __name__ == '__main__':
    unittest.main(verbosity=2)
