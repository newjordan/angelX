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
import tempfile
import time


DEFAULT_BENCH = Path("/home/frosty40/angel_tests/angelX-bench/polyglot-20260921")
SELECTION_SEED = "angelx-polyglot-speed-v1:"
LANGUAGES = ("js", "py", "rust", "cpp")
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


def tree_manifest(root: Path) -> dict[str, str]:
    return {
        str(path.relative_to(root)): sha256(path)
        for path in sorted(root.rglob("*"))
        if path.is_file() and "__pycache__" not in path.parts
    }


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


def snapshot_fixtures(bench: Path, output: Path, selected: list[dict]) -> None:
    for row in selected:
        fixture = Path(row["fixture"])
        if fixture.is_absolute() or ".." in fixture.parts or fixture.parts[0] != "fixtures":
            raise ValueError(f"unsupported fixture location for {row['name']}")
        # The evaluator deliberately confines fixtures to tasks.json's parent.
        # Symlinking the historical fixture root would violate that contract.
        target = output / fixture
        if not target.exists():
            shutil.copytree(bench / fixture, target)


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


def terminate_group(process: subprocess.Popen) -> None:
    try:
        os.killpg(process.pid, signal.SIGTERM)
    except ProcessLookupError:
        return
    try:
        process.wait(timeout=15)
    except subprocess.TimeoutExpired:
        try:
            os.killpg(process.pid, signal.SIGKILL)
        except ProcessLookupError:
            pass
        process.wait()


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
    task_path = bench / "tasks-polyglot-v1.json"
    catalog = json.loads(task_path.read_text())
    selected = select_tasks(catalog, args.per_language, args.task)
    # Preserve exact public rows and their relative fixture layout.
    python = bench / "prime_v1/.venv/bin/python"
    evaluator = bench / "prime_v1/.venv/bin/eval"
    adapter = bench / "prime_v1/angel_action_v1"
    if not all(path.exists() for path in (python, evaluator, adapter)):
        raise ValueError("benchmark evaluator installation is incomplete")
    output.mkdir(parents=True, mode=0o700)
    package_root = output / "evaluator"
    shutil.copytree(adapter, package_root / "angel_action_v1",
                    ignore=shutil.ignore_patterns("__pycache__", "*.pyc"))
    snapshot_fixtures(bench, output, selected)
    tasks = output / "tasks.json"
    tasks.write_text(json.dumps(selected, indent=2) + "\n")
    pin = output / "bin"
    pin.mkdir()
    pinned_binary = pin / "angel"
    pinned_helper = pin / "angel-sandbox"
    shutil.copy2(binary, pinned_binary)
    shutil.copy2(helper, pinned_helper)
    pinned_binary.chmod(0o500)
    pinned_helper.chmod(0o500)
    if sha256(pinned_binary) != build_info["executable_sha256"]:
        raise ValueError("binary changed while snapshotting")
    resources = output / "resources"
    for relative in ("scripts", "lib", "cockpit/assets", "cockpit/skills", "cockpit/graphs",
                     "cockpit/personas", "cockpit/src"):
        source = resource_source / relative
        if source.is_dir():
            shutil.copytree(source, resources / relative,
                            ignore=shutil.ignore_patterns("__pycache__", "*.pyc"))
    shutil.copy2(resource_source / "cockpit/Cargo.toml", resources / "cockpit/Cargo.toml")
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
        "--env.taskset.gold-root", str(bench / "gold"),
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
        "catalog_sha256": sha256(task_path),
        "grading_sha256": sha256(bench / "grading.json"),
        "grade_script_sha256": sha256(bench / "grade.py"),
        "selected_rows_sha256": json_digest(selected),
        "selection": "explicit" if args.task else SELECTION_SEED,
        "task_names": [row["name"] for row in selected],
        "fixture_files": {row["name"]: tree_manifest(bench / row["fixture"]) for row in selected},
        "gold_files": {row["name"]: tree_manifest(bench / "gold" / row["gold"]) for row in selected},
        "model": args.model, "effort": args.effort, "wall_secs": args.wall_secs,
        "max_hops": args.max_hops, "max_tokens": 8192, "attempts": 1, "concurrency": 1,
        "output_budget_note": "8192 is the evaluator sampling record; the direct Codex endpoint uses a plan-managed output budget and accepts no output-token cap.",
        "harness_env": harness_env, "binary": build_info,
        "sandbox_sha256": sha256(pinned_helper),
        "resource_files_sha256": json_digest(tree_manifest(resources)),
        "resource_source": str(resource_source),
        "adapter_files": tree_manifest(package_root),
        "runner_sha256": sha256(Path(__file__)),
        "command": command,
        "sampling_note": "No model seed is claimed; paired cells hold settings and task order fixed.",
        "cache_note": "Compile/download caches are shared, as in the original evaluation. Provider prompt caches may be warm across cells.",
    }
    manifest_path = output / "manifest.json"
    manifest_path.write_text(json.dumps(manifest, indent=2) + "\n")
    print(f"{manifest['mode']} model={args.model} effort={args.effort} tasks={len(selected)} output={output}", flush=True)
    with tempfile.TemporaryDirectory(prefix="angelx-polyglot-codex-") as temporary:
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
            str(tasks), str(bench / "gold"),
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
            process = subprocess.Popen(command, env=env, cwd=package_root, stdout=log,
                                       stderr=subprocess.STDOUT, start_new_session=True)
            try:
                status = process.wait(timeout=timeout)
            except subprocess.TimeoutExpired:
                terminate_group(process)
                status = 124
                manifest["runner_timed_out"] = True
            except KeyboardInterrupt:
                terminate_group(process)
                status = 130
                manifest["runner_interrupted"] = True
        manifest["finished_at_unix"] = time.time()
        manifest["exit_code"] = status
        manifest_path.write_text(json.dumps(manifest, indent=2) + "\n")
        print(f"exit={status} log={output / 'eval.log'}", flush=True)
        return status


if __name__ == "__main__":
    raise SystemExit(main())
