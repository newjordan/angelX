#!/usr/bin/env python3
"""Model-free M05 process contract; only a loopback scripted provider is used."""
from __future__ import annotations
import argparse
import importlib.util
import json
import os
import subprocess
import tempfile
import time
from pathlib import Path

REPO = Path(__file__).resolve().parents[1]
spec = importlib.util.spec_from_file_location('harness_stress', REPO / 'scripts/harness-stress.py')
stress = importlib.util.module_from_spec(spec)
spec.loader.exec_module(stress)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--angel-bin', default=str(REPO / 'cockpit/target/debug/angel'))
    parser.add_argument('--out', default=str(REPO / 'docs/audits/evidence/2026-09-08-straight-a/M05'))
    parser.add_argument('--provider', choices=['loopback', 'practice'], default='loopback')
    args = parser.parse_args()
    (REPO / '.m05-tmp').mkdir(exist_ok=True)
    with tempfile.TemporaryDirectory(prefix='m05-', dir=REPO / '.m05-tmp') as temporary:
        root = Path(temporary)
        workspace = root / 'workspace'
        workspace.mkdir()
        home = root / 'home'
        home.mkdir()
        caddy_root = root / 'caddy'
        ledger = root / 'experience.jsonl'
        env = {'PATH': os.defpath, 'HOME': str(home), 'TMPDIR': str(root),
               'GIT_CEILING_DIRECTORIES': str(root),
               'ANGEL_DRIVER': 'openrouter', 'ANGEL_API_CLUBS': 'openrouter',
               'ANGEL_OPENROUTER_KEY': 'offline-m05', 'OPENROUTER_API_KEY': 'offline-m05',
               'ANGEL_OPENROUTER_MODEL': 'offline-m05-model', 'ANGEL_CADDY': '1',
               'ANGEL_CADDY_DIR': str(caddy_root), 'ANGEL_EXPERIENCE': '1',
               'ANGEL_EXPERIENCE_LOG': str(ledger), 'ANGEL_PROJECT_DOC': '0',
               'ANGEL_TASK_RECON': '0', 'ANGEL_SKILL_HINT': '0', 'ANGEL_ADVISOR': '0',
               'ANGEL_VERIFY_BEFORE_DONE': '0', 'ANGEL_FIRST_WRITE_CALLS': '0',
               'ANGEL_TASK_STRICT_EXIT': '0', 'ANGEL_YOLO': '1',
               'ANGEL_HARNESS_ROLLOUT_DIR': str(root / 'rollouts')}
        if args.provider == 'practice':
            env['ANGEL_DRIVER'] = 'practice'
            env['ANGEL_API_CLUBS'] = ''
            for key in ['ANGEL_OPENROUTER_KEY', 'OPENROUTER_API_KEY', 'ANGEL_OPENROUTER_MODEL']:
                env.pop(key, None)
        commands = []
        def run(policy, task):
            cmd = [args.angel_bin, '--yolo', '--task-json', '--workspace', str(workspace),
                   '--tool-profile', 'essential', '--rollout', 'off', task]
            commands.append(cmd)
            if args.provider == 'practice':
                # The existing deterministic offline echo driver exercises the
                # same real process/envelope when the host denies AF_INET.
                return subprocess.run(cmd, env=env, capture_output=True, text=True, timeout=180)
            with stress.PolicyServer(policy) as server:
                env['ANGEL_OPENROUTER_URL'] = server.url
                return subprocess.run(cmd, env=env, capture_output=True, text=True, timeout=180)
        # Ask the binary's ledger for its canonical key; no duplicate hash implementation.
        probe = run(lambda hop, req: ('text', 'M05_PROBE'), 'Answer M05_PROBE.')
        assert probe.returncode == 0, f'probe failed: {probe.returncode}: {probe.stderr[-1000:]}'
        rows = [json.loads(line) for line in ledger.read_text().splitlines()]
        keys = [row['repo']['key'] for row in rows if isinstance(row.get('repo'), dict)]
        assert keys, 'probe did not emit a canonical repo identity'
        caddy = caddy_root / keys[-1]
        caddy.mkdir(parents=True, exist_ok=True)
        (caddy / 'recipes.jsonl').write_text('{broken json}\n')
        (caddy / 'hazards.jsonl').write_text('{"ts_ms":1,"command":"h","diagnostic":"d","tool":"shell"}')
        proc = run(lambda hop, req: ('text', 'M05_DONE'), 'Answer M05_DONE.')
        envelope = json.loads(proc.stdout)
        health = envelope.get('memory_health', {})
        rows = [json.loads(line) for line in ledger.read_text().splitlines()]
        events = [row for row in rows if row.get('kind') == 'caddy_health']
        checks = {
            'exit_zero': proc.returncode == 0,
            'completed': envelope.get('status') == 'completed',
            'malformed_count': health.get('malformed_rows', 0) >= 1,
            'truncated_count': health.get('incomplete_tail', 0) >= 1,
            'counts_only': bool(health) and all(type(v) is int for v in health.values()),
            'stderr_health': 'memory-health' in proc.stderr,
            'ledger_classified': any(e.get('class') == 'Environment' for e in events),
        }
        receipt = {'provider': args.provider, 'commands': commands, 'exit_code': proc.returncode, 'checks': checks,
                   'memory_health': health, 'health_event_count': len(events)}
        out = Path(args.out)
        out.mkdir(parents=True, exist_ok=True)
        (out / f'continuation-taskjson-health-{time.time_ns()}.json').write_text(json.dumps(receipt, indent=2) + '\n')
        print(json.dumps(receipt, indent=2))
        print('M05_TASKJSON_HEALTH ' + ('PASS' if all(checks.values()) else 'FAIL'))
        return 0 if all(checks.values()) else 1


if __name__ == '__main__':
    raise SystemExit(main())
