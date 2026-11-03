#!/usr/bin/env python3
"""Read-only iteration-45 evidence reconciliation; NOT product acceptance.

Reads named historical artifacts and recomputes paired statistics. Runs only the
existing read-only ownership verifier and protection check, never a compiler,
benchmark, launcher, network client, installer, cleanup or seal operation.
Exit 0: archival reconciliation holds, even if product qualification is blocked.
Output is atomic, no-clobber, and includes explicit product/candidate verdicts.
"""
import argparse
import hashlib
import json
import math
import os
import re
from pathlib import Path
import statistics
import subprocess
import sys
import tempfile

ROOT = Path(__file__).resolve().parents[2]
PERF = ROOT / 'reports/performance'


def sha(path):
    with path.open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()


def paired(rows, pinned=False):
    """Require the complete fixed AB/BA schedule; never select a faster subset."""
    if pinned:
        assert len(rows) == 50
        primes = [r for r in rows if r['pair'] == -1]
        warm = [r for r in rows if r['pair'] != -1]
        value = lambda r: r['ns'] / 1e6
    else:
        assert len(rows) == 50
        primes = [r for r in rows if r['phase'] == 'prime']
        warm = [r for r in rows if r['phase'] == 'warm']
        value = lambda r: r['elapsed_ms']
    assert len(primes) == 2 and {r['arm'] for r in primes} == {'baseline', 'candidate'}
    assert all(r['exit'] == 0 and not r.get('timed_out', False) for r in rows)
    assert len(warm) == 48
    assert all(math.isfinite(value(r)) and value(r) > 0 for r in rows)
    deltas = []
    for index in range(24):
        group = warm[index * 2:index * 2 + 2]
        assert [r['pair'] for r in group] == [index, index]
        assert [r['arm'] for r in group] == (['baseline', 'candidate'] if index % 2 == 0 else ['candidate', 'baseline'])
        arms = {r['arm']: value(r) for r in group}
        deltas.append(arms['baseline'] - arms['candidate'])
    medians = {a: statistics.median(value(r) for r in warm if r['arm'] == a)
               for a in ('baseline', 'candidate')}
    delta = statistics.median(deltas)
    return dict(medians_ms=medians, paired_reductions_ms=deltas,
                paired_median_reduction_ms=delta,
                paired_percent_of_baseline_median=100 * delta / medians['baseline'],
                candidate_faster_pairs=sum(d > 0 for d in deltas))


def publish(path, body):
    """Same-filesystem link commits complete bytes without replacing any name."""
    path.parent.mkdir(parents=True, exist_ok=True)
    fd, temporary = tempfile.mkstemp(prefix='.audit45-', dir=path.parent)
    try:
        with os.fdopen(fd, 'w') as stream:
            stream.write(body)
            stream.flush()
            os.fsync(stream.fileno())
        os.link(temporary, path)
    finally:
        os.unlink(temporary)


def audit():
    inputs, checks, findings = {}, [], {}

    def read(name):
        path = ROOT / name
        assert path.is_relative_to(ROOT) and path.is_file(), name
        inputs[name] = sha(path)
        return path.read_text()

    def load(name):
        return json.loads(read('reports/performance/' + name))

    def check(name, holds, detail=None):
        checks.append(dict(claim=name, holds=bool(holds), detail=detail))

    # Preserve the previous audit as history, not a current-source certificate.
    old = load('startup-evidence-checkpoint-iteration30-r3.json')
    check('iteration30 retained archival verdict', old['checks_count'] == 302 and not old['failures'])
    for name, digest in old['inputs_sha256'].items():
        read(name)
        check('iteration30 primary receipt unchanged: ' + name, inputs[name] == digest)
    for iteration in list(range(30, 42)) + [43, 44]:
        read(f'reports/research-82abbb708cda5ae57589ee8a17aeaaaa-{iteration}.md')
    findings['iteration42'] = 'No named top-level report or performance artifact located; no result or test count credited.'

    # Six separate experiments: these scopes cannot be added or multiplied.
    specs = [
        (31, 'launcher-clock-iteration31/timing', 'applied logging-only launcher change'),
        (32, 'launcher-artifact-iteration32/timing', 'Python artifact selector rejected/unapplied'),
        (33, 'launcher-jq-artifact-iteration33/timing', 'jq selector withheld/unapplied; overhead bound, not speedup'),
        (34, 'launcher-builtin-discovery-iteration34/timing', 'builtin discovery rejected/restored'),
        (37, 'launcher-text-iteration37/timing', 'applied short-text admission change'),
        (38, 'launcher-pin-hash-iteration38', 'applied pin hardlink hash elimination'),
    ]
    timings = {}
    for iteration, directory, decision in specs:
        pinned = iteration == 38
        prefix = 'timing-' if pinned else ''
        summary = load(directory + '/' + prefix + 'summary.json')
        rows = [json.loads(line) for line in read('reports/performance/' + directory + '/' + prefix + 'raw.jsonl').splitlines() if line.strip()]
        result = paired(rows, pinned)
        for key, value in result.items():
            skey = {'paired_median_reduction_ms': 'median_paired_reduction_ms',
                    'paired_percent_of_baseline_median': 'paired_percent_of_baseline',
                    'candidate_faster_pairs': 'wins'}.get(key, key) if pinned else key
            expected = summary[skey]
            if isinstance(value, dict):
                holds = all(math.isclose(v, expected[k], abs_tol=1e-8) for k, v in value.items())
            elif isinstance(value, list):
                holds = len(value) == len(expected) and all(math.isclose(a, b, abs_tol=1e-8) for a, b in zip(value, expected))
            else:
                holds = math.isclose(value, expected, abs_tol=1e-8)
            check(f'iteration{iteration} primary paired arithmetic: {key}', holds)
        gate_key = 'thresholds_held' if pinned else ('no_slowdown_gate_held' if iteration in (32, 33) else 'pilot_threshold_met')
        gate = summary[gate_key]
        reduction = result['paired_median_reduction_ms']
        pct = result['paired_percent_of_baseline_median']
        wins = result['candidate_faster_pairs']
        if iteration in (32, 33):
            recomputed_gate = -reduction <= 1 and -pct <= 5
        else:
            minimum_ms, minimum_pct, minimum_wins = {31: (.5, 2, 18), 34: (.5, 2, 18),
                37: (2, 5, 20), 38: (15, 10, 18)}[iteration]
            recomputed_gate = reduction >= minimum_ms and pct >= minimum_pct and wins >= minimum_wins
        check(f'iteration{iteration} pilot formula recomputed, not copied', gate is recomputed_gate)
        check(f'iteration{iteration} retained pilot verdict', gate is (iteration in (31, 33, 37, 38)))
        timings[str(iteration)] = dict(**result, historical_pilot_gate=gate, decision=decision,
            scope='warm sparse 256 MiB pinned-image fixture' if pinned else 'warm tiny dependency-free real-Cargo fixture; not angelX readiness')
    status34 = load('blaze-rustc-status-iteration34/result.json')
    check('iteration34 seven finite status cases match recorded expectations', len(status34['cases']) == 7 and all(r['expected'] for r in status34['cases']))
    findings['status34_limit'] = 'Uninstalled helper-only candidate; returning None alone did not propagate explicit toolchain refusal. Iterations35/36 extend the error path; do not credit iteration34 as a composed production repair.'

    # Artifact relocation remains a live correctness risk, despite Cargo freshness.
    baseline = load('launcher-artifact-iteration32/repro-baseline/summary.json')
    candidate = load('launcher-artifact-iteration32/repro-candidate/summary.json')
    check('iteration32 baseline has four stale launches', len(baseline['stale_launches']) == 4 and baseline['launch_attempts'] == 9)
    check('iteration32 unapplied selector fixes reduced cases', candidate['stale_launches'] == [] and candidate['launch_attempts'] == 9)
    findings['artifact_location'] = dict(baseline_stale_cases=baseline['stale_launches'], fix_installed=False,
        consequence='Cargo success alone does not qualify the independently guessed executable under output relocation.')

    # Probe checks progressed to a real upstream test target, not full linking.
    probes = load('blaze-full-probes-iteration36/integration-results-v2.json')
    labels = ['existing-plan-tests', 'existing-state-tests'] + [f'integration-{i:02d}' for i in range(9)] + ['final-checked-probe-tests']
    check('iteration36 final named commands succeeded', [r['label'] for r in probes] == labels and all(r['exit_code'] == 0 for r in probes))
    counts = []
    for row in probes:
        log = read('reports/performance/blaze-full-probes-iteration36/v2-' + row['label'] + '.log')
        matches = re.findall(r'test result: ok\. (\d+) passed; 0 failed;', log)
        check('iteration36 nonempty passing test log: ' + row['label'], bool(matches) and sum(map(int, matches)) > 0)
        counts.append(sum(map(int, matches)))
    probe35 = load('blaze-toolchain-probes-iteration35/result.json')
    check('iteration35 complete 40+40 expected-case matrix', len(probe35['cases']) == 80 and all(r['met'] for r in probe35['cases']) and all(sum(r['arm'] == arm for r in probe35['cases']) == 40 for arm in ('baseline', 'candidate')))
    mask = load('blaze-toolchain-probes-iteration35/answer-mask-result.json')['cases']
    check('iteration35 answer synthesis masks refusal; direct and off preserve it', len(mask) == 6 and {r['mode'] for r in mask} == {'direct', 'answers-own', 'answers-no-own'} and all(r['exit'] == (0 if r['mode'] == 'answers-own' else 23) for r in mask))
    findings['toolchain35_36'] = dict(final_log_test_counts=counts, scope='Checked probes and Plan-local refusal remain uninstalled. Real ra-jit target tested on Linux, not Windows linking or daemon execution. Answer synthesis can bypass refusal; composition unqualified. Final v2 receipt has two Cargo commands plus ten direct test-binary calls. Historical five-Cargo-attempt accounting not independently re-established here.')
    disabled = load('blaze-disabled-key-iteration37/result.json')
    cases = disabled['cases']
    expected_tests = {'off_success', 'off_failure', 'no_home', 'enabled_cache', 'own_success', 'own_miss', 'no_disabled_key_regression'}
    check('iteration37 disabled-key complete expected matrix including failing baseline', len(cases) == 14 and all(r['exit'] == r['expected'] == (101 if r['arm'] == 'baseline' and r['test'] == 'no_disabled_key_regression' else 0) for r in cases) and {(r['arm'], r['test']) for r in cases} == {(arm, test) for arm in ('baseline', 'candidate') for test in expected_tests})
    findings['disabled_keys37'] = dict(receipt_keys=list(disabled), decision='Uninstalled correctness candidate; no latency promotion.')
    findings['launcher37_temporal_limit'] = 'Full 46-test suite and differential covered R1. Final R2 had three targeted tests and timing. Do not attribute the R1 full suite to R2; later iteration38 fixture suites are separate evidence.'
    cwd = load('blaze-cwd-identity-iteration39/r2/matrix-result.json')
    check('iteration39 explicit-cwd defect and conservative repair', cwd['baseline_mismatches'] == 12 and cwd['candidate_exact_matches'] == 20 and len(cwd['rows']) == 40)
    for arm, expected in [('baseline', 8), ('candidate', 20)]:
        selected = [r for r in cwd['rows'] if r['arm'] == arm]
        check('iteration39 row totals: ' + arm, len(selected) == 20 and sum(r['exact_equal'] for r in selected) == expected)
    queries = load('blaze-cwd-identity-iteration39/r2/query-receipts.json')
    for row in cwd['rows']:
        # One baseline-arm reference is shared by BOTH answer arms. Warm-cache,
        # repeat and sentinel queries are extra calls, not more matrix rows.
        refs = [r for r in queries if r['case'] == row['case'] and r['flag'] == row['flag'] and r['mode'] == 'reference']
        reference = refs[0] if len(refs) == 1 else None
        answers = [r for r in queries if all(r[k] == row[k] for k in ('arm', 'case', 'flag')) and r['mode'] == 'answer' and reference and r['argv'][2:] == reference['argv'][2:] and r['cwd'] == reference['cwd']]
        check('iteration39 direct output identity: ' + repr((row['arm'], row['case'], row['flag'])), reference is not None and bool(answers) and (reference['result']['output'] == answers[0]['result']['output']) is row['exact_equal'] and reference['exit'] == answers[0]['exit'] == 0)
    migration = queries[-3:]
    check('iteration39 cache namespace poison control', cwd['migration_entries'] == 2 and [r['arm'] for r in migration] == ['baseline', 'baseline', 'candidate'] and migration[1]['result']['output']['stdout'] == list(b'CWD39-OLD-NAMESPACE-POISON\n') and migration[2]['result']['output'] == migration[0]['result']['output'])
    findings['cwd39'] = 'Conservative all-explicit-cwd delegation plus cache namespace migration; uninstalled; safety has an unmeasured acceleration cost.'

    visits = load('blaze-directory-visits-iteration40/summary.json')
    roots = load('blaze-complete-roots-iteration41/summary.json')
    check('iteration40 fails its performance gate', visits['performance_gate'] is False)
    check('iteration40 single-tree slowdown exceeds five percent', visits['summary']['single-tree']['paired_reduction_pct'] < -5)
    check('iteration41 passes ONLY historical stable-fixture performance gate', roots['performance_gate'] is True)
    watcher_stats = {}
    for iteration, directory, summary, control in [(40, 'blaze-directory-visits-iteration40', visits, 'baseline'), (41, 'blaze-complete-roots-iteration41', roots, 'entry_types')]:
        samples = load(directory + '/samples.json')
        stats = {}
        check(f'iteration{iteration} raw sample count', len(samples) == 72)
        for case in ('overlap', 'single-tree', 'file-directives'):
            selected = [r for r in samples if r['case'] == case]
            check(f'iteration{iteration} complete case pairs: {case}', len(selected) == 24 and sorted(r['index'] for r in selected) == list(range(24)))
            deltas = [r[control + '_ns'] - r['candidate_ns'] for r in selected]
            pct = statistics.median(deltas) / statistics.median(r[control + '_ns'] for r in selected) * 100
            wins = sum(d > 0 for d in deltas)
            recorded = summary['summary'][case] if iteration == 40 else summary['summary'][case]['entry_types']
            check(f'iteration{iteration} raw watcher arithmetic: {case}', math.isclose(pct, recorded['paired_reduction_pct'], abs_tol=1e-8) and wins == recorded['wins'])
            stats[case] = dict(pct=pct, wins=wins)
        gate = stats['overlap']['pct'] >= 15 and stats['overlap']['wins'] >= 20 and all(stats[c]['pct'] >= -5 for c in ('single-tree', 'file-directives'))
        check(f'iteration{iteration} watcher performance gate recomputed', gate is summary['performance_gate'])
        watcher_stats[str(iteration)] = stats
    findings['watcher_raw_arithmetic'] = watcher_stats
    descendant = load('blaze-directory-visits-iteration40/descendant-diagnostic/result.json')
    check('iteration40 descendant diagnostic is expected failing candidate, not acceptance', descendant['candidate_test_exit'] == 101 and descendant['defect_reproduced'] is True)
    diagnostic_source = 'reports/performance/blaze-directory-visits-iteration40/descendant-diagnostic/reduction.rs'
    read(diagnostic_source)
    diagnostic_log = read('reports/performance/blaze-directory-visits-iteration40/descendant-diagnostic/test.stdout')
    diagnostic_receipt = load('blaze-directory-visits-iteration40/descendant-diagnostic/test.receipt.json')
    check('iteration40 diagnostic source bound to failed test log', inputs[diagnostic_source] == descendant['source_sha256'] and 'test result: FAILED. 0 passed; 1 failed;' in diagnostic_log and 101 in diagnostic_receipt.values())
    consumer = load('blaze-consumer-keys-verdict-iteration43.json')
    check('iteration43 does not promote completed-root reuse', consumer['promotion'].startswith('withheld'))
    for name in ['blaze-consumer-keys-iteration43', 'blaze-consumer-keys-r2-iteration43']:
        execution = load(name + '/execution.json')
        log = read('reports/performance/' + name + '/test.log')
        check('iteration43 execution/log digest: ' + name, execution['exit'] == 0 and hashlib.sha256(log.encode()).hexdigest() == execution['test_log_sha256'])
        if 'r2' in name:
            check('iteration43 stale-HIT diagnostic primary lines', 'EVIDENCE mutation-schedule:baseline includes new leaf; MISS after content edit' in log and 'EVIDENCE mutation-schedule:candidate omitted new leaf; stale old-result HIT after content edit (qualification FAILURE)' in log and 'test result: ok. 5 passed; 0 failed;' in log)
    findings['watchers40_43'] = 'Reject directory dedup (transient descendant loss plus slowdown) and completed-root suppression (inter-directive creation omitted; later leaf edit stale HIT). Iteration41 stable-fixture success is superseded for promotion by iteration43 mutation diagnostic. Green diagnostic tests that detect stale HIT are not candidate acceptance.'

    # Rerun only the inexpensive source/receipt checker, NOT its experiment.
    verifier = ROOT / 'reports/performance/verify-blaze-path-ownership-iteration44.py'
    read(str(verifier.relative_to(ROOT)))
    verified = subprocess.run([sys.executable, '-B', str(verifier)], cwd=ROOT,
                              text=True, capture_output=True, timeout=60)
    check('iteration44 current read-only verifier exit', verified.returncode == 0, verified.stderr)
    ownership = json.loads(verified.stdout) if verified.returncode == 0 else {}
    saved = load('blaze-path-ownership-audit-r2-iteration44.json')
    check('iteration44 saved corrected audit reproduces exactly', ownership == saved)
    check('iteration44 performance still fails both formulas', ownership.get('performance_gate_preregistered_formula') is False and ownership.get('performance_gate_frozen_runner_formula') is False)
    check('iteration44 overlap consistency gate failed', ownership.get('timing', {}).get('overlap', {}).get('wins') == 16)
    ownership_samples = load('blaze-path-ownership-iteration44/samples.json')
    formulas = {}
    for case in ('overlap', 'single-tree', 'file-directives'):
        selected = [r for r in ownership_samples if r['case'] == case]
        check('iteration44 complete timing pairs: ' + case, len(selected) == 24 and sorted(r['index'] for r in selected) == list(range(24)))
        deltas = [r['baseline_ns'] - r['candidate_ns'] for r in selected]
        formulas[case] = dict(runner_pct=statistics.median(deltas) / statistics.median(r['baseline_ns'] for r in selected) * 100,
            prereg_pct=statistics.median(100 * (r['baseline_ns'] - r['candidate_ns']) / r['baseline_ns'] for r in selected), wins=sum(d > 0 for d in deltas))
    for label, field in [('preregistered', 'prereg_pct'), ('frozen_runner', 'runner_pct')]:
        gate = all(formulas[c][field] >= 3 and formulas[c]['wins'] >= 18 for c in ('overlap', 'single-tree')) and formulas['file-directives'][field] >= -5
        key = 'performance_gate_preregistered_formula' if label == 'preregistered' else 'performance_gate_frozen_runner_formula'
        check('iteration44 both formulas independently recomputed: ' + label, gate is ownership[key] and gate is False)
    findings['ownership44_independently_recomputed_formulas'] = formulas
    findings['ownership44'] = ownership
    findings['failed_receipts'] = 'First iteration44 audit requested wrong fn visibility and left an empty capture; corrected audit now reproduced. Iteration43 final capture/manifest command failure is not performance evidence; later diagnostic and explicit withheld verdict remain. No old receipt overwritten.'

    # Live state: report guard failure independently from archival success.
    live_names = ['bin/angelX', 'cockpit/Cargo.toml', 'cockpit/src/drive/loop_ctl.rs',
                  'locks/locked.sha256', 'tests/scripts/native-launch.test.mjs']
    for name in live_names:
        read(name)
    launcher = (ROOT / 'bin/angelX').read_text()
    check('live launcher source unchanged since iteration44 read-only inspection', inputs['bin/angelX'] == load('blaze-path-ownership-iteration44/final-read-only-inspection.json')['selected_hashes']['bin/angelX'])
    check('auto cannot enter explicit experimental Blaze branch', 'if [ "${ANGEL_NO_BUILD:-}" != 1 ] && [ "$BUILD_BACKEND" = blaze ]; then' in launcher and 'else\n  BUILD_BACKEND=cargo\nfi' in launcher)
    guard = subprocess.run(['bash', 'scripts/check/check-locks.sh'], cwd=ROOT,
                           text=True, capture_output=True, timeout=20)
    mismatches = []
    for line in (ROOT / 'locks/locked.sha256').read_text().splitlines():
        if not line.strip() or line.startswith('#'):
            continue
        expected, name = line.split(maxsplit=1)
        name = name.lstrip('*')
        if sha(ROOT / name) != expected:
            mismatches.append(name)
    check('known seal blocker not silently cleared', guard.returncode == 1 and mismatches == ['cockpit/src/drive/loop_ctl.rs'])
    head = subprocess.run(['git', 'show', 'HEAD:cockpit/src/drive/loop_ctl.rs'], cwd=ROOT,
                          capture_output=True, timeout=20)
    check('protected source matches committed HEAD, not seal', head.returncode == 0 and hashlib.sha256(head.stdout).hexdigest() == inputs['cockpit/src/drive/loop_ctl.rs'])
    failed = [c for c in checks if not c['holds']]
    return dict(schema='angel.startup-evidence-checkpoint/v1', iteration=45,
        status='reported' if not failed else 'inconclusive',
        audit_scope='Worker-authored archival/source/receipt verification. Not app tests, independent execution qualification or a new speed measurement.',
        auditor_sha256=sha(Path(__file__)), inputs_sha256=inputs, checks=checks,
        checks_count=len(checks), failures=failed, timings=timings, findings=findings,
        gate=dict(exit_code=guard.returncode, stdout=guard.stdout, stderr=guard.stderr, mismatches=mismatches),
        qualification=dict(product='BLOCKED', automatic_backend='cargo', blaze='explicit experimental',
            new_end_to_end_speedup_measured=False, loop_complete=False,
            next_action='Operator disposition of source/seal mismatch; then qualify actual full-feature source/image freshness and edit/restore. Artifact relocation and Blaze stale restore remain independent correctness blockers.'))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', required=True, type=Path)
    args = parser.parse_args()
    output = args.output.resolve()
    if not output.is_relative_to(ROOT) or output.exists():
        parser.error('output must be an unused workspace path')
    try:
        result = audit()
    except (AssertionError, KeyError, OSError, ValueError, subprocess.SubprocessError) as error:
        result = dict(schema='angel.startup-evidence-checkpoint/v1', iteration=45,
                      status='inconclusive', audit_error=repr(error), qualification={'product': 'NOT QUALIFIED'})
    publish(output, json.dumps(result, indent=2) + '\n')
    print(json.dumps({k: result[k] for k in ('status', 'checks_count', 'failures', 'audit_error', 'qualification') if k in result}, indent=2))
    return 0 if result.get('status') == 'reported' and not result.get('failures') else 1


if __name__ == '__main__':
    sys.exit(main())
