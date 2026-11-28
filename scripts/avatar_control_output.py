"""Keep signed control on a non-inherited fd; native/compiler stdout is log-only."""
import os
import sys


def private_control_output():
    sys.stdout.flush()
    control = os.fdopen(os.dup(sys.stdout.fileno()), 'wb')
    os.set_inheritable(control.fileno(), False)
    try:
        os.dup2(sys.stderr.fileno(), sys.stdout.fileno())
        if os.name == 'nt':
            import ctypes
            from ctypes import wintypes
            import msvcrt
            kernel = ctypes.WinDLL('kernel32', use_last_error=True)
            kernel.SetStdHandle.argtypes = [wintypes.DWORD, wintypes.HANDLE]
            kernel.SetStdHandle.restype = wintypes.BOOL
            if not kernel.SetStdHandle(0xfffffff5, msvcrt.get_osfhandle(sys.stdout.fileno())):
                raise ctypes.WinError(ctypes.get_last_error())
        return control
    except BaseException:
        control.close()
        raise
