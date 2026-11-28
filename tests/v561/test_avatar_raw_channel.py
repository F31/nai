import sys
import socket
import struct
import threading
import time
import unittest
from pathlib import Path
ROOT=Path(__file__).resolve().parents[2]
sys.path[:0]=[str(ROOT/'scripts'),str(ROOT/'packages/contracts-python/src/generated')]
from avatar_raw_channel import receive, connect
from v561 import avatar_stream_pb2 as wire


class RawTests(unittest.TestCase):
    def pair(self):
        listener=socket.socket(); listener.bind(('127.0.0.1',0)); listener.listen(1)
        self.addCleanup(listener.close)
        client=socket.create_connection(listener.getsockname()); server,_=listener.accept()
        listener.close()
        self.addCleanup(client.close); self.addCleanup(server.close)
        return client,server

    def test_bad_header_or_mismatching_metadata_closes_channel(self):
        for invalid_size in [0,257]:
            sender,receiver=self.pair(); sender.sendall(struct.pack('<I',invalid_size))
            with self.assertRaises(ValueError): receive(receiver,wire.Chunk(),lambda:None)
            self.assertEqual(receiver.fileno(),-1)
        sender,receiver=self.pair()
        chunk=wire.Chunk(raw_payload_id=2,samples=1)
        data=chunk.SerializeToString(); sender.sendall(struct.pack('<I',len(data))+data)
        with self.assertRaises(ValueError): receive(receiver,wire.Chunk(raw_payload_id=1,samples=1),lambda:None)

    def test_partial_pcm_cancel_is_bounded_and_closes_socket(self):
        sender,receiver=self.pair(); cancelled=threading.Event()
        chunk=wire.Chunk(raw_payload_id=1,samples=2); data=chunk.SerializeToString()
        sender.sendall(struct.pack('<I',len(data))+data+b'\x01')
        timer=threading.Timer(0.03,cancelled.set); timer.start()
        self.addCleanup(timer.join)
        def check():
            if cancelled.is_set(): raise RuntimeError('CANCELLED')
        start=time.monotonic()
        with self.assertRaises(RuntimeError): receive(receiver,chunk,check)
        self.assertLess(time.monotonic()-start,0.5)
        self.assertEqual(receiver.fileno(),-1)

    def test_eof_is_failure_and_external_address_is_rejected(self):
        sender,receiver=self.pair(); sender.close()
        with self.assertRaises(EOFError): receive(receiver,wire.Chunk(),lambda:None)
        for address in ['192.0.2.1','0.0.0.0']:
            with self.assertRaises(ValueError): connect(address,10000,bytes(32),lambda:None)

    def test_wrong_server_proof_is_rejected(self):
        listener=socket.socket(); self.addCleanup(listener.close)
        listener.bind(('127.0.0.1',0)); listener.listen(1)
        port=listener.getsockname()[1]
        def server():
            channel,_=listener.accept()
            with channel:
                channel.settimeout(1)
                channel.sendall(bytes(32))
                proof=b''
                while len(proof)<32: proof+=channel.recv(32-len(proof))
                channel.sendall(bytes(32))
        thread=threading.Thread(target=server); thread.start()
        try:
            with self.assertRaisesRegex(ValueError,'RAW_AUTH'):
                connect('127.0.0.1',port,bytes([7])*32,lambda:None)
        finally: thread.join(timeout=2)
        self.assertFalse(thread.is_alive())


if __name__=='__main__':unittest.main()
