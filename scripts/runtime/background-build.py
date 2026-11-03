#!/usr/bin/env python3
"""One detached, low-priority source build per checkout; never owns a cockpit."""
import fcntl
import json
import os
from pathlib import Path
import subprocess
import sys
import time


def publish(target, **status):
    path = target / "background-build.json"
    temporary = path.with_suffix(f".{os.getpid()}.tmp")
    try:
        temporary.write_text(json.dumps({"schema": "angel.background-build/v1", **status}) + "\n")
        os.replace(temporary, path)
    finally:
        temporary.unlink(missing_ok=True)


def run(root, profile, lock_fd):
    target = root / "cockpit/target"
    log = target / "background-build.log"
    pending = target / ".background-build.pending"
    try:
        os.nice(10)
        if log.exists() and log.stat().st_size > 1024 * 1024:
            log.replace(log.with_suffix(".log.old"))
        while True:
            status = dict(profile=profile, pid=os.getpid(), started_at=time.time(), log=str(log))
            try:
                with log.open("a") as output:
                    publish(target, state="running", **status)
                    env = dict(os.environ, ANGEL_BACKGROUND_BUILD="0", ANGEL_NO_BUILD="0")
                    env.setdefault("CARGO_BUILD_JOBS", "2")
                    result = subprocess.run(
                        ["bash", str(root / "bin/angelX"), "build", "--profile", profile],
                        cwd=root, env=env, stdin=subprocess.DEVNULL, stdout=output,
                        stderr=subprocess.STDOUT, pass_fds=(lock_fd,), check=False,
                    )
                outcome = dict(state="succeeded" if result.returncode == 0 else "failed",
                               exit_code=result.returncode)
            except Exception as error:
                outcome = dict(state="failed", exit_code=1, error=str(error))
            # Serialize the idle transition with new requests. Release the lease
            # here, not after this mutex: otherwise a last request could be left
            # behind after we decided to exit. Never hold this mutex during Cargo.
            with (target / ".background-build.requests.lock").open("a") as requests:
                fcntl.flock(requests, fcntl.LOCK_EX)
                if pending.exists():
                    profile = pending.read_text().strip()
                    pending.unlink()
                    if profile in ("fast", "debug", "release"):
                        continue
                publish(target, finished_at=time.time(), **outcome, **status)
                os.close(lock_fd)
                lock_fd = None
                return
    finally:
        if lock_fd is not None:
            os.close(lock_fd)


def start(root, profile, queue=False):
    target = root / "cockpit/target"
    target.mkdir(parents=True, exist_ok=True)
    # Never unlink the lock: replacing its inode would allow two owners. Kernel
    # ownership survives launcher exit and is released even after a worker crash.
    with (target / ".background-build.requests.lock").open("a") as requests:
        fcntl.flock(requests, fcntl.LOCK_EX)
        pending = target / ".background-build.pending"
        with (target / ".background-build.lock").open("a") as lock:
            try:
                fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
            except BlockingIOError:
                if queue:
                    pending.write_text(profile + "\n")
                return
            pending.unlink(missing_ok=True)  # a new owner supersedes stale requests
            status = dict(profile=profile, requested_at=time.time(), log=str(target / "background-build.log"))
            publish(target, state="queued", **status)
            try:
                subprocess.Popen(
                    [sys.executable, str(Path(__file__).resolve()), "--run", str(root), profile, str(lock.fileno())],
                    cwd=root, stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL,
                    stderr=subprocess.DEVNULL, start_new_session=True, pass_fds=(lock.fileno(),),
                )
            except Exception as error:
                publish(target, state="failed", exit_code=1, error=str(error),
                        finished_at=time.time(), **status)
                raise
            # The detached worker inherits the same open file description and keeps
            # the lease; closing this copy does not unlock it. No waiting or pipes.


if __name__ == "__main__":
    try:
        if len(sys.argv) == 5 and sys.argv[1] == "--run":
            run(Path(sys.argv[2]), sys.argv[3], int(sys.argv[4]))
        elif len(sys.argv) == 3 or (len(sys.argv) == 4 and sys.argv[3] == "--queue"):
            if sys.argv[2] not in ("fast", "debug", "release"):
                raise ValueError("unknown build profile")
            start(Path(sys.argv[1]).resolve(), sys.argv[2], queue=len(sys.argv) == 4)
        else:
            raise ValueError("usage: background-build.py ROOT PROFILE [--queue]")
    except Exception as error:
        print(f"angelX: cannot start background build: {error}", file=sys.stderr)
        sys.exit(1)
