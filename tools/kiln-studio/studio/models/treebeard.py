"""The treebeard router: OpenAI-compatible chat, quick text.

Tried in order: KILN_TREEBEARD, the local router at 127.0.0.1:8011, then
KILN_TREEBEARD_PEER (a GPU host's own router, as the wish painter's `ask_gemma`).
Text only here: brief expansion, names, tags.
"""
import json
import os
import time
import urllib.request

from .base import Adapter, Answer

URLS = [u for u in [os.environ.get("KILN_TREEBEARD"),
                    "http://127.0.0.1:8011/v1/chat/completions",
                    os.environ.get("KILN_TREEBEARD_PEER")] if u]


class Treebeard(Adapter):
    name = "treebeard"

    def available(self):
        for url in URLS:
            try:
                urllib.request.urlopen(url.replace("/v1/chat/completions", "/health"), timeout=3)
                return True, url
            except urllib.error.HTTPError:
                return True, url  # it answers, if not on /health
            except Exception:
                continue
        return False, "no router answered: " + ", ".join(URLS)

    def ask(self, prompt, images=(), workdir=None, budget=90, system=None, max_tokens=1500, temperature=0.4, json_out=False):
        msgs = ([{"role": "system", "content": system}] if system else []) + [{"role": "user", "content": prompt}]
        body = {"model": "treebeard", "max_tokens": max_tokens, "temperature": temperature, "messages": msgs}
        t0 = time.time()
        deadline = t0 + budget
        last = None
        for url in URLS:
            left = deadline - time.time()
            if left < 1:
                break
            try:
                req = urllib.request.Request(url, json.dumps(body).encode(), {"Content-Type": "application/json"})
                data = json.load(urllib.request.urlopen(req, timeout=left))
                text = data["choices"][0]["message"]["content"] or ""
                ans = Answer(model="treebeard", text=text, seconds=time.time() - t0,
                             tokens=(data.get("usage") or {}).get("total_tokens") or 0, turns=1, session=url)
                if json_out:
                    try:
                        ans.data = json.loads(text[text.find("{"): text.rfind("}") + 1])
                    except ValueError:
                        ans.ok, ans.error = False, "the answer was not JSON: " + text[:300]
                return ans
            except Exception as e:
                last = f"{url}: {e}"
        return Answer(model="treebeard", ok=False, error=f"no router answered ({last})", seconds=time.time() - t0)
