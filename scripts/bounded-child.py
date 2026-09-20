#!/usr/bin/env python3
"""Run one foreground process group with a wall deadline and parent cleanup."""
import argparse
import ctypes
import os
import signal
import subprocess
import sys


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--timeout-ms', required=True, type=int)
    parser.add_argument('command', nargs=argparse.REMAINDER)
    args = parser.parse_args()
    command = args.command[1:] if args.command[:1] == ['--'] else args.command
    if args.timeout_ms <= 0 or not command:
        parser.error('a positive timeout and command are required')
    child = None

    def stop(signum, _frame):
        raise SystemExit(128 + signum)

    for sig in (signal.SIGTERM, signal.SIGINT, signal.SIGHUP):
        signal.signal(sig, stop)
    # Linux is the qualified runtime. Parent loss must also stop nested model
    # clients/tool processes; killing only the immediate shell leaves them alive.
    if sys.platform.startswith('linux'):
        parent = os.getppid()
        if ctypes.CDLL(None).prctl(1, signal.SIGTERM, 0, 0, 0) != 0:
            raise RuntimeError('cannot arm parent-death cleanup')
        if os.getppid() != parent:
            return 143
    try:
        child = subprocess.Popen(command, start_new_session=True)
        try:
            returncode = child.wait(timeout=args.timeout_ms / 1000)
            return returncode if returncode >= 0 else 128 - returncode
        except subprocess.TimeoutExpired:
            print('bounded-child: deadline exceeded', file=sys.stderr)
            return 124
    finally:
        if child is not None:
            try:
                os.killpg(child.pid, signal.SIGKILL)
            except ProcessLookupError:
                pass
            child.wait()


if __name__ == '__main__':
    sys.exit(main())
