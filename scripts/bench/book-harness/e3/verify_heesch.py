import json
import subprocess
import sys

run = subprocess.run([".venv-bench/bin/python", "-m", "heesch_verify", "submission/best.heesch"],
                     capture_output=True, text=True)
try:
    d = json.loads(run.stdout)
except ValueError:
    print("FAIL heesch_verify rejected submission/best.heesch:")
    print((run.stdout + run.stderr)[-1500:])
    sys.exit(1)
hc = d.get("hc_verified") or 0
print(f"grid {d.get('grid')} cells {d.get('cell_count')} hc_verified {hc} "
      f"hh_verified {d.get('hh_verified')} claim: {d.get('verified_claim')}")
evidence = d.get("non_tiler_evidence") or ""
print(f"non-tiler evidence: {evidence or 'none (outside the census; proof not checked here)'}")
if hc >= 5 and evidence:
    print("PASS: hc_verified >= 5 on a proven non-tiler")
    sys.exit(0)
print("FAIL: need hc_verified >= 5 on a proven non-tiler (the first survivor; the class record is 4)")
sys.exit(1)
