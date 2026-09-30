#!/usr/bin/env python3
"""e2_count.py <route> <label>... — E2: send each stub session's final request
to the provider (Chat Completions, smallest output cap) and print the prompt
tokens it reports. Routes: glm-flash, deepseek-flash, muse (keys from env)."""
import json, os, subprocess, sys, time, urllib.request, urllib.error

ROUTES = {
    "glm-flash": ("https://api.z.ai/api/coding/paas/v4", "ZAI_API_KEY", "glm-5.3-flash", {"thinking": {"type": "disabled"}}),
    "deepseek-flash": ("https://api.deepseek.com/v1", "DEEPSEEK_API_KEY", "deepseek-flash", {}),
    "muse": ("https://api.meta.ai/v1", "ANGEL_META_KEY", "muse-spark-1.3-contributor", {}),
    "grok": ("https://api.x.ai/v1", None, "grok-4.7", {"reasoning_effort": "low"}),
}
route, labels = sys.argv[1], sys.argv[2:]
url, key_env, model, extra = ROUTES[route]
for label in labels:
    last = json.loads(open(f"out/{label}.jsonl").readlines()[-1])["body"]
    if route == "deepseek-flash":  # thinking mode wants each assistant turn's reasoning back
        for m in last["messages"]:
            if m.get("role") == "assistant":
                m.setdefault("reasoning_content", "")
    body = {"model": model, "messages": last["messages"], "tools": last.get("tools", []),
            "max_tokens": 16, "stream": False, **extra}
    key = os.environ[key_env] if key_env else subprocess.run(
        [sys.executable, os.path.join(os.path.dirname(__file__), "grok_bearer.py")],
        capture_output=True, text=True, check=True).stdout.strip()
    req = urllib.request.Request(url + "/chat/completions", data=json.dumps(body).encode(), method="POST",
                                 headers={"Content-Type": "application/json", "Authorization": "Bearer " + key})
    for attempt in range(3):
        try:
            usage = json.load(urllib.request.urlopen(req, timeout=300)).get("usage", {})
            break
        except urllib.error.HTTPError as e:
            usage = {"error": e.code, "body": e.read()[:200].decode("utf-8", "replace")}
            if e.code not in (429, 500, 502, 503):
                break
            time.sleep(5)
    print(json.dumps({"route": route, "label": label, "turns": len(last["messages"]),
                      "prompt_tokens": usage.get("prompt_tokens"), "usage": usage}), flush=True)
