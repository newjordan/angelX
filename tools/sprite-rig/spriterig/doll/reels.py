"""Curated showcase reels: python -m spriterig.doll.reels [outdir]
Loadout swap, weapon-attack reel, emote reel (labelled grids, looping GIFs)."""
import os, sys
from . import Doll, Loadout, dsl
from . import anim as A
from .out import tile, save_gif

SIZE = (96, 104)


def reel(cells, path, cols, fps=12, view=90, scale=3, max_len=None):
    lib = dsl.library()
    seqs, labels = [], []
    for lo, clip, lab in cells:
        d = Doll(Loadout(lo))
        c = lib[clip]
        seqs.append(A.frames(d, c, facing=view, size=SIZE, pad=26))
        labels.append(lab)
    n = max_len or max(len(s) for s in seqs)
    ims = [tile([s[k % len(s)] if (k < len(s) or s is None or True) else s[-1] for s in seqs],
                scale=scale, labels=labels, cols=cols) for k in range(n)]
    save_gif(ims, path, fps)
    return path, n, os.path.getsize(path)


def main(out="showcase/curated"):
    os.makedirs(out, exist_ok=True)
    los = ["peasant", "knight", "knight_blue", "paladin", "ranger", "rogue", "spearman", "mage",
           "goblin_raider", "dwarf_berserker", "king"]
    # one figure, same clip, loadout swaps every loop (+ the full roster grid below)
    lib = dsl.library(); ims = []
    seq = [("peasant", "idle"), ("knight", "slash"), ("knight_blue", "slash"), ("paladin", "shield_bash"),
           ("ranger", "bow_shot"), ("rogue", "dagger_flurry"), ("spearman", "thrust"), ("mage", "staff_cast"),
           ("goblin_raider", "slash"), ("dwarf_berserker", "overhead"), ("king", "victory")]
    for lo, cl in seq:
        fr = A.frames(Doll(Loadout(lo)), lib[cl], facing=70, size=SIZE, pad=26)
        ims += [tile([f], scale=5, labels=[f"{lo}  /  {cl}"]) for f in fr]
    save_gif(ims, os.path.join(out, "doll_loadout_swap.gif"), 12)
    r = [(os.path.join(out, "doll_loadout_swap.gif"), len(ims), os.path.getsize(os.path.join(out, "doll_loadout_swap.gif")))]
    r += [reel([(l, "idle", l) for l in los] + [("king", "take_a_bow", "king bows")],
              os.path.join(out, "doll_roster.gif"), cols=6, view=60)]
    atk = [("knight", "slash", "slash"), ("dwarf_berserker", "overhead", "overhead"), ("spearman", "thrust", "thrust"),
           ("dwarf_berserker", "spin", "spin"), ("knight_blue", "combo", "combo"), ("paladin", "shield_bash", "shield bash"),
           ("ranger", "bow_shot", "bow"), ("mage", "staff_cast", "staff"), ("rogue", "dagger_flurry", "daggers"),
           ("goblin_raider", "slash", "goblin slash")]
    r.append(reel(atk, os.path.join(out, "doll_attack_reel.gif"), cols=5))
    emo = [("peasant", "dab", "dab"), ("rogue", "floss", "floss"), ("knight", "chicken_dance", "chicken"),
           ("mage", "facepalm", "facepalm"), ("dwarf_berserker", "flex", "flex"), ("peasant", "trip", "trip"),
           ("goblin_raider", "tpose_glitch", "t-pose glitch"), ("king", "take_a_bow", "bow"), ("ranger", "laugh", "laugh"),
           ("spearman", "shrug", "shrug"), ("paladin", "sit", "sit"), ("mage", "sleep", "sleep"),
           ("knight_blue", "jump", "jump"), ("rogue", "victory", "victory"), ("goblin_raider", "walk", "walk")]
    r.append(reel(emo, os.path.join(out, "doll_emote_reel.gif"), cols=5, view=60))
    for p, n, b in r:
        print(f"{p}  {n} frames  {b / 1e6:.2f} MB")


if __name__ == "__main__":
    main(*sys.argv[1:])
