import sys
from v561 import live_session_pb2 as wire
r=wire.Request.FromString(sys.stdin.buffer.read())
assert r.version==1 and r.sequence==9007199254740993 and r.command in (wire.COMMAND_START,wire.COMMAND_PREPARE_CONTENT,wire.COMMAND_PLAY_PREPARED,wire.COMMAND_ASK) and r.text=='你好。'
e=wire.Event(version=1,sequence=r.sequence,request_sequence=r.sequence,kind=wire.KIND_CONTENT_PREPARED if r.command==wire.COMMAND_PREPARE_CONTENT else wire.KIND_COMPLETED,
             visual_prepared=True,tts_prepared=True,content_prepared=True,worker_pid=11,samples=24000,played_samples=24000,epoch=3)
sys.stdout.buffer.write(e.SerializeToString())

