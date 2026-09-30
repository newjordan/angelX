"""Split each hop into: reply→call (reply handling), call→done (tool), done→next
request (post-tool work and request building), from the stub log and the
timestamped task events."""
import json, sys, re, statistics as st
from collections import defaultdict
label=sys.argv[1]
reqs=[json.loads(l)["t"] for l in open(f"out/{label}.jsonl")]
ev=[]
for line in open(f"out/{label}.stderr", errors="replace"):
    m=re.match(r"(\d+\.\d+) \[task-event\] (call|done) (\w+)", line)
    if m: ev.append((float(m.group(1)), m.group(2), m.group(3)))
import bisect
ph=defaultdict(lambda: defaultdict(list))
for i,t0 in enumerate(reqs[:-1]):
    t3=reqs[i+1]
    inside=[e for e in ev if t0<=e[0]<=t3]
    calls=[e for e in inside if e[1]=="call"]; dones=[e for e in inside if e[1]=="done"]
    if not calls or not dones: continue
    name=calls[0][2]; q=min(3, i*4//len(reqs))
    ph[name]["reply"].append((calls[0][0]-t0)*1000)
    ph[name]["tool"].append((dones[-1][0]-calls[0][0])*1000)
    ph[name]["post"].append((t3-dones[-1][0])*1000)
    ph[name][f"post_q{q}"].append((t3-dones[-1][0])*1000)
    ph[name][f"reply_q{q}"].append((calls[0][0]-t0)*1000)
for name,d in ph.items():
    print(name, " ".join(f"{k}={st.median(v):.1f}" for k,v in sorted(d.items())))
