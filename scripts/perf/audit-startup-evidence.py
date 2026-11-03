#!/usr/bin/env python3
"""Reconcile retained startup research without building or launching the product.

This is a same-author archival verifier, NOT an application acceptance test.
Only explicitly named receipts are read. No benchmark, compiler, network, cache
cleanup, source swap or seal mutation. Output is exclusively created, never
replaced. Exit 0 means archival checks held; qualification can remain blocked.
"""
import argparse
import hashlib
import json
from pathlib import Path
import statistics
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[2]
PERF = ROOT / "reports/performance"
INPUTS = {}
CHECKS = []


def sha(path):
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def load(name):
    path = PERF / name
    INPUTS[str(path.relative_to(ROOT))] = sha(path)
    return json.loads(path.read_text())


def load_rows(name):
    path = PERF / name
    INPUTS[str(path.relative_to(ROOT))] = sha(path)
    return [json.loads(line) for line in path.read_text().splitlines() if line.strip()]


def check(label, condition, detail=None):
    CHECKS.append({"claim": label, "holds": bool(condition), "detail": detail})


def exact(a, b):
    return all(a[k] == b[k] for k in ("exit_code", "stdout", "stderr"))


def query_matrix(name, total, baseline, candidate):
    data = load(name)
    rows = data["results"]
    got = {"queries": len(rows), "baseline_exact": 0,
           "candidate_exact": 0, "baseline_false_success": 0}
    for row in rows:
        b = exact(row["rustc"], row["baseline"])
        c = exact(row["rustc"], row["candidate"])
        got["baseline_exact"] += b
        got["candidate_exact"] += c
        got["baseline_false_success"] += (row["rustc"]["exit_code"] != 0
                                           and row["baseline"]["exit_code"] == 0)
        check(name + ": row flags agree with primary streams",
              b == row["baseline_exact"] and c == row["candidate_exact"])
    check(name + ": headline totals", (len(rows), got["baseline_exact"],
                                      got["candidate_exact"]) == (total, baseline, candidate), got)
    return got


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", required=True, type=Path)
    args = parser.parse_args()
    output = args.output.resolve()
    if not output.is_relative_to(ROOT) or output.exists():
        parser.error("output must be a new workspace file")

    old = load("startup-evidence-audit-iteration15.json")
    archive_mismatches = []
    for name, expected in old["input_sha256"].items():
        path = ROOT / name
        current = sha(path) if path.is_file() else None
        check("iteration15 primary receipt unchanged: " + name, current == expected)
        if current != expected:
            archive_mismatches.append(name)
    historical_sources = {}
    for sources in old["historical_manifest_sources"].values():
        for name, entry in sources.items():
            historical_sources[name] = entry["sha256_at_audit"]
    for name, expected in historical_sources.items():
        path = ROOT / name
        current = sha(path) if path.is_file() else None
        check("earlier exhibit source unchanged since iteration15: " + name, current == expected)
        if current != expected:
            archive_mismatches.append(name)
    # Record drift separately: an old benchmark cannot qualify new source bytes.
    drift = []
    for group, values in old.items():
        if not isinstance(values, dict):
            continue
        for name, binding in values.items():
            if not isinstance(binding, dict) or "current" not in binding or "expected" not in binding:
                continue
            path = ROOT / name
            current = sha(path) if path.is_file() else None
            if current != binding["current"]:
                drift.append({"group": group, "path": name,
                              "historical_sha256": binding["current"], "current_sha256": current})

    stale = load("dev-rebuild-baseline-20261028.json")
    steps = {s["name"]: s for s in stale["steps"]}
    edit = json.loads(steps["semantic-edit"]["stdout"])
    restore = json.loads(steps["restore"]["stdout"])
    stale_facts = {"commands_succeeded": all(steps[n]["exit_code"] == 0 for n in ("semantic-edit", "restore")),
                   "source_restored": stale["source_restored"],
                   "same_image": edit["executable_sha256"] == restore["executable_sha256"],
                   "edit_marker_after_restore": "_dev_rebuild_probe" in restore["resources"],
                   "restored_image_verified": stale["restored_image_verified"]}
    check("full-host stale-image failure is retained, not cleared", all(stale_facts[k] for k in
          ("commands_succeeded", "source_restored", "same_image", "edit_marker_after_restore"))
          and not stale_facts["restored_image_verified"], stale_facts)

    integrity = {}
    # These are historical same-author counts, not independent validations.
    audit_specs = [
        (19, "blaze-copy-result-summary-iteration19.json", "assertions", 63),
        (20, "blaze-ready-evidence-audit-iteration20.json", "assertions", 789),
        (21, "blaze-script-wrap-evidence-audit-iteration21.json", "assertions", 76),
        (22, "blaze-dispatch-evidence-audit-r2-iteration22.json", "assertions", 153),
        (23, "blaze-child-env-evidence-audit-iteration23.json", "checks_count", 321),
        (26, "blaze-target-cache-iteration26/audit.json", "assertions", 139),
        (27, "blaze-file-input-iteration27/integrity.json", "held", 133),
        (28, "launcher-probe-iteration28/integrity.json", "assertions", 94),
        (29, "blaze-command-env-iteration29/integrity.json", "assertions", 178),
    ]
    for iteration, name, key, expected in audit_specs:
        data = load(name)
        integrity[str(iteration)] = {"receipt": name, "count": data[key], "independent": False}
        check("historical audit count: " + name, data[key] == expected)
        outcomes = []
        for entry in data.get("checks", []):
            if isinstance(entry, dict):
                outcomes.extend(entry[k] for k in ("passed", "observed", "held", "holds") if k in entry)
        check("historical audit outcomes: " + name,
              all(v is True for v in outcomes)
              and all(data[k] == 0 for k in ("failed", "exit_code", "audit_exit") if k in data)
              and all(data[k] == expected for k in ("held", "holding") if k in data))
    negative = load("blaze-copy-result-summary-iteration19.json")
    check("clone-first proposal failed preregistered promotion bar",
          negative["preregistered_threshold_met"] is False)

    queries = {
        "target26": query_matrix("blaze-target-cache-iteration26/differential.json", 14, 6, 14),
        "file27": query_matrix("blaze-file-input-iteration27/r2/differential.json", 58, 35, 58),
    }
    check("file27 false-success count", queries["file27"]["baseline_false_success"] == 23)
    env = load("blaze-command-env-iteration29/summary.json")
    env_rows = load_rows("blaze-command-env-iteration29/matrix-raw.jsonl")
    env_counts = {"rows": len(env_rows), "baseline_exact": 0, "candidate_exact": 0,
                  "baseline_wrong_success": 0, "false_success_exit": 0}
    for row in env_rows:
        b = all(row["rustc"][k] == row["baseline"][k] for k in ("exit", "stdout", "stderr"))
        c = all(row["rustc"][k] == row["candidate"][k] for k in ("exit", "stdout", "stderr"))
        env_counts["baseline_exact"] += b
        env_counts["candidate_exact"] += c
        env_counts["baseline_wrong_success"] += not b and row["baseline"]["exit"] == 0
        env_counts["false_success_exit"] += row["rustc"]["exit"] != 0 and row["baseline"]["exit"] == 0
        check("env29 row flags: " + row["label"], b == row["baseline_exact"] and c == row["candidate_exact"])
    check("env29 primary totals", env_counts == {"rows": 34, "baseline_exact": 25,
          "candidate_exact": 34, "baseline_wrong_success": 9, "false_success_exit": 1})
    check("env29 summary matches primary rows", all(env[k] == v for k, v in env_counts.items()))
    env_tests = load("blaze-command-env-iteration29/test-summary.json")
    check("env29 retained regression red and reduced candidate green",
          env_tests == {"baseline_regression_exit": 101, "candidate_exit": 0})

    pilot = load("launcher-probe-iteration28/timing-r2/timing.json")
    pilot_inputs = load("launcher-probe-iteration28/timing-r2/timing-inputs.json")
    check("launcher pilot uses the current applied candidate",
          pilot_inputs["launchers"]["candidate"] == sha(ROOT / "bin/angelX"))
    check("pilot private fixture inputs stable throughout measurement",
          pilot["unchanged_inputs_and_image"] == pilot["post_measurement_inputs_and_image"])
    check("pilot paths describe the retained private source fixture, not the host",
          all(hashlib.sha256(body.encode()).hexdigest() == pilot["unchanged_inputs_and_image"][name]
              for name, body in pilot_inputs["sources"].items()))
    warm = [r for r in pilot["records"] if r["phase"] == "warm"]
    primes = [r for r in pilot["records"] if r["phase"] == "prime"]
    pairs = {}
    for record in warm:
        pairs.setdefault(record["pair"], []).append(record)
    check("launcher pilot records complete and successful", len(primes) == 2 and len(warm) == 24
          and len(pilot["records"]) == 26 and all(r["exit"] == 0 for r in pilot["records"])
          and {r["arm"] for r in primes} == {"baseline", "candidate"}
          and set(pairs) == set(range(12))
          and all(len(rs) == 2 and {r["arm"] for r in rs} == {"baseline", "candidate"} for rs in pairs.values()))
    reductions = []
    for pair in sorted(pairs):
        arms = {r["arm"]: r["elapsed_ms"] for r in pairs[pair]}
        reductions.append(arms["baseline"] - arms["candidate"])
    medians = {arm: statistics.median(r["elapsed_ms"] for r in warm if r["arm"] == arm)
               for arm in ("baseline", "candidate")}
    check("launcher pilot reductions match primary pairs", reductions == pilot["paired_reductions_ms"])
    check("launcher pilot arm medians match records", medians == pilot["medians_ms"])
    paired = statistics.median(reductions)
    check("launcher pilot paired arithmetic", abs(paired - pilot["paired_median_reduction_ms"]) < 1e-9)
    pilot_scope = {"medians_ms": medians, "paired_median_reduction_ms": paired,
                   "pairs": len(pairs),
                   "scope": "tiny offline Cargo fixture; not current full-host startup"}

    lock_path = ROOT / "locks/locked.sha256"
    locked = []
    for line in lock_path.read_text().splitlines():
        if not line or line.startswith("#"):
            continue
        expected, name = line.split(maxsplit=1)
        name = name.lstrip("*")
        current = sha(ROOT / name)
        locked.append({"path": name, "expected": expected, "current": current, "matches": current == expected})
    mismatches = [x["path"] for x in locked if not x["matches"]]
    check("protected blocker remains exactly the diagnosed file",
          mismatches == ["cockpit/src/drive/loop_ctl.rs"], mismatches)
    check("protected manifest unchanged since env29",
          sha(lock_path) == "f6c890be2ce0c59b7999cde6fca9b495ef945da8e5b829e447cc4df487875c53")

    # A real source-bound verification of the mandatory adoption gate. Never --seal.
    gate = subprocess.run(["bash", "scripts/check/check-locks.sh"], cwd=ROOT,
                          capture_output=True, text=True, timeout=10)
    check("existing gate reports blocked (expected diagnostic, NOT a green product test)",
          gate.returncode == 1 and "loop_ctl.rs" in gate.stderr)
    launcher = (ROOT / "bin/angelX").read_text()
    check("launcher still warns against unqualified explicit Blaze adoption",
          "has not passed edit/restore image freshness" in launcher)
    upstream = ROOT / "cockpit/target/tools/blaze-src"
    revision = subprocess.run(["git", "-C", str(upstream), "rev-parse", "HEAD"],
                              capture_output=True, text=True, timeout=10)
    status = subprocess.run(["git", "-C", str(upstream), "status", "--porcelain"],
                            capture_output=True, text=True, timeout=10)
    check("pinned upstream remains unchanged", revision.returncode == status.returncode == 0
          and revision.stdout.strip() == "b383b0bcf5da33816809b00a37e1c36a0e761efb" and not status.stdout)
    current_launcher_sha = sha(ROOT / "bin/angelX")
    prior = load("blaze-command-env-iteration29/non-edit-after.json")
    check("applied launcher has not drifted since iteration29 preservation receipt",
          prior["bin/angelX"] == current_launcher_sha)
    for name, expected in prior.items():
        path = ROOT / name
        check("preserved product/tool input since iteration29: " + name,
              path.is_file() and sha(path) == expected)

    test_log = PERF / "startup-evidence-launcher-tests-iteration30.log"
    test_exit = PERF / "startup-evidence-launcher-tests-iteration30.exit"
    INPUTS[str(test_log.relative_to(ROOT))] = sha(test_log)
    INPUTS[str(test_exit.relative_to(ROOT))] = sha(test_exit)
    log = test_log.read_text()
    check("existing launcher test entrypoint retained six passing selected tests",
          test_exit.read_text().strip() == "0"
          and "tests 6" in log and "pass 6" in log and "fail 0" in log)

    failures = [c for c in CHECKS if not c["holds"]]
    result = {"schema": "angel.startup-evidence-checkpoint/v1", "status": "reported",
              "scope": "same-author archival verification, not product acceptance or new timing",
              "auditor_sha256": sha(Path(__file__)), "inputs_sha256": INPUTS,
              "checks": CHECKS, "checks_count": len(CHECKS), "failures": failures,
              "source_drift_since_iteration15": drift, "archive_mismatches": archive_mismatches,
              "historical_audit_counts": integrity, "query_counts_recomputed": queries,
              "env29_retained_summary": env, "env29_primary_counts": env_counts, "launcher_pilot": pilot_scope,
              "stale_full_host": stale_facts, "protected": locked,
              "gate": {"argv": ["bash", "scripts/check/check-locks.sh"], "exit_code": gate.returncode,
                       "stdout": gate.stdout, "stderr": gate.stderr},
              "qualification": "BLOCKED: operator seal mismatch and unresolved Blaze full-host freshness",
              "product_speedup_measured_this_iteration": False}
    with output.open("x") as stream:
        json.dump(result, stream, indent=2)
        stream.write("\n")
    print(f"Archival checks: {len(CHECKS)-len(failures)}/{len(CHECKS)}; product qualification: BLOCKED; gate exit {gate.returncode}")
    for failure in failures:
        print("FAIL: " + failure["claim"])
    return 1 if failures else 0


if __name__ == "__main__":
    sys.exit(main())
