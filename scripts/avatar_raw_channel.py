"""Loopback-only raw PCM reader, separate from signed protobuf control.

Application-key peer proof is not Windows PID/ACL authentication. Production
worker startup must explicitly bind this endpoint; worker6 does not use it.
"""
import hmac
import ipaddress
import socket
import struct
from v561 import avatar_stream_pb2 as wire

DOMAIN = b'nai.avatar.raw.v1\0'
SERVER_DOMAIN = b'nai.avatar.raw.server.v1\0'


def read_exact(channel, count, check):
    if not 0 <= count <= 76800:
        raise ValueError('RAW_SIZE')
    channel.settimeout(0.01)
    data = bytearray()
    while len(data) < count:
        check()
        try:
            part = channel.recv(count-len(data))
        except socket.timeout:
            continue
        if not part:
            raise EOFError('RAW_CLOSED')
        data.extend(part)
    check()
    return bytes(data)


def connect(address, port, key, check):
    if not ipaddress.ip_address(address).is_loopback or not 1 <= port <= 65535 or len(key) != 32:
        raise ValueError('RAW_ENDPOINT')
    check()
    channel = socket.create_connection((address, port), timeout=1)
    try:
        channel.setsockopt(socket.SOL_SOCKET, socket.SO_RCVBUF, 76800)
        challenge = read_exact(channel, 32, check)
        proof = hmac.digest(key, DOMAIN+challenge, 'sha256')
        channel.sendall(proof)
        confirmation = read_exact(channel, 32, check)
        if not hmac.compare_digest(confirmation, hmac.digest(key, SERVER_DOMAIN+challenge+proof, 'sha256')):
            raise ValueError('RAW_AUTH')
        check()
        return channel
    except BaseException:
        channel.close()
        raise


def receive(channel, expected, check):
    try:
        size, = struct.unpack('<I', read_exact(channel, 4, check))
        if not 1 <= size <= 256:
            raise ValueError('RAW_HEADER_SIZE')
        actual = wire.Chunk.FromString(read_exact(channel, size, check))
        if actual != expected or not 1 <= actual.samples <= 38400:
            raise ValueError('RAW_IDENTITY')
        return read_exact(channel, actual.samples*2, check)
    except BaseException:
        channel.close()
        raise
