"""Checked host-clock phases between consecutive captured requests.

Tool time is the span from the first call to the last balanced completion,
as in the original diagnostics, rather than the sum of concurrent tool times.
Events identify tool names, not invocation IDs; no per-invocation claim is made.
Response latency is done minus arrival, including proxy/transport work and
completion logging after forwarding; it is not provider computation time.
"""
import collections
import importlib.util
import json
from pathlib import Path
import re
import statistics


spec = importlib.util.spec_from_file_location("phase_receipt", Path(__file__).with_name("receipt_usage.py"))
receipt = importlib.util.module_from_spec(spec)
spec.loader.exec_module(receipt)
EVENT = re.compile(r"^(\S+) \[task-event\] (call|done) ([\w-]+)(?=[:\s]|$)")
EVENT_START = re.compile(r"^(\S+) \[task-event\] (call|done)(?:\s|$)")
FIELDS = ("response_latency", "reply", "tool", "post", "harness")


def read_requests(path):
    rows = []
    with Path(path).open() as source:
        for number, line in enumerate(source, 1):
            if not line.strip():
                continue
            try:
                row = json.loads(line)
            except ValueError as error:
                raise ValueError(f"invalid capture JSON: {path}:{number}") from error
            if not isinstance(row, dict):
                raise ValueError(f"capture must be an object: {path}:{number}")
            rows.append(row)
    return rows


def read_events(path):
    events = []
    with Path(path).open(errors="replace") as source:
        for number, line in enumerate(source, 1):
            match = EVENT.match(line)
            malformed = False
            if not match:
                partial = EVENT_START.match(line)
                if partial:
                    timestamp, kind = partial.groups()
                    name, malformed = None, True
            if match:
                timestamp, kind, name = match.groups()
            if match or malformed:
                try:
                    timestamp = receipt.number(float(timestamp))
                except ValueError:
                    timestamp = None
                events.append({"t": timestamp, "kind": kind, "name": name, "line": number, "malformed": malformed})
    return events


def summaries(rows, fields=FIELDS):
    totals, subtotals, coverage = receipt.history.receipt_fields(
        {field: [row.get(field) for row in rows] for field in fields})
    for field in fields:
        estimated = sum(row.get(field) is not None and row.get("response_scope") == "instant-stub-assumption"
                        for row in rows) if field in ("reply", "harness") else 0
        coverage[field].update(observed=coverage[field]["reported"] - estimated, estimated=estimated)
    return {"totals": totals, "reported_subtotals": subtotals, "metric_coverage": coverage}


def analyze(requests, events, assume_instant_stub=False):
    events = [{**event, "t": receipt.number(event.get("t")),
               "malformed": bool(event.get("malformed")) or event.get("kind") not in ("call", "done")
               or not isinstance(event.get("name"), str) or not event.get("name")} for event in events]
    arrivals = [receipt.number(row.get("t")) for row in requests]
    global_issues = []
    if any(event["t"] is None for event in events):
        global_issues.append("missing/invalid event clock prevents event association")
    if any(event.get("malformed") for event in events):
        global_issues.append("incomplete task-event record prevents event association")
    if any(a["t"] is not None and b["t"] is not None and b["t"] < a["t"] for a, b in zip(events, events[1:])):
        global_issues.append("event clocks regress in recorded order")
    if any(arrival is None for arrival in arrivals):
        global_issues.append("missing/invalid request clock prevents event association")
    if any(start is not None and end is not None and end < start for start, end in zip(arrivals, arrivals[1:])):
        global_issues.append("request clocks regress in captured order")
    windows = []
    owners = collections.defaultdict(list)
    for index, (start, end) in enumerate(zip(arrivals, arrivals[1:])):
        selected = [number for number, event in enumerate(events)
                    if start is not None and end is not None and start <= end
                    and event["t"] is not None and start <= event["t"] <= end]
        windows.append(selected)
        for number in selected:
            owners[number].append(index)
    hops = []
    for index, (request, start, end) in enumerate(zip(requests, arrivals, arrivals[1:])):
        hop = {"request": index + 1, "next_request": index + 2, "tool_name": "unknown",
               "quarter": min(3, index * 4 // len(requests)), "response_scope": "unknown",
               **dict.fromkeys(FIELDS), "issues": []}
        complete = receipt.number(request.get("done"))
        if start is not None and complete is not None and complete >= start:
            hop.update(response_latency=complete - start, response_scope="recorded-response-completion")
        elif "done" not in request and assume_instant_stub and start is not None:
            complete = start
            hop["response_scope"] = "instant-stub-assumption"
        else:
            complete = None
            hop["issues"].append("missing/invalid response completion clock")
        if start is None or end is None or end < start:
            hop["issues"].append("missing/invalid or regressing request arrival clocks")
            hops.append(hop)
            continue
        selected = [events[number] for number in windows[index]]
        calls = [event for event in selected if event["kind"] == "call"]
        dones = [event for event in selected if event["kind"] == "done"]
        if calls:
            hop["tool_name"] = calls[0]["name"] or "unknown"
        ambiguous_boundary = any(len(owners[number]) > 1 for number in windows[index])
        if global_issues or ambiguous_boundary:
            hop["issues"].extend(global_issues)
            if ambiguous_boundary:
                hop["issues"].append("event lies on a shared request boundary")
            hops.append(hop)
            continue
        if calls and complete is not None and complete <= calls[0]["t"]:
            hop["reply"] = calls[0]["t"] - complete
        elif calls and complete is not None:
            hop["issues"].append("response completion is after the first tool call")
        pending = collections.Counter()
        association = True
        for event in selected:
            name = event["name"]
            if event["kind"] == "call":
                pending[name] += 1
            elif pending[name] > 0:
                pending[name] -= 1
            else:
                association = False
                hop["issues"].append("completion has no preceding call of the same tool")
        if any(pending.values()):
            association = False
            hop["issues"].append("tool calls lack matching completions")
        if not calls or not dones:
            association = False
            hop["issues"].append("no complete tool span in this request interval")
        if association:
            hop.update(tool=dones[-1]["t"] - calls[0]["t"], post=end - dones[-1]["t"])
            if hop["reply"] is not None:
                hop["harness"] = hop["reply"] + hop["post"]
        hops.append(hop)
    return {"requests": len(requests), "hops": hops,
            "scope": "between consecutive request arrivals; final request has no closing next arrival",
            "response_latency_scope": "recorded request/response interval, including proxy/transport work and forwarding completion logging",
            "tool_association": "tool-name balance in recorded event order; elapsed tool span, not invocation durations",
            "assume_instant_stub": assume_instant_stub, "clock_issues": global_issues, **summaries(hops)}


def load(label, out_dir=Path("out"), assume_instant_stub=False):
    if not label or label in (".", "..") or Path(label).name != label:
        raise ValueError("label must name a capture inside the output directory")
    return analyze(read_requests(Path(out_dir) / (label + ".jsonl")),
                   read_events(Path(out_dir) / (label + ".stderr")), assume_instant_stub)


def phase_groups(result):
    grouped = collections.defaultdict(list)
    for hop in result["hops"]:
        grouped[hop["tool_name"]].append(hop)
    out = []
    for name, hops in grouped.items():
        fields = collections.defaultdict(list)
        for hop in hops:
            for field in ("response_latency", "reply", "tool", "post"):
                fields[field].append((hop[field], hop))
            for field in ("reply", "post"):
                fields[f"{field}_q{hop['quarter']}"].append((hop[field], hop))
        medians, reported, coverage = {}, {}, {}
        for field, entries in fields.items():
            values = [value for value, _ in entries if value is not None]
            estimated = sum(value is not None and hop["response_scope"] == "instant-stub-assumption"
                            for value, hop in entries) if field.startswith("reply") else 0
            reported[field] = statistics.median(values) if values else None
            complete = bool(entries) and len(values) == len(entries)
            medians[field] = reported[field] if complete else None
            coverage[field] = {"reported": len(values), "attempts": len(entries), "complete": complete,
                               "observed": len(values) - estimated, "estimated": estimated}
        out.append({"tool": name, "hops": len(hops), "medians": medians,
                    "reported_medians": reported, "metric_coverage": coverage})
    return out
