#!/usr/bin/env python3
"""Source-pinned live Sloptomizer pilot: off/on, then on/off on the same tasks.

Uses the existing Polyglot evaluator and grading contract. Default is a dry run;
--execute permits real calls to the operator's selected, configured endpoint.
Only the live-memory flag differs between arms. Fresh task homes make this a
cold-start, within-task comparison, not a claim about cross-session transfer.
"""
from __future__ import annotations

import argparse
import importlib.util
import json
from pathlib import Path
import subprocess
import time
import tomllib


SCRIPT = Path(__file__).resolve()
spec = importlib.util.spec_from_file_location("polyglot_snapshot", SCRIPT.parents[1] / "run-polyglot-loop.py")
shared = importlib.util.module_from_spec(spec)
spec.loader.exec_module(shared)

PROVIDERS = {
    "glm": ("glm-flash.toml", "glm-direct", "ZAI_API_KEY"),
    "deepseek": ("deepseek-flash.toml", "deepseek-direct", "DEEPSEEK_API_KEY"),
}


def write_json(path, value):
    temp = path.with_suffix(path.suffix + ".tmp")
    temp.write_text(json.dumps(value, indent=2, sort_keys=True) + "\n")
    temp.replace(path)


def prepare(args):
    source = args.source.resolve(strict=True)
    bench = args.bench_root.resolve(strict=True)
    output = args.output.resolve()
    if output.exists():
        raise ValueError("output already exists; previous evidence must stay intact")
    if output.is_relative_to(bench):
        raise ValueError("new evidence belongs outside the historical benchmark tree")
    binary = args.angel_bin.resolve(strict=True)
    helper = args.sandbox_bin.resolve(strict=True)
    info = shared.checked_build_info(binary)
    identity = shared.checked_resource_source(source, info["cockpit_source_sha256"])
    output.mkdir(parents=True, mode=0o700)
    pin = output / "bin"
    pin.mkdir()
    for original, name in [(binary, "angel"), (helper, "angel-sandbox")]:
        shared.copy_file_snapshot(original, pin / name)
        (pin / name).chmod(0o500)
    catalog_path = bench / "tasks-polyglot-v1.json"
    catalog = json.loads(catalog_path.read_text())
    selected = shared.select_tasks(catalog, 1, [])
    # Predeclared stress cases from the existing loop-prone cohort. Keep them
    # labelled; they are not a representative estimate of the full 136 tasks.
    selected += [next(row for row in catalog if row["name"] == name)
                 for name in ("py-react", "py-transpose")]
    fixtures = shared.snapshot_fixtures(bench, output, selected)
    grading = output / "private-grading"
    grading.mkdir(mode=0o700)
    gold = shared.snapshot_gold(bench, grading, selected)
    for name in ("grade.py", "grading.json"):
        shared.copy_file_snapshot(bench / name, grading / name)
    write_json(output / "tasks.json", selected)
    package = output / "evaluator"
    shared.copy_tree_snapshot(bench / "prime_v1/angel_action_v1", package / "angel_action_v1")
    resources = shared.snapshot_resources(source, output / "resources")
    if shared.checked_resource_source(source, info["cockpit_source_sha256"]) != identity:
        raise ValueError("source changed while preparing the experiment")
    config_name, route, credential_var = PROVIDERS[args.provider]
    shared.copy_file_snapshot(bench / "configs" / config_name, output / "client.toml")
    config = tomllib.loads((output / "client.toml").read_text())
    if config["client"]["api_key_var"] != credential_var:
        raise ValueError("provider configuration has an unexpected credential variable")
    manifest = {
        "schema": "angel.sloptomizer-ab/v1",
        "mode": "execute" if args.execute else "dry-run",
        "hypothesis": "Compact live check relationships reduce repeated work without reducing task success.",
        "falsification": "Lower correctness, or more calls/tokens/time without a compensating success gain.",
        "selection": {"balanced": shared.SELECTION_SEED, "per_language": 1,
                      "diagnostic": ["py-react", "py-transpose"]},
        "task_names": [row["name"] for row in selected],
        "catalog_sha256": shared.sha256(catalog_path),
        "selected_rows_sha256": shared.json_digest(selected),
        "fixture_files": fixtures, "gold_files": gold,
        "grading_sha256": shared.sha256(grading / "grading.json"),
        "grade_script_sha256": shared.sha256(grading / "grade.py"),
        "binary": info, "sandbox_sha256": shared.sha256(pin / "angel-sandbox"),
        "resource_source_identity": identity,
        "resource_files_sha256": shared.json_digest(resources),
        "adapter_files": shared.tree_manifest(package),
        "runner_sha256": shared.sha256(SCRIPT),
        "config_sha256": shared.sha256(output / "client.toml"),
        "provider": args.provider, "route": route, "model": config["model"],
        "endpoint": config["client"]["base_url"], "credential_var": credential_var,
        "effort": config["sampling"]["reasoning_effort"],
        "max_tokens": config["sampling"]["max_tokens"],
        "wall_secs": args.wall_secs, "max_hops": 60, "attempts": 1, "concurrency": 1,
        "order": ["r1-off", "r1-on", "r2-on", "r2-off"],
        "scope": "Cold-start per-task evidence; no seeded memory or cross-session/model-transfer claim.",
        "cache_note": "Compile/download and provider caches can be warm; arms are interleaved AB/BA.",
        "sampling_note": "Recorded provider sampling is identical; no deterministic model seed is claimed.",
    }
    write_json(output / "manifest.json", manifest)
    return output, bench, manifest


def environment(bench, output, manifest, execute):
    agent_path = f"{bench}/toolbin:/usr/local/bin:/usr/bin:/bin"
    key = "dry-run-placeholder"
    if execute:
        # Match the established run-cell.sh credential boundary. The value is
        # never written to manifests or command lines; the agent gets only its
        # evaluator proxy credential, in a fresh HOME.
        key = subprocess.check_output([
            "bash", "-c",
            'source "$HOME/.config/host_env/api_keys.sh" >/dev/null 2>&1; printf %s "${!1:-}"',
            "sloptomizer-credential", manifest["credential_var"],
        ], text=True).strip()
        if not key:
            raise ValueError("configured provider credential is unavailable")
    return {
        "HOME": str(Path.home()), "USER": Path.home().name, "LOGNAME": Path.home().name,
        "LANG": "C.UTF-8", "TERM": "dumb",
        "PATH": f"{agent_path}:{bench}/prime_v1/.venv/bin",
        "PYTHONPATH": str(output / "evaluator"), "PYTHONDONTWRITEBYTECODE": "1",
        manifest["credential_var"]: key, "PRIME_AGENT_ISOLATION": "fresh-home",
        "PRIME_AGENT_PATH": agent_path, "PRIME_REAL_HOME": str(Path.home()),
        "PRIME_AGENT_WALL_SECS": str(manifest["wall_secs"]), "PRIME_AGENT_GRADE_ANY_EXIT": "1",
    }


def run_cell(bench, output, manifest, name, env, execute):
    cell = output / name
    cell.mkdir(mode=0o700)
    enabled = name.endswith("-on")
    harness_env = {
        **shared.HARNESS_ENV,
        "ANGEL_SLOPTOMIZER_LIVE": "1" if enabled else "0",
        "ANGEL_TASK_HANDOFF_WARM": "0",
        "ANGEL_RESOURCE_DIR": str(output / "resources"),
        "ANGEL_SANDBOX_HELPER": str(output / "bin/angel-sandbox"),
        "ANGEL_WIRE_LOG_DIR": str(cell / "run/wire"),
    }
    command = [
        str(bench / "prime_v1/.venv/bin/eval"), "angel-action-v1", "@", str(output / "client.toml"),
        "--env.taskset.id", "angel-action-v1", "--env.taskset.tasks-path", str(output / "tasks.json"),
        "--env.taskset.gold-root", str(output / "private-grading/gold"),
        "--env.agent.runtime.type", "subprocess", "--env.agent.harness.id", "angel-action-v1",
        "--env.agent.harness.angel-bin", str(output / "bin/angel"),
        "--env.agent.harness.angel-bin-sha256", manifest["binary"]["executable_sha256"],
        "--env.agent.harness.angel-cockpit-source-sha256", manifest["binary"]["cockpit_source_sha256"],
        "--env.agent.harness.route", manifest["route"],
        "--env.agent.harness.reasoning-effort", manifest["effort"],
        "--env.agent.harness.moa-max-tokens", str(manifest["max_tokens"]),
        "--env.agent.harness.max-hops", str(manifest["max_hops"]),
    ]
    for key, value in harness_env.items():
        command.extend([f"--env.agent.harness.env.{key}", value])
    command.extend(["-n", str(len(manifest["task_names"])), "-r", "1", "-c", "1",
                    "--no-serve", "--no-rich", "--no-push", "-o", str(cell), "--run.dir", "run"])
    if not execute:
        command.extend(["--dry-run", "True"])
    receipt = {**manifest, "arm": name, "harness_env": harness_env, "command": command,
               "started_at_unix": time.time()}
    write_json(cell / "manifest.json", receipt)
    print(json.dumps({"cell": name, "tasks": len(manifest["task_names"]), "status": "started"}), flush=True)
    with (cell / "eval.log").open("w") as log:
        status = shared.run_evaluator(
            command, env=env, cwd=output / "evaluator", log=log,
            timeout=len(manifest["task_names"]) * (manifest["wall_secs"] + 120) + 120 if execute else 120,
            manifest=receipt,
        )
    receipt.update(exit_code=status, finished_at_unix=time.time())
    write_json(cell / "manifest.json", receipt)
    print(json.dumps({"cell": name, "status": "finished", "exit_code": status}), flush=True)
    return status


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source", type=Path, required=True)
    parser.add_argument("--provider", choices=sorted(PROVIDERS), required=True)
    parser.add_argument("--angel-bin", type=Path, required=True)
    parser.add_argument("--sandbox-bin", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--bench-root", type=Path, default=shared.DEFAULT_BENCH)
    parser.add_argument("--wall-secs", type=int, default=600)
    parser.add_argument("--execute", action="store_true")
    args = parser.parse_args()
    if args.wall_secs < 1:
        parser.error("wall-secs must be positive")
    output, bench, manifest = prepare(args)
    env = environment(bench, output, manifest, args.execute)
    python = bench / "prime_v1/.venv/bin/python"
    inspection = json.loads(subprocess.check_output([
        str(python), "-c",
        "import json,sys,importlib.metadata,angel_action_v1,verifiers.v1; "
        "from angel_action_v1 import AngelActionTaskset,AngelActionTasksetConfig; "
        "tasks=AngelActionTaskset(AngelActionTasksetConfig(tasks_path=sys.argv[1],gold_root=sys.argv[2])).load_tasks(); "
        "print(json.dumps({'adapter':angel_action_v1.__file__,'verifiers':verifiers.v1.__file__,"
        "'version':importlib.metadata.version('verifiers'),'tasks':[t.data.name for t in tasks]}))",
        str(output / "tasks.json"), str(output / "private-grading/gold"),
    ], env=env, cwd=output / "evaluator", text=True, timeout=60))
    if (not Path(inspection["adapter"]).is_relative_to(output / "evaluator")
            or inspection["tasks"] != manifest["task_names"]):
        raise ValueError("evaluator did not load the pinned adapter and exact cohort")
    manifest["evaluator"] = inspection
    manifest["evaluator_files_sha256"] = shared.json_digest(shared.tree_manifest(Path(inspection["verifiers"]).parent))
    write_json(output / "manifest.json", manifest)
    for name in manifest["order"]:
        status = run_cell(bench, output, manifest, name, env, args.execute)
        if status != 0:
            # Preserve an infrastructure failure before spending another cell;
            # task failures are normally scored by the evaluator with exit 0.
            return status
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
