import os
import signal
import sys

try:
    os.setsid()
except PermissionError:
    if os.getpgrp() != os.getpid():
        raise
signal.signal(signal.SIGINT, signal.SIG_DFL)
signal.signal(signal.SIGTERM, signal.SIG_DFL)
if sys.argv[1] == "--timeout":
    import math
    import subprocess

    seconds = float(sys.argv[2])
    if not math.isfinite(seconds) or seconds <= 0:
        raise SystemExit("timeout must be a finite positive number")
    child = subprocess.Popen(sys.argv[3:])
    try:
        status = child.wait(timeout=seconds)
    except subprocess.TimeoutExpired:
        os.killpg(os.getpgrp(), signal.SIGKILL)
    raise SystemExit(status if status >= 0 else 128 - status)
os.execvp(sys.argv[1], sys.argv[1:])
