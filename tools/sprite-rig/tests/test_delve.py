import json, os, tempfile
import numpy as np
import unittest
from spriterig.delve import source
from spriterig.delve.cast import cast
from spriterig.delve.cutout import Renderer
from spriterig.delve import out

ROOT = source.DEFAULT_ROOT
HAVE = os.path.isdir(os.path.join(ROOT, "crates", "delve"))


@unittest.skipUnless(HAVE, "angelX delve sources not found (set ANGELX_ROOT)")
class DelveTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.chars = cast(ROOT)


    def test_cast_size(self):
        chars = self.chars
        assert len(chars) == 9


    def test_rest_identity(self):
        chars = self.chars
        for ch in chars:
            r = Renderer(ch, pad=8)
            f = r.frame({})
            rows = r.to_rows(f)
            got = [s.strip(".") for s in rows if s.strip(".")]
            want = [s.strip(".") for s in ("".join(x) for x in ch.rest()) if s.strip(".")]
            assert got == want, ch.name


    def test_palette_only(self):
        chars = self.chars
        pal = source.palette()
        for ch in chars[:3]:
            r, o, _ = out.render_all(ch, ["walk", "attack_right", "death"])
            for frames in o.values():
                for f in frames[0]:
                    for row in r.to_rows(f):
                        assert all(c == "." or c in pal for c in row)


    def test_export_json_rust_flip(self):
        chars = self.chars
        d = tempfile.mkdtemp()
        out.export(chars[-1], source.palette(), d, anims=["walk"])
        js = json.load(open(os.path.join(d, chars[-1].name + ".json")))
        assert js["anims"]["walk"]["frames"]
        rs = open(os.path.join(d, chars[-1].name + "_frames.rs")).read()
        assert "pub const WALK" in rs
        from PIL import Image
        a = np.array(Image.open(os.path.join(d, "walk_right.png")))
        b = np.array(Image.open(os.path.join(d, "walk_left.png")))
        n = len(js["anims"]["walk"]["frames"])
        cw = a.shape[1] // n
        for i in range(n):
            assert (a[:, i * cw:(i + 1) * cw][:, ::-1] == b[:, i * cw:(i + 1) * cw]).all()


if __name__ == "__main__":
    unittest.main()
