"""The convenience gate must not measure a failed or fabricated baseline."""
from pathlib import Path
import subprocess
import sys

import pytest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from tools import review_quick as R


def test_fresh_baseline_runs_real_export_before_review(tmp_path, monkeypatch):
    calls = []
    def export(command, **kwargs):
        calls.append(command)
        assert command[-1] == 'export_worn_native'
        assert kwargs['check'] is True
        assert kwargs['env']['WORN_NATIVE_LEGACY'] == '1'
        path = Path(kwargs['env']['WORN_NATIVE_OUT'])
        path.write_text('{"frames": []}')
    monkeypatch.setattr(R.subprocess, 'run', export)
    def review(game, native, out, **kwargs):
        assert Path(native).exists()
        assert kwargs['rig'] == 'fixup'
        assert kwargs['image'] == 'source'
        return {'ok': True}
    monkeypatch.setattr(R, 'review', review)
    assert R.run('game', 'fixup', 'source', tmp_path)['ok']
    assert len(calls) == 1


def test_failed_export_is_a_prerequisite_failure_not_quality(tmp_path, monkeypatch):
    def fail(*args, **kwargs):
        raise subprocess.CalledProcessError(101, args[0])
    monkeypatch.setattr(R.subprocess, 'run', fail)
    monkeypatch.setattr(R, 'review', lambda *a, **kw: pytest.fail('measured failed baseline'))
    with pytest.raises(subprocess.CalledProcessError):
        R.run('game', 'fixup', 'source', tmp_path)


def test_explicit_native_skips_cargo_not_validation(tmp_path, monkeypatch):
    monkeypatch.setattr(R.subprocess, 'run', lambda *a, **kw: pytest.fail('unexpected export'))
    def review(game, native, out, **kwargs):
        assert native == 'explicit'
        return {'ok': False}
    monkeypatch.setattr(R, 'review', review)
    assert not R.run('game', 'fixup', 'source', tmp_path, native='explicit')['ok']
