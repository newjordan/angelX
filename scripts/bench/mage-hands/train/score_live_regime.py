"""Grade a fake-shop task from verifier state, never a model's claimed success."""
import argparse
import json
from pathlib import Path


def score(state, expected, agent_rc=0):
    if agent_rc != 0:
        return {"pass": False, "status": "infra-blocked", "agent_rc": agent_rc,
                "reason": "agent execution unsuccessful; do not mix into task pass rate"}
    if not isinstance(state, dict) or not all(k in state for k in ("orders", "cart", "view", "shipping")):
        return {"pass": False, "status": "infra-blocked", "reason": "missing independent state"}
    orders = list(state["orders"].values())
    observed = {"order_count": len(orders), "cart": state["cart"], "final_cart": state["cart"],
                "shipping": state["shipping"], "view": state["view"], "address": state.get("address")}
    if len(orders) == 1:
        observed.update(ordered_cart=orders[0]["cart"], total_cents=orders[0]["total_cents"],
                        shipping=orders[0]["shipping"], address=orders[0].get("address"))
    errors = {key: {"expected": want, "observed": observed.get(key)}
              for key, want in expected.items() if observed.get(key) != want}
    return {"pass": not errors, "status": "pass" if not errors else "task-failed",
            "mismatches": errors, "observed": observed,
            "scope": "independent fake-shop state; not an offline pointing score"}


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--state", required=True)
    parser.add_argument("--task", required=True)
    parser.add_argument("--tasks", default=str(Path(__file__).with_name("live-regime-tasks.json")))
    parser.add_argument("--agent-rc", type=int, default=0)
    parser.add_argument("--output")
    args = parser.parse_args()
    tasks = json.loads(Path(args.tasks).read_text())["shopping"]
    task = next(t for t in tasks if t["id"] == args.task)
    try:
        state = json.loads(Path(args.state).read_text())
    except (OSError, ValueError):
        state = None
    result = {"task": args.task, **score(state, task["expected"], args.agent_rc)}
    text = json.dumps(result, indent=2) + "\n"
    if args.output:
        Path(args.output).write_text(text)
    print(text, end="")
    raise SystemExit(0 if result["pass"] else 2 if result["status"] == "infra-blocked" else 1)
