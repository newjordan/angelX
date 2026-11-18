#!/usr/bin/env python3
import argparse
import ctypes
import errno
import os
import signal
import subprocess
import sys


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--timeout-ms', required=True, type=int)
    parser.add_argument('--status-fd', type=int)
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
    if sys.platform.startswith('linux'):
        parent = os.getppid()
        if ctypes.CDLL(None).prctl(1, signal.SIGTERM, 0, 0, 0) != 0:
            raise RuntimeError('cannot arm parent-death cleanup')
        if os.getppid() != parent:
            return 143
    try:
        pass_fds = []
        for fd in range(3, args.status_fd or 3):
            try:
                os.fstat(fd)
            except OSError as error:
                if error.errno != errno.EBADF:
                    raise
            else:
                pass_fds.append(fd)
        child = subprocess.Popen(command, start_new_session=True, pass_fds=pass_fds)
        try:
            returncode = child.wait(timeout=args.timeout_ms / 1000)
            return returncode if returncode >= 0 else 128 - returncode
        except subprocess.TimeoutExpired:
            if args.status_fd is not None:
                os.write(args.status_fd, b'deadline\n')
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
