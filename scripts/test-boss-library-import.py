#!/usr/bin/env python3
"""Source fidelity must reject silhouette damage and painted-over identity."""
import importlib.util
import unittest
import runpy
from pathlib import Path
from PIL import Image
spec = importlib.util.spec_from_file_location('importer', Path(__file__).with_name('boss-library-import.py'))
A = importlib.util.module_from_spec(spec); spec.loader.exec_module(A)

class Fidelity(unittest.TestCase):
    def test_floor_contrast_thresholds_reject_each_failure(self):
        gate = runpy.run_path(str(Path(__file__).with_name('check-boss-library.py')))
        self.assertEqual(gate['failures'](15, .80, .50), [])
        self.assertIn('body median floor contrast', gate['failures'](14.99, .80, .50))
        self.assertIn('body floor-contrast coverage', gate['failures'](15, .799, .50))
        self.assertIn('silhouette floor-contrast coverage', gate['failures'](15, .80, .499))

    def test_dark_body_passes_dark_floor_but_fails_a_matching_floor(self):
        gate = runpy.run_path(str(Path(__file__).with_name('check-boss-library.py')))
        points = {(0, 0), (1, 0)}
        body = lambda p: (55, 65, 90)
        dark = gate['contrast_metrics'](body, lambda p: (10, 12, 18), points, points)
        matching = gate['contrast_metrics'](body, body, points, points)
        self.assertEqual(gate['failures'](dark['median_delta'], dark['visible_fraction'], dark['edge_fraction']), [])
        self.assertEqual(matching['median_delta'], 0)
        self.assertEqual(matching['visible_fraction'], 0)
        self.assertEqual(matching['edge_fraction'], 0)

    def test_local_floor_and_contour_cannot_be_hidden_by_a_bright_core(self):
        gate = runpy.run_path(str(Path(__file__).with_name('check-boss-library.py')))
        points = {(x, 0) for x in range(10)}
        edge = {(0, 0), (9, 0)}
        body = lambda p: (10, 10, 10) if p in edge else (160, 160, 160)
        result = gate['contrast_metrics'](body, lambda p: (10, 10, 10), points, edge)
        self.assertGreater(result['median_delta'], 15)
        self.assertEqual(result['visible_fraction'], .8)
        self.assertEqual(result['edge_fraction'], 0)
        # A remote black floor swatch would falsely approve this bright floor.
        same = gate['contrast_metrics'](body, body, points, edge)
        self.assertEqual(same['visible_fraction'], 0)

    def test_silhouette_counts_dark_contours_and_canvas_boundaries(self):
        gate = runpy.run_path(str(Path(__file__).with_name('check-boss-library.py')))
        points = {(x, y) for x in range(7) for y in range(7)}
        edge = gate['silhouette_edge'](points, 2)
        self.assertIn((0, 0), edge)
        self.assertNotIn((3, 3), edge)
        result = gate['contrast_metrics'](lambda p: (10, 10, 10), lambda p: (90, 90, 90), points, edge)
        self.assertEqual(result['edge_fraction'], 1)
        self.assertEqual(result['visible_fraction'], 0)

    def test_warning_pigment_mask_rejects_material_and_decodes_actual_glyph(self):
        gate = runpy.run_path(str(Path(__file__).with_name('check-boss-art-sendback.py')))
        ink = (120,144,168)
        raw = [{'glyph':'▀','fg':list(ink),'bg':[0,0,0]}]
        self.assertEqual(gate['pigment_shape'](raw,(0,0,8,16),{ink})[(0,0)],15)
        raw[0]['fg'] = [228,210,175]
        self.assertEqual(gate['pigment_shape'](raw,(0,0,8,16),{ink})[(0,0)],0)

    def test_deleted_spear_fails_silhouette(self):
        src = A.sample(A.bank('iron-tusk-chief', 'overhead')[0], (46,54))
        native = A.convert(src)
        native.paste((0,0,0,0),(0,0,23,54))
        self.assertLess(A.fidelity(src,native)['silhouette_iou'], A.LIMITS['silhouette_iou'])

    def test_flat_body_fails_colour_and_edge(self):
        src = A.sample(A.bank('cinderjaw', 'overhead')[0], (58,48))
        native = A.convert(src)
        native.putdata([(25,29,34,255) if c[3] else c for c in native.get_flattened_data()])
        scores = A.fidelity(src,native)
        self.assertLess(scores['key_colour_agreement'], A.LIMITS['key_colour_agreement'])
        # A dark fill naturally agrees with many source contour samples.
        # Destroy the contour separately to exercise its own rejection gate.
        native.putdata([(200,180,120,255) if c[3] else c for c in native.get_flattened_data()])
        scores = A.fidelity(src,native)
        self.assertLess(scores['edge_agreement'], A.LIMITS['edge_agreement'])

    def test_failed_fidelity_candidate_cannot_be_activated(self):
        import json
        for boss, config in A.BOSSES.items():
            record=json.loads((A.OUTPUT/boss/'library-import.json').read_text())
            if not record.get('gate_failures'):
                continue
            self.assertFalse(config['active'])
            config['active']=True
            try:
                with self.assertRaisesRegex(AssertionError,'key_colour_agreement'):
                    A.run(boss,check=True)
            finally:
                config['active']=False

    def test_transparency_and_palette_are_exact(self):
        for boss in A.BOSSES:
            for pose in A.bank(boss,'overhead'):
                src = A.sample(pose,A.BOSSES[boss]['v2']); native = A.convert(src)
                self.assertEqual(A.fidelity(src,native)['silhouette_iou'],1)
                self.assertTrue(all(c[3] in (0,255) for c in native.get_flattened_data()))
                self.assertTrue(all(c[:3] in A.palette() for c in native.get_flattened_data() if c[3]))

if __name__ == '__main__': unittest.main()
