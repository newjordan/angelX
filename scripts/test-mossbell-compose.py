#!/usr/bin/env python3
"""Registration and source-provenance gates for composed Mossbell pieces."""
import hashlib
import importlib.util
import json
from pathlib import Path
import unittest
from PIL import Image

spec = importlib.util.spec_from_file_location('compose',Path(__file__).with_name('mossbell-compose.py'))
c = importlib.util.module_from_spec(spec);spec.loader.exec_module(c)


class CompositionTests(unittest.TestCase):
    def test_saved_frames_and_source_crops_match_recorded_hashes(self):
        m=json.loads((c.DEST/'composition.json').read_text())
        for name,record in m['outputs'].items():
            self.assertEqual(hashlib.sha256((c.DEST/name).read_bytes()).hexdigest(),record['sha256'])
            with Image.open(c.DEST/name) as im:
                self.assertEqual(list(im.size),record['wh'])
        for kind,record in m['sources'].items():
            self.assertEqual(c.slots.digest(c.DEST/f'source-{kind}.png'),record['crop_sha256'])

    def test_caps_are_never_stretched_or_recoloured(self):
        for kind,caps in [('flippers',(4,3)),('root',(10,13)),('crown',(3,3))]:
            src=Image.open(c.DEST/f'native-{kind}.png')
            for stretch in [False,True]:
                out=c.three_slice(src,src.width+100,caps,stretch)
                self.assertEqual(out.crop((0,0,caps[0],src.height)).tobytes(),src.crop((0,0,caps[0],src.height)).tobytes())
                self.assertEqual(out.crop((out.width-caps[1],0,out.width,src.height)).tobytes(),src.crop((src.width-caps[1],0,src.width,src.height)).tobytes())

    def test_exact_contracts_palette_and_old_crown_air_gap(self):
        for tag,paddle,pair,root,gate in [('legacy',42,114,179,51),('current',53,144,229,65)]:
            with Image.open(c.DEST/f'{tag}-paddle.png') as im:
                self.assertEqual(im.size,(paddle,5))
            for stage in range(4):
                for kind,size in [('flippers',(pair,26)),('root',(root,11)),('crown',(gate,24))]:
                    im=Image.open(c.DEST/f'{tag}-{kind}-{stage}.png').convert('RGBA')
                    self.assertEqual(im.size,size)
                    for p in im.get_flattened_data():
                        self.assertIn(p[3],(0,255))
                        if p[3]:
                            self.assertIn(p[:3],c.slots.palette_cache)
                            self.assertLessEqual(max(p[:3]),224)
                old=Image.open(c.DEST.parent/f'crown-vines-{stage}.png').convert('RGBA')
                new=Image.open(c.DEST/f'{tag}-crown-{stage}.png').convert('RGBA')
                for y in range(24):
                    for x in range(gate):
                        p=old.getpixel((x*old.width//gate,y))
                        self.assertEqual(bool(new.getpixel((x,y))[3]),bool(p[3] and p[:3]!=(24,43,43)))

    def test_every_pose_keeps_physics_pivots_and_tips_connected(self):
        for scale,tag in [(5/4,'legacy'),(8/5,'current')]:
            fx=lambda x:int(x*scale)
            span=fx(70)-fx(37)
            for state in range(4):
                im=Image.open(c.DEST/f'{tag}-flippers-{state}.png').convert('RGBA')
                for side in range(2):
                    for along in range(span+1):
                        x=3+along if side==0 else im.width-4-along
                        y=11+int(along*(-8 if state&(1<<side) else 11)/span)
                        self.assertEqual(im.getpixel((x,y))[3],255,(tag,state,side,along))


if __name__=='__main__':unittest.main()
