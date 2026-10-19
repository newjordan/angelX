#!/usr/bin/env python3
"""Synthetic checks for crop order, edge cleaning and registration invariants."""
import importlib.util
from pathlib import Path
import unittest
import tempfile
import hashlib
import json
from types import SimpleNamespace
from PIL import Image, ImageDraw
spec=importlib.util.spec_from_file_location('slots',Path(__file__).with_name('slot-art-import.py'))
s=importlib.util.module_from_spec(spec);spec.loader.exec_module(s)

class ImportTests(unittest.TestCase):
    def test_weapon_height_does_not_reorder_row(self):
        pieces=[{'index':0,'source_rect_xywh':[10,10,20,100]}, {'index':1,'source_rect_xywh':[80,30,20,80]}, {'index':2,'source_rect_xywh':[45,60,20,50]}, {'index':3,'source_rect_xywh':[10,200,20,100]}]
        self.assertEqual([p['index'] for p in s.reading_order(pieces)],[0,2,1,3])
    def test_despill_preserves_alpha_and_interior(self):
        im=Image.new('RGBA',(20,20));d=ImageDraw.Draw(im);d.rectangle((2,2,17,17),fill=(170,30,150,200));d.rectangle((5,5,14,14),fill=(55,70,85,253))
        clean,m=s.despill(im)
        self.assertGreater(m['fringe_before'],0);self.assertEqual(m['fringe_after_boundary'],0)
        self.assertEqual(clean.getchannel('A').tobytes(),im.getchannel('A').tobytes())
        self.assertEqual(clean.getpixel((10,10)),im.getpixel((10,10)))
        self.assertEqual(clean.getpixel((2,2))[:3],(55,70,85))
    def test_opaque_ink_contour_is_preserved(self):
        im=Image.new('RGBA',(20,20));d=ImageDraw.Draw(im)
        d.rectangle((2,2,17,17),fill=(18,14,22,253));d.rectangle((5,5,14,14),fill=(100,125,155,253))
        clean,_=s.despill(im)
        self.assertEqual(im.tobytes(),clean.tobytes())
    def test_sampling_is_binary_palette_and_repeatable(self):
        im=Image.new('RGBA',(40,80),(53,76,98,252))
        ref,a=s.sample(im,(32,40),(20,80),(12,40),.5)
        _,b=s.sample(im,(32,40),(20,80),(12,40),.5)
        self.assertEqual(s.png(a),s.png(b));self.assertEqual(a.getbbox(),(2,0,22,40))
        self.assertTrue(all(c[3] in (0,255) for c in a.get_flattened_data()))
        self.assertTrue(all(c[:3] in s.palette_cache for c in a.get_flattened_data() if c[3]))
    def test_cover_crops_without_stretching_or_transparent_gutters(self):
        im=Image.new('RGBA',(90,70));d=ImageDraw.Draw(im)
        d.rectangle((5,15,84,64),fill=(80,95,110,255))
        d.rectangle((40,15,49,64),fill=(160,120,80,255))
        ref,a=s.sample(im,(16,16),(45,35),(8,8),.1,'cover',{'value':.7,'contrast':.7},.045,True)
        self.assertEqual(a.getbbox(),(0,0,16,16))
        self.assertTrue(all(c[3]==255 for c in a.get_flattened_data()))
        self.assertEqual(ref.getpixel((0,8)),ref.getpixel((15,8)))
        self.assertNotEqual(ref.getpixel((8,8)),ref.getpixel((0,8)))
        _,b=s.sample(im,(16,16),(45,35),(8,8),.1,'cover',{'value':.7,'contrast':.7},.045,True)
        self.assertEqual(s.png(a),s.png(b))

    def test_upscale_is_rejected(self):
        with self.assertRaises(AssertionError):s.sample(Image.new('RGBA',(4,4)),(8,8),(2,4),(4,8),2)

    def test_reviewed_nonblue_material_and_sparse_erasure(self):
        native=Image.new('RGBA',(8,8),s.palette_cache[5]+(255,))
        piece={'reviewed_processing':{'reviewed':True,'replace_inferred_material':True,
               'material_runs':[[1,1,3,2]],'erase_runs':[[1,2,1]],'cleanup_rects':[[0,6,8,2]]}}
        result,mask,fingerprint=s.reviewed_native(native,native,piece,'v1',Path('.'),1,[4,8])
        self.assertEqual(mask.getpixel((1,1)),(140,140,140,255))
        self.assertEqual(mask.getpixel((2,1)),(0,0,0,0))
        self.assertEqual(result.getpixel((3,6)),(0,0,0,0))
        self.assertEqual(result.getpixel((1,1)),native.getpixel((1,1)))
        self.assertEqual(len(fingerprint),64)
        piece['reviewed_processing']['erase_runs'].append([2,1,2])
        self.assertNotEqual(fingerprint,s.reviewed_native(native,native,piece,'v1',Path('.'),1,[4,8])[2])

    def test_source_material_mask_hash_and_registration(self):
        with tempfile.TemporaryDirectory() as d:
            dest=Path(d);mask=Image.new('RGBA',(16,16));mask.putpixel((8,8),(70,70,70,255))
            path=dest/'cloth.png';path.write_bytes(s.png(mask))
            piece={'source_rect_xywh':[20,20,16,16],'source_pivot_xy':[8,16],
                   'reviewed_processing':{'reviewed':True,'material_mask':{'path':'cloth.png','sha256':s.digest(path),'space':'source'}}}
            im=Image.new('RGBA',(16,16),(70,70,70,255))
            _,result,_=s.reviewed_native(im,im,piece,'v1',dest,1,[8,16])
            self.assertEqual(result.tobytes(),mask.tobytes())
            path.write_bytes(s.png(im))
            with self.assertRaisesRegex(AssertionError,'mask changed'):s.reviewed_native(im,im,piece,'v1',dest,1,[8,16])

    def test_cleanup_cannot_escape_canvas_or_bypass_review(self):
        im=Image.new('RGBA',(4,4))
        for control in ({'cleanup_rects':[[0,0,2,2]]},{'reviewed':True,'cleanup_rects':[[3,0,2,2]]}):
            with self.assertRaises(AssertionError):s.reviewed_native(im,im,{'reviewed_processing':control},'v1',Path('.'),1,[2,4])

    def test_legacy_processing_is_a_noop(self):
        im=Image.new('RGBA',(4,4),(42,32,23,255))
        result,mask,fingerprint=s.reviewed_native(im,im,{},'v1',Path('.'),1,[2,4])
        self.assertIs(result,im);self.assertIsNone(mask);self.assertIsNone(fingerprint)

    def test_reviewed_subcrop_preserves_original_source_pivot(self):
        piece={'source_pivot_xy':[10,20],'reviewed_processing':{'reviewed':True,'source_crop_xywh':[3,4,12,16]}}
        im=Image.new('RGBA',(20,24),(104,70,41,255));crop,pivot=s.registered_crop(piece,im)
        self.assertEqual(crop.size,(12,16));self.assertEqual(pivot,[7,16]);self.assertEqual(piece['source_pivot_xy'],[10,20])
        _,a=s.sample(im,(20,24),piece['source_pivot_xy'],[10,24],.5)
        _,b=s.sample(crop,(20,24),pivot,[10,24],.5)
        self.assertEqual(a.getpixel((10,20)),b.getpixel((10,20)))

    def test_reviewed_subset_import_reproduces_and_rejects_source_drift(self):
        with tempfile.TemporaryDirectory() as d:
            root=Path(d);source=root/'delivery';source.mkdir();dest=root/'out'/'test.knight';dest.mkdir(parents=True)
            sheet=Image.new('RGBA',(32,32),(104,70,41,255));(source/'source-sheet.png').write_bytes(s.png(sheet));(source/'notes.md').write_text('Reviewed synthetic material test.\n')
            meta={'pieces':[{'index':0,'source_rect_xywh':[0,0,16,16]}, {'index':1,'source_rect_xywh':[16,0,16,32]}]};(source/'source-sheet.json').write_bytes(s.js(meta))
            req={'role':'player knight','fit':'8x8','timing':'unchanged','geometry':'test','frame_groups':[]}
            mapping={'source_sha256':s.digest(source/'source-sheet.png'),'notes_sha256':s.digest(source/'notes.md'),
                     'registration':'shared-feet','sizes':{'v1':[8,8],'v2':[8,8]},'anchor_xy':[4,8],'anchor_canvas_wh':[8,8],
                     'expected_existing_pieces':2,'runtime':True,'scale_reference_pieces':[0],
                     'pieces':[{'piece':0,'source_rect_xywh':[0,0,16,16],'source_pivot_xy':[8,16],'name':'idle','state':'idle','frame':0,'approved':True,'runtime':True,
                                'reviewed_processing':{'reviewed':True,'replace_inferred_material':True,'material_runs':[[0,0,8,2]],'cleanup_rects':[[0,0,1,1]]}},
                               {'piece':1,'source_rect_xywh':[16,0,16,32],'source_pivot_xy':[8,32],'name':'extra','state':'extra','frame':0,'approved':False,'runtime':False}]}
            (dest/'test.knight.map.json').write_bytes(s.js(mapping))
            previous=s.OUTPUT;s.OUTPUT=dest.parent
            args=SimpleNamespace(source_dir=source,v1_height=None,v2_height=None,terminal_metrics=None,check=False)
            try:
                record=s.run('test.knight',req,args,'OK','')
                self.assertEqual(record['outputs']['v1/idle.png']['source_scale'],.5)
                args.check=True;s.run('test.knight',req,args,'OK','')
                mask=Image.open(dest/'v1/idle-seat-mask.png');self.assertEqual(mask.getpixel((1,0)),(140,140,140,255));self.assertEqual(mask.getpixel((0,0)),(0,0,0,0))
                mapping['scale_reference_pieces']=[1]
                with self.assertRaisesRegex(AssertionError,'approved active subset'):s.validate(mapping,req,meta,source/'source-sheet.png')
                (source/'source-sheet.png').write_bytes(s.png(Image.new('RGBA',(32,32))))
                with self.assertRaisesRegex(AssertionError,'source changed'):s.run('test.knight',req,args,'OK','')
            finally:s.OUTPUT=previous

if __name__=='__main__':unittest.main()
