#!/usr/bin/env python3
"""answer.py <run-dir> <reply-json-file>: answer the oldest pending request,
wait for the next one (or the run's exit), and refresh <run-dir>.pending.txt."""
import glob
import json
import os
import subprocess
import sys
import time

run, reply_file = sys.argv[1], sys.argv[2]
reply = json.load(open(reply_file))
reqs = sorted(glob.glob(os.path.join(run, "*.req.json")), key=lambda p: int(os.path.basename(p).split(".")[0]))
pending = [p for p in reqs if not os.path.exists(p.replace(".req.json", ".resp.json"))]
if not pending:
    sys.exit(f"{run}: nothing pending")
n = int(os.path.basename(pending[0]).split(".")[0])
with open(pending[0].replace(".req.json", ".resp.json"), "w") as f:
    json.dump(reply, f, ensure_ascii=False)
here = os.path.dirname(os.path.abspath(__file__))
for _ in range(600):
    time.sleep(0.5)
    out = subprocess.run([sys.executable, os.path.join(here, "pending.py"), run], capture_output=True, text=True).stdout
    first = out.split("\n", 1)[0]
    if first != "WAIT" and first != f"REQUEST {n}":
        open(run + ".pending.txt", "w").write(out)
        print(f"{os.path.basename(run)}: {first}")
        break
else:
    print(f"{os.path.basename(run)}: still waiting")
