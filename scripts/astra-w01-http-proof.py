"""Model-free real-process deadline proof; all state stays in this worktree."""
import argparse
import datetime
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import socket
import subprocess
import tempfile
import threading
import time

ROOT = Path(__file__).resolve().parents[1]
spec = importlib.util.spec_from_file_location('harness_stress', ROOT / 'scripts/harness-stress.py')
stress = importlib.util.module_from_spec(spec)
spec.loader.exec_module(stress)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--angel-bin', required=True)
    parser.add_argument('--receipt-dir', type=Path, default=ROOT / 'docs/audits/evidence/2026-09-08-straight-a/W01/cont')
    args = parser.parse_args()
    binary = Path(args.angel_bin).resolve()
    receipts = args.receipt_dir.resolve()
    receipts.relative_to(ROOT)
    receipts.mkdir(parents=True, exist_ok=True)
    stamp = datetime.datetime.now(datetime.timezone.utc).strftime('%Y%m%dT%H%M%S%fZ')
    receipt_path = receipts / f'task-json-{stamp}.json'
    listener = socket.socket()
    listener.bind(('127.0.0.1', 0))
    listener.listen()
    listener.settimeout(12)
    fixture = {'accepted': False, 'client_closed': False}

    def hung():
        try:
            with listener.accept()[0] as client:
                fixture['accepted'] = True
                client.settimeout(12)
                request = b''
                while not request.endswith(b'\r\n\r\n'):
                    request += client.recv(1)
                # Match the measured failure: body-less 404, HTTP/1.1 keepalive.
                client.sendall(b'HTTP/1.1 404 Not Found\r\nContent-Type: text/plain\r\n\r\n')
                fixture['client_closed'] = client.recv(1) == b''
        except (OSError, TimeoutError) as error:
            fixture['error'] = type(error).__name__
        finally:
            listener.close()

    url = f'http://127.0.0.1:{listener.getsockname()[1]}/hung'
    server = threading.Thread(target=hung)
    server.start()

    def policy(hop, _request):
        if hop == 0:
            return ('calls', [('hung-fetch', 'web_fetch', {'url': url})])
        return ('text', 'Unexpected extra model hop after deadline.')

    fixture_root = ROOT / 'cockpit/target/w01-fixtures'
    fixture_root.mkdir(parents=True, exist_ok=True)
    receipt = {'utc_start': stamp, 'binary_sha256': hashlib.sha256(binary.read_bytes()).hexdigest(), 'deadline_secs': 5, 'grace_secs': 2, 'fixture': fixture}
    with tempfile.TemporaryDirectory(dir=fixture_root) as work:
        workspace = Path(work)
        home = workspace / 'home'
        home.mkdir()
        # A minimal environment prevents real credentials or provider discovery.
        env = {
            'PATH': os.environ.get('PATH', ''), 'HOME': str(home),
            'TMPDIR': str(workspace), 'CODEX_HOME': str(home / 'codex'),
            'ANGEL_DRIVER': 'openrouter', 'ANGEL_API_CLUBS': 'openrouter',
            'ANGEL_OPENROUTER_KEY': 'offline-fixture', 'OPENROUTER_API_KEY': 'offline-fixture',
            'ANGEL_OPENROUTER_MODEL': 'offline-fixture',
            'ANGEL_TASK_RECON': '0', 'ANGEL_SKILL_HINT': '0', 'ANGEL_PROJECT_DOC': '0',
            'ANGEL_ADVISOR': '0', 'ANGEL_FIRST_WRITE_CALLS': '0',
            'ANGEL_TASK_STRICT_EXIT': '0', 'ANGEL_STREAM_STALL_SECS': '60',
            'ANGEL_TURN_IDLE_TIMEOUT_SECS': '120', 'ANGEL_YOLO': '1',
        }
        command = [str(binary), '--yolo', '--task-json', '--workspace', str(workspace),
                   '--deadline-secs', '5', '--tool-profile', 'essential', '--rollout', 'off',
                   'Read the fixture URL using web_fetch and report what it returns.']
        receipt['command'] = command
        receipt['environment_names'] = sorted(env)
        with stress.PolicyServer(policy) as provider:
            env['ANGEL_OPENROUTER_URL'] = provider.url
            start = time.monotonic()
            process = subprocess.Popen(command, cwd=workspace, env=env, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
            try:
                stdout, stderr = process.communicate(timeout=12)
            except subprocess.TimeoutExpired:
                process.kill()
                stdout, stderr = process.communicate()
                receipt['proof_watchdog_fired'] = True
            receipt['elapsed_secs'] = time.monotonic() - start
            receipt['exit_code'] = process.returncode
            receipt['provider_requests'] = len(provider.log)
        receipt['stderr'] = stderr
        try:
            receipt['envelope'] = json.loads(stdout)
        except json.JSONDecodeError:
            receipt['stdout'] = stdout
    server.join()
    envelope = receipt.get('envelope', {})
    text = json.dumps(envelope)
    checks = {
        'within_deadline_plus_grace': receipt['elapsed_secs'] <= 7,
        'deadline_stop_reason': envelope.get('stop_reason') == 'deadline',
        'partial_receipt_in_envelope': 'stalled ' in text and 'bytes_received' in text and 'elapsed_ms' in text and 'bound' in text,
        'ledger_transient': any(row.get('tool') == 'web_fetch' and row.get('error_class') == 'Transient' and row.get('avoidable') is False for row in envelope.get('tools', [])),
        'one_scripted_hop': receipt['provider_requests'] == 1,
        'socket_closed': fixture['client_closed'],
        'no_proof_watchdog': not receipt.get('proof_watchdog_fired', False),
    }
    receipt['checks'] = checks
    receipt['utc_end'] = datetime.datetime.now(datetime.timezone.utc).isoformat()
    receipt_path.write_text(json.dumps(receipt, indent=2) + '\n')
    print(f'receipt: {receipt_path.relative_to(ROOT)}')
    print(json.dumps(checks, sort_keys=True))
    assert all(checks.values()), checks
    print('task-json HTTP deadline proof: PASS')


if __name__ == '__main__':
    main()
