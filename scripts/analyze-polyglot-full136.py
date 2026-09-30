#!/usr/bin/env python3
"""Audit every Polyglot trace directory, preserving all-task tails and provenance."""
from __future__ import annotations
import argparse
from collections import Counter
import hashlib
import importlib.util
import json
from pathlib import Path
import statistics
import subprocess


def load_module(name):
    spec = importlib.util.spec_from_file_location(name.replace('-', '_'), Path(__file__).with_name(name + '.py'))
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


history = load_module('analyze-polyglot-history')


def span(value):
    """Verifiers TimeSpan serializes timestamps, not its duration property."""
    start, end = history.number(value.get('start')), history.number(value.get('end'))
    return end - start if start is not None and end is not None and end >= start and end > 0 else None


def stats(values):
    known = [x for x in values if x is not None]
    return {'reported': len(known), 'total': sum(known) if known else None,
            'mean': statistics.mean(known) if known else None,
            'median': statistics.median(known) if known else None,
            'p90': history.percentile(known, .9) if known else None,
            'max': max(known) if known else None}


def char_size(value):
    return len(value) if isinstance(value, str) else len(json.dumps(value, ensure_ascii=False)) if value is not None else 0


EXTRA = ('episode_s', 'setup_s', 'scoring_s', 'call_time_sum_s', 'content_chars', 'reasoning_chars', 'argument_chars')


def summarize(rows):
    result = history.aggregate(rows)
    result['metrics'] = {key: stats([row.get(key) for row in rows]) for key in (*history.METRICS, *EXTRA)}
    result['slow_tasks_60s'] = sum(row['wall_s'] is not None and row['wall_s'] > 60 for row in rows)
    result['slow_tasks_120s'] = sum(row['wall_s'] is not None and row['wall_s'] > 120 for row in rows)
    result['failed_wall_s'] = sum(row['wall_s'] or 0 for row in rows if not row['solved'])
    result['timeout_wall_s'] = sum(row['wall_s'] or 0 for row in rows if row['timeouts'])
    result['model_partition_unknown_wall_s'] = sum(row['wall_s'] or 0 for row in rows if row['model_s'] is None)
    return result


def read_trace(root, path):
    relative = str(path.parent.relative_to(root))
    rows, contract, pins, audit = [], {}, {}, Counter()
    with path.open() as stream:
        for line_no, line in enumerate(stream, 1):
            if not line.strip():
                continue
            record = json.loads(line)
            for trace in record.get('traces') or []:
                row = history.extract_row(record, trace, relative)
                task_data = (record.get('task') or {}).get('data') or {}
                row['task_contract_sha256'] = hashlib.sha256(json.dumps(
                    {key: value for key, value in task_data.items() if key != 'idx'}, sort_keys=True).encode()).hexdigest()
                timing = trace.get('timing') or {}
                agent = timing.get('agent') or {}
                calls = trace.get('calls') or []
                info = trace.get('info') or {}
                nodes = trace.get('nodes') or []
                row['line'] = line_no
                row['wall_s'] = span(agent)
                row['language'] = (row['task'] or '').split('-')[0]
                row['setup_s'] = span(timing.get('setup') or {})
                row['scoring_s'] = span(timing.get('scoring') or {})
                row['episode_s'] = span({'start': timing.get('start'), 'end': (timing.get('scoring') or {}).get('end')})
                call_spans = [span(call.get('time') or {}) for call in calls]
                row['call_time_sum_s'] = sum(x for x in call_spans if x is not None) if calls else None
                row['calls_missing_end'] = sum(x is None for x in call_spans)
                shapes = []
                for call in calls:
                    if not isinstance(call.get('node'), int):
                        continue
                    message = nodes[call['node']]['message']
                    tools = message.get('tool_calls') or []
                    shapes.append((char_size(message.get('content')), char_size(message.get('reasoning_content')),
                        sum(char_size(tool.get('arguments') if 'arguments' in tool else (tool.get('function') or {}).get('arguments')) for tool in tools)))
                for i, key in enumerate(('content_chars', 'reasoning_chars', 'argument_chars')):
                    row[key] = sum(s[i] for s in shapes) if shapes else None
                row['sampled_response_calls'] = len(shapes)
                proxy_model = history.number((agent.get('model') or {}).get('duration'))
                proxy_harness = history.number((agent.get('harness') or {}).get('duration'))
                if row['wall_s'] is not None and proxy_model is not None and proxy_harness is not None:
                    if abs(row['wall_s'] - proxy_model - proxy_harness) > .001:
                        audit['agent_partition_mismatch'] += 1
                if calls and row['wall_s'] is not None and proxy_model is not None:
                    expected = min(row['call_time_sum_s'], row['wall_s'])
                    if abs(expected - proxy_model) > .001:
                        audit['proxy_call_duration_mismatch'] += 1
                if not calls and proxy_model == 0 and row['model_s'] is not None and row['model_s'] > 0:
                    audit['direct_route_proxy_zero_not_used'] += 1
                if row['model_s'] is None:
                    audit['model_partition_unknown_tasks'] += 1
                if row['wall_s'] is None:
                    audit['invalid_agent_timestamps'] += 1
                rows.append(row)
                if not contract:
                    cfg = (trace.get('agent') or {}).get('config') or {}
                    harness = cfg.get('harness') or {}
                    safe_env = {k: v for k, v in (harness.get('env') or {}).items()
                                if k in ('ANGEL_CONFIRM_GREEN_RUNS', 'ANGEL_VERIFY_BEFORE_DONE', 'ANGEL_VERIFY_NUDGES',
                                         'ANGEL_SPIN_LIMIT', 'ANGEL_ERROR_LIMIT', 'ANGEL_UNPRODUCTIVE_STREAK_STOP',
                                         'ANGEL_TOOLCALL_STORM', 'ANGEL_OPENROUTER_MAX_TOKENS')}
                    contract = {'model': cfg.get('model'), 'sampling': cfg.get('sampling'), 'max_hops': harness.get('max_hops'),
                                'tool_timeout': harness.get('tool_timeout'), 'safe_env': safe_env,
                                'wall_cap': ((info.get('agent_exit') or {}).get('wall_cap') or {}).get('secs')}
                    pins = {k: {'sha256': v.get('sha256'), 'cockpit_source_sha256': v.get('cockpit_source_sha256')}
                            for k in ('angel_binary_contract', 'opencode_binary_contract', 'omp_binary_contract')
                            if isinstance(v := info.get(k), dict)}
    parts = path.relative_to(root).parts
    return {'id': relative, 'trace_sha256': hashlib.sha256(path.read_bytes()).hexdigest(), 'paths': [relative],
            'model_family': path.parents[2].name, 'harness': path.parents[1].name,
            'contract': contract, 'pins': pins, 'rows': rows, 'summary': summarize(rows), 'timing_audit': dict(audit)}


def pair(own, peer, solved_only=False):
    a = {row['task']: row for row in own['rows']}
    p = {row['task']: row for row in peer['rows']}
    shared = sorted(a.keys() & p.keys())
    if solved_only:
        shared = [task for task in shared if a[task]['solved'] and p[task]['solved']]
    deltas = []
    for task in shared:
        row = {'task': task, 'language': a[task]['language'], 'angel_solved': a[task]['solved'],
               'peer_solved': p[task]['solved'], 'angel_timeout': a[task]['timeouts'], 'peer_timeout': p[task]['timeouts'],
               'angel_wall_s': a[task]['wall_s'], 'peer_wall_s': p[task]['wall_s'],
               'angel_calls': a[task]['calls'], 'peer_calls': p[task]['calls'],
               'angel_line': a[task]['line'], 'peer_line': p[task]['line'],
               'angel_run': a[task]['run'], 'peer_run': p[task]['run']}
        for key in (*history.METRICS, *EXTRA):
            x, y = a[task].get(key), p[task].get(key)
            row[key] = x-y if x is not None and y is not None else None
        deltas.append(row)
    metrics = {key: stats([row[key] for row in deltas]) for key in (*history.METRICS, *EXTRA)}
    return {'angel': own['id'], 'peer': peer['id'], 'scope': 'jointly_solved' if solved_only else 'all_tasks',
            'tasks': len(shared), 'angel_slower_tasks': sum((r['wall_s'] or 0)>0 for r in deltas),
            'task_contract_mismatches': [task for task in shared if a[task].get('task_contract_sha256') != p[task].get('task_contract_sha256')],
            'angel_faster_tasks': sum((r['wall_s'] or 0)<0 for r in deltas), 'metrics': metrics,
            'language': {lang: {key: stats([r[key] for r in deltas if r['language']==lang])
                               for key in ('wall_s','model_s','non_model_s','calls','output','reasoning_chars','content_chars')}
                         for lang in ('js','py','rust','cpp')},
            'failure_involved_delta_s': sum(r['wall_s'] or 0 for r in deltas if not(r['angel_solved'] and r['peer_solved'])),
            'largest_slowdowns': sorted(deltas,key=lambda r:r['wall_s'] or 0,reverse=True)[:20],
            'largest_speedups': sorted(deltas,key=lambda r:r['wall_s'] or 0)[:10], 'rows': deltas}


def publications(root, repo, cohorts):
    """Match published per-task timings to raw cells, allowing display rounding."""
    result = []
    paths = [repo/'website/js/bench-data.js', repo/'docs/telemetry/polyglot-benchmark-20260921.json',
             root/'report-muse-harness/out/bench-data.json',
             root/'report-0.1.6/video/infographic-kit/src/run-details.ts']
    for path in paths:
        if not path.exists():
            continue
        text = path.read_text()
        data = json.loads(text.partition('=')[2].strip().rstrip(';')) if path.suffix in ('.js','.ts') else json.loads(text)
        entries = []
        if isinstance(data, list):
            entries = [{'model': cell['key'], 'harness': 'angelx', 'attempts': cell['attempts']} for cell in data]
        elif isinstance(data.get('cells'), list):
            entries = data['cells']
        elif isinstance(data.get('rows'), list):
            groups = {}
            for row in data['rows']:
                groups.setdefault((row['model'], row['harness']), []).append(
                    {'task': row['task'], 'wall_s': row['agent_wall_s'], 'solved': row.get('solved',row.get('reward')==1)})
            entries = [{'model': model, 'harness': harness, 'attempts': rows} for (model,harness),rows in groups.items()]
        for cell in entries:
            attempts = cell.get('attempts') or []
            if not attempts:
                continue
            recorded = {row['task']: row for row in attempts}
            candidates = [c for c in cohorts if c['model_family']==cell['model'] and c['harness']==cell['harness']
                          and len(c['rows'])==len(attempts) and {r['task'] for r in c['rows']}==set(recorded)]
            matches = []
            for candidate in candidates:
                errors = [abs(row['wall_s']-recorded[row['task']]['wall_s']) for row in candidate['rows']]
                matches.append({'run':candidate['id'], 'maximum_wall_rounding_error_s':max(errors),
                                'solve_mismatches':sum(row['solved']!=recorded[row['task']]['solved'] for row in candidate['rows'])})
            result.append({'source':str(path),'source_sha256':hashlib.sha256(path.read_bytes()).hexdigest(),
                           'model':cell['model'],'harness':cell['harness'],'attempts':len(attempts),
                           'published_wall_total_s':sum(row['wall_s'] for row in attempts),
                           'best_raw_match':min(matches,key=lambda x:x['maximum_wall_rounding_error_s']) if matches else None})
    return result


def classify(cell, names):
    identity = cell['id']
    root = identity.split('/')[0]
    if root.startswith('QUARANTINE'):
        return 'quarantined'
    if root.startswith('archive-aborted'):
        return 'aborted'
    if root.startswith('archive'):
        return 'archived calibration'
    if root == 'runs-qwen':
        return 'disjoint Qwen chunk'
    if len(cell['rows']) == 136 and {r['task'] for r in cell['rows']} == names:
        return 'full 136-task cohort'
    if root in ('repro-robot', 'repro-robot-fixed', 'repro-robot-confirm', 'probes', 'checks',
                'smoke', 'runs-compact-smoke', 'runs-qwen-smoke', 'tmp', 'calibration', 'escalate-high', 'harness-roots'):
        return 'diagnostic/reproduction/selective'
    if root == 'runs' and 'smoke' in identity:
        return 'diagnostic/reproduction/selective'
    if root == 'runs' and 'aborted' in identity:
        return 'aborted'
    if root == 'runs':
        return 'partial cohort'
    return 'UNKNOWN: retained for review'


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--root',type=Path,default=Path('/home/frosty40/angel_tests/angelX-bench/polyglot-20260921'))
    parser.add_argument('--output',type=Path,required=True)
    args=parser.parse_args();root=args.root.resolve()
    paths=[Path(p) for p in subprocess.check_output(['rg','--files','--hidden',str(root),'-g','traces.jsonl',
           '-g','!**/.venv/**','-g','!**/node_modules/**','-g','!**/py-venv/**'],text=True).splitlines()]
    cells=[read_trace(root,path) for path in sorted(paths)]
    names={row['name'] for row in json.loads((root/'tasks-polyglot-v1.json').read_text())}
    full=[cell for cell in cells if len(cell['rows'])==136 and {r['task'] for r in cell['rows']}==names]
    for harness in ('angelx','opencode','omp'):
        parts=[c for c in cells if c['id'].startswith(f'runs-qwen/qwen/{harness}/')]
        rows=[row for cell in parts for row in cell['rows']]
        if len(rows)!=136 or {r['task'] for r in rows}!=names:
            raise ValueError(f'Qwen {harness} does not have one exact full cohort')
        if any(c['contract']!=parts[0]['contract'] for c in parts):
            raise ValueError(f'Qwen {harness} settings changed between chunks')
        if any(c['pins']!=parts[0]['pins'] for c in parts):
            raise ValueError(f'Qwen {harness} binary pins changed between chunks')
        full.append({**parts[0],'id':f'runs-qwen/qwen/{harness}/combined',
                     'paths':[c['id'] for c in parts], 'trace_sha256_by_run':{c['id']:c['trace_sha256'] for c in parts},
                     'rows':rows,'summary':summarize(rows),
                     'timing_audit':dict(sum((Counter(c['timing_audit']) for c in parts),Counter()))})
    peers={family:max((c for c in full if c['model_family']==family and c['harness']=='opencode'),key=lambda c:c['id'])
           for family in ('deepseek','glm','grok','muse','qwen')}
    eligible=[c for c in full if not any(s in c['id'].lower() for s in ('quarantine','archive','calibration'))]
    pairs=[pair(c,peers[c['model_family']],scope) for c in eligible
           if c['harness']=='angelx' and c['model_family'] in peers for scope in (False,True)]
    confirmation=[]
    for seed in ('seed1','seed2'):
        arms=[next(c for c in eligible if c['id'].startswith(f'ab-confirm/{arm}/') and c['id'].split('/')[-1].startswith(seed))
              for arm in ('on','off')]
        confirmation.append({'seed_label':seed,'same_binary':arms[0]['pins']==arms[1]['pins'],
                             'on_minus_off_all_tasks':pair(*arms), 'on_minus_off_jointly_solved':pair(*arms,True)})
    for c in full:
        c['by_language']={lang:summarize([r for r in c['rows'] if r['language']==lang]) for lang in ('js','py','rust','cpp')}
    result={'schema':'angelx-full136-audit/v1','root':str(root),'catalog_sha256':hashlib.sha256((root/'tasks-polyglot-v1.json').read_bytes()).hexdigest(),
            'inventory':[{'id':c['id'],'records':len(c['rows']),'trace_sha256':c['trace_sha256'], 'classification':classify(c,names),
                          'full_catalog':len(c['rows'])==136 and {r['task'] for r in c['rows']}==names} for c in cells],
            'full_cohorts':full,'pairings':pairs,'confirmation_ab':confirmation,
            'published_reconciliation':publications(root,Path(__file__).resolve().parents[1],cells+full),
            'empty_gentrim_log':(root/'runs-gentrim-full.log').exists() and (root/'runs-gentrim-full.log').stat().st_size==0}
    args.output.parent.mkdir(parents=True,exist_ok=True)
    args.output.write_text(json.dumps(result,indent=2)+'\n')
    print(f'inventory={len(cells)} full_cohorts={len(full)} pairings={len(pairs)} output={args.output}')
    for c in eligible:
        m=c['summary']['metrics'];print(c['id'],c['summary']['solved'],'wall',round(m['wall_s']['total'],3),
           'mean',round(m['wall_s']['mean'],3),'median',round(m['wall_s']['median'],3),'p90',round(m['wall_s']['p90'],3))


if __name__=='__main__':main()
