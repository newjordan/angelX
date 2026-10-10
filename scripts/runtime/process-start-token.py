#!/usr/bin/env python3
"""Return the Darwin libproc birth token used by Rust process receipts."""
import ctypes
import sys


class ProcBsdInfo(ctypes.Structure):
    # Fixed-width layout matches libc::proc_bsdinfo in the locked libc crate.
    _fields_ = [
        ('pbi_flags', ctypes.c_uint32),
        ('pbi_status', ctypes.c_uint32),
        ('pbi_xstatus', ctypes.c_uint32),
        ('pbi_pid', ctypes.c_uint32),
        ('pbi_ppid', ctypes.c_uint32),
        ('pbi_uid', ctypes.c_uint32),
        ('pbi_gid', ctypes.c_uint32),
        ('pbi_ruid', ctypes.c_uint32),
        ('pbi_rgid', ctypes.c_uint32),
        ('pbi_svuid', ctypes.c_uint32),
        ('pbi_svgid', ctypes.c_uint32),
        ('rfu_1', ctypes.c_uint32),
        ('pbi_comm', ctypes.c_char * 16),
        ('pbi_name', ctypes.c_char * 32),
        ('pbi_nfiles', ctypes.c_uint32),
        ('pbi_pgid', ctypes.c_uint32),
        ('pbi_pjobc', ctypes.c_uint32),
        ('e_tdev', ctypes.c_uint32),
        ('e_tpgid', ctypes.c_uint32),
        ('pbi_nice', ctypes.c_int32),
        ('pbi_start_tvsec', ctypes.c_uint64),
        ('pbi_start_tvusec', ctypes.c_uint64),
    ]


def token_for(pid):
    if sys.platform != 'darwin' or pid <= 0 or pid > 0x7fffffff:
        return None
    info = ProcBsdInfo()
    try:
        libproc = ctypes.CDLL('/usr/lib/libproc.dylib', use_errno=True)
        proc_pidinfo = libproc.proc_pidinfo
        proc_pidinfo.argtypes = [
            ctypes.c_int, ctypes.c_int, ctypes.c_uint64, ctypes.c_void_p, ctypes.c_int
        ]
        proc_pidinfo.restype = ctypes.c_int
        written = proc_pidinfo(pid, 3, 0, ctypes.byref(info), ctypes.sizeof(info))
    except (AttributeError, OSError, OverflowError, TypeError, ValueError):
        return None
    if written != ctypes.sizeof(info) or info.pbi_pid != pid:
        return None
    if info.pbi_start_tvusec >= 1_000_000:
        return None
    return info.pbi_start_tvsec * 1_000_000 + info.pbi_start_tvusec


def main(argv):
    if len(argv) != 2 or not argv[1].isascii() or not argv[1].isdigit():
        return 2
    token = token_for(int(argv[1]))
    if token is None or token <= 0:
        return 1
    sys.stdout.write(f'{token}\n')
    return 0


if __name__ == '__main__':
    try:
        raise SystemExit(main(sys.argv))
    except (OSError, OverflowError, RecursionError, TypeError, ValueError):
        raise SystemExit(1)
