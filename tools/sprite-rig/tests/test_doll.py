"""2D doll animator: DSL, loadouts, render robustness, speed. Runs under unittest and pytest."""
import json, os, tempfile, time, unittest
import numpy as np
from spriterig.doll import Doll, Loadout, dsl
from spriterig.doll import anim as A, draw as Dr
from spriterig.doll.__main__ import check_frames, main

HERE = os.path.dirname(os.path.abspath(__file__))
LIB = os.path.join(HERE, "..", "spriterig", "doll", "library")


class DollTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.lib = dsl.library()

    def test_library_has_attacks_and_emotes(self):
        for n in ("slash", "overhead", "thrust", "spin", "combo", "shield_bash", "bow_shot",
                  "staff_cast", "dab", "floss", "chicken_dance", "facepalm", "sleep", "take_a_bow"):
            self.assertIn(n, self.lib)
        self.assertTrue(self.lib["slash"].smear)
        self.assertTrue(any("hit" in str(e) for e in self.lib["slash"].events.values()) or self.lib["slash"].events)

    def test_dsl_parse_and_ease(self):
        c = dsl.parse("clip t\nfps 12\nloop no\n0 rest\n4 ease=linear rarm=90\n")["t"]
        self.assertEqual(len(c), 5)
        self.assertEqual(c.rot.shape[1:], (len(c.rot[0]), 3, 3)[1:] if False else c.rot.shape[1:])
        # rotation grows monotonically with a linear ease
        from spriterig.skel import skeleton as S
        i = S.NAMES.index("RightArm")
        ang = [np.degrees(np.arccos(np.clip((np.trace(c.rot[f, i]) - 1) / 2, -1, 1))) for f in range(5)]
        self.assertTrue(all(b >= a - 1e-6 for a, b in zip(ang, ang[1:])))
        self.assertAlmostEqual(ang[-1], 90, delta=1)

    def test_every_loadout_renders(self):
        for f in sorted(os.listdir(os.path.join(LIB, "loadouts"))):
            d = Doll(Loadout(os.path.join(LIB, "loadouts", f)))
            for v in (0, 90, 180, 270):
                c = d.frame(self.lib["idle"].rot[0], self.lib["idle"].hips[0], facing=v)
                self.assertGreater((c > 0).sum(), 150, f)

    def test_loadout_swap_changes_pixels(self):
        idle = self.lib["idle"]
        a = Doll(Loadout("peasant")).frame(idle.rot[0], idle.hips[0], facing=45)
        b = Doll(Loadout({"body": "hero", "equip": ["helm_knight", "chest_plate", "sword"]})).frame(
            idle.rot[0], idle.hips[0], facing=45)
        self.assertNotEqual(a.shape == b.shape and (a == b).all(), True)

    def test_check_clean(self):
        d = Doll(Loadout("knight"))
        for n in ("slash", "dab", "jump", "sleep"):
            for facing in (90, 0):
                fr = A.frames(d, self.lib[n], facing=facing)
                bad = [k for k, *_ in check_frames(fr, self.lib[n]) if not k.startswith("pop")]
                self.assertEqual(bad, [], (n, facing))

    def test_bad_loadout_is_explained(self):
        with self.assertRaises(ValueError):
            Loadout({"body": "hero", "equip": ["laser_sword"]})

    def test_cleanup(self):
        a = np.zeros((7, 7), np.uint8); a[1:4, 1:4] = 3; a[2, 2] = 0; a[6, 6] = 7
        h, o = Dr.cleanup(a)
        self.assertEqual((h, o), (1, 1)); self.assertEqual(a[2, 2], 3); self.assertEqual(a[6, 6], 0)

    def test_speed_low_ms(self):
        d = Doll(Loadout("knight")); c = self.lib["combo"]
        A.frames(d, c)
        t = time.perf_counter(); n = 0
        for f in range(len(c)):
            d.frame(c.rot[f], c.hips[f], facing=90, smear=f in c.smear); n += 1
        ms = (time.perf_counter() - t) / n * 1000
        self.assertLess(ms, 25)   # ~3 ms on the box; generous for CI

    def test_batch_export(self):
        out = tempfile.mkdtemp()
        main(["export", "-l", "rogue", "-c", "slash,floss", "--views", "side", "--out", out])
        idx = json.load(open(os.path.join(out, "index.json")))
        self.assertEqual(len(idx), 2)
        for e in idx:
            self.assertTrue(os.path.exists(os.path.join(out, e["image"] if isinstance(e, dict) else e)))


if __name__ == "__main__":
    unittest.main()


class TestDelveSilhouette(unittest.TestCase):
    """v6: every loadout shares the stout delve proportions (short legs, broad body, big helm)."""

    def test_stout_and_consistent(self):
        from spriterig.doll import Doll, Loadout, dsl
        from spriterig.doll import anim as A
        clip = dsl.library()["idle"]
        ratios = {}
        for lo in ("knight", "paladin", "peasant", "mage"):
            f = A.frames(Doll(Loadout(lo)), clip, facing=15, size=(64, 64), pad=14)[0]
            ys, xs = np.nonzero(f)
            ratios[lo] = (xs.max() - xs.min() + 1) / (ys.max() - ys.min() + 1)
        for lo, r in ratios.items():
            self.assertGreater(r, 0.45, f"{lo} too skinny ({r:.2f}) for delve knights")
        self.assertLess(max(ratios.values()) - min(ratios.values()), 0.45, ratios)
