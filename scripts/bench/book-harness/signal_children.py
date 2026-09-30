"""Stop a Bash driver's recorded direct children through stable Linux handles."""

import argparse
import math
import os
from pathlib import Path
import select
import signal
import time


def stop_children(parent, children, grace=12):
    handles = {}
    try:
        for pid in children:
            if pid == os.getpid():
                continue  # A recycled job PID must not stop this helper itself.
            try:
                descriptor = os.pidfd_open(pid)
            except ProcessLookupError:
                continue
            try:
                # Open the stable handle first. A recycled numeric PID can
                # never redirect its signal; only this driver's direct jobs
                # qualify for cancellation, including newly exited zombies.
                fields = Path(f"/proc/{pid}/stat").read_text().rsplit(")", 1)[1].split()
                if int(fields[1]) != parent:
                    continue
                handles[descriptor] = pid
                descriptor = None
            except FileNotFoundError:
                pass
            finally:
                if descriptor is not None:
                    os.close(descriptor)
        polling = select.poll()
        for descriptor in handles:
            polling.register(descriptor, select.POLLIN)
            try:
                signal.pidfd_send_signal(descriptor, signal.SIGTERM)
            except ProcessLookupError:
                pass
        pending = set(handles)
        deadline = time.monotonic() + grace
        while pending:
            remaining = deadline - time.monotonic()
            if remaining <= 0:
                break
            for descriptor, _ in polling.poll(max(1, min(100, int(remaining * 1000)))):
                pending.discard(descriptor)
                polling.unregister(descriptor)
        for descriptor in pending:
            try:
                signal.pidfd_send_signal(descriptor, signal.SIGKILL)
            except ProcessLookupError:
                pass
    finally:
        for descriptor in handles:
            os.close(descriptor)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("parent", type=int)
    parser.add_argument("children", type=int, nargs="+")
    parser.add_argument("--grace-secs", type=float, default=12)
    args = parser.parse_args()
    if args.parent <= 0 or any(pid <= 0 for pid in args.children):
        parser.error("process identities must be positive")
    if not math.isfinite(args.grace_secs) or args.grace_secs <= 0:
        parser.error("grace must be finite and positive")
    stop_children(args.parent, args.children, args.grace_secs)


if __name__ == "__main__":
    main()
