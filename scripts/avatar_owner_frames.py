"""Three shared slots, signed offer via owner callback, exact release before reuse."""
import hashlib
import queue
import secrets
from multiprocessing import shared_memory
from v561.media_pb2 import FrameRef, PIXEL_FORMAT_BGRA8

class OwnerFrames:
    def __init__(self, width, height, job, emit):
        self.width, self.height, self.job, self.emit = width, height, job, emit
        self.slot_bytes=width*height*4
        self.pool=secrets.token_hex(16)
        self.memory=shared_memory.SharedMemory(create=True,size=self.slot_bytes*3)
        self.sent=0
        self.generations=[0]*3

    def send_rgb(self, image):
        import numpy as np
        self.job.check()
        if image.dtype != np.uint8 or image.shape != (self.height,self.width,3):
            raise ValueError('FRAME_GEOMETRY')
        slot=self.sent%3; self.generations[slot]+=1
        offset=slot*self.slot_bytes
        target=np.ndarray((self.height,self.width,4),dtype=np.uint8,buffer=self.memory.buf,offset=offset)
        target[:,:,0],target[:,:,1],target[:,:,2],target[:,:,3]=image[:,:,2],image[:,:,1],image[:,:,0],255
        del target
        view=self.memory.buf[offset:offset+self.slot_bytes]
        try: digest=hashlib.sha256(view).digest()
        finally: view.release()
        frame=FrameRef(pool_id=self.pool,epoch=self.job.request.epoch,sequence=self.sent,source_sample=self.sent*960,source_sample_rate=24000,pts_ns=self.sent*40000000,width=self.width,height=self.height,stride_bytes=self.width*4,pixel_format=PIXEL_FORMAT_BGRA8,slot_index=slot,slot_generation=self.generations[slot],payload_bytes=self.slot_bytes,content_sha256=digest)
        self.job.check();self.emit(self.job.request,self.memory.name,frame)
        while True:
            self.job.check()
            try: release=self.job.releases.get(timeout=0.01)
            except queue.Empty: continue
            if (release.pool_id,release.epoch,release.sequence,release.slot_index,release.slot_generation)!=(frame.pool_id,frame.epoch,frame.sequence,frame.slot_index,frame.slot_generation):
                raise ValueError('FRAME_RELEASE')
            self.job.check();self.sent+=1;return

    def finish(self):
        self.job.check()
        return {'frames_acked':self.sent}

    def abort(self):
        self.memory.close();self.memory.unlink()
