#!/usr/bin/env python3
"""Silhouette controls for the actual-cell BOSS-3 acceptance gate."""
import copy
import runpy
import unittest
from pathlib import Path

GATE=runpy.run_path(str(Path(__file__).with_name('check-boss-art-sendback.py')))

class CellShapes(unittest.TestCase):
    def setUp(self):
        self.background=[{'glyph':' ','fg':[220,200,160],'bg':[0,0,0]} for _ in range(96*28)]
        self.raw=copy.deepcopy(self.background)

    def halves(self):
        return GATE['half_cells'](GATE['shape'](self.raw,self.background,(0,0,8,16)))

    def test_unused_bright_foreground_is_not_coverage(self):
        self.assertEqual(self.halves(),set())

    def test_recolouring_a_blob_does_not_animate_its_silhouette(self):
        self.raw[0]={'glyph':'▀','fg':[200,170,100],'bg':[0,0,0]}
        before=self.halves()
        self.raw[0]['fg']=[160,120,80]
        self.assertEqual(before,self.halves())

    def test_turning_between_halves_changes_two_half_cells(self):
        self.raw[0]={'glyph':'▀','fg':[200,170,100],'bg':[0,0,0]}
        before=self.halves()
        self.raw[0]['glyph']='▄'
        self.assertEqual(len(before^self.halves()),2)

    def test_braille_uses_the_emitted_dot_bits(self):
        self.raw[0]={'glyph':'⠁','fg':[200,170,100],'bg':[0,0,0]}
        self.assertEqual(GATE['shape'](self.raw,self.background,(0,0,8,16))[(0,0)],1)

class WardNotches(unittest.TestCase):
    def test_solid_bars_have_no_ward_cutouts(self):
        self.assertEqual(GATE['notches']({(x,y):255 for x in range(3) for y in range(2)}),0)

    def test_a_coarse_c_shaped_ward_keeps_its_open_notch(self):
        self.assertEqual(GATE['notches']({(0,0):255,(0,1):15,(1,0):15,(1,1):15}),1)

if __name__=='__main__':unittest.main()
