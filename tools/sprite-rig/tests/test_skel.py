import os, tempfile, time, unittest, numpy as np
from spriterig.delve import source
from spriterig.skel import skeleton as S, clips as C, billboard as BB

def test_fk_rest_height():
    _, X = S.fk(np.tile(np.eye(3), (len(S.NAMES), 1, 1)))
    assert abs(X[:, 1].max() - X[:, 1].min() - S.height()) < 1e-6

def test_bvh_roundtrip():
    c = C.procedural("walk"); p = os.path.join(tempfile.mkdtemp(), "w.bvh"); C.save_bvh(c, p)
    d = C.load_bvh(p)
    assert np.allclose(c.rot, d.rot, atol=1e-4)

def test_billboard_fast_and_palette():
    from spriterig.delve import source
    b = BB.hero("1", source.DEFAULT_ROOT); c = C.procedural("walk"); t = time.time()
    for f in range(len(c)):
        idx, view, mir = b.frame(*c.sample(f / c.fps), facing=f * 30, cam=BB.Camera(f * 17, 20))
    assert (time.time() - t) / len(c) < 0.01
    assert set(np.unique(idx)) <= set(range(len(b.inks) + 1)) or idx.dtype.kind in "iu"

MIX = os.environ.get("MIXAMO_DIR", "")
FBX = os.path.join(MIX, "Walking.fbx")
@unittest.skipUnless(MIX and os.path.exists(FBX), "set MIXAMO_DIR to a folder with Walking.fbx")
def test_fbx_clip():
    from spriterig.skel import fbx
    c = fbx.load_clip(FBX, "walk")
    assert c.rot.shape[1] == len(S.NAMES) and len(c) > 10


HAVE = os.path.isdir(os.path.join(__import__("spriterig.delve.source", fromlist=["x"]).DEFAULT_ROOT, "crates", "delve"))


class SkelTest(unittest.TestCase):
    def test_fk_rest_height(self): _test_fk_rest_height()
    def test_bvh_roundtrip(self): _test_bvh_roundtrip()
    @unittest.skipUnless(HAVE, "angelX delve sources not found (set ANGELX_ROOT)")
    def test_billboard_fast_and_palette(self): _test_billboard_fast_and_palette()
    @unittest.skipUnless(MIX and os.path.exists(FBX), "set MIXAMO_DIR")
    def test_fbx_clip(self): _test_fbx_clip()


# plain functions above are for pytest; hide them from unittest double-collection
for _n in [k for k in list(globals()) if k.startswith("test_")]:
    globals()["_" + _n] = globals().pop(_n)

if __name__ == "__main__":
    unittest.main()
