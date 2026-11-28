"""Bounded Windows joint-run resource capture; does not retry the supplied run."""
import argparse
import ctypes
import json
from pathlib import Path
import subprocess
import sys
import threading
import time


class Memory(ctypes.Structure):
    _fields_=[('length',ctypes.c_ulong),('load',ctypes.c_ulong)]+[(name,ctypes.c_ulonglong) for name in
        ['total_phys','avail_phys','total_commit','avail_commit','total_virtual','avail_virtual','extended_virtual']]


def main():
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument('--run-cmd',type=Path,required=True);p.add_argument('--output-dir',type=Path,required=True)
    args=p.parse_args();out=args.output_dir
    if not out.is_absolute() or not args.run_cmd.is_absolute() or not args.run_cmd.is_file():raise ValueError('PATHS')
    out.mkdir(exist_ok=True,parents=True)
    stop=out/'telemetry.stop'
    if stop.exists() or (out/'gpu.jsonl').exists():raise ValueError('EXISTING_RUN')
    flags=getattr(subprocess,'CREATE_NO_WINDOW',0)
    log=(out/'sampler.log').open('xb')
    sampler=subprocess.Popen([sys.executable,'-B',str(Path(__file__).with_name('flashhead-gpu-telemetry.py')),
        '--output',str(out/'gpu.jsonl'),'--duration-seconds','300','--interval-seconds','1','--stop-file',str(stop)],
        stdout=log,stderr=log,creationflags=flags)
    finished=threading.Event()
    def host_memory():
        kernel=ctypes.WinDLL('kernel32',use_last_error=True)
        kernel.GlobalMemoryStatusEx.argtypes=[ctypes.POINTER(Memory)];kernel.GlobalMemoryStatusEx.restype=ctypes.c_int
        with (out/'memory.jsonl').open('x',encoding='utf-8',buffering=1) as file:
            while not finished.is_set():
                m=Memory();m.length=ctypes.sizeof(m)
                ok=bool(kernel.GlobalMemoryStatusEx(ctypes.byref(m)))
                file.write(json.dumps(dict(unix_ms=int(time.time()*1000),available=ok,physical_load_percent=m.load if ok else None,
                    total_physical_bytes=m.total_phys if ok else None,available_physical_bytes=m.avail_phys if ok else None,
                    commit_limit_bytes=m.total_commit if ok else None,available_commit_bytes=m.avail_commit if ok else None))+'\n')
                finished.wait(1)
    thread=threading.Thread(target=host_memory);thread.start()
    try:
        started=int(time.time()*1000)
        result=subprocess.run(['cmd.exe','/d','/c',str(args.run_cmd)],timeout=300,creationflags=flags)
        (out/'run.json').write_text(json.dumps(dict(started_unix_ms=started,finished_unix_ms=int(time.time()*1000),exit_code=result.returncode,retries=0),indent=2))
        if result.returncode:raise SystemExit(result.returncode)
    finally:
        finished.set();thread.join();stop.touch()
        try:sampler.wait(timeout=10)
        except subprocess.TimeoutExpired:sampler.kill();sampler.wait()
        log.close()


if __name__=='__main__':main()
