"""Local-only validator and missing-field census for the single trace contract.

Displayed scores (V04 score-identity scope)
-------------------------------------------
A number is a *displayed score* when a human-facing document shows it as a
result: (a) the ground-truth scorecard — `docs/audits/2026-09-07-ground-truth-scorecard.md`
and `docs/audits/evidence/2026-09-07-ground-truth/scorecard.json` (its
`evidence_paths` links and grade-rationale fields such as `audit_evidence`,
`change`, `a_gate`) — or (b) any `summary.md` / `*.md` table under
`docs/audits/evidence/2026-09-08-straight-a/**`, or (c) a JSON reducer output
that an (a)/(b) document links to. Raw rows (`rows.jsonl`, `timeline.json`,
`*-records-*`, telemetry, calibration inputs, command receipts) are *inputs*,
never displayed scores, and are audited only via the receipt that cites them.
"""
import json
import math
from collections import Counter
from pathlib import Path
from functools import lru_cache

# Documents whose visible numbers the score-identity auditor must grade.
DISPLAYED_SCORE_SOURCES = (
    'docs/audits/2026-09-07-ground-truth-scorecard.md',
    'docs/audits/evidence/2026-09-07-ground-truth/scorecard.json',
    'docs/audits/evidence/2026-09-08-straight-a/**/*.md',
)

# Raw record families that are reducer inputs, not displayed scores.
DISPLAYED_SCORE_INPUT_FAMILIES = ('rows.jsonl', 'timeline.json', '*-records-*',
                                  '*-telemetry-*', '*-calibration-input*',
                                  '*-command-receipt*')

WORKSPACE_START_FIELDS = ("head_tree", "start_tree_sha256", "start_dirty_paths_sha256")

SCHEMA_PATH = Path(__file__).resolve().parents[2] / 'docs/telemetry/trace-schema-v1.json'


@lru_cache(maxsize=1)
def schema():
    return json.loads(SCHEMA_PATH.read_text())


@lru_cache(maxsize=1)
def validator():
    # No RefResolver or remote references: the entire schema is a local document.
    try:
        from jsonschema import Draft202012Validator
        return Draft202012Validator(schema())
    except ImportError:
        return None


def validation_errors(record):
    reasons = set()
    v = validator()
    if v is not None:
        for error in v.iter_errors(record):
            path = '.'.join(map(str, error.absolute_path))
            if error.validator == 'required' and isinstance(error.instance, dict):
                for key in error.validator_value:
                    if key not in error.instance:
                        reasons.add((path + '.' if path else '') + key + ':required')
            else:
                reasons.add(path + ':' + error.validator)
    else:
        s = schema()
        for key in s.get("required", []):
            if key not in record:
                reasons.add(key + ':required')
    reasons.update(timing_errors(record))
    reasons.update(tools_output_errors(record))
    return sorted(reasons)


def timing_errors(record):
    """Additional arithmetic checks for lifecycle-aware v1 records only."""
    timing = record.get('timing')
    if not isinstance(timing, dict) or 'tool_overhead_ms' not in timing:
        return []
    errors = []
    integer = lambda v: type(v) is int and v >= 0
    tools = record.get('tools')
    if isinstance(tools, list) and all(isinstance(t, dict) and integer(t.get('ms')) for t in tools):
        if integer(timing.get('tool_ms')) and integer(timing.get('tool_overhead_ms')):
            if timing['tool_ms'] != sum(t['ms'] for t in tools) + timing['tool_overhead_ms']:
                errors.append('timing.tool_overhead_ms:arithmetic')
    calls = timing.get('calls')
    samples = calls.get('model_calls') if isinstance(calls, dict) else None
    if not isinstance(samples, list) or len(samples) != timing.get('model_calls'):
        errors.append('timing.calls.model_calls:cardinality')
    if isinstance(samples, list) and all(isinstance(c, dict) and integer(c.get('ms')) for c in samples):
        backoff = calls.get('retry_backoff_ms', 0)
        if integer(backoff) and integer(timing.get('model_ms')):
            if sum(c['ms'] for c in samples) + backoff != timing['model_ms']:
                errors.append('timing.calls.model_calls:arithmetic')
    components = [timing.get(k) for k in ('startup_ms', 'shutdown_ms', 'model_ms', 'tool_ms', 'other_ms')]
    if all(integer(v) for v in components) and integer(timing.get('wall_ms')):
        if sum(components) != timing['wall_ms']:
            errors.append('timing.wall_ms:arithmetic')
    return errors



def field_coverage(records):
    spec = schema()
    fields = {}
    def register(node, path=''):
        for name, child in node.get('properties', {}).items():
            key = path + name
            fields[key] = {'present': 0, 'explicit_null': 0, 'missing': 0}
            register(child, key + '.')
            if 'items' in child:
                register(child['items'], key + '[].')
    register(spec)
    missing = object()
    def visit(node, value, path=''):
        for name, child in node.get('properties', {}).items():
            key = path + name
            item = value.get(name, missing) if isinstance(value, dict) else missing
            counter = fields[key]
            counter['missing' if item is missing else 'explicit_null' if item is None else 'present'] += 1
            visit(child, item, key + '.')
            if 'items' in child and isinstance(item, list):
                for entry in item:
                    visit(child['items'], entry, key + '[].')
    total = valid = 0
    cost_counts = Counter()
    plan_fees = set()
    reasons = Counter()
    for record in records:
        total += 1
        visit(spec, record)
        cost = record.get('cost')
        cost = cost if isinstance(cost, dict) else {}
        paid = cost.get('paid')
        numeric_paid = type(paid) in (int, float) and math.isfinite(paid) and paid >= 0
        basis = cost.get('basis')
        resolved = numeric_paid and (basis == 'list-price' or (
            basis == 'subscription-marginal' and paid == 0 and cost.get('currency') == 'USD'))
        cost_counts['resolved' if resolved else 'unresolved'] += 1
        if basis == 'subscription-marginal':
            cost_counts['subscription_marginal'] += 1
            fee = cost.get('plan_fee_usd_month')
            if type(fee) in (int, float) and math.isfinite(fee) and fee >= 0:
                cost_counts['plan_fee_reported'] += 1
                plan_fees.add(fee)
        errors = validation_errors(record)
        valid += not errors
        reasons.update(errors)
    start_coverage = {key: fields['workspace_state.' + key] for key in WORKSPACE_START_FIELDS}
    cost_report = {key: cost_counts[key] for key in ('resolved', 'unresolved', 'subscription_marginal')}
    cost_report['plan_fee'] = {
        'reported_records': cost_counts['plan_fee_reported'],
        'monthly_fees_usd': sorted(plan_fees),
        'amortised_paid_usd': None,
        'reason': 'Monthly fees are not per-record charges; complete billing-month account usage and shared-plan identity are not captured here.',
    }
    return {'cost': cost_report, 'workspace_start_fields': start_coverage, 'schema': 'angel.trace-coverage/v1', 'records': total, 'fields': fields,
            'validation': {'valid': valid, 'invalid': total - valid, 'by_reason': dict(reasons)},
            'verdict': 'PASS: all records schema-valid' if total and total == valid else
                       'INCOMPLETE: empty store' if not total else 'FAIL: schema-invalid records present'}


def tools_output_errors(record):
    """Conservation for the turn-local ingress inventory (UTF-8 content bytes)."""
    output = record.get('tools_output')
    if not isinstance(output, dict):
        return []
    values = [output.get(k) for k in ('produced_bytes', 'retained_bytes', 'aged_bytes', 'dropped_bytes')]
    if all(type(v) is int and v >= 0 for v in values) and values[0] != sum(values[1:]):
        return ['tools_output:arithmetic']
    return []


# Additive receipt contract; independent of the per-turn trace validator.
RECEIPT_FIELDS = {
    "dataset": ("id", "sha256"),
    "candidate": ("binary_sha256", "source_digest", "head"),
    "evaluator": ("path", "sha256"),
}


def receipt_header_errors(header):
    """Return structural errors. Explicit unknowns are valid, not qualified."""
    import re

    if not isinstance(header, dict):
        return ["receipt_header:object"]
    errors = []
    for section, fields in {**RECEIPT_FIELDS, "units": ()}.items():
        obj = header.get(section)
        if not isinstance(obj, dict):
            errors.append(section + ":object")
            continue
        keys = fields if section != "units" else tuple(k for k in obj if k != "reason")
        if section == "units" and not keys:
            errors.append("units:empty")
        reasons = obj.get("reason", {})
        for key in keys:
            path = section + "." + key
            if key not in obj:
                errors.append(path + ":required")
                continue
            value = obj[key]
            if value is None:
                if not isinstance(reasons, dict) or not isinstance(reasons.get(key), str) or not reasons[key].strip():
                    errors.append(path + ":null_requires_reason")
            elif not isinstance(value, str) or not value.strip() or value.lower() in {"unknown", "unbound", "mixed", "none", "null"}:
                errors.append(path + ":nonempty_string")
            elif key in {"sha256", "binary_sha256", "source_digest"} and section != "units":
                if not re.fullmatch(r"[0-9a-fA-F]{64}", value):
                    errors.append(path + ":sha256")
            elif section == "candidate" and key == "head" and not re.fullmatch(r"[0-9a-fA-F]{40}|[0-9a-fA-F]{64}", value):
                errors.append(path + ":git_object_id")
    return errors


def make_receipt_header(*, dataset=None, candidate=None, evaluator=None, units=None):
    """Build from explicit observations only; no checkout/model/path guessing."""
    header = {}
    for name, keys in RECEIPT_FIELDS.items():
        supplied = {"dataset": dataset, "candidate": candidate, "evaluator": evaluator}[name] or {}
        section = {key: supplied.get(key) for key in keys}
        section["reason"] = {key: supplied.get("reason", {}).get(key, "Not captured, invalid, or conflicting in this score scope; no identity inferred.")
                             for key in keys if section[key] is None}
        header[name] = section
    header["units"] = units or {"*": None, "reason": {"*": "No declared unit for this metric; requires producer-specific semantics."}}
    errors = receipt_header_errors(header)
    if errors:
        raise ValueError("receipt header: " + "; ".join(errors))
    return header


def validate_receipt_header(header):
    """Validate the receipt contract, raising on malformed headers."""
    errors = receipt_header_errors(header)
    if errors:
        raise ValueError('receipt header: ' + '; '.join(errors))
    return header


def receipt_file_identity(path, root=None):
    """Hash a local evidence file, refusing traversal and symlink components."""
    import hashlib

    root = Path(root or Path(__file__).absolute().parents[2]).absolute()
    path = Path(path)
    if not path.is_absolute():
        path = root / path
    try:
        parts = path.relative_to(root).parts
    except ValueError:
        return {}
    if ".." in parts or "off-limits" in parts:
        return {}
    cursor = root
    for part in parts:
        cursor /= part
        if cursor.is_symlink():
            return {}
    if not cursor.is_file():
        return {}
    return {"path": str(cursor.relative_to(root)), "sha256": hashlib.sha256(cursor.read_bytes()).hexdigest()}


def attach_receipt_header(document, *, metadata=None, dataset=None, candidate=None,
                          evaluator_path=None, units=None, root=None):
    """Attach an additive header in place and return the document.

    Only enclosing, explicitly named identity metadata is adapted. Collections
    are never collapsed into a single identity. The evaluator is an existing
    evidence file, never this reducer's source or a newly written summary.
    """
    import re

    if "receipt_header" in document:
        errors = receipt_header_errors(document["receipt_header"])
        if errors:
            raise ValueError("receipt header: " + "; ".join(errors))
        return document
    meta = metadata if metadata is not None else document
    scopes = [meta]
    for key in ("identity", "candidate_identity", "binary", "meta", "build_info"):
        if isinstance(meta.get(key), dict):
            scopes.append(meta[key])
            if isinstance(meta[key].get("build_info"), dict):
                scopes.append(meta[key]["build_info"])

    def observed(keys, pattern=None):
        values = {obj[k] for obj in scopes for k in keys
                  if isinstance(obj.get(k), str) and obj[k].strip()
                  and obj[k].lower() not in {"unknown", "unbound", "mixed", "none", "null", "unreported"}
                  and (pattern is None or re.fullmatch(pattern, obj[k]))}
        return next(iter(values)) if len(values) == 1 else None

    digest = r"[0-9a-fA-F]{64}"
    if candidate is None:
        candidate = {
            "binary_sha256": observed(("binary_sha256", "executable_sha256", "angel_sha256"), digest),
            "source_digest": observed(("source_digest", "cockpit_source_sha256", "source_sha256"), digest),
            "head": None,
        }
        # A reducer checkout HEAD is not the evaluated binary's build HEAD.
        binary = meta.get("binary")
        builds = [obj['build_info'] for obj in scopes if isinstance(obj.get('build_info'), dict)]
        heads = {build[key] for build in builds for key in ('git_commit', 'git_head', 'head')
                 if isinstance(build.get(key), str) and re.fullmatch(
                     r'[0-9a-fA-F]{40}|[0-9a-fA-F]{64}', build[key])}
        if len(heads) == 1:
            candidate['head'] = next(iter(heads))
        binary_values = {obj[key] for obj in scopes
                         for key in ("binary_sha256", "executable_sha256", "angel_sha256")
                         if isinstance(obj.get(key), str) and re.fullmatch(digest, obj[key])}
        if isinstance(binary, dict) and isinstance(binary.get("sha256"), str) and re.fullmatch(digest, binary["sha256"]):
            binary_values.add(binary["sha256"])
        candidate["binary_sha256"] = next(iter(binary_values)) if len(binary_values) == 1 else None
    if dataset is None:
        dataset = {"id": observed(("dataset_id",)),
                   "sha256": observed(("dataset_sha256", "sealed_manifest_sha256", "task_list_sha256", "tasks_sha256"), digest)}
        if isinstance(meta.get("dataset"), dict):
            raw_dataset = meta["dataset"]
            dataset = {"id": raw_dataset.get("id"), "sha256": raw_dataset.get("sha256")}
            value = dataset["id"]
            if not isinstance(value, str) or not value.strip() or value.lower() in {"unknown", "unbound", "mixed", "none", "null", "unreported"}:
                dataset["id"] = None
            value = dataset["sha256"]
            if not isinstance(value, str) or not re.fullmatch(digest, value):
                dataset["sha256"] = None
    for extra in ("reference_sha256",):
        # Seed provenance: a reference-backed seed run names the held-out
        # reference-solutions digest alongside the task list digest.
        value = meta.get(extra)
        if isinstance(value, str) and re.fullmatch(digest, value):
            dataset = {**dataset, extra: value}
            break
    if dataset.get("sha256") and not dataset.get("id"):
        dataset = {**dataset, "id": "sha256:" + dataset["sha256"]}
    evaluator = receipt_file_identity(evaluator_path, root) if evaluator_path else {}
    if not evaluator and isinstance(meta.get("evaluator_evidence_path"), str):
        evaluator = receipt_file_identity(meta["evaluator_evidence_path"], root)
    if units is None:
        declared = {"wall_ms": "ms", "wall_s": "s", "solve_rate": "fraction",
                    "accuracy_pct": "percent", "solve_pct": "percent",
                    "tokens_per_solved": "tokens/solved outcome", "usd_per_solved": "USD/solved outcome",
                    "time_to_first_verified_task_s": "s", "precision": "fraction", "recall": "fraction"}
        units = {"*": None, "reason": {"*": "No declared unit for this metric; requires producer-specific semantics."}}
        def collect(value):
            if isinstance(value, dict):
                for key, child in value.items():
                    if key == "receipt_header":
                        continue
                    if key in declared:
                        units["field:" + key] = declared[key]
                    collect(child)
            elif isinstance(value, list):
                for child in value:
                    collect(child)
        collect(document)
    document["receipt_header"] = make_receipt_header(
        dataset=dataset, candidate=candidate, evaluator=evaluator, units=units)
    return document


def receipt_header_markdown(document):
    """Include the exact machine header alongside human-displayed scores."""
    return "\n\nReceipt identity (null means unqualified):\n\n```json\n" + json.dumps(
        document["receipt_header"], indent=2, sort_keys=True) + "\n```\n"


if __name__ == '__main__':
    import sys
    data = json.loads(Path(sys.argv[1]).read_text())
    rows = data if isinstance(data, list) else [data]
    report = field_coverage(rows)
    print(json.dumps(report, sort_keys=True))
    raise SystemExit(0 if rows and not report['validation']['invalid'] else 1)
