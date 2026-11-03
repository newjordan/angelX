import os, sys, json, tempfile, unittest
import numpy as np
sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
sys.path.insert(0, os.path.join(os.path.dirname(os.path.dirname(os.path.abspath(__file__))), "samples"))
from make_knight import knight
from spriterig import load_sprite, segment, build_rig, ANIMS, render_frames
from spriterig.rig import bone_matrices, skin
from spriterig.__main__ import main


class T(unittest.TestCase):
    def setUp(self):
        self.d = tempfile.mkdtemp()
        self.p = os.path.join(self.d, "k.png"); knight(self.p)
        self.rgba = load_sprite(self.p)

    def test_weights_sum_to_one(self):
        lab, J = segment(self.rgba)
        rig = build_rig(self.rgba, lab, J)
        for p in rig["parts"]:
            np.testing.assert_allclose(p.weights.sum(1), 1)

    def test_rest_pose_identity(self):
        lab, J = segment(self.rgba)
        rig = build_rig(self.rgba, lab, J)
        M = bone_matrices(J, {})
        for p in rig["parts"]:
            np.testing.assert_allclose(skin(p, M), p.verts, atol=1e-9)

    def test_all_parts_found(self):
        lab, _ = segment(self.rgba)
        self.assertEqual(len(set(np.unique(lab)) - {-1}), 10)

    def test_walk_moves_feet(self):
        lab, J = segment(self.rgba)
        rig = build_rig(self.rgba, lab, J)
        f = render_frames(rig, ANIMS["walk"][0], 4)
        self.assertGreater(np.abs(f[0].astype(int) - f[1].astype(int)).sum(), 0)

    def test_keyed_background(self):
        p = os.path.join(self.d, "g.png"); knight(p, bg=(40, 140, 60, 255))
        a = load_sprite(p)[..., 3]
        self.assertEqual(a[0, 0], 0); self.assertGreater((a > 0).sum(), 1000)

    def test_cli(self):
        out = os.path.join(self.d, "out")
        main([self.p, "--out", out, "--anims", "idle,walk,attack"])
        meta = json.load(open(os.path.join(out, "walk_sheet.json")))
        self.assertEqual(len(meta["frames"]), ANIMS["walk"][1])
        self.assertTrue(os.path.exists(os.path.join(out, "attack", "%03d.png" % (ANIMS["attack"][1] - 1))))

    def test_all_motions_render(self):
        from spriterig import DEFAULT_SET
        lab, J = segment(self.rgba); rig = build_rig(self.rgba, lab, J)
        self.assertGreaterEqual(len(DEFAULT_SET.split(",") if isinstance(DEFAULT_SET, str) else DEFAULT_SET), 15)
        names = DEFAULT_SET.split(",") if isinstance(DEFAULT_SET, str) else DEFAULT_SET
        for name in names:
            fn, n, fps = ANIMS[name]
            f = render_frames(rig, fn, 3)
            self.assertEqual(len(f), 3); self.assertGreater(f[1][..., 3].sum(), 0, name)

    def test_aa_vs_nearest(self):
        from spriterig.render import render_pose
        lab, J = segment(self.rgba); rig = build_rig(self.rgba, lab, J)
        h, w = self.rgba.shape[:2]
        pose = ANIMS["attack_right"][0](0.5, h)
        a = render_pose(rig, pose, (w * 2, h * 2), (w // 2, h // 2), 1, "aa")
        n = render_pose(rig, pose, (w * 2, h * 2), (w // 2, h // 2), 1, "nearest")
        partial = lambda x: ((x[..., 3] > 0) & (x[..., 3] < 255)).sum()
        self.assertEqual(partial(n), 0)          # nearest keeps hard alpha
        self.assertGreater(partial(a), 0)        # aa produces smooth edges
        self.assertLess(abs(int((a[..., 3] > 127).sum()) - int((n[..., 3] > 127).sum())), 0.05 * (n[..., 3] > 127).sum())

    def test_poly_override_last_wins(self):
        from spriterig.segment import PARTS
        h, w = self.rgba.shape[:2]
        o = {"polys": [["torso", [[0, 0], [w, 0], [w, h], [0, h]]], ["head", [[0, 0], [w, 0], [w, 20], [0, 20]]]]}
        lab, _ = segment(self.rgba, o)
        op = self.rgba[..., 3] > 0
        self.assertTrue((lab[:20][op[:20]] == PARTS.index("head")).all())
        self.assertTrue((lab[30:][op[30:]] == PARTS.index("torso")).all())

    def test_pose_mapper_mirrors(self):
        from spriterig.rig import make_pose_mapper
        r = make_pose_mapper("r", "right")({"upper_arm_r": 0.5})
        l = make_pose_mapper("r", "left")({"upper_arm_r": 0.5})
        self.assertNotEqual(r, l)

    def test_cli_gifs(self):
        out = os.path.join(self.d, "g")
        main([self.p, "--out", out, "--anims", "idle,jump", "--no-frames", "--name", "k"])
        from PIL import Image
        g = Image.open(os.path.join(out, "jump.gif")); self.assertGreater(g.n_frames, 4)
        self.assertTrue(os.path.exists(os.path.join(out, "k_montage.gif")))
        self.assertIn("render_ms_per_frame", json.load(open(os.path.join(out, "timing.json"))))

    def test_frosty_samples(self):
        root = os.path.join(os.path.dirname(os.path.dirname(os.path.abspath(__file__))), "samples", "frosty")
        from spriterig.segment import load_override
        for n in ("red_knight", "blue_knight", "kimi_robe", "loom_warden"):
            o = load_override(os.path.join(root, n + ".json")); a = load_sprite(os.path.join(root, n + ".png"))
            lab, J = segment(a, o)
            self.assertEqual(len(set(np.unique(lab)) - {-1}), 10, n)


if __name__ == "__main__":
    unittest.main()
