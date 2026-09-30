#!/usr/bin/env python3
"""Run an isolated, source-pinned Luna Polyglot cell without editing old runs.

The default resolves the evaluator config only; --execute enables model calls.
Each invocation needs a new output directory. Compare cells with the same task
selection, model, effort and limits; record repeated rounds as separate cells.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import signal
import subprocess
import sys
import tempfile
import time


DEFAULT_BENCH = Path("/home/frosty40/angel_tests/angelX-bench/polyglot-20260921")
SELECTION_SEED = "angelx-polyglot-speed-v1:"
LANGUAGES = ("js", "py", "rust", "cpp")
RESOURCE_DIRECTORIES = ("lib", "cockpit/assets", "cockpit/skills",
                        "cockpit/graphs", "cockpit/personas", "cockpit/src")
HARNESS_ENV = {
    "ANGEL_SPIN_LIMIT": "4",
    "ANGEL_UNPRODUCTIVE_STREAK_STOP": "16",
    "ANGEL_ERROR_LIMIT": "6",
    "ANGEL_VERIFY_BEFORE_DONE": "0",
    "ANGEL_VERIFY_NUDGES": "4",
    "ANGEL_TOOLCALL_STORM": "1",
}


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def tree_manifest(root: Path, ignore_python_cache: bool = True) -> dict[str, str]:
    if root.is_symlink() or not root.is_dir():
        raise ValueError(f"snapshot source is not a plain directory: {root}")
    result = {}
    for path in sorted(root.rglob("*")):
        relative = path.relative_to(root)
        if ignore_python_cache and ("__pycache__" in relative.parts or path.suffix == ".pyc"):
            continue
        if path.is_symlink():
            raise ValueError(f"snapshot contains a symlink: {path}")
        if path.is_file():
            result[str(relative)] = sha256(path)
    return result


def confined_directory(root: Path, relative: str, prefix: str | None = None) -> Path:
    if not isinstance(relative, str) or not relative:
        raise ValueError("snapshot directory must be a nonempty relative path")
    path = Path(relative)
    if (path.is_absolute() or not path.parts or ".." in path.parts
            or (prefix is not None and (len(path.parts) < 2 or path.parts[0] != prefix))):
        raise ValueError(f"unsupported snapshot location: {relative}")
    source = root
    for part in path.parts:
        source /= part
        if source.is_symlink():
            raise ValueError(f"snapshot location contains a symlink: {source}")
    if not source.resolve(strict=True).is_relative_to(root.resolve(strict=True)) or not source.is_dir():
        raise ValueError(f"snapshot location is not a confined directory: {source}")
    return source


def copy_tree_snapshot(source: Path, target: Path, ignore_python_cache: bool = True) -> dict[str, str]:
    expected = tree_manifest(source, ignore_python_cache)
    if not target.exists():
        # Preserve any link introduced during a copy rather than following it.
        # The post-copy manifests then reject the link before evaluation starts.
        shutil.copytree(source, target, symlinks=True,
                        ignore=shutil.ignore_patterns("__pycache__", "*.pyc") if ignore_python_cache else None)
    actual = tree_manifest(target, ignore_python_cache)
    if actual != expected or tree_manifest(source, ignore_python_cache) != expected:
        raise ValueError(f"snapshot source changed or copied bytes differ: {source}")
    return actual


def copy_file_snapshot(source: Path, target: Path) -> str:
    if source.is_symlink() or not source.is_file():
        raise ValueError(f"snapshot source is not a plain file: {source}")
    expected = sha256(source)
    shutil.copy2(source, target, follow_symlinks=False)
    if target.is_symlink() or sha256(target) != expected or sha256(source) != expected:
        raise ValueError(f"file changed while snapshotting: {source}")
    return expected


def json_digest(value: object) -> str:
    return hashlib.sha256(json.dumps(value, sort_keys=True).encode()).hexdigest()


def select_tasks(rows: list[dict], per_language: int, names: list[str]) -> list[dict]:
    indexed = {row["name"]: row for row in rows}
    if len(indexed) != len(rows):
        raise ValueError("duplicate task names in catalog")
    if names:
        if len(set(names)) != len(names):
            raise ValueError("duplicate --task names")
        unknown = set(names) - indexed.keys()
        if unknown:
            raise ValueError(f"unknown tasks: {sorted(unknown)}")
        return [indexed[name] for name in names]
    selected = []
    for language in LANGUAGES:
        group = [row for row in rows if row["name"].startswith(language + "-")]
        group.sort(key=lambda row: hashlib.sha256(
            (SELECTION_SEED + row["name"]).encode()
        ).hexdigest())
        if len(group) < per_language:
            raise ValueError(f"not enough {language} tasks for selection")
        selected.extend(group[:per_language])
    return selected


def snapshot_fixtures(bench: Path, output: Path, selected: list[dict]) -> dict[str, dict[str, str]]:
    manifests = {}
    for row in selected:
        source = confined_directory(bench, row["fixture"], "fixtures")
        # The evaluator deliberately confines fixtures to tasks.json's parent.
        # Symlinking the historical fixture root would violate that contract.
        manifests[row["name"]] = copy_tree_snapshot(source, output / row["fixture"], ignore_python_cache=False)
    return manifests


def snapshot_gold(bench: Path, private_root: Path, selected: list[dict]) -> dict[str, dict[str, str]]:
    manifests = {}
    gold = confined_directory(bench, "gold")
    for row in selected:
        source = confined_directory(gold, row["gold"])
        manifests[row["name"]] = copy_tree_snapshot(source, private_root / "gold" / row["gold"], ignore_python_cache=False)
    return manifests


def checked_resource_source(source: Path, expected_digest: str) -> dict[str, str]:
    top = subprocess.check_output(["git", "-C", str(source), "rev-parse", "--show-toplevel"], text=True, timeout=30).strip()
    if Path(top).resolve(strict=True) != source.resolve(strict=True):
        raise ValueError("resource root must be the source checkout's top-level directory")
    script = source / "scripts/check/cockpit-source-digest.sh"
    if not script.is_file() or script.is_symlink():
        raise ValueError("resource checkout has no plain cockpit-source-digest.sh")
    script = confined_directory(source, "scripts/check") / script.name
    trusted_script = Path(__file__).resolve().parent / "check/cockpit-source-digest.sh"
    if trusted_script.is_symlink() or not trusted_script.is_file():
        raise ValueError("runner has no plain trusted cockpit-source-digest.sh")
    trusted_digest = sha256(trusted_script)
    if sha256(script) != trusted_digest:
        raise ValueError("resource digest helper does not match the runner's trusted helper; unsupported helper version")
    # Run the supported helper at its resource path: BASH_SOURCE must resolve
    # the candidate checkout, rather than the runner's own source inventory.
    result = subprocess.run(["bash", str(script)], cwd=source, text=True,
                            capture_output=True, timeout=60)
    confined_directory(source, "scripts/check")
    if script.is_symlink() or sha256(script) != trusted_digest:
        raise ValueError("resource digest helper changed while verifying source")
    if result.returncode:
        raise ValueError(f"resource checkout is not source-bound: {result.stderr.strip()}")
    digest = result.stdout.strip()
    if not re.fullmatch(r"[0-9a-f]{64}", digest) or digest != expected_digest:
        raise ValueError(f"resource checkout does not match binary source pin: expected={expected_digest} actual={digest}")
    revision = subprocess.check_output(["git", "-C", str(source), "rev-parse", "HEAD"], text=True, timeout=30).strip()
    return {"revision": revision, "cockpit_source_sha256": digest, "digest_script_sha256": trusted_digest}


def snapshot_resources(source_root: Path, target: Path) -> dict[str, str]:
    # Retain the scripts tree's tracked support tools without carrying ignored
    # benchmark wires, dependency installs or caches into a runtime bundle.
    tracked = subprocess.check_output(["git", "-C", str(source_root), "ls-files", "-z", "--", "scripts"], timeout=30)
    script_paths = [Path(os.fsdecode(name)) for name in tracked.split(b"\0") if name]
    script_manifest = {}
    for relative in script_paths:
        parent = confined_directory(source_root, str(relative.parent))
        source = parent / relative.name
        destination = target / relative
        destination.parent.mkdir(parents=True, exist_ok=True)
        script_manifest[str(relative)] = copy_file_snapshot(source, destination)
    for relative in script_paths:
        if sha256(source_root / relative) != script_manifest[str(relative)]:
            raise ValueError(f"script changed while snapshotting: {source_root / relative}")
    for relative in RESOURCE_DIRECTORIES:
        if (source_root / relative).is_dir():
            copy_tree_snapshot(confined_directory(source_root, relative), target / relative)
    (target / "cockpit").mkdir(parents=True, exist_ok=True)
    copy_file_snapshot(source_root / "cockpit/Cargo.toml", target / "cockpit/Cargo.toml")
    return tree_manifest(target)


def checked_build_info(binary: Path) -> dict:
    info = json.loads(subprocess.check_output(
        [str(binary), "--build-info", "--json"], text=True, timeout=30
    ))
    if not re.fullmatch(r"[0-9a-f]{64}", info.get("cockpit_source_sha256", "")):
        raise ValueError("binary has no bound cockpit_source_sha256; rebuild with a source pin")
    if info.get("executable_sha256") != sha256(binary):
        raise ValueError("binary build-info digest does not match its bytes")
    return info


def evaluator_environment(bench: Path, package_root: Path, model: str,
                          effort: str, wall: int, codex_home: Path) -> dict[str, str]:
    real_home = Path.home()
    agent_path = f"{bench}/toolbin:/usr/local/bin:/usr/bin:/bin"
    # An allowlist keeps host credentials, feature switches and operator config
    # out of the evaluator and each agent. The direct Codex seat uses its own
    # ephemeral auth directory; the proxy placeholder is not a real credential.
    return {
        "HOME": str(real_home),
        "USER": real_home.name,
        "LOGNAME": real_home.name,
        "LANG": "C.UTF-8",
        "TERM": "dumb",
        "PATH": f"{agent_path}:{bench}/prime_v1/.venv/bin",
        "PYTHONPATH": str(package_root),
        "PYTHONDONTWRITEBYTECODE": "1",
        "OPENAI_API_KEY": "unused-luna-direct",
        "LUNA_MODEL": model,
        "LUNA_EFFORT": effort,
        "LUNA_CODEX_HOME": str(codex_home),
        "PRIME_AGENT_ISOLATION": "fresh-home",
        "PRIME_AGENT_PATH": agent_path,
        "PRIME_REAL_HOME": str(real_home),
        "PRIME_AGENT_WALL_SECS": str(wall),
        "PRIME_AGENT_GRADE_ANY_EXIT": "1",
    }


def wait_evaluator_exit(process: subprocess.Popen, timeout: float, cancelled_signal=None) -> int:
    """Observe our direct child without releasing the session leader's PID."""
    deadline = time.monotonic() + timeout
    while True:
        if cancelled_signal is not None:
            signum = cancelled_signal()
            if signum is not None:
                raise EvaluatorCancelled(signum)
        if process.returncode is not None:
            raise ChildProcessError("evaluator leader was already reaped")
        observed = os.waitid(os.P_PID, process.pid, os.WEXITED | os.WNOHANG | os.WNOWAIT)
        if observed is not None:
            return observed.si_status if observed.si_code == os.CLD_EXITED else -observed.si_status
        remaining = deadline - time.monotonic()
        if remaining <= 0:
            raise subprocess.TimeoutExpired(process.args, timeout)
        time.sleep(min(.01, remaining))


def terminate_group(process: subprocess.Popen, grace_secs: float = 15) -> None:
    """Retire an owned session before consuming its leader's wait status."""
    if process.returncode is not None:
        return  # A consumed status no longer proves ownership of this PGID.
    try:
        os.waitid(os.P_PID, process.pid, os.WEXITED | os.WNOHANG | os.WNOWAIT)
    except ChildProcessError:
        return  # Another reaper released the PID; never signal a recycled group.
    try:
        os.killpg(process.pid, signal.SIGTERM)
    except ProcessLookupError:
        process.wait()
        return
    try:
        wait_evaluator_exit(process, grace_secs)
    except subprocess.TimeoutExpired:
        pass
    # The unreaped direct child still reserves the leader PID/PGID. Descendants
    # may outlive it or ignore TERM; signal the group before releasing that PID.
    try:
        os.killpg(process.pid, signal.SIGKILL)
    except ProcessLookupError:
        pass
    process.wait()


class EvaluatorCancelled(BaseException):
    def __init__(self, signum):
        self.signum = signum


def run_evaluator(command, *, env, cwd, log, timeout, manifest, grace_secs=15, cleanup=None):
    """Own the evaluator session across normal exit, timeout and PID signals."""
    process = None
    interrupts = (signal.SIGINT, signal.SIGTERM)
    previous_handlers = {sig: signal.getsignal(sig) for sig in interrupts}
    # Do not deliver cancellation between spawning and recording the group.
    previous_mask = signal.pthread_sigmask(signal.SIG_BLOCK, interrupts)
    cancellation = None

    def cancelled(signum, _frame):
        nonlocal cancellation
        # Record cancellation rather than throwing from an arbitrary cleanup
        # instruction. The owned-child observation loop handles the first signal.
        if cancellation is None:
            cancellation = signum

    try:
        for sig in interrupts:
            signal.signal(sig, cancelled)
        try:
            try:
                # The spawn mask is inherited across exec. Restore the caller's
                # mask in this same-PID launcher before running the evaluator,
                # so TERM reaches its ordinary handler during cleanup.
                launcher = ("import os,signal,sys; "
                            f"signal.pthread_sigmask(signal.SIG_SETMASK, {json.dumps(sorted(previous_mask))}); "
                            "signal.signal(signal.SIGINT, signal.SIG_DFL); "
                            "signal.signal(signal.SIGTERM, signal.SIG_DFL); "
                            "os.execvpe(sys.argv[1], sys.argv[1:], os.environ)")
                process = subprocess.Popen([sys.executable, "-c", launcher, *command], env=env, cwd=cwd, stdout=log,
                                           stderr=subprocess.STDOUT, start_new_session=True)
            finally:
                signal.pthread_sigmask(signal.SIG_SETMASK, previous_mask)
            return wait_evaluator_exit(process, timeout, lambda: cancellation)
        except subprocess.TimeoutExpired:
            manifest["runner_timed_out"] = True
            return 124
        except KeyboardInterrupt:
            manifest["runner_interrupted"] = True
            return 130
        except EvaluatorCancelled as interruption:
            manifest["runner_terminated_signal"] = signal.Signals(interruption.signum).name
            if interruption.signum == signal.SIGINT:
                manifest["runner_interrupted"] = True
            return 128 + interruption.signum
        finally:
            # Further signals cannot interrupt owned-group and HOME cleanup.
            for sig in interrupts:
                signal.signal(sig, signal.SIG_IGN)
            try:
                if process is not None:
                    terminate_group(process, grace_secs)
            finally:
                if cleanup is not None:
                    cleanup()
    finally:
        for sig, handler in previous_handlers.items():
            signal.signal(sig, handler)
        signal.pthread_sigmask(signal.SIG_SETMASK, previous_mask)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--bench-root", type=Path, default=DEFAULT_BENCH)
    parser.add_argument("--output", type=Path, required=True,
                        help="new cell directory; existing directories are rejected")
    parser.add_argument("--angel-bin", type=Path, required=True)
    parser.add_argument("--sandbox-bin", type=Path,
                        help="defaults to angel-sandbox beside --angel-bin")
    parser.add_argument("--resource-root", type=Path, required=True,
                        help="source checkout matching the binary; shipped resources are copied per cell")
    parser.add_argument("--model", default="gpt-6-luna")
    parser.add_argument("--effort", choices=("none", "low", "medium", "high", "max"),
                        default="medium")
    parser.add_argument("--per-language", type=int, default=3)
    parser.add_argument("--task", action="append", default=[],
                        help="explicit cohort instead of deterministic language-balanced pilot")
    parser.add_argument("--wall-secs", type=int, default=600)
    parser.add_argument("--max-hops", type=int, default=60)
    parser.add_argument("--openai-replay", choices=("on", "off"),
                        help="explicit native Responses replay pin; omitted preserves the binary default")
    parser.add_argument("--execute", action="store_true",
                        help="enable real model calls; default is config-only validation")
    args = parser.parse_args()
    if args.per_language < 1 or args.wall_secs < 1 or args.max_hops < 6:
        parser.error("per-language/wall must be positive and max-hops must be at least 6")
    bench = args.bench_root.expanduser().resolve(strict=True)
    binary = args.angel_bin.expanduser().resolve(strict=True)
    helper = (args.sandbox_bin or binary.with_name("angel-sandbox")).expanduser().resolve(strict=True)
    resource_source = args.resource_root.expanduser().resolve(strict=True)
    output = args.output.expanduser().resolve()
    if output.exists():
        parser.error(f"output already exists; choose a new cell directory: {output}")
    if output == bench or bench in output.parents:
        parser.error("keep new cells outside the historical benchmark directory")
    build_info = checked_build_info(binary)
    resource_identity = checked_resource_source(resource_source, build_info["cockpit_source_sha256"])
    task_path = bench / "tasks-polyglot-v1.json"
    catalog_bytes = task_path.read_bytes()
    catalog = json.loads(catalog_bytes)
    selected = select_tasks(catalog, args.per_language, args.task)
    # Preserve exact public rows and their relative fixture layout.
    python = bench / "prime_v1/.venv/bin/python"
    evaluator = bench / "prime_v1/.venv/bin/eval"
    adapter = bench / "prime_v1/angel_action_v1"
    if not all(path.exists() for path in (python, evaluator, adapter)):
        raise ValueError("benchmark evaluator installation is incomplete")
    output.mkdir(parents=True, mode=0o700)
    package_root = output / "evaluator"
    copy_tree_snapshot(confined_directory(bench, "prime_v1/angel_action_v1"), package_root / "angel_action_v1")
    fixture_files = snapshot_fixtures(bench, output, selected)
    private_grading = output / "private-grading"
    private_grading.mkdir(mode=0o700)
    gold_files = snapshot_gold(bench, private_grading, selected)
    grading_sha256 = copy_file_snapshot(bench / "grading.json", private_grading / "grading.json")
    grade_script_sha256 = copy_file_snapshot(bench / "grade.py", private_grading / "grade.py")
    tasks = output / "tasks.json"
    tasks.write_text(json.dumps(selected, indent=2) + "\n")
    pin = output / "bin"
    pin.mkdir()
    pinned_binary = pin / "angel"
    pinned_helper = pin / "angel-sandbox"
    binary_sha256 = copy_file_snapshot(binary, pinned_binary)
    sandbox_sha256 = copy_file_snapshot(helper, pinned_helper)
    pinned_binary.chmod(0o500)
    pinned_helper.chmod(0o500)
    if binary_sha256 != build_info["executable_sha256"]:
        raise ValueError("binary changed while snapshotting")
    resources = output / "resources"
    resource_files = snapshot_resources(resource_source, resources)
    if checked_resource_source(resource_source, build_info["cockpit_source_sha256"]) != resource_identity:
        raise ValueError("resource checkout identity changed while snapshotting")
    config = output / "client.toml"
    config.write_text(
        f"model = {json.dumps(args.model)}\n\n[client]\n"
        'base_url = "https://chatgpt.com/backend-api/codex"\n'
        'api_key_var = "OPENAI_API_KEY"\n\n[sampling]\n'
        'temperature = 1.0\nmax_tokens = 8192\n'
        f"reasoning_effort = {json.dumps(args.effort)}\n"
    )
    command = [
        str(evaluator), "angel-action-v1", "@", str(config),
        "--env.taskset.id", "angel-action-v1",
        "--env.taskset.tasks-path", str(tasks),
        "--env.taskset.gold-root", str(private_grading / "gold"),
        "--env.agent.runtime.type", "subprocess",
        "--env.agent.harness.id", "angel-action-v1",
        "--env.agent.harness.angel-bin", str(pinned_binary),
        "--env.agent.harness.angel-bin-sha256", build_info["executable_sha256"],
        "--env.agent.harness.angel-cockpit-source-sha256", build_info["cockpit_source_sha256"],
        "--env.agent.harness.route", "luna-direct",
        "--env.agent.harness.moa-max-tokens", "8192",
        "--env.agent.harness.reasoning-effort", args.effort,
        "--env.agent.harness.max-hops", str(args.max_hops),
    ]
    harness_env = {**HARNESS_ENV}
    if args.openai_replay is not None:
        harness_env["ANGEL_OPENAI_REPLAY"] = "1" if args.openai_replay == "on" else "0"
    for key, value in harness_env.items():
        command += [f"--env.agent.harness.env.{key}", value]
    command += ["--env.agent.harness.env.ANGEL_RESOURCE_DIR", str(resources),
                "--env.agent.harness.env.ANGEL_SANDBOX_HELPER", str(pinned_helper)]
    command += ["--env.agent.harness.env.ANGEL_WIRE_LOG_DIR", str(output / "run/wire"),
                "-n", str(len(selected)), "-r", "1", "-c", "1",
                "--no-serve", "--no-rich", "--no-push", "-o", str(output), "--run.dir", "run"]
    if not args.execute:
        command += ["--dry-run", "True"]
    manifest = {
        "schema": "angelx-polyglot-loop/v1", "mode": "execute" if args.execute else "dry-run",
        "catalog_sha256": hashlib.sha256(catalog_bytes).hexdigest(),
        "grading_sha256": grading_sha256,
        "grade_script_sha256": grade_script_sha256,
        "selected_rows_sha256": json_digest(selected),
        "selection": "explicit" if args.task else SELECTION_SEED,
        "task_names": [row["name"] for row in selected],
        "fixture_files": fixture_files,
        "gold_files": gold_files,
        "model": args.model, "effort": args.effort, "wall_secs": args.wall_secs,
        "max_hops": args.max_hops, "max_tokens": 8192, "attempts": 1, "concurrency": 1,
        "output_budget_note": "8192 is the evaluator sampling record; the direct Codex endpoint uses a plan-managed output budget and accepts no output-token cap.",
        "harness_env": harness_env, "binary": build_info,
        "sandbox_sha256": sandbox_sha256,
        "resource_files_sha256": json_digest(resource_files),
        "resource_source": str(resource_source),
        "resource_source_identity": resource_identity,
        "resource_source_note": "The binary source digest binds the clean build-input inventory; copied auxiliary resources and evaluator inputs are hashed independently.",
        "adapter_files": tree_manifest(package_root),
        "runner_sha256": sha256(Path(__file__)),
        "command": command,
        "sampling_note": "No model seed is claimed; paired cells hold settings and task order fixed.",
        "cache_note": "Compile/download caches are shared, as in the original evaluation. Provider prompt caches may be warm across cells.",
    }
    manifest_path = output / "manifest.json"
    manifest_path.write_text(json.dumps(manifest, indent=2) + "\n")
    print(f"{manifest['mode']} model={args.model} effort={args.effort} tasks={len(selected)} output={output}", flush=True)
    codex_directory = tempfile.TemporaryDirectory(prefix="angelx-polyglot-codex-")
    with codex_directory as temporary:
        codex_home = Path(temporary)
        if args.execute:
            source_auth = Path.home() / ".codex/auth.json"
            shutil.copyfile(source_auth, codex_home / "auth.json")
            (codex_home / "auth.json").chmod(0o600)
            cache = Path.home() / ".codex/models_cache.json"
            if cache.is_file():
                shutil.copyfile(cache, codex_home / "models_cache.json")
            (codex_home / "config.toml").write_text(
                f"model = {json.dumps(args.model)}\nmodel_reasoning_effort = {json.dumps(args.effort)}\n"
            )
        env = evaluator_environment(bench, package_root, args.model, args.effort, args.wall_secs, codex_home)
        # Verify Python resolved the copied adapter, not the editable historical
        # package in site-packages. Record the real evaluator installation too.
        inspection = json.loads(subprocess.check_output([
            str(python), "-c",
            "import json,sys,importlib.metadata,angel_action_v1,verifiers.v1; "
            "from angel_action_v1 import AngelActionTaskset,AngelActionTasksetConfig; "
            "tasks=AngelActionTaskset(AngelActionTasksetConfig(tasks_path=sys.argv[1],gold_root=sys.argv[2])).load_tasks(); "
            "print(json.dumps({'adapter':angel_action_v1.__file__, "
            "'verifiers':verifiers.v1.__file__, 'version':importlib.metadata.version('verifiers'), "
            "'loaded_task_names':[t.data.name for t in tasks]}))",
            str(tasks), str(private_grading / "gold"),
        ], env=env, cwd=package_root, text=True, timeout=60))
        if not Path(inspection["adapter"]).is_relative_to(package_root):
            raise ValueError("evaluator imported the historical adapter instead of its snapshot")
        if inspection["loaded_task_names"] != manifest["task_names"]:
            raise ValueError("evaluator task load did not preserve the selected cohort and order")
        manifest["evaluator"] = inspection
        manifest["evaluator_files_sha256"] = json_digest(tree_manifest(Path(inspection["verifiers"]).parent))
        manifest["started_at_unix"] = time.time()
        manifest_path.write_text(json.dumps(manifest, indent=2) + "\n")
        timeout = len(selected) * (args.wall_secs + 120) + 120 if args.execute else 120
        with (output / "eval.log").open("w") as log:
            status = run_evaluator(command, env=env, cwd=package_root, log=log,
                                   timeout=timeout, manifest=manifest, cleanup=codex_directory.cleanup)
        manifest["finished_at_unix"] = time.time()
        manifest["exit_code"] = status
        manifest_path.write_text(json.dumps(manifest, indent=2) + "\n")
        print(f"exit={status} log={output / 'eval.log'}", flush=True)
        return status


if __name__ == "__main__":
    raise SystemExit(main())
