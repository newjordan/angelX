#!/usr/bin/env python3
"""polyglot-v1 kit: Aider's polyglot benchmark (136 tasks: JS, Python, Rust, C++)
as an agent regression test. See README.md.

  polyglot.py setup                     install the test toolchain into the work dir
  polyglot.py build                     fetch the pinned exercises, build the 136 tasks
  polyglot.py validate                  prove every task: stub red, reference green
  polyglot.py run --agent ... --config  run an agent over the tasks and grade them
  polyglot.py summary RUN               print a run's summary
  polyglot.py compare BASE HEAD         flips, time and tokens; exit 1 on regression
"""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

from common import KIT, load_tasks, select_tasks  # noqa: E402


def main() -> int:
    parser = argparse.ArgumentParser(prog="polyglot.py", description=__doc__,
                                     formatter_class=argparse.RawDescriptionHelpFormatter)
    sub = parser.add_subparsers(dest="cmd", required=True)

    sub.add_parser("setup", help="install pytest, cmake (if missing) and the JS test deps; then doctor")
    sub.add_parser("doctor", help="check the toolchain")
    p_build = sub.add_parser("build", help="build the task catalog")
    p_build.add_argument("--upstream", type=Path, help="an existing polyglot-benchmark checkout")

    p_val = sub.add_parser("validate", help="stub must fail, reference must pass, for every task")
    p_val.add_argument("--tasks", help="names, languages (js,py,rust,cpp) or @file")
    p_val.add_argument("--jobs", type=int, default=8)

    p_run = sub.add_parser("run", help="run an agent over the tasks")
    p_run.add_argument("--agent", required=True,
                       help="shell command run in each task's workspace, or a preset name from agents/ (e.g. angelx)")
    p_run.add_argument("--config", help="model config: a path, or a name from configs/ (e.g. deepseek-flash)")
    p_run.add_argument("--out", type=Path, help="run directory (default runs/<label>)")
    p_run.add_argument("--label", help="a name for this run, e.g. angelx-0.2.87-deepseek")
    p_run.add_argument("--tasks", help="names, languages (js,py,rust,cpp) or @file; default all 136")
    p_run.add_argument("--jobs", type=int, default=1, help="tasks in parallel (published runs: 1)")
    p_run.add_argument("--wall", type=int, default=600, help="per-task wall clock in seconds (published: 600)")
    p_run.add_argument("--pass-env", action="append", default=[], metavar="NAME",
                       help="also hand this env var to the agent (repeatable), e.g. ANGEL_BIN")
    p_run.add_argument("--home-template", help="copied into every fresh HOME (an agent's login or settings)")
    p_run.add_argument("--resume", action="store_true", help="continue a run directory, skipping finished tasks")
    p_run.add_argument("--keep-workspaces", action="store_true")

    p_sum = sub.add_parser("summary", help="summarize a run")
    p_sum.add_argument("run")

    p_cmp = sub.add_parser("compare", help="compare two runs; exit 1 on regression")
    p_cmp.add_argument("base")
    p_cmp.add_argument("head")
    p_cmp.add_argument("--allow-regressions", type=int, default=0)
    p_cmp.add_argument("--max-time-ratio", type=float, help="fail if head's agent time / base's exceeds this")
    p_cmp.add_argument("--strict-flaky", action="store_true", help="count known-flaky tasks as regressions too")

    args = parser.parse_args()

    if args.cmd == "setup":
        from setup_env import setup
        return setup()
    if args.cmd == "doctor":
        from setup_env import doctor
        return doctor()
    if args.cmd == "build":
        from build_tasks import build
        result = build(args.upstream)
        print(json.dumps(result, indent=1))
        if not result["matches_published"]:
            print("warning: catalog digest differs from catalog.sha256; this build is not the published task set")
            return 1
        return 0
    if args.cmd == "validate":
        from validate import validate
        names = [t["name"] for t in select_tasks(load_tasks(), args.tasks)]
        return validate(names, args.jobs)
    if args.cmd == "run":
        from run import run
        preset = KIT / "agents" / f"{args.agent}.sh"
        if preset.exists():
            args.agent = f'bash "{preset}"'
        args.label = args.label or (args.out.name if args.out else "run")
        args.out = (args.out or KIT / "runs" / args.label).resolve()
        return run(args)
    if args.cmd == "summary":
        from compare import summary_text
        print(summary_text(args.run), end="")
        return 0
    if args.cmd == "compare":
        from compare import compare
        return compare(args.base, args.head, args.allow_regressions, args.max_time_ratio, args.strict_flaky)
    return 2


if __name__ == "__main__":
    sys.exit(main())
