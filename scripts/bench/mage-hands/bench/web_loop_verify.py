#!/usr/bin/env python3
"""Third act: can Mage check its own click, and through which channel?

`web_loop_bench.py` measures one shot. `web_loop_retry.py` measured the only
verifier a headless controller had without a model — "did the frame change near
where I clicked" — and found it asymmetric: an unchanged frame is evidence a
click failed; a changed frame is not evidence it worked (a neighbour's focus
ring counts as a change).

This script asks whether the *model* can close that loop, and compares the two
channels it could use:

  nl        ask it in language, over a before/after composite of the clicked
            region: "did the click activate <target>? YES or NO."
  reground  ask it in its own native channel — a pointing question — about the
            *state the click was supposed to produce*: "click the control that
            is outlined in red" (the fixture's visible answer to a click). A
            verifier that points at the control it just clicked has confirmed
            the activation; one that points elsewhere has not.

Every target gets two real mouse clicks, so both classes are measured on the
same scene:

  real   the shipped ground cascade's answer
  decoy  a point 60-170 px away on page background, probed and confirmed inert
         by the page's own counters

The page answers a click in two modes, which is what makes the result
interpretable:

  ring   the activated control is outlined in red
  none   activation leaves no visible mark anywhere (CSS suppresses it); the
         DOM counters still move, so the ground truth is unchanged

`none` is the control: with no state marker to point at, a verifier that still
claims success is inventing it. So the claim is not "the model's accuracy" but
the gap between P(verified | actuated) in `ring` and in `none`, against the
pixel rule on the identical crops.

Usage (Chrome, node, PIL venv; the model already serving on 127.0.0.1:30001):

  $PY -I web_loop_verify.py --work ~/mage/verify --verifier reground \\
      --mode ring [--fixture-limit N] [--real 0] [--out ...jsonl]
"""
import argparse, importlib.util, json, math, os, re, sys, time
from PIL import Image, ImageDraw

HERE = os.path.dirname(os.path.abspath(__file__))


def _load(name, alias):
    spec = importlib.util.spec_from_file_location(alias, os.path.join(HERE, name + ".py"))
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


W = _load("web_loop_bench", "wlb")
R = _load("web_loop_retry", "wlr")

PANEL = (1920, 540)        # one panel of the before/after composite
CROP = (640, 180)          # CSS px of the screen shown in each panel
PIX_THRESH = 20            # differing crop pixels that count as "the crop changed"
STATE_Q = "the control that is outlined in red"   # a state, not an instruction


def slug(s):
    return re.sub(r"[^a-z0-9]+", "-", s.lower()).strip("-")[:40]


# ----------------------------------------------------------- channel 1: language
NL_SYS = ("You inspect two screenshots of the same region of a desktop, one from just before a "
          "mouse click and one from just after. Answer the question on the first line with one "
          "word, YES or NO, then at most one sentence of reasoning.")


def nl_prompt(target):
    t = R.instruction(target)
    return ("The top panel is before the click; the bottom panel is after it. Same region, same "
            f"scale; the click landed at the exact centre of both. Instruction: click {t}.\n\n"
            f"Did that click activate {t}? YES or NO.")


def ask_nl(composite, target):
    prompt = (f"<|im_start|>system\n{NL_SYS}<|im_end|>\n<|im_start|>user\n<|vision_start|>"
              f"<|image_pad|><|vision_end|>{nl_prompt(target)}<|im_end|>\n<|im_start|>assistant\n"
              "<think>\n\n</think>\n\n")
    out = W.post("/generate", {"text": prompt, "image_data": [W.jpeg_b64(composite)],
                               "sampling_params": {"temperature": 0.0, "max_new_tokens": 96}})
    txt = (out.get("text") or "").strip()
    head = txt.split("\n")[0].lower()
    m = re.search(r"\b(yes|no)\b", head) or re.search(r"\b(yes|no)\b", txt.lower())
    return (m.group(1) == "yes" if m else None), txt[:160]


# ------------------------------------------------------- channel 2: re-grounding
def ask_reground(post, view):
    """Point at the click's visible result, in the model's own coordinate channel."""
    at, passes = W.ground(post, STATE_Q, cap=(1920, 1080), zooms=(2, 3))
    return at, passes


# ------------------------------------------------------------------ the frame work
def crop_pad(im, x, y, crop=CROP):
    """A `crop` window centred on (x, y); out-of-frame area is black.

    Padding keeps the click at the panel centre for targets at the top edge too,
    so "the click landed at the centre" is literally true in every pair.
    """
    w, h = im.size
    cw, ch = crop
    x0, y0 = int(round(x - cw / 2)), int(round(y - ch / 2))
    out = Image.new("RGB", (cw, ch), (0, 0, 0))
    src = (max(x0, 0), max(y0, 0), min(x0 + cw, w), min(y0 + ch, h))
    if src[2] > src[0] and src[3] > src[1]:
        out.paste(im.crop(src), (src[0] - x0, src[1] - y0))
    return out


def composite(before, after, x, y):
    top = crop_pad(before, x, y).resize(PANEL, Image.LANCZOS)
    bot = crop_pad(after, x, y).resize(PANEL, Image.LANCZOS)
    out = Image.new("RGB", (PANEL[0], PANEL[1] * 2), (0, 0, 0))
    out.paste(top, (0, 0))
    out.paste(bot, (0, PANEL[1]))
    return out


def changed_in_crop(before, after, x, y):
    from PIL import ImageChops
    a, b = crop_pad(before, x, y), crop_pad(after, x, y)
    d = ImageChops.difference(a.convert("L"), b.convert("L"))
    return sum(d.histogram()[11:])


# ------------------------------------------------------------------ the fixture
def page_html(mode):
    """The bench fixture, with the page's visible answer to a click switched off in `none`.

    `none` is not "no truth": the DOM counters still move, so the arm scores
    against the same ground truth. Only the visible answer is suppressed.
    """
    html = W.fixture_html()
    if mode == "none":
        html = html.replace(".hit { outline:2px solid #d33; outline-offset:1px; }", ".hit { }")
        html = html.replace("#status { font:12px/1.4 ui-monospace, monospace; color:#16181d; margin:6px 0 0; }",
                            "#status { display:none; }")
    return html


def reset_marks(drv):
    """Each target is an independent trial: no ring from an earlier click."""
    drv.call("eval", {"expr": "document.querySelectorAll('.hit').forEach(e=>e.classList.remove('hit'));"
                              "var s=document.getElementById('status'); if(s) s.textContent='last action: none';"
                              "window.__act=Object.create(null); 'ok'"})
    time.sleep(0.15)


def act_state(drv):
    return drv.call("eval", {"expr": "JSON.stringify(window.__act || {})"})["value"] or "{}"


def pick_decoy(drv, truth, view, elems, spin):
    """A point 60-170 px from the target centre that is provably page background."""
    b = truth["box"]
    cx, cy = (b[0] + b[2]) / 2, (b[1] + b[3]) / 2
    others = [e["box"] for e in elems]
    for r in (60, 90, 130, 170):
        for k in range(8):
            a = 2 * math.pi * ((k + spin) % 8) / 8
            x, y = cx + r * math.cos(a), cy + r * math.sin(a)
            if not (8 <= x < view[0] - 8 and 8 <= y < view[1] - 8):
                continue
            if any(bb[0] <= x <= bb[2] and bb[1] <= y <= bb[3] for bb in others):
                continue
            p = drv.call("probe", {"x": round(x, 1), "y": round(y, 1)}).get("value") or {}
            if p.get("tag") in ("button", "a", "input", "select", "textarea", "label"):
                continue
            return (round(x, 1), round(y, 1))
    return None


def judge(point, truth_box, click, scale=1.0):
    """Did the verifier point at the control the click was meant for?"""
    if point is None:
        return None
    x, y = point[0] / scale, point[1] / scale
    b = truth_box
    inside = b[0] - 2 <= x <= b[2] + 2 and b[1] - 2 <= y <= b[3] + 2
    return {"at": [round(x, 1), round(y, 1)], "on_target": bool(inside),
            "to_click_px": round(((x - click[0]) ** 2 + (y - click[1]) ** 2) ** 0.5, 1)}


# ------------------------------------------------------------------ one target
def row_pair(drv, name, truth, kind, url, view, mode, a):
    out = {"kind": kind, "target": name, "id": truth.get("id"), "mode": mode,
           "verifier": a.verifier}
    b = truth["box"]
    cxp, cyp = (b[0] + b[2]) / 2, (b[1] + b[3]) / 2
    reset_marks(drv)
    elems = drv.call("elements")["elements"]
    pre, meta = W.shot_image(drv)
    scale = meta.get("scale", 1.0)

    def check(before_im, after_im, click, ok, prev):
        rec = {"click": [round(click[0], 1), round(click[1], 1)],
               "changed_px": changed_in_crop(before_im, after_im, *click),
               "pixel_changed": changed_in_crop(before_im, after_im, *click) > PIX_THRESH,
               "actuated": bool(ok)}
        rec["pixel_says_done"] = bool(rec["pixel_changed"])
        if a.verifier == "reground":
            t0 = time.time()
            at, passes = ask_reground(after_im, view)
            rec["passes"] = passes
            rec["reground"] = judge(at, b, click, scale)
            rec["says_done"] = bool(rec["reground"] and rec["reground"]["on_target"])
            rec["secs"] = round(time.time() - t0, 1)
        else:
            comp = composite(prev, after_im, *click)
            if a.save_crops:
                comp.save(os.path.join(a.save_crops,
                                       f"{mode}-{kind}-{slug(name)}-{'real' if ok else 'decoy'}.png"))
            t0 = time.time()
            verdict, raw = ask_nl(comp, name)
            rec["nl"] = {"yes": verdict, "raw": raw}
            rec["says_done"] = bool(verdict)
            rec["secs"] = round(time.time() - t0, 1)
        return rec

    # 1) the decoy: a real click on background, so nothing may answer
    dec = pick_decoy(drv, truth, view, elems, hash(name) % 8)
    if dec is not None:
        state0 = act_state(drv)
        drv.call("click", {"x": dec[0], "y": dec[1]})
        time.sleep(a.pause)
        after, _ = W.shot_image(drv)
        inert = act_state(drv) == state0
        out["decoy"] = check(pre, after, dec, False, pre) | {"at": list(dec), "inert": inert}
        pre = after

    # 2) the real thing: shipped cascade -> real click -> the page's answer
    at, passes = W.ground(pre, name, cap=(1920, 1080), zooms=(2, 3))
    if at is None:
        out["parsed"] = False
        return out
    x, y = at[0] / scale, at[1] / scale
    x, y = min(max(x, 0), view[0] - 1), min(max(y, 0), view[1] - 1)
    landed = drv.call("probe", {"x": round(x, 1), "y": round(y, 1)}).get("value")
    before_n = json.loads(act_state(drv)).get(truth["id"], 0) if kind == "fixture" else None
    if kind == "real":
        before_url = drv.call("eval", {"expr": "location.href"})["value"]
        tabs0 = len(drv.call("targets")["pages"])
    drv.call("click", {"x": x, "y": y})
    time.sleep(a.pause)
    post, _ = W.shot_image(drv)
    if kind == "fixture":
        ok = json.loads(act_state(drv)).get(truth["id"], 0) > before_n
    else:
        now = drv.call("eval", {"expr": "location.href"})["value"]
        bare = lambda u: u.split("://")[-1]
        opened = len(drv.call("targets")["pages"]) > tabs0
        ok = opened or (now != url and (bare(truth["href"]) in bare(now)
                                        or now.split("#")[0] == truth["href"]))
        if opened:
            drv.call("closetabs")
    rec = check(pre, post, (x, y), ok, pre)
    inside = b[0] <= x <= b[2] and b[1] <= y <= b[3]
    out.update(rec)
    out.update({"parsed": True, "cascade_passes": passes, "landed_on": landed,
                "box_hit": bool(inside),
                "err_px": round(((x - cxp) ** 2 + (y - cyp) ** 2) ** 0.5, 1)})
    print(f"  {mode:5} {a.verifier:8} {kind:7} {name:30} act={str(ok):5} "
          f"says_done={str(rec['says_done']):5} px={rec['changed_px']:6} "
          f"decoy_done={str((out.get('decoy') or {}).get('says_done')):5}", flush=True)
    if kind == "real":
        drv.call("nav", {"url": url})
    return out


def rate(xs):
    xs = [v for v in xs if v is not None]
    return round(sum(1 for v in xs if v) / len(xs), 3) if xs else None


def tally(rows):
    real = [r for r in rows if r.get("parsed")]
    dec = [r["decoy"] for r in rows if r.get("decoy") and r["decoy"].get("inert")]
    act = [r for r in real if r["actuated"]]
    dead = [r for r in real if not r["actuated"]]
    return {
        "n_targets": len(rows), "n_real": len(real), "n_actuated": len(act), "n_decoy": len(dec),
        # the verifier's two jobs, same pairs, both channels
        "says_done_on_actuated": rate([r["says_done"] for r in act]),
        "says_not_done_on_failed": rate([not r["says_done"] if r["says_done"] is not None else None for r in dead]),
        "says_not_done_on_decoy": rate([not d["says_done"] if d["says_done"] is not None else None for d in dec]),
        "unparsed": rate([r["says_done"] is None for r in real]),
        # the pixel rule on the identical crops
        "pixel_done_on_actuated": rate([r["pixel_changed"] for r in act]),
        "pixel_not_done_on_failed": rate([not r["pixel_changed"] for r in dead]),
        "pixel_not_done_on_decoy": rate([not d["pixel_changed"] for d in dec]),
        "median_changed_px_act": sorted(r["changed_px"] for r in act)[len(act) // 2] if act else None,
        "median_changed_px_fail": sorted(r["changed_px"] for r in dead)[len(dead) // 2] if dead else None,
        "median_changed_px_decoy": sorted(d["changed_px"] for d in dec)[len(dec) // 2] if dec else None,
        "box_hit": rate([r["box_hit"] for r in real]),
        "median_verifier_secs": sorted(r["secs"] for r in real if "secs" in r)[len(real) // 2] if real else None,
    }


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--work", default=os.path.join(os.environ.get("MAGE_ROOT") or os.path.expanduser("~/mage"), "verify"))
    ap.add_argument("--port", type=int, default=9461)
    ap.add_argument("--scale", type=float, default=1.0)
    ap.add_argument("--view", default="1920x1080")
    ap.add_argument("--mode", default="ring", choices=("ring", "none"))
    ap.add_argument("--verifier", default="reground", choices=("reground", "nl"))
    ap.add_argument("--fixture-limit", type=int, default=0)
    ap.add_argument("--real", type=int, default=0, help="real pages to try after the fixture")
    ap.add_argument("--real-targets", type=int, default=4)
    ap.add_argument("--no-fixtures", action="store_true")
    ap.add_argument("--pause", type=float, default=0.5)
    ap.add_argument("--save-crops", default="")
    ap.add_argument("--out", default="")
    a = ap.parse_args()

    os.makedirs(a.work, exist_ok=True)
    if a.save_crops:
        os.makedirs(a.save_crops, exist_ok=True)
    view = tuple(int(v) for v in a.view.split("x"))
    started = time.time()
    results = []
    drv = W.Driver(os.path.join(HERE, "web_loop_driver.mjs"), a.work, a.port, a.scale, None, view)
    try:
        if not a.no_fixtures:
            with open(os.path.join(a.work, "fixture.html"), "w") as f:
                f.write(page_html(a.mode))
            httpd, url = W.serve(a.work)
            drv.call("nav", {"url": url})
            elems = drv.call("elements")["elements"]
            # the same 26 targets web_loop_bench.py scores, in the same order, so
            # these arms pair with the grounding arms already on record
            rows = [(n, W.box_of(elems, id=i)) for i, _k, n in W.FIXTURE_TARGETS]
            rows = [r for r in rows if r[1]]
            if a.fixture_limit:
                rows = rows[: a.fixture_limit]
            print(f"fixture: {len(rows)} targets, mode {a.mode}, verifier {a.verifier}", flush=True)
            for nm, truth in rows:
                results.append(row_pair(drv, nm, truth, "fixture", url, view, a.mode, a))
            httpd.shutdown()

        if a.real:
            for real_url, named in W.REAL[: a.real]:
                drv.call("nav", {"url": real_url})
                elems = drv.call("elements")["elements"]
                for nm, truth in R.real_rows(elems, named, a.real_targets, real_url):
                    drv.call("nav", {"url": real_url})
                    results.append(row_pair(drv, nm, truth, "real", real_url, view, a.mode, a))
    finally:
        drv.close()

    summary = {k: tally([r for r in results if r.get("kind") == k])
               for k in ("fixture", "real") if any(r.get("kind") == k for r in results)}
    summary["all"] = tally(results)
    summary.update({"mode": a.mode, "verifier": a.verifier, "view": list(view),
                    "wall_s": round(time.time() - started, 1)})
    out = a.out or os.path.join(a.work, f"verify-{a.verifier}-{a.mode}.jsonl")
    with open(out, "w") as f:
        for r in results:
            f.write(json.dumps(r) + "\n")
    with open(out.replace(".jsonl", "-summary.json"), "w") as f:
        json.dump(summary, f, indent=1)
    print(json.dumps(summary, indent=1))
    print("wrote", out)
    return 0


if __name__ == "__main__":
    sys.exit(main())
