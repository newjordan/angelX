"""Run the benchmark task in a session owned by its recorded child PID."""

import os
import signal
import sys

try:
    os.setsid()
except PermissionError:
    # Bash with job control already gives this child its own process group.
    # Keeping that group preserves the recorded PID = PGID ownership.
    if os.getpgrp() != os.getpid():
        raise
# Background shell commands inherit an ignored SIGINT; restore task defaults.
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
        # This launcher leads the private task group, so the deadline stops
        # the command and inherited pipe holders together, including us.
        os.killpg(os.getpgrp(), signal.SIGKILL)
    raise SystemExit(status if status >= 0 else 128 - status)
os.execvp(sys.argv[1], sys.argv[1:])
