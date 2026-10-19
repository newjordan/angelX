#!/usr/bin/env python3
"""Regression fixtures for measurements that must reject a bad art batch."""
import runpy
import copy
import json
import unittest
from pathlib import Path
from unittest.mock import patch
from PIL import Image
art = runpy.run_path(str(Path(__file__).with_name('boss-art.py')))

class PlayerGates(unittest.TestCase):
    def test_library_original_gate_uses_the_paired_floor_bank(self):
        entry = next(e for e in json.loads(Path(__file__).with_name('boss-art-crypt.json').read_text())
                     if Path(e['boss']).stem == 'waxen-warden')
        captures, floors = Path('/unused/legacy-captures'), Path('/unused/paired-floors')
        calls = []
        def floor_gate(selected, path):
            calls.append((set(selected), path))
            return ['local contrast failed'], []
        with patch.object(art['runpy'], 'run_path', return_value={'floor_contrast_gates': floor_gate}):
            self.assertEqual(art['original_gates']([entry], captures, contrast=floors), ['local contrast failed'])
        self.assertEqual(calls, [({'waxen-warden'}, floors)])

    def test_identity_review_rejects_missing_and_uninspected_props(self):
        entry = json.loads((Path(__file__).with_name('boss-art-reliquary.json')).read_text())[0]
        entry = copy.deepcopy(entry)
        entry['review'].pop('identity_props', None)
        self.assertTrue(any('missing identity-props' in e for e in art['identity_gates']([entry])))
        entry['review']['identity_props'] = {
            'props': ['locked treasure chest', 'hood and teal eyes'],
            'source_text': 'not the boss text',
            'compared_original_1x': True,
            'compared_original_cells': True,
            'inspected_cells_2x': False,
            'evidence': [],
        }
        errors = art['identity_gates']([entry])
        self.assertTrue(any('unchanged .boss text' in e for e in errors))
        self.assertTrue(any('inspected_cells_2x' in e for e in errors))
        self.assertTrue(any('both kits' in e for e in errors))

    def test_changed_archived_boss_also_requires_identity_review(self):
        entry = json.loads((Path(__file__).with_name('boss-art-crypt.json')).read_text())[0]
        entry = copy.deepcopy(entry)
        entry['review'].pop('identity_props', None)
        with patch.object(art['subprocess'], 'check_output', return_value=entry['output']+'/v1.png\n'):
            self.assertTrue(any('missing identity-props' in e for e in art['identity_gates']([entry])))

    def test_new_boss_cannot_use_the_archived_review_schema(self):
        entry = {'boss': 'cockpit/assets/dungeon/bosses/new-chief.boss',
                 'output': 'cockpit/assets/dungeon/bosses/new-chief', 'review': {}}
        self.assertTrue(any('missing identity-props' in e for e in art['identity_gates']([entry])))

    def test_colour_identity_is_required_when_an_archived_asset_changes(self):
        entry = copy.deepcopy(json.loads(Path(__file__).with_name('boss-art-pit-debt.json').read_text())[0])
        record = entry['review']['identity_props']
        for field in ('preserved_original_hues', 'preserved_original_value_contrast',
                      'judged_cells_2x_first', 'identity_colour_blocks_distinct'):
            record[field] = False
        with patch.object(art['subprocess'], 'check_output', return_value=entry['output']+'/v1.png\n'):
            errors = art['identity_gates']([entry])
        self.assertEqual(sum('colour identity checklist incomplete' in e for e in errors), 4)

    def test_original_gate_rejects_dimming_and_one_lost_body_pixel(self):
        self.assertEqual(art['presence_regressions']((50,100),(50,100)), [])
        self.assertEqual(art['presence_regressions']((50,100),(49.99,101)), ['median L*'])
        self.assertEqual(art['presence_regressions']((50,100),(80,99)), ['presence pixels'])

    def test_original_gate_counts_coverage_not_unused_foreground(self):
        cell={'glyph':' ', 'fg':[255,210,150], 'bg':[0,0,0]}
        self.assertEqual(art['terminal_colour'](cell,0,0),[0,0,0])
        cell['glyph']='▀'
        self.assertEqual(art['terminal_colour'](cell,0,0),cell['fg'])
        self.assertEqual(art['terminal_colour'](cell,0,2),cell['bg'])
        cell['glyph']='⠁'
        self.assertEqual(art['terminal_colour'](cell,0,0),cell['fg'])
        self.assertEqual(art['terminal_colour'](cell,1,0),cell['bg'])
        self.assertEqual(art['terminal_colour'](cell,0,1),cell['bg'])

    def test_original_gate_presence_uses_floor_and_exact_boundary(self):
        floor=(38,38,38)
        threshold=art['lightness'](floor)+15
        self.assertEqual(art['presence']([threshold-.001,threshold],floor)[1],1)
        self.assertEqual(art['body_points'](['.x','xx'],(4,4),(5,6)),
                         [(7,6),(8,6),(7,7),(8,7),(5,8),(6,8),(7,8),(8,8),(5,9),(6,9),(7,9),(8,9)])

    def test_cie_black_white_and_two_step_boundary(self):
        self.assertAlmostEqual(art['lightness']((0,0,0)),0)
        self.assertAlmostEqual(art['lightness']((255,255,255)),100)
        im=Image.new('RGBA',(4,4),(38,38,38,255))
        edge,_=art['separation'](im,(0,0,0),(0,0,2,2),(2,2,4,4))
        self.assertGreater(edge,15)
        im.putpixel((0,0),(37,37,37,255))
        edge,_=art['separation'](im,(0,0,0),(0,0,2,2),(2,2,4,4))
        self.assertLess(edge,15)

    def test_eye_cannot_stand_in_for_head_material(self):
        im=Image.new('RGBA',(8,8),(60,60,60,255))
        im.putpixel((2,2),(240,210,120,255))
        _,material=art['separation'](im,(0,0,0),(0,0,8,4),(0,4,8,8))
        self.assertEqual(material,0)

    def test_rectangle_inside_rounded_light_fails(self):
        colours={'b':(60,40,40),'r':(180,80,30),'c':(255,200,96)}
        pixels=[[x,y,'r'] for y in range(1,6) for x in range(1,6) if (x,y) not in [(1,1),(5,1),(1,5),(5,5)]]
        pixels += [[x,y,'c'] for y in range(2,5) for x in range(2,5)]
        # Use final, unique pixels as recipes do.
        pixels=list({(x,y):(x,y,k) for x,y,k in pixels}.values())
        spec={'rows':['bbbbbbb']*7,'signature_lights':[{'name':'maw','hues':{k:'#'+bytes(v).hex() for k,v in colours.items()},'pixels':pixels,'falloff':[[0,2,'b']]}]}
        self.assertTrue(any('four-sided' in e for e in art['signature_gate'](spec,colours)))

    def test_three_by_two_light_bar_fails_but_cut_corner_passes(self):
        colours={'b':(60,40,40),'c':(255,200,96)}
        pixels=[[x,y,'c'] for y in range(1,3) for x in range(1,4)]
        spec={'rows':['bbbbb']*4,'signature_lights':[{'name':'gill','hues':{k:'#'+bytes(v).hex() for k,v in colours.items()},'pixels':pixels,'falloff':[[0,1,'b']]}]}
        self.assertTrue(any('four-sided' in e for e in art['signature_gate'](spec,colours)))
        pixels.pop()
        self.assertEqual(art['signature_gate'](spec,colours),[])

    def test_falloff_requires_real_body_and_one_step(self):
        colours={'b':(90,80,70),'r':(255,200,96)}
        spec={'rows':['bbbbb']*5,'signature_lights':[{'name':'eye','hues':{k:'#'+bytes(v).hex() for k,v in colours.items()},'pixels':[[2,2,'r']],'falloff':[[1,2,'r']]}]}
        self.assertTrue(any('one ramp step' in e for e in art['signature_gate'](spec,colours)))
        spec['signature_lights'][0]['falloff']=[[1,2,'b']]
        self.assertEqual(art['signature_gate'](spec,colours),[])

if __name__=='__main__': unittest.main()
