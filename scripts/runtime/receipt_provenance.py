"""Producer-side receipt provenance. Never infer a historical binary from HEAD."""
import hashlib
import json
from pathlib import Path
import subprocess

from trace_schema import attach_receipt_header, receipt_file_identity

ARENA_UNITS = {
    'fixture_files': 'files', 'fixture_bytes': 'bytes', 'round': 'index',
    'verify_exit': 'exit code', 'exit': 'exit code', 'wall_s': 's',
    'tokens': 'tokens', 'tool_calls': 'count', 'hops': 'count', 'solved': 'count',
    'wall_ms': 'ms', 'startup_ms': 'ms', 'shutdown_ms': 'ms', 'model_ms': 'ms',
    'tool_ms': 'ms', 'other_ms': 'ms', 'tool_overhead_ms': 'ms',
    'retry_backoff_ms': 'ms', 'ms': 'ms', 'calls': None, 'model_calls': 'count',
    'retries': 'count', 'attempt': 'index', 'ordinal': 'index',
}
ARENA_UNITS.update({key: 'ms' for key in (
    'model_retry_ms', 'tool_max_ms', 'envelope_wall_ms', 'startup_shutdown_ms',
    'identity_prewarm_ms', 'pre_request_ms', 'project_doc_ms', 'registry_ms',
    'workspace_recon_ms', 'serial_overhead_ms', 'residual_ms', 'overlap_ms',
    'compaction_ms', 'aging_ms', 'reflex_tick_ms', 'idle_probe_ms')})


def declared_dataset(identifier, tasks):
    """Identity of the exact predeclared, JSON-serializable task definition."""
    payload = json.dumps(tasks, sort_keys=True, separators=(',', ':'),
                         ensure_ascii=False, allow_nan=False).encode()
    return {'id': identifier, 'sha256': hashlib.sha256(payload).hexdigest()}


def file_dataset(path, root=None):
    identity = receipt_file_identity(path, root)
    return {'id': identity.get('path'), 'sha256': identity.get('sha256')}


def binary_metadata(binary):
    """Read the evaluated executable's identity; never launch a build wrapper."""
    import sys
    sys.path.insert(0, str(Path(__file__).resolve().parent))
    from producer_build_identity import binary_identity
    identity = binary_identity(Path(binary))
    info = dict(identity.pop('build_info'))
    if identity.get('head'):
        info['git_commit'] = identity['head']
    return {'binary_sha256': identity['binary_sha256'], 'build_info': info,
            'source_digest': identity['source_digest'],
            'git_commit': identity['head']}


def numeric_units(document, declarations=None):
    """Enumerate exact JSON pointers, with explicit unknowns for undeclared units.

    Ancestor declarations describe distributions (e.g. wall_ms/p95). A leaf
    declaration overrides its ancestor. No blanket count/score assumption.
    """
    declarations = declarations or {}
    units, reasons = {}, {}

    def visit(value, path='', inherited=None):
        if isinstance(value, dict):
            for key, child in value.items():
                if key == 'receipt_header':
                    continue
                unit = declarations.get(key, inherited)
                visit(child, path + '/' + key.replace('~', '~0').replace('/', '~1'), unit)
        elif isinstance(value, list):
            for index, child in enumerate(value):
                visit(child, path + '/' + str(index), inherited)
        elif type(value) in (int, float) or (isinstance(value, bool) and inherited is not None):
            units[path] = inherited
            if inherited is None:
                reasons[path] = 'Producer has not declared the semantics of this numeric field.'

    visit(document)
    # Repeated row fields share one declaration; retain exact pointers whenever
    # the same leaf has different semantics in different distributions.
    grouped = {}
    for pointer, unit in units.items():
        leaf = pointer.rsplit('/', 1)[-1]
        if not leaf.isdigit():
            grouped.setdefault(leaf, []).append((pointer, unit))
    for leaf, entries in grouped.items():
        if len(entries) > 1 and len({unit for _, unit in entries}) == 1:
            name = 'field:' + leaf.replace('~1', '/').replace('~0', '~')
            units[name] = entries[0][1]
            for pointer, _ in entries:
                units.pop(pointer)
                reasons.pop(pointer, None)
            if entries[0][1] is None:
                reasons[name] = 'Producer has not declared the semantics of this numeric field.'
    if not units:
        units['*'] = None
        reasons['*'] = 'No numeric observations in this document.'
    if reasons:
        units['reason'] = reasons
    return units


def attach_producer_header(document, *, metadata=None, dataset=None,
                           evaluator_path=None, declarations=None, root=None):
    return attach_receipt_header(
        document, metadata=metadata, dataset=dataset, evaluator_path=evaluator_path,
        units=numeric_units(document, declarations), root=root)
