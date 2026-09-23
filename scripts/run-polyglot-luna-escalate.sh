#!/usr/bin/env bash
# Retest only the polyglot tasks Luna-medium missed, escalating effort
# high -> xhigh -> max. A task drops out once it scores 1.0.
set -euo pipefail

RR=/home/frosty40/angel_tests/angelX-bench/polyglot-20260921
WS=/home/frosty40/angelX
SRC=$RR/tasks-polyglot-v1.json
RUNNER=$WS/scripts/run-polyglot-luna-medium.sh
TABLE=$WS/scripts/polyglot_luna_table.py
LOG=$RR/logs/luna-escalate.log
mkdir -p "$RR/logs" "$RR/configs"

python3 - "$SRC" "$RR/tasks-luna-misses.json" <<'PY'
import json, sys
src, dest = sys.argv[1:]
want = [
    "rust-decimal",
    "js-food-chain",
    "rust-forth",
    "rust-scale-generator",
    "py-scale-generator",
]
rows = json.loads(open(src).read())
picked = [row for row in rows if row.get("name") in want]
missing = [name for name in want if name not in {row.get("name") for row in picked}]
if missing:
    raise SystemExit(f"missing tasks: {missing}")
json.dump(picked, open(dest, "w"))
print(f"wrote {len(picked)} tasks to {dest}")
PY

remaining="$RR/tasks-luna-misses.json"
for effort in high xhigh max; do
  count=$(python3 -c 'import json,sys; print(len(json.load(open(sys.argv[1]))))' "$remaining")
  if [[ "$count" == 0 ]]; then
    echo "nothing left before $effort"
    break
  fi
  echo "===== effort=$effort n=$count ====="
  set +e
  N_TASKS="$count" \
    TASKS_FILE="$remaining" \
    LUNA_EFFORT="$effort" \
    OUT_ROOT="escalate-$effort" \
    "$RUNNER"
  status=$?
  set -e
  run=$(cat "$RR/logs/luna-angelx-current-run.txt")
  echo "effort=$effort exit=$status run=$run"
  python3 "$TABLE" "$run" || true
  next="$RR/tasks-luna-misses-${effort}-left.json"
  python3 - "$remaining" "$run" "$next" <<'PY'
import json, sys
from pathlib import Path
manifest, run, dest = sys.argv[1:]
solved = set()
path = Path(run) / "traces.jsonl"
if path.is_file():
    for line in path.read_text().splitlines():
        if not line.strip():
            continue
        rec = json.loads(line)
        name = ((rec.get("task") or {}).get("data") or {}).get("name")
        for trace in rec.get("traces") or []:
            score = ((trace.get("rewards") or {}).get("technical_outcome") or {}).get("score")
            if score == 1:
                solved.add(name)
rows = json.loads(open(manifest).read())
left = [row for row in rows if row.get("name") not in solved]
json.dump(left, open(dest, "w"))
print(f"solved={sorted(solved)} left={[row.get('name') for row in left]}")
PY
  remaining="$next"
done

echo "===== escalation done ====="
python3 - "$RR" <<'PY'
import json
from pathlib import Path
root = Path("/home/frosty40/angel_tests/angelX-bench/polyglot-20260921")
print("final remaining files:")
for p in sorted(root.glob("tasks-luna-misses-*-left.json")):
    rows = json.loads(p.read_text())
    print(p.name, [r.get("name") for r in rows])
PY
