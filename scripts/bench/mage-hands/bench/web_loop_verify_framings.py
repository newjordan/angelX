#!/usr/bin/env python3
"""Which framing, if any, makes Mage answer a verification question in words?

`web_loop_verify.py --verifier nl` asks the question in the runtime's own chat
format (`<think>` pre-filled, as `hands.rs` builds it) and got a token loop on
26/26 replies. This lab asks the same five saved before/after pairs under six
other framings — the runtime format with thinking left open, a neutral system
prompt, the chat template, and each with and without BEFORE/AFTER labels drawn
on the panels — so the negative result is about the model's channel, not about
one prompt string.

  $PY -I web_loop_verify_framings.py <crops-dir>

<crops-dir> must hold the five `ring-none-fixture-*-{real,decoy}.png` pairs
`web_loop_verify.py --verifier nl --save-crops DIR` writes. Truth per pair is
in the CASES table. Output: one line per (framing, pair) with the model's
extractable verdict, or ??? when there is none.
"""
import importlib.util, os, re, sys
from PIL import Image, ImageDraw, ImageFont

HERE = os.path.dirname(os.path.abspath(__file__))


def _load(name, alias):
    spec = importlib.util.spec_from_file_location(alias, os.path.join(HERE, name + ".py"))
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


W = _load("web_loop_bench", "wlb")

NEUTRAL_SYS = ("You are a careful visual inspector of desktop screenshots. You answer one "
               "short question about what changed on screen. You never guess: if the two "
               "frames look the same where the click landed, the answer is NO.")

CASES = [("ring-none-fixture-save-decoy.png", "Save", "nothing happened"),
         ("ring-none-fixture-save-real.png", "Save", "miss: clicked Export report"),
         ("ring-none-fixture-print-decoy.png", "Print", "nothing happened"),
         ("ring-none-fixture-print-real.png", "Print", "activated"),
         ("ring-none-fixture-share-real.png", "Share", "activated")]

Q = ("Two screenshots of the same part of a desktop, one above the other. The top one is from "
     "just before a click; the bottom one is from just after. The click landed at the exact "
     "centre of both images. Instruction: Click {t}.\n\nDid the click activate {t}? Answer YES "
     "if the target responded or changed, NO if it did not. One word only: YES or NO.")
Q_LBL = ("The image has two panels, labelled BEFORE the click and AFTER the click. Both show "
         "the same part of a desktop at the same scale. The click landed at the exact centre of "
         "both panels. Instruction: Click {t}.\n\nDid the click activate {t}? Answer YES if the "
         "target responded or changed, NO if it did not. One word only: YES or NO.")
Q_PLAIN = ("Answer the question at the end.\n\nBefore this click the screen looked like the "
           "top panel; after it, like the bottom panel. Same region, same scale, click at the "
           "centre.\n\nQuestion: did clicking activate {t}? Answer YES or NO.")


def labelled(im, top="BEFORE the click", bot="AFTER the click"):
    out = im.copy()
    d = ImageDraw.Draw(out)
    try:
        f = ImageFont.truetype("/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf", 44)
    except OSError:
        f = None
    for y, t in ((12, top), (552, bot)):
        d.rectangle([0, y - 8, 470, y + 52], fill=(0, 0, 0))
        d.text((12, y), t, fill=(255, 255, 255), font=f)
    return out


def wrap(sysmsg, q):
    return (f"<|im_start|>system\n{sysmsg}<|im_end|>\n<|im_start|>user\n<|vision_start|>"
            f"<|image_pad|><|vision_end|>{q}<|im_end|>\n<|im_start|>assistant\n")


def ask(im, text, ntok):
    out = W.post("/generate", {"text": text, "image_data": [W.jpeg_b64(im)],
                               "sampling_params": {"temperature": 0.0, "max_new_tokens": ntok}})
    return (out.get("text") or "").strip()


VARIANTS = {
    # name: (system, question, composite transform, tokens, pre-filled think?)
    "A_ground_open": (W.SYSTEM, Q, None, 512, False),
    "B_neutral_open": (NEUTRAL_SYS, Q, None, 512, False),
    "C_neutral_prefill": (NEUTRAL_SYS, Q, None, 16, True),
    "D_ground_labelled": (W.SYSTEM, Q_LBL, labelled, 512, False),
    "E_neutral_labelled": (NEUTRAL_SYS, Q_LBL, labelled, 512, False),
    "F_neutral_plain": (NEUTRAL_SYS, Q_PLAIN, labelled, 512, False),
}


def main():
    d = sys.argv[1] if len(sys.argv) > 1 else os.path.join(os.environ.get("MAGE_ROOT") or os.path.expanduser("~/mage"), "crops-ring-none")
    for name, (sysmsg, q, fx, ntok, prefill) in VARIANTS.items():
        print("=" * 20, name, flush=True)
        for fn, target, truth in CASES:
            path = os.path.join(d, fn)
            if not os.path.exists(path):
                print(f"  missing {path}")
                continue
            im = Image.open(path).convert("RGB")
            img = fx(im) if fx else im
            text = wrap(sysmsg, q.format(t=target)) + ("<think>\n\n</think>\n\n" if prefill else "")
            txt = ask(img, text, ntok)
            m = re.search(r"\b(yes|no)\b", txt.strip().lower())
            print(f"  {fn[16:-4]:12} truth={truth:22} -> "
                  f"{(m.group(1).upper() if m else '???'):4} | {txt[-100:]!r}", flush=True)


if __name__ == "__main__":
    main()
