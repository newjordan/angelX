"""Total harness time of a run: every hop's reply handling plus post-tool work,
excluding the tool's own execution (and the model, which is instant)."""
import json, sys, re
def total(label):
    reqs=[json.loads(l)["t"] for l in open(f"out/{label}.jsonl")]
    ev=[]
    for line in open(f"out/{label}.stderr", errors="replace"):
        m=re.match(r"(\d+\.\d+) \[task-event\] (call|done) (\w+)", line)
        if m: ev.append((float(m.group(1)), m.group(2)))
    harness=tool=0.0
    for t0,t3 in zip(reqs, reqs[1:]):
        inside=[e for e in ev if t0<=e[0]<=t3]
        calls=[e[0] for e in inside if e[1]=="call"]; dones=[e[0] for e in inside if e[1]=="done"]
        if not calls or not dones: continue
        tool+=dones[-1]-calls[0]; harness+=(t3-t0)-(dones[-1]-calls[0])
    return harness, tool
for label in sys.argv[1:]:
    h,t=total(label); print(f"{label:12s} harness={h:6.2f}s tools={t:6.2f}s")
