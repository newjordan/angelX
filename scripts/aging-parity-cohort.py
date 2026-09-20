#!/usr/bin/env python3
"""C03b paired held-out aging experiment, with explicit telemetry coverage.

Live routes are inherited from the environment, as in research-grounding-cohort.
No run deadline or hop cap is installed. Scripted mode tests plumbing, not quality.
Missing ledger bytes or full parked bodies are gaps, never estimated successes.
"""
from __future__ import annotations

import argparse
from collections import defaultdict
from contextlib import nullcontext
import hashlib
import importlib.util
import json
import math
import os
from pathlib import Path
import random
import shutil
import statistics
import stat
import subprocess
import sys
import time
import traceback

REPO = Path(__file__).resolve().parents[1]
BENCH = REPO / "benchmarks/action-agent/heldout_v1"
EVIDENCE = REPO / "docs/audits/evidence/2026-09-08-straight-a/C03b"
MARK = "[tool output elided"
# Cohort qualification threshold; product aging uses hop and token tails.
AGING_REQUEST_THRESHOLD = 300_000
LONG_SHARDS = 48
MANIFEST_PAGES = 20
MANIFEST_PAGE_LINES = 200
MANIFEST_LINE_BYTES = 96  # 200 * 97 ≈ 19 KiB including newlines (read_file 20 KiB pages)
BOUNDARY_MARK = "[tool-aging boundary:"
DEFAULT_CONTEXT_BUDGET_TOKENS = 80000
# Both arms preserve the provider prefix; aging runs at compaction boundaries.
# ANGEL_COMPACT_BG_PCT=0: the synchronous compact boundary is the one under test
# (background pass at 80% would race and is not the product path this cohort measures).
COMMON = {
    "ANGEL_CACHE_STABLE": "1", "ANGEL_TOOL_RESULT_DEDUP": "0",
    # Eager handle offload removes bulk before aging can see it. Isolate both arms.
    "ANGEL_HANDLE_STORE": "0",
    # Keep 1 hop so the first 40k-token compact can age earlier bulk (product keep=4
    # would skip aging when the small documented budget fires at hop 1–2).
    "ANGEL_TOOL_AGE_KEEP_HOPS": "0", "ANGEL_TOOL_AGE_PROTECT_TOKENS": "8000",
    "ANGEL_TOOL_AGE_MIN_BYTES": "512", "ANGEL_TOOL_AGE_EFFECTS": "0",
    "ANGEL_TOOL_AGE_EXCERPT_HOPS": "12", "ANGEL_PROJECT_DOC": "0",
    "ANGEL_TASK_RECON": "0", "ANGEL_SKILL_HINT": "0", "ANGEL_ADVISOR": "0",
    "ANGEL_TASK_STRICT_EXIT": "0", "ANGEL_TRAJECTORY_LOG": "1",
    "ANGEL_MAX_HOPS": "0", "ANGEL_YOLO": "1",
    "ANGEL_COMPACT_BG_PCT": "0",
    "ANGEL_CONTEXT_BUDGET_TOKENS": str(DEFAULT_CONTEXT_BUDGET_TOKENS),
}
METRICS = ("request_bytes", "response_bytes", "raw_input", "paid_input", "cached_input", "output", "total_tokens")
USAGE_FIELDS = ("input", "uncached_input", "cache_read", "cache_write", "output",
                "reasoning", "total_prompt", "generation_output")
_TOKEN_EFFICIENCY = []


def token_contracts():
    """token-efficiency.py owns the provider wire contracts; a row only cites them."""
    if not _TOKEN_EFFICIENCY:
        scripts = str(REPO / "scripts")
        if scripts not in sys.path:
            sys.path.insert(0, scripts)
        spec = importlib.util.spec_from_file_location("aging_token_efficiency",
                                                      REPO / "scripts/token-efficiency.py")
        module = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(module)
        _TOKEN_EFFICIENCY.append(module)
    return _TOKEN_EFFICIENCY[0]


def convention_value(attempts):
    """The ledger's convention when every hop agrees, else mixed:<a>,<b>; None when unreported."""
    counts = attempts if isinstance(attempts, dict) else {}
    active = sorted(name for name, count in counts.items()
                    if isinstance(count, int) and not isinstance(count, bool) and count > 0)
    if not active:
        return None
    return active[0] if len(active) == 1 else "mixed:" + ",".join(active)


def usage_record(record):
    """Every usage column one ledger record carries, with its conventions and hop coverage."""
    usage = record.get("usage") if isinstance(record.get("usage"), dict) else {}
    def value(field):
        raw = usage.get(field)
        return raw if isinstance(raw, (int, float)) and not isinstance(raw, bool) else None
    columns = {field: value(field) for field in USAGE_FIELDS}
    reported = usage.get("reported_attempts") if isinstance(usage.get("reported_attempts"), dict) else {}
    counts = [count for count in reported.values()
              if isinstance(count, int) and not isinstance(count, bool) and count >= 0]
    attempts = usage.get("attempts") if isinstance(usage.get("attempts"), int) else None
    if counts:
        hops_with_usage = max(counts)
    else:
        hops_with_usage = (attempts or 0) if any(v is not None for v in columns.values()) else 0
    hops = record.get("hops") if isinstance(record.get("hops"), int) else attempts
    return {**columns,
            "cache_convention": convention_value(usage.get("cache_convention_attempts")),
            "reasoning_convention": convention_value(usage.get("reasoning_convention_attempts")),
            "hops_with_usage": hops_with_usage, "hops_total": hops, "absent": {}}


def usage_summary(records, identity):
    """The run's summed usage record: columns the ledgers all report, conventions, identity.

    A column is summed only when every ledger record reports it; otherwise it stays null
    (never 0). Contract-absent columns are named, not silently zeroed.
    """
    per = [usage_record(record) for record in records if isinstance(record, dict)]
    columns = {}
    for field in USAGE_FIELDS:
        values = [entry[field] for entry in per]
        columns[field] = sum(values) if per and all(v is not None for v in values) else None
    conventions = {}
    for field in ("cache_convention", "reasoning_convention"):
        seen = sorted({entry[field] for entry in per if entry[field]})
        conventions[field] = seen[0] if len(seen) == 1 else ("mixed:" + ",".join(seen) if seen else None)
    hops_total = sum(entry["hops_total"] or 0 for entry in per)
    usage = {**columns, **conventions,
             "hops_with_usage": sum(entry["hops_with_usage"] for entry in per),
             "hops_total": hops_total or None, "absent": {}}
    model = (identity or {}).get("model")
    spec = token_contracts().PROVIDER_CONTRACTS
    provider = next((name for name, contract in spec.items()
                     if any(tag in (model or "").lower() for tag in contract["match"])), "other")
    usage["absent"] = {column: "not on the wire (" + reason + ")"
                       for column, reason in token_contracts().not_on_wire_for(provider).items()
                       if usage.get(column) is None}
    usage.update(model=model, provider=provider, route=(identity or {}).get("route"),
                 endpoint_kind=(identity or {}).get("endpoint_kind"))
    return usage


def stamp():
    return time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime())


def digest(data):
    return hashlib.sha256(data).hexdigest()


def write_json(path, value):
    path.write_text(json.dumps(value, indent=2, sort_keys=True) + "\n")


def plan(tasks, repetitions, seed):
    if repetitions < 3:
        raise ValueError("at least three repetitions required")
    rng = random.Random(seed)
    pairs = []
    for task in tasks:
        for repetition in range(repetitions):
            arms = ["on", "off"]
            rng.shuffle(arms)
            pairs.append({"pair": f"{task['name']}-{repetition}",
                          "task": task["name"], "language": task["language"], "group": task.get("group", "heldout"),
                          "repetition": repetition, "order": arms})
    return pairs


def wilson(k, n):
    if not n:
        return None
    z = 1.959963984540054
    p, den = k / n, 1 + z * z / n
    mid = (p + z * z / (2 * n)) / den
    half = z * math.sqrt(p * (1 - p) / n + z * z / (4 * n * n)) / den
    return [max(0, mid - half), min(1, mid + half)]


def sign_test(wins, losses):
    n = wins + losses
    return min(1.0, 2 * sum(math.comb(n, i) for i in range(min(wins, losses) + 1)) / 2**n) if n else 1.0


def bootstrap(clusters, seed=17, draws=4000):
    """Resample tasks, retaining repetitions within each task (cluster bootstrap)."""
    if not clusters:
        return {"mean": None, "ci95": None, "tasks": 0}
    values = list(clusters.values())
    mean = statistics.mean(x for group in values for x in group)
    if len(values) < 2:
        return {"mean": mean, "ci95": None, "tasks": len(values)}
    rng = random.Random(seed)
    samples = sorted(statistics.mean(x for group in rng.choices(values, k=len(values)) for x in group)
                     for _ in range(draws))
    return {"mean": mean, "ci95": [samples[int(draws * .025)], samples[int(draws * .975)]],
            "tasks": len(values)}


def recoverability(messages, originals=None):
    """Match each aged call to an original digest and a full *ledger* body.

    Scripted request observations can supply expected digests, but cannot count
    as recovery: full bytes must independently be in trajectory messages.
    """
    originals = originals or {}
    full = defaultdict(set)
    parked = {}
    for msg in messages:
        if msg.get("role") != "tool" or not isinstance(msg.get("content"), str):
            continue
        body = msg["content"]
        call = msg.get("tool_call_id")
        if MARK in body:
            parked[call] = body
        else:
            full[call].add(digest(body.encode()))
    checks = []
    for call, body in parked.items():
        expected = originals.get(call)
        # Without an independent digest, a single original ledger body sharing
        # the call id establishes identity. Reused/ambiguous ids fail closed.
        candidates = full.get(call, set())
        if expected is None and len(candidates) == 1:
            expected = next(iter(candidates))
        ok = expected is not None and expected in candidates
        checks.append({"call": call, "sha256": expected, "recovered": ok,
                       "receipt_sha256": digest(body.encode())})
    return {"parked": len(checks), "recovered": sum(c["recovered"] for c in checks),
            "fraction": sum(c["recovered"] for c in checks) / len(checks) if checks else None,
            "checks": checks, "scope": "aged receipts retained in trajectory; compacted-away receipts unknown"}


def resolve_evidence(ledger, sha):
    """Resolve a content digest without following links or trusting recorded sizes."""
    if not isinstance(sha, str) or len(sha) != 64 or any(c not in "0123456789abcdef" for c in sha):
        raise ValueError("invalid evidence digest")
    directory = Path(ledger) / "evidence"
    directory_fd = os.open(directory, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW)
    try:
        fd = os.open(sha, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK, dir_fd=directory_fd)
    finally:
        os.close(directory_fd)
    with os.fdopen(fd, "rb") as stream:
        if not stat.S_ISREG(os.fstat(stream.fileno()).st_mode):
            raise ValueError("evidence body is not a regular file")
        body = stream.read()
    if digest(body) != sha:
        raise ValueError("evidence digest mismatch")
    return body


def recover_parking_events(events, ledger):
    checks = []
    for event in events:
        sha = event.get("digest_sha256")
        try:
            body = resolve_evidence(ledger, sha)
            ok = len(body) == event.get("original_bytes")
            if "path" in event:
                ok = ok and event["path"] == f"evidence/{sha}"
        except (OSError, ValueError, TypeError):
            ok = False
        checks.append({"call": event.get("tool_call_id"), "sha256": sha,
                       "reason": event.get("reason"), "recovered": ok})
    recovered = sum(c["recovered"] for c in checks)
    return {"parked": len(checks), "recovered": recovered,
            "fraction": recovered / len(checks) if checks else None, "checks": checks,
            "scope": "all durable parking events, including compacted-away results"}


def extract(records, originals=None, ledger=None, parking_events=None, identity=None):
    messages, per_hop = [], []
    for record in records:
        messages.extend(record.get("messages", []))
        calls = (record.get("timing") or {}).get("calls") or {}
        for sample in calls.get("provider_calls", []):
            usage = sample.get("usage") or [None] * 5
            inp, out, _, cache, _ = usage
            # Only derive paid input when the ledger declares included caching.
            conventions = (record.get("usage") or {}).get("cache_convention_attempts", {})
            included = conventions.get("included", 0) > 0 and not any(
                value for key, value in conventions.items() if key != "included")
            paid = inp - cache if included and inp is not None and cache is not None and inp >= cache else None
            counters = sample.get("counters")
            if counters is not None:
                inp, out, cache, paid = (counters.get(k) for k in ("raw_input", "output", "cached_input", "paid_input"))
            status = sample.get("accounting_status", "unreported" if all(v is None for v in usage) else "legacy")
            per_hop.append({"hop": sample["model_call"], "attempt": sample["id"], "accounting_status": status,
                            "request_bytes": sample.get("request_bytes"),
                            "response_bytes": sample.get("response_bytes"), "raw_input": inp, "paid_input": paid,
                            "cached_input": cache, "output": out,
                            "total_tokens": counters.get("total_tokens") if counters is not None else
                                (inp + out if included and inp is not None and out is not None else None)})
    totals = {key: sum(p[key] for p in per_hop) if per_hop and all(p[key] is not None for p in per_hop) else None
              for key in METRICS}
    gaps = [f"ledger missing complete {key}" for key, value in totals.items() if value is None]
    events_known = parking_events is not None or all("parking_events" in r for r in records)
    if not events_known:
        gaps.append("ledger missing durable parking events")
    events = parking_events if parking_events is not None else [e for r in records for e in r.get("parking_events", [])]
    recovery = recover_parking_events(events, ledger) if events_known else recoverability(messages, originals)
    if recovery["recovered"] != recovery["parked"]:
        gaps.append("parked full bodies not recovered by digest from trajectory")
    events = events or []
    aging_events = [e for e in events if e.get("reason") == "aging"]
    aging_parked = len(aging_events)
    aged_bytes = sum(max(0, (e.get("original_bytes") or 0) - (e.get("retained_bytes") or 0))
                     for e in aging_events)
    boundaries = sum(1 for msg in messages if isinstance(msg.get("content"), str)
                     and BOUNDARY_MARK in msg["content"])
    return {"per_hop": per_hop, "totals": totals, "recoverability": recovery,
            "usage": usage_summary(records, identity),
            "retained_parked": recovery["recovered"],
            "aging_parked": aging_parked, "aged_bytes": aged_bytes,
            "boundaries_crossed": boundaries,
            "hops": sum(r.get("hops", 0) for r in records), "ledger_records": len(records),
            "tools_output": [r.get("tools_output") for r in records],
            "parking_events": events, "reached_size": reached_size(per_hop), "gaps": gaps}


def reduce_rows(rows, schedule):
    grouped = defaultdict(list)
    for row in rows:
        grouped[row["pair"]].append(row)
    complete, incomplete = [], []
    for pair in schedule:
        found = grouped.pop(pair["pair"], [])
        arms = {r["arm"]: r for r in found}
        if len(found) != 2 or set(arms) != {"on", "off"} or not all(r.get("completed") for r in found):
            incomplete.append(pair["pair"])
        else:
            complete.append(arms)

    def group_summary(pairs):
        wins = sum(p["on"]["solved"] and not p["off"]["solved"] for p in pairs)
        losses = sum(p["off"]["solved"] and not p["on"]["solved"] for p in pairs)
        arms = {}
        for arm in ("on", "off"):
            solved = sum(p[arm]["solved"] for p in pairs)
            arms[arm] = {"solved": solved, "n": len(pairs), "wilson95": wilson(solved, len(pairs))}
        differences = {}
        for metric in METRICS:
            clusters = defaultdict(list)
            for pair in pairs:
                a, b = (pair[arm]["totals"][metric] for arm in ("on", "off"))
                if a is not None and b is not None:
                    clusters[pair["on"]["task"]].append(a - b)
            differences[metric] = {**bootstrap(clusters), "pairs": sum(map(len, clusters.values()))}
        recovery = [p["on"]["recoverability"] for p in pairs]
        parked, recovered = (sum(r[k] for r in recovery) for k in ("parked", "recovered"))
        return {"pairs": len(pairs), "arms": arms, "on_only": wins, "off_only": losses,
                "sign_test_two_sided_p": sign_test(wins, losses), "differences_on_minus_off": differences,
                "recoverability": {"parked": parked, "recovered": recovered,
                                   "fraction": recovered / parked if parked else None}}
    return {"overall": group_summary(complete),
            "per_language": {lang: group_summary([p for p in complete if p["on"]["language"] == lang])
                             for lang in sorted({p["language"] for p in schedule})},
            "per_group": {group: group_summary([p for p in complete if p["on"].get("group", "heldout") == group])
                          for group in sorted({p.get("group", "heldout") for p in schedule})},
            "incomplete_pairs": incomplete, "unexpected_pairs": sorted(grouped),
            "reached_size_per_pair": [{"pair": p["on"]["pair"],
                **{arm: p[arm].get("reached_size") for arm in ("on", "off")}} for p in complete],
            "interpretation": "Sign-test nonsignificance does not establish equivalence. Wilson intervals are descriptive; repeated tasks are correlated. Bootstrap resamples tasks. Negative differences favor aging. Scripted solves are not quality evidence."}


def manifest_page_paths():
    return [f"MANIFEST/page_{page + 1:02d}.md" for page in range(MANIFEST_PAGES)]


def manifest_prompt_pages():
    return "; ".join(f"page {i + 1}: read_file path={path}"
                     for i, path in enumerate(manifest_page_paths()))


def write_manifest(workspace):
    directory = workspace / "MANIFEST"
    directory.mkdir(parents=True, exist_ok=True)
    target = 20 * 1024 - 64
    for page, path in enumerate(manifest_page_paths()):
        lines = []
        header = f"PAGE {page + 1}/{MANIFEST_PAGES} {path}\n"
        lines.append(header)
        while sum(len(x) for x in lines) < target:
            raw = f"P{page + 1:02d}L{len(lines):03d} " + "x" * MANIFEST_LINE_BYTES
            lines.append(raw[:MANIFEST_LINE_BYTES - 1] + "\n")
        text = "".join(lines)
        if len(text.encode()) > 20480:
            text = text.encode()[:20480].decode("utf-8", "ignore")
        (workspace / path).write_text(text)


def long_tasks():
    pages = manifest_prompt_pages()
    return [{"name": f"{lang}-long-shard-sums", "language": lang, "group": "long",
             "prompt": "Repair every shard's checksum: sum both numeric columns in all 48 source modules. "
                       "Do not edit or run commands until you have read MANIFEST.md in full. "
                       f"The file is 20 pages of ~19 KiB (read_file pages at ~20 KiB). Read these pages with read_file: {pages}. "
                       "Then read src/shard_0 through src/shard_47. "
                       "Their differing rows are the input. "
                       "Preserve all rows, the public API, and the checks. Run the local long_workload check.",
             "verify": "cargo test --offline --locked long_workload" if lang == "rust" else
                       "node --test checks/long.test.mjs",
             "protected_paths": ["tests/long.rs", "Cargo.toml", "Cargo.lock"] if lang == "rust" else
                                ["checks/long.test.mjs"]}
            for lang in ("rust", "js")]


def long_source(language, shard):
    rows = [(shard * 200 + row, row + 1) for row in range(150)]
    entries = "".join(f"    {'(' if language == 'rust' else '['}{a}, {b}{'),' if language == 'rust' else '],'} // row {i}: {'checksum input ' * 7}\n"
                      for i, (a, b) in enumerate(rows))
    if language == "rust":
        return "pub const ROWS: &[(u32, u32)] = &[\n" + entries + "];\npub fn checksum() -> u32 { ROWS.iter().map(|(a, _)| *a).sum() }\n"
    return "export const rows = [\n" + entries + "];\nexport function checksum() { return rows.reduce((sum, [a, b]) => sum + a, 0); }\n"


def materialize_long(task, workspace):
    language = task["language"]
    workspace.mkdir(parents=True, exist_ok=True)
    write_manifest(workspace)
    (workspace / "src").mkdir(parents=True, exist_ok=True)
    ext = "rs" if language == "rust" else "mjs"
    for shard in range(LONG_SHARDS):
        (workspace / "src" / f"shard_{shard}.{ext}").write_text(long_source(language, shard))
    if language == "rust":
        (workspace / "Cargo.toml").write_text('[package]\nname = "aging-long"\nversion = "0.1.0"\nedition = "2021"\n')
        (workspace / "Cargo.lock").write_text('version = 4\n\n[[package]]\nname = "aging-long"\nversion = "0.1.0"\n')
        (workspace / "src/lib.rs").write_text("".join(f"pub mod shard_{i};\n" for i in range(LONG_SHARDS)))
        (workspace / "tests").mkdir()
        checks = "".join(f"assert_eq!(aging_long::shard_{i}::checksum(), {sum(i * 200 + row + row + 1 for row in range(150))});\n"
                         f"assert_eq!(aging_long::shard_{i}::ROWS.len(), 150);\n"
                         f"for (r, pair) in aging_long::shard_{i}::ROWS.iter().enumerate() {{ assert_eq!(*pair, ({i} * 200 + r as u32, r as u32 + 1)); }}\n"
                         for i in range(LONG_SHARDS))
        (workspace / "tests/long.rs").write_text("#[test] fn long_workload() {\n" + checks + "}\n")
    else:
        (workspace / "checks").mkdir()
        (workspace / "checks/long.test.mjs").write_text(
            "import assert from 'node:assert/strict';\nimport test from 'node:test';\n" +
            "".join(f"import * as s{i} from '../src/shard_{i}.mjs';\n" for i in range(LONG_SHARDS)) +
            "test('long_workload', () => {\n" +
            "".join(f"assert.equal(s{i}.checksum(), {sum(i * 200 + row + row + 1 for row in range(150))});\n"
                    f"assert.deepEqual(s{i}.rows, Array.from({{length:150}}, (_,r) => [{i}*200+r,r+1]));\n"
                    for i in range(LONG_SHARDS)) + "});\n")


def reached_size(samples):
    early = [s.get("request_bytes") for s in samples if s["hop"] <= 5]
    known = [size for size in early if size is not None]
    peak = max(known, default=None)
    return {"through_hop": 6, "hop_index_base": 0,
            "peak_request_bytes": peak, "known_attempts": len(known),
            "unknown_attempts": len(early) - len(known),
            "reached": peak is not None and peak >= AGING_REQUEST_THRESHOLD}


def activation_gaps(row, long_workload=False):
    gaps = []
    samples = row.get("per_hop") or []
    threshold_crossed = any((s.get("request_bytes") or 0) >= AGING_REQUEST_THRESHOLD for s in samples)
    boundaries = row.get("boundaries_crossed") or 0
    aged_bytes = row.get("aged_bytes") or 0
    retained = row.get("retained_parked")
    if retained is None:
        retained = row.get("recoverability", {}).get("recovered", 0)
    aging_parked = row.get("aging_parked")
    if aging_parked is None:
        aging_parked = retained
    if row["arm"] == "on":
        failed = []
        if boundaries < 1:
            failed.append("boundaries")
        if retained < 1:
            failed.append("retained_parked")
        if aged_bytes <= 0:
            failed.append("aged_bytes")
        if failed:
            if threshold_crossed or long_workload:
                gaps.append(
                    f"AGING ACTIVATION FAILURE: {row.get('pair', 'attempt')} on failed {','.join(failed)}"
                    f" (boundaries={boundaries}, retained_parked={retained}, aged_bytes={aged_bytes})"
                )
            else:
                gaps.append("no retained parked receipts; aging activation unproven")
    if row["arm"] == "off" and aging_parked:
        gaps.append(f"OFF arm parked aging originals: {aging_parked}")
    size = reached_size(samples)
    if long_workload and not size["reached"]:
        if size["unknown_attempts"] or size["peak_request_bytes"] is None:
            gaps.append("LONG WORKLOAD UNKNOWN: incomplete request-byte coverage through hop 6")
        else:
            pass  # Measured shortfalls are attributed once per pair by long_pair_gaps.
    if long_workload:
        if row["arm"] == "on" and boundaries < 2:
            gaps.append(f"LONG BOUNDARY SHORT: boundaries={boundaries} by hop 6")
    return gaps


def cohort_gaps(rows, schedule):
    return sorted({f"{row['pair']} {row['arm']}: {gap}"
                   for row in rows for gap in row.get("gaps", [])}
                  | set(long_pair_gaps(rows, schedule)))


def long_pair_gaps(rows, schedule):
    """Only a strict majority of scheduled long pairs is a run-level failure."""
    gaps, short = [], 0
    pairs = [p for p in schedule if p.get("group") == "long"]
    for pair in pairs:
        arms = [r for r in rows if r["pair"] == pair["pair"]]
        sizes = [reached_size(r.get("per_hop", [])) for r in arms]
        # Incomplete/unknown pairs already have coverage gaps, not measured failures.
        if len(arms) != 2 or {r["arm"] for r in arms} != {"on", "off"}:
            continue
        if any(s["unknown_attempts"] or s["peak_request_bytes"] is None for s in sizes):
            continue
        if not all(s["reached"] for s in sizes):
            short += 1
            peak = min(s["peak_request_bytes"] for s in sizes)
            gaps.append(f"LONG WORKLOAD SHORT: {pair['pair']} peak {peak}")
    if short * 2 > len(pairs):
        gaps.append(f"LONG WORKLOAD FAILURE: {short}/{len(pairs)} pairs fell short")
    return gaps


def scripted_policy(hops, originals, language=None):
    pages = manifest_page_paths()
    def policy(hop, request):
        for msg in request.get("messages", []):
            if msg.get("role") == "tool" and MARK not in (msg.get("content") or ""):
                originals[msg.get("tool_call_id")] = digest((msg.get("content") or "").encode())
        if hop < hops:
            if language:
                # First three hops: 6+6+4 MANIFEST pages so hop 3 request bytes ≥ 300k.
                if hop < 3:
                    start = 0 if hop == 0 else (8 if hop == 1 else 16)
                    count = 8 if hop < 2 else 4
                    batch = pages[start:start + count]
                    return "calls", [
                        (f"manifest{hop}-{i}", "read_file", {"path": path})
                        for i, path in enumerate(batch)
                    ]
                ext = "rs" if language == "rust" else "mjs"
                src_hop = hop - 3
                return "calls", [(f"read{hop}-{i}", "read_file", {"path": f"src/shard_{(src_hop * 4 + i) % LONG_SHARDS}.{ext}"})
                                 for i in range(4)]
            return "calls", [(f"read{hop}", "read_file", {"path": f"aging-inputs/read-{hop}.txt"})]
        return "text", "Scripted aging dialogue complete; no repair attempted."
    return policy


def confined(path):
    path = Path(path).resolve()
    if not path.is_relative_to(REPO):
        raise ValueError("all outputs and scratch must be inside this worktree")
    return path


def run_arm(task, pair, arm, args, root, env_base, stress):
    run = root / (pair["pair"] + "-" + arm)
    run.mkdir()
    ws = run / "workspace"
    is_long = task.get("group") == "long"
    if is_long:
        materialize_long(task, ws)
    else:
        shutil.copytree(BENCH / task["fixture"], ws)
    if args.provider == "scripted" and not is_long:
        (ws / "aging-inputs").mkdir()
        for hop in range(args.scripted_hops):
            (ws / "aging-inputs" / f"read-{hop}.txt").write_text(
                "".join(f"sample {hop} row {line}: deterministic aging payload {'x' * 100}\n" for line in range(100)))
    before = {str(p.relative_to(ws)): digest(p.read_bytes()) for p in ws.rglob("*") if p.is_file()}
    home, tmp, ledger = (run / name for name in ("home", "tmp", "trajectory"))
    for directory in (home, tmp, ledger):
        directory.mkdir()
    env = {**env_base, **COMMON,
           "ANGEL_CONTEXT_BUDGET_TOKENS": str(getattr(args, "context_budget_tokens", DEFAULT_CONTEXT_BUDGET_TOKENS)),
           "HOME": str(home), "TMPDIR": str(tmp), "TMP": str(tmp), "TEMP": str(tmp),
           "CODEX_HOME": str(home / "codex"), "XDG_CACHE_HOME": str(home / "cache"),
           "XDG_CONFIG_HOME": str(home / "config"), "XDG_DATA_HOME": str(home / "data"),
           "ANGEL_SESSION_DIR": str(run / "sessions"), "ANGEL_TRAJECTORY_DIR": str(ledger),
           "ANGEL_TOOL_AGING": "1" if arm == "on" else "0", "CARGO_BUILD_JOBS": "2",
           "CARGO_NET_OFFLINE": "true", "RUSTUP_AUTO_INSTALL": "0"}
    prefix = "ANGEL_" + env.get("ANGEL_DRIVER", "openrouter").upper()
    env[prefix + "_CACHE_STABLE"] = "1"
    if not is_long:
        env["ANGEL_CONTEXT_BUDGET_TOKENS"] = "40000"
    # Identity the predeclared run manifest declares, carried onto every row it produces.
    identity = {"model": env.get(prefix + "_MODEL"),
                "route": "{}:{}".format(env.get("ANGEL_DRIVER"), env.get(prefix + "_MODEL")),
                "endpoint_kind": "scripted loopback" if stress else "live provider"}
    originals = {}
    server = stress.PolicyServer(scripted_policy(args.scripted_hops, originals, task["language"] if is_long else None)) if stress else nullcontext()
    cmd = [str(Path(args.angel_bin).resolve()), "--yolo", "--task-json", "--workspace", str(ws),
           "--max-hops", "0", "--deadline-secs", "0", "--tool-profile", "essential", "--rollout", "off", task["prompt"]]
    started = stamp()
    t0 = time.monotonic()
    with server as srv:
        if srv:
            env["ANGEL_OPENROUTER_URL"] = srv.url
        proc = subprocess.run(cmd, env=env, cwd=ws, capture_output=True, text=True)
        server_bytes = [entry["req_bytes"] for entry in srv.log] if srv else None
    wall = time.monotonic() - t0
    # Raw process artifacts stay in scratch, not receipts (can be large).
    (run / "stdout.txt").write_text(proc.stdout)
    (run / "stderr.txt").write_text(proc.stderr)
    try:
        envelope = json.loads(proc.stdout)
    except json.JSONDecodeError:
        envelope = {}
    records, paths = [], []
    for path in sorted(ledger.glob("*.jsonl")):
        paths.append({"path": str(path.relative_to(REPO)), "sha256": digest(path.read_bytes())})
        for line in path.read_text().splitlines():
            record = json.loads(line)
            if record.get("schema") == "angel-trajectory/v2":
                records.append(record)
    intact = all((ws / p).is_file() and digest((ws / p).read_bytes()) == before[p]
                 for p in task.get("protected_paths", []))
    # Verify in the copy only; don't use arena.materialize (it creates commits).
    verify = subprocess.run(task["verify"], shell=True, cwd=ws, env=env, capture_output=True, text=True)
    (run / "verify.txt").write_text(verify.stdout + verify.stderr)
    journal = ledger / "evidence" / "parking-events.jsonl"
    parking_events = [json.loads(line) for line in journal.read_text().splitlines()] if journal.is_file() else None
    row = {**pair, "arm": arm, "started_utc": started, "finished_utc": stamp(), "command": cmd,
           "fixture_sha256": digest(json.dumps(before, sort_keys=True).encode()), "wall_s": wall,
           "exit_code": proc.returncode, "completed": proc.returncode == 0 and bool(records),
           "status": envelope.get("status"), "stop_reason": envelope.get("stop_reason"),
           "scripted_dialogue_complete": envelope.get("status") == "completed" if stress else None,
           "solved": verify.returncode == 0 and intact, "verify_command": task["verify"],
           "verify_exit": verify.returncode, "protected_intact": intact,
           "ledger_files": paths, "scratch": str(run.relative_to(REPO)),
           "scripted_wire_request_bytes": server_bytes, **extract(records, originals, ledger, parking_events, identity)}
    row["request_bytes_source"] = "ledger"
    if stress and row["totals"]["request_bytes"] is None and server_bytes:
        row["totals"]["request_bytes"] = sum(server_bytes)
        row["request_bytes_source"] = "scripted_wire_fallback"
        row["gaps"].append("request bytes use scripted wire fallback; ledger coverage incomplete")
    if stress and not row["scripted_dialogue_complete"]:
        row["gaps"].append("scripted dialogue did not reach final answer")
    row["gaps"].extend(activation_gaps(row, is_long))
    if arm == "on" and row["recoverability"]["recovered"] < row["recoverability"]["parked"]:
        row["gaps"].append("parked full bodies not recovered by digest from trajectory")
    return row


def failed_arm(pair, arm, error):
    """An absent ledger is missing coverage, including failures before launch."""
    frames = traceback.extract_tb(error.__traceback__)
    operation = {"module": Path(frames[-1].filename).name, "function": frames[-1].name,
                 "line": frames[-1].lineno} if frames else None
    return {**pair, "arm": arm, "completed": False,
            "error_type": type(error).__name__, "error_errno": getattr(error, "errno", None),
            "error_operation": operation,
            "gaps": ["run failed before complete ledger: " + type(error).__name__]}


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--provider", choices=("scripted", "live"), default="scripted")
    parser.add_argument("--angel-bin", default=str(REPO / ".inputs/angel"))
    parser.add_argument("--out", default=str(EVIDENCE / "cohort"))
    parser.add_argument("--repetitions", type=int, default=3)
    parser.add_argument("--seed", type=int, default=20260908)
    parser.add_argument("--scripted-hops", type=int, default=12)
    parser.add_argument("--group", choices=("heldout", "long", "all"), default="heldout")
    parser.add_argument("--smoke", action="store_true", help="scripted first task only, still three pairs")
    parser.add_argument("--context-budget-tokens", type=int, default=DEFAULT_CONTEXT_BUDGET_TOKENS,
                        help="ANGEL_CONTEXT_BUDGET_TOKENS for both arms (default 80000)")
    args = parser.parse_args(argv)
    if args.smoke and args.provider != "scripted":
        parser.error("--smoke is scripted-only")
    if args.scripted_hops < 8:
        parser.error("scripted dialogue requires at least eight hops")
    tasks = json.loads((BENCH / "tasks.json").read_text()) if args.group != "long" else []
    if args.group in ("long", "all"):
        tasks += long_tasks()
    if args.smoke and args.group == "heldout":
        tasks = tasks[:1]
    schedule = plan(tasks, args.repetitions, args.seed)
    out = confined(args.out)
    out.mkdir(parents=True, exist_ok=False)
    root = REPO / ".astra-c03b" / (out.name + "-" + str(time.time_ns()))
    root.mkdir(parents=True)
    env = dict(os.environ)
    stress = None
    if args.provider == "scripted":
        # Allow only local process plumbing into the synthetic provider child.
        env = {k: v for k, v in env.items() if k in ("PATH", "LANG", "LC_ALL", "RUSTUP_HOME")}
        env.update(ANGEL_DRIVER="openrouter", ANGEL_OPENROUTER_KEY="offline-c03b",
                   OPENROUTER_API_KEY="offline-c03b", ANGEL_OPENROUTER_MODEL="offline-c03b",
                   ANGEL_API_CLUBS="openrouter")
        spec = importlib.util.spec_from_file_location("aging_stress", REPO / "scripts/harness-stress.py")
        stress = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(stress)
    prefix = "ANGEL_" + env.get("ANGEL_DRIVER", "openrouter").upper()
    route = {"driver": env.get("ANGEL_DRIVER"), "model": env.get(prefix + "_MODEL"),
             "effort": env.get(prefix + "_REASONING_EFFORT", env.get("ANGEL_REASONING_EFFORT", "default"))}
    if args.provider == "live" and (not route["driver"] or not route["model"]):
        parser.error("predeclare ANGEL_DRIVER and its ANGEL_<DRIVER>_MODEL in the environment")
    config = {"schema": "angel-aging-parity/v1", "started_utc": stamp(), "provider": args.provider,
              "argv": sys.argv if argv is None else argv, "route": route, "common_env": COMMON,
              "timeout": "unbounded (--deadline-secs 0; no subprocess timeout)", "seed": args.seed,
              "binary_sha256": digest(Path(args.angel_bin).read_bytes()), "schedule": schedule,
              "tasks": [{k: t[k] for k in ("name", "language", "prompt", "verify")} for t in tasks],
              "manifest_sha256": digest((BENCH / "tasks.json").read_bytes()),
              "scripted_hops": args.scripted_hops, "group": args.group,
              "aging_request_threshold_bytes": AGING_REQUEST_THRESHOLD,
              "generator_sha256": digest(Path(__file__).read_bytes()), "scratch": str(root.relative_to(REPO))}
    write_json(out / "predeclared.json", config)
    write_json(out / "schedule.json", schedule)
    rows = []
    lookup = {t["name"]: t for t in tasks}
    for pair in schedule:
        for arm in pair["order"]:
            try:
                row = run_arm(lookup[pair["task"]], pair, arm, args, root, env, stress)
            except (OSError, ValueError) as error:
                row = failed_arm(pair, arm, error)
            rows.append(row)
            write_json(out / "rows.json", rows)
            print(f"{pair['pair']} {arm}: completed={row['completed']}", flush=True)
    summary = reduce_rows(rows, schedule)
    summary["gaps"] = cohort_gaps(rows, schedule)
    summary["finished_utc"] = stamp()
    from receipt_provenance import binary_metadata
    from trace_schema import attach_receipt_header, receipt_header_markdown
    identity = binary_metadata(args.angel_bin)
    identity["dataset_sha256"] = config["manifest_sha256"]
    identity["dataset_id"] = "c03b/aging-parity:" + args.group
    attach_receipt_header(summary, metadata=identity,
        dataset={"id": identity["dataset_id"], "sha256": identity["dataset_sha256"]},
        evaluator_path=out / "predeclared.json",
        units={"*": None, "field:n": "count", "field:solved": "count",
               "field:pairs": "count", "field:incomplete_pairs": "count",
               "field:complete_pairs": "count",
               "field:p": "fraction", "field:lo": "fraction", "field:hi": "fraction",
               "reason": {"*": "Producer has not declared the semantics of this numeric field."}})
    deviations = [
        "Both arms ANGEL_CACHE_STABLE=1 (product default; aging deferred to compaction boundaries).",
        f"ANGEL_CONTEXT_BUDGET_TOKENS={args.context_budget_tokens} so the long group crosses >=2 "
        "synchronous compact boundaries by hop 6.",
        "ANGEL_COMPACT_BG_PCT=0: the synchronous boundary is the one under test; background "
        "compaction at the product 80% mark is disabled for this measurement.",
        "ANGEL_TOOL_AGE_KEEP_HOPS=1 so the first compact boundary can age bulk (product keep=4 "
        "would skip aging when the documented 80k budget fires at hop 1-2).",
        "ON=ANGEL_TOOL_AGING=1, OFF=ANGEL_TOOL_AGING=0; everything else identical.",
    ]
    summary["deviations"] = deviations
    config["deviations"] = deviations
    write_json(out / "predeclared.json", config)
    write_json(out / "summary.json", summary)
    (out / "summary.md").write_text("# C03b aging parity\n\n" + summary["interpretation"] +
                                    receipt_header_markdown(summary) +
                                    "\n\n```json\n" + json.dumps(summary, indent=2) + "\n```\n")
    print(f"cohort result: {summary['overall']['pairs']} complete pairs; {len(summary['incomplete_pairs'])} incomplete pairs; {len(summary['gaps'])} telemetry gaps")
    return 1 if summary["incomplete_pairs"] or summary["gaps"] else 0


if __name__ == "__main__":
    raise SystemExit(main())
