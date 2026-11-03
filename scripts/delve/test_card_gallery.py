"""Offline renderer regression tests: python3 -m unittest discover -s scripts/delve -p 'test_card_gallery.py'."""
import tempfile
import unittest
import xml.etree.ElementTree as ET
from pathlib import Path

from card_gallery import ROOT, audit, gallery, load_card
from inks_hex import INK_RS, inks


class CardGalleryTests(unittest.TestCase):
    def test_rooftop_and_furnace_cards(self):
        palette = {key: rgb for key, rgb, _ in inks(INK_RS.read_text())}
        directory = ROOT / 'cockpit/assets/dungeon/cards'
        registry = (ROOT / 'crates/delve/src/rules/cards.rs').read_text()
        result = audit(list(directory.glob('*.card')), palette)
        for stem, kind, effects in (
                ('widow-kite', 'spell', 'cooldown 23\ncast ring\nblink 2'),
                ('cinder-seahorse', 'hold', 'pierce 1\nvelocity 8')):
            target = directory / (stem + '.card')
            with self.subTest(card=stem):
                name, actual_kind, rows = load_card(target, palette)
                self.assertEqual(actual_kind, kind)
                self.assertLessEqual(len(name), 22)
                self.assertEqual([len(row) for row in rows], [16] * 16)
                self.assertIn(effects, target.read_text())
                self.assertIn(f'("{stem}", include_str!', registry)
                for signal in ('similar_silhouettes', 'translated_silhouettes'):
                    self.assertFalse(any(str(target) in pair['paths'] for pair in result[signal]))
                self.assertFalse(any(str(target) in group for group in result['exact_rendered_duplicates']))

    def test_dragonfruit_kebab(self):
        palette = {key: rgb for key, rgb, _ in inks(INK_RS.read_text())}
        directory = ROOT / 'cockpit/assets/dungeon/cards'
        target = directory / 'dragonfruit-kebab.card'
        name, kind, rows = load_card(target, palette)
        self.assertEqual(name, 'Dragonfruit Kebab')
        self.assertEqual(kind, 'play')
        self.assertEqual([len(row) for row in rows], [16] * 16)
        self.assertIn('heal 12\nnova 28', target.read_text())
        registry = (ROOT / 'crates/delve/src/rules/cards.rs').read_text()
        self.assertIn('("dragonfruit-kebab", include_str!', registry)
        result = audit(list(directory.glob('*.card')), palette)
        for signal in ('similar_silhouettes', 'translated_silhouettes'):
            self.assertFalse(any(str(target) in pair['paths'] for pair in result[signal]))
        self.assertFalse(any(str(target) in group for group in result['exact_rendered_duplicates']))

    def test_matchstick_ferry_and_pearl_diver_fork(self):
        palette = {key: rgb for key, rgb, _ in inks(INK_RS.read_text())}
        directory = ROOT / 'cockpit/assets/dungeon/cards'
        registry = (ROOT / 'crates/delve/src/rules/cards.rs').read_text()
        result = audit(list(directory.glob('*.card')), palette)
        for stem, kind, effects in (
                ('matchstick-ferry', 'spell', 'cooldown 18\ncast bolt\nblink 2'),
                ('pearl-diver-fork', 'hold', 'fork 1\nvelocity 6')):
            target = directory / (stem + '.card')
            with self.subTest(card=stem):
                name, actual_kind, rows = load_card(target, palette)
                self.assertEqual(actual_kind, kind)
                self.assertLessEqual(len(name), 22)
                self.assertEqual([len(row) for row in rows], [16] * 16)
                self.assertIn(effects, target.read_text())
                self.assertIn(f'("{stem}", include_str!', registry)
                for signal in ('similar_silhouettes', 'translated_silhouettes'):
                    self.assertFalse(any(str(target) in pair['paths'] for pair in result[signal]))
                self.assertFalse(any(str(target) in group for group in result['exact_rendered_duplicates']))


    def test_sawtooth_menu_and_stormpipe_organ(self):
        palette = {key: rgb for key, rgb, _ in inks(INK_RS.read_text())}
        directory = ROOT / 'cockpit/assets/dungeon/cards'
        registry = (ROOT / 'crates/delve/src/rules/cards.rs').read_text()
        result = audit(list(directory.glob('*.card')), palette)
        for stem, kind, effects in (
                ('stormpipe-organ', 'spell', 'cooldown 22\ncast ring\nward 3'),
                ('sawtooth-menu', 'hold', 'pierce 1\ndamage 4')):
            target = directory / (stem + '.card')
            with self.subTest(card=stem):
                name, actual_kind, rows = load_card(target, palette)
                self.assertEqual(actual_kind, kind)
                self.assertLessEqual(len(name), 22)
                self.assertEqual([len(row) for row in rows], [16] * 16)
                self.assertIn(effects, target.read_text())
                self.assertIn(f'("{stem}", include_str!', registry)
                for signal in ('similar_silhouettes', 'translated_silhouettes'):
                    self.assertFalse(any(str(target) in pair['paths'] for pair in result[signal]))
                self.assertFalse(any(str(target) in group for group in result['exact_rendered_duplicates']))

    def test_scorpion_tap_and_mooncrab_cracker(self):
        palette = {key: rgb for key, rgb, _ in inks(INK_RS.read_text())}
        directory = ROOT / 'cockpit/assets/dungeon/cards'
        registry = (ROOT / 'crates/delve/src/rules/cards.rs').read_text()
        result = audit(list(directory.glob('*.card')), palette)
        for stem, kind, effects in (
                ('scorpion-tap', 'spell', 'cooldown 19\ncast bolt\nward 2'),
                ('mooncrab-cracker', 'play', 'heal 18\nward 2')):
            target = directory / (stem + '.card')
            with self.subTest(card=stem):
                name, actual_kind, rows = load_card(target, palette)
                self.assertEqual(actual_kind, kind)
                self.assertLessEqual(len(name), 22)
                self.assertEqual([len(row) for row in rows], [16] * 16)
                self.assertIn(effects, target.read_text())
                self.assertIn(f'("{stem}", include_str!', registry)
                for signal in ('similar_silhouettes', 'translated_silhouettes'):
                    self.assertFalse(any(str(target) in pair['paths'] for pair in result[signal]))
                self.assertFalse(any(str(target) in group for group in result['exact_rendered_duplicates']))

    def test_grillmaster_and_drowned_service_relics(self):
        palette = {key: rgb for key, rgb, _ in inks(INK_RS.read_text())}
        directory = ROOT / 'cockpit/assets/dungeon/cards'
        registry = (ROOT / 'crates/delve/src/rules/cards.rs').read_text()
        result = audit(list(directory.glob('*.card')), palette)
        for stem, kind, effects in (
                ('undertow-bell', 'spell', 'cooldown 20\ncast ring\nward 2'),
                ('mantis-tongs', 'hold', 'fork 1\nvelocity 4')):
            target = directory / (stem + '.card')
            with self.subTest(card=stem):
                name, actual_kind, rows = load_card(target, palette)
                self.assertEqual(actual_kind, kind)
                self.assertLessEqual(len(name), 22)
                self.assertEqual([len(row) for row in rows], [16] * 16)
                self.assertIn(effects, target.read_text())
                self.assertIn(f'("{stem}", include_str!', registry)
                for signal in ('similar_silhouettes', 'translated_silhouettes'):
                    self.assertFalse(any(str(target) in pair['paths'] for pair in result[signal]))
                self.assertFalse(any(str(target) in group for group in result['exact_rendered_duplicates']))

    def test_spectral_billiards_and_dock_relics(self):
        palette = {key: rgb for key, rgb, _ in inks(INK_RS.read_text())}
        directory = ROOT / 'cockpit/assets/dungeon/cards'
        registry = (ROOT / 'crates/delve/src/rules/cards.rs').read_text()
        result = audit(list(directory.glob('*.card')), palette)
        for stem, kind, effects in (
                ('phantom-cue', 'spell', 'cooldown 18\ncast bolt\nblink 2'),
                ('harpoon-ratchet', 'hold', 'pierce 1\ndamage 6')):
            target = directory / (stem + '.card')
            with self.subTest(card=stem):
                name, actual_kind, rows = load_card(target, palette)
                self.assertEqual(actual_kind, kind)
                self.assertLessEqual(len(name), 22)
                self.assertEqual([len(row) for row in rows], [16] * 16)
                self.assertIn(effects, target.read_text())
                self.assertIn(f'("{stem}", include_str!', registry)
                for signal in ('similar_silhouettes', 'translated_silhouettes'):
                    self.assertFalse(any(str(target) in pair['paths'] for pair in result[signal]))
                self.assertFalse(any(str(target) in group for group in result['exact_rendered_duplicates']))

    def test_shooter_workshop_relics(self):
        palette = {key: rgb for key, rgb, _ in inks(INK_RS.read_text())}
        directory = ROOT / 'cockpit/assets/dungeon/cards'
        registry = (ROOT / 'crates/delve/src/rules/cards.rs').read_text()
        result = audit(list(directory.glob('*.card')), palette)
        for stem, kind, effects in (
                ('railjaw-stapler', 'hold', 'pierce 1\nvelocity 10'),
                ('copperhead-spool', 'hold', 'fork 1')):
            target = directory / (stem + '.card')
            with self.subTest(card=stem):
                name, actual_kind, rows = load_card(target, palette)
                self.assertEqual(actual_kind, kind)
                self.assertLessEqual(len(name), 22)
                self.assertEqual([len(row) for row in rows], [16] * 16)
                self.assertIn(effects, target.read_text())
                self.assertIn(f'("{stem}", include_str!', registry)
                for signal in ('similar_silhouettes', 'translated_silhouettes'):
                    self.assertFalse(any(str(target) in pair['paths'] for pair in result[signal]))
                self.assertFalse(any(str(target) in group for group in result['exact_rendered_duplicates']))

    def test_harbor_telegraph_and_comet_snack(self):
        palette = {key: rgb for key, rgb, _ in inks(INK_RS.read_text())}
        directory = ROOT / 'cockpit/assets/dungeon/cards'
        registry = (ROOT / 'crates/delve/src/rules/cards.rs').read_text()
        result = audit(list(directory.glob('*.card')), palette)
        for stem, kind, effects in (
                ('lobster-telegraph', 'spell', 'cooldown 20\ncast bolt\nward 3'),
                ('comet-dumpling', 'play', 'heal 17\nnova 19')):
            target = directory / (stem + '.card')
            with self.subTest(card=stem):
                name, actual_kind, rows = load_card(target, palette)
                self.assertEqual(actual_kind, kind)
                self.assertLessEqual(len(name), 22)
                self.assertEqual([len(row) for row in rows], [16] * 16)
                self.assertIn(effects, target.read_text())
                self.assertIn(f'("{stem}", include_str!', registry)
                for signal in ('similar_silhouettes', 'translated_silhouettes'):
                    self.assertFalse(any(str(target) in pair['paths'] for pair in result[signal]))
                self.assertFalse(any(str(target) in group for group in result['exact_rendered_duplicates']))

    def test_midnight_music_and_dockside_snack(self):
        palette = {key: rgb for key, rgb, _ in inks(INK_RS.read_text())}
        directory = ROOT / 'cockpit/assets/dungeon/cards'
        registry = (ROOT / 'crates/delve/src/rules/cards.rs').read_text()
        result = audit(list(directory.glob('*.card')), palette)
        for stem, kind, effects in (
                ('witching-turntable', 'spell', 'cooldown 21\ncast ring\nward 3'),
                ('candlefish-skewer', 'play', 'heal 14\nnova 22')):
            target = directory / (stem + '.card')
            with self.subTest(card=stem):
                name, actual_kind, rows = load_card(target, palette)
                self.assertEqual(actual_kind, kind)
                self.assertLessEqual(len(name), 22)
                self.assertEqual([len(row) for row in rows], [16] * 16)
                self.assertIn(effects, target.read_text())
                self.assertIn(f'("{stem}", include_str!', registry)
                for signal in ('similar_silhouettes', 'translated_silhouettes'):
                    self.assertFalse(any(str(target) in pair['paths'] for pair in result[signal]))
                self.assertFalse(any(str(target) in group for group in result['exact_rendered_duplicates']))

    def test_bar_buckler_and_crab_pauldron(self):
        palette = {key: rgb for key, rgb, _ in inks(INK_RS.read_text())}
        directory = ROOT / 'cockpit/assets/dungeon/cards'
        registry = (ROOT / 'crates/delve/src/rules/cards.rs').read_text()
        result = audit(list(directory.glob('*.card')), palette)
        for stem in ('crabclaw-pauldron', 'last-call-buckler'):
            target = directory / (stem + '.card')
            with self.subTest(card=stem):
                name, kind, rows = load_card(target, palette)
                self.assertEqual(kind, 'hold')
                self.assertLessEqual(len(name), 22)
                self.assertEqual([len(row) for row in rows], [16] * 16)
                self.assertIn('drop 2\nchest 3\narmor 1\nart\n', target.read_text())
                self.assertIn(f'("{stem}", include_str!', registry)
                for signal in ('similar_silhouettes', 'translated_silhouettes'):
                    self.assertFalse(any(str(target) in pair['paths'] for pair in result[signal]))
                self.assertFalse(any(str(target) in group for group in result['exact_rendered_duplicates']))

    def test_viper_decanter_and_roulette_spur(self):
        palette = {key: rgb for key, rgb, _ in inks(INK_RS.read_text())}
        directory = ROOT / 'cockpit/assets/dungeon/cards'
        registry = (ROOT / 'crates/delve/src/rules/cards.rs').read_text()
        result = audit(list(directory.glob('*.card')), palette)
        for stem, kind, effects in [
                ('viper-decanter', 'play', 'nova 26\nheal 9'),
                ('roulette-spur', 'spell', 'cooldown 19\ncast ring\nblink 2')]:
            target = directory / (stem + '.card')
            with self.subTest(card=stem):
                name, actual_kind, rows = load_card(target, palette)
                self.assertEqual(actual_kind, kind)
                self.assertLessEqual(len(name), 22)
                self.assertIn(effects, target.read_text())
                self.assertIn('drop 2\nchest 3', target.read_text())
                self.assertIn(f'("{stem}", include_str!', registry)
                self.assertEqual([len(row) for row in rows], [16] * 16)
                self.assertLessEqual(len(next(line[5:] for line in target.read_text().splitlines() if line.startswith('text '))), 64)
                for signal in ('similar_silhouettes', 'translated_silhouettes'):
                    self.assertFalse(any(str(target) in pair['paths'] for pair in result[signal]))
                self.assertFalse(any(str(target) in group for group in result['exact_rendered_duplicates']))

    def test_railway_shaker_and_orchard_hourglass(self):
        palette = {key: rgb for key, rgb, _ in inks(INK_RS.read_text())}
        directory = ROOT / 'cockpit/assets/dungeon/cards'
        registry = (ROOT / 'crates/delve/src/rules/cards.rs').read_text()
        result = audit(list(directory.glob('*.card')), palette)
        for stem, kind, effects in [
                ('orchard-hourglass', 'play', 'heal 16\nward 3'),
                ('railway-shaker', 'spell', 'cooldown 17\ncast bolt\nward 2')]:
            target = directory / (stem + '.card')
            with self.subTest(card=stem):
                name, actual_kind, rows = load_card(target, palette)
                self.assertEqual(actual_kind, kind)
                self.assertLessEqual(len(name), 22)
                self.assertIn(effects, target.read_text())
                self.assertIn('drop 2\nchest 3', target.read_text())
                self.assertIn(f'("{stem}", include_str!', registry)
                self.assertEqual([len(row) for row in rows], [16] * 16)
                self.assertLessEqual(len(next(line[5:] for line in target.read_text().splitlines() if line.startswith('text '))), 64)
                for signal in ('similar_silhouettes', 'translated_silhouettes'):
                    self.assertFalse(any(str(target) in pair['paths'] for pair in result[signal]))
                self.assertFalse(any(str(target) in group for group in result['exact_rendered_duplicates']))

    def test_cellar_thunder_and_cutting_chorus(self):
        palette = {key: rgb for key, rgb, _ in inks(INK_RS.read_text())}
        directory = ROOT / 'cockpit/assets/dungeon/cards'
        registry = (ROOT / 'crates/delve/src/rules/cards.rs').read_text()
        result = audit(list(directory.glob('*.card')), palette)
        for stem, kind, effects in [
                ('blackcap-reserve', 'play', 'nova 30\nward 3'),
                ('crescendo-clef', 'spell', 'cooldown 18\ncast ring\nward 1')]:
            target = directory / (stem + '.card')
            with self.subTest(card=stem):
                name, actual_kind, rows = load_card(target, palette)
                self.assertEqual(actual_kind, kind)
                self.assertLessEqual(len(name), 22)
                self.assertIn(effects, target.read_text())
                self.assertIn('drop 2\nchest 3', target.read_text())
                self.assertIn(f'("{stem}", include_str!', registry)
                self.assertEqual([len(row) for row in rows], [16] * 16)
                self.assertLessEqual(len(next(line[5:] for line in target.read_text().splitlines() if line.startswith('text '))), 64)
                for signal in ('similar_silhouettes', 'translated_silhouettes'):
                    self.assertFalse(any(str(target) in pair['paths'] for pair in result[signal]))
                self.assertFalse(any(str(target) in group for group in result['exact_rendered_duplicates']))
        bottle = load_card(directory / 'blackcap-reserve.card', palette)[2]
        self.assertEqual(bottle[8][5:9], 'WYYW')
        clef = load_card(directory / 'crescendo-clef.card', palette)[2]
        self.assertEqual(clef[9][6], '.')  # Open negative space inside the curl.

    def test_eventide_fan_and_widows_toast(self):
        palette = {key: rgb for key, rgb, _ in inks(INK_RS.read_text())}
        directory = ROOT / 'cockpit/assets/dungeon/cards'
        registry = (ROOT / 'crates/delve/src/rules/cards.rs').read_text()
        result = audit(list(directory.glob('*.card')), palette)
        for stem, kind, effects in [
                ('eventide-fan', 'spell', 'cooldown 19\ncast ring\nward 2'),
                ('widows-toast', 'play', 'nova 24\nheal 10')]:
            target = directory / (stem + '.card')
            with self.subTest(card=stem):
                name, actual_kind, rows = load_card(target, palette)
                self.assertEqual(actual_kind, kind)
                self.assertLessEqual(len(name), 22)
                self.assertIn(effects, target.read_text())
                self.assertIn('drop 2\nchest 3', target.read_text())
                self.assertIn(f'("{stem}", include_str!', registry)
                self.assertEqual([len(row) for row in rows], [16] * 16)
                self.assertLessEqual(len(next(line[5:] for line in target.read_text().splitlines() if line.startswith('text '))), 64)
                for signal in ('similar_silhouettes', 'translated_silhouettes'):
                    self.assertFalse(any(str(target) in pair['paths'] for pair in result[signal]))
                self.assertFalse(any(str(target) in group for group in result['exact_rendered_duplicates']))
        fan = load_card(directory / 'eventide-fan.card', palette)[2]
        self.assertEqual(fan[6][6:9], 'hYh')  # Brass ribs separate the cyan leaves.
        toast = load_card(directory / 'widows-toast.card', palette)[2]
        self.assertEqual(toast[5][4:12], 'RRRRRRRR')  # Red wine beneath brass rim.
        self.assertEqual(toast[11][6:10], 'hYYh')  # Slender goblet stem.

    def test_ghostlight_lantern_and_kraken_corkscrew(self):
        palette = {key: rgb for key, rgb, _ in inks(INK_RS.read_text())}
        directory = ROOT / 'cockpit/assets/dungeon/cards'
        registry = (ROOT / 'crates/delve/src/rules/cards.rs').read_text()
        result = audit(list(directory.glob('*.card')), palette)
        for stem, kind, effects in [
                ('ghostlight-lantern', 'spell', 'cooldown 20\ncast bolt\nward 3'),
                ('kraken-corkscrew', 'play', 'heal 18\nward 4')]:
            target = directory / (stem + '.card')
            with self.subTest(card=stem):
                name, actual_kind, rows = load_card(target, palette)
                self.assertEqual(actual_kind, kind)
                self.assertLessEqual(len(name), 22)
                self.assertIn(effects, target.read_text())
                self.assertIn('drop 2\nchest 3', target.read_text())
                self.assertIn(f'("{stem}", include_str!', registry)
                self.assertEqual([len(row) for row in rows], [16] * 16)
                self.assertLessEqual(len(next(line[5:] for line in target.read_text().splitlines() if line.startswith('text '))), 64)
                for signal in ('similar_silhouettes', 'translated_silhouettes'):
                    self.assertFalse(any(str(target) in pair['paths'] for pair in result[signal]))
                self.assertFalse(any(str(target) in group for group in result['exact_rendered_duplicates']))
        lantern = load_card(directory / 'ghostlight-lantern.card', palette)[2]
        self.assertEqual(lantern[2][7:9], '..')  # Open carrying handle.
        self.assertEqual(lantern[7][7:9], 'WW')  # White flame core.
        screw = load_card(directory / 'kraken-corkscrew.card', palette)[2]
        self.assertEqual(screw[2][1:5], 'hYWh')  # Curled left tentacle.
        self.assertEqual(screw[11][6:10], 'hYWh')  # Alternating screw helix.

    def test_abyssal_bellhelm_and_pinwheel_break(self):
        palette = {key: rgb for key, rgb, _ in inks(INK_RS.read_text())}
        directory = ROOT / 'cockpit/assets/dungeon/cards'
        registry = (ROOT / 'crates/delve/src/rules/cards.rs').read_text()
        result = audit(list(directory.glob('*.card')), palette)
        for stem, kind, effects in [
                ('abyssal-bellhelm', 'hold', 'armor 1'),
                ('pinwheel-break', 'spell', 'cooldown 18\ncast ring\nblink 2')]:
            target = directory / (stem + '.card')
            with self.subTest(card=stem):
                name, actual_kind, rows = load_card(target, palette)
                self.assertEqual(actual_kind, kind)
                self.assertLessEqual(len(name), 22)
                self.assertIn(effects, target.read_text())
                self.assertIn('drop 2\nchest 3', target.read_text())
                self.assertIn(f'("{stem}", include_str!', registry)
                self.assertEqual([len(row) for row in rows], [16] * 16)
                self.assertLessEqual(len(next(line[5:] for line in target.read_text().splitlines() if line.startswith('text '))), 64)
                for signal in ('similar_silhouettes', 'translated_silhouettes'):
                    self.assertFalse(any(str(target) in pair['paths'] for pair in result[signal]))
                self.assertFalse(any(str(target) in group for group in result['exact_rendered_duplicates']))
        helm = load_card(directory / 'abyssal-bellhelm.card', palette)[2]
        self.assertEqual(helm[7][7:9], 'hh')  # Porthole crossbar.
        self.assertEqual(helm[14][2:14], 'YYYYYYYYYYYY')  # Flared collar.
        wheel = load_card(directory / 'pinwheel-break.card', palette)[2]
        self.assertEqual(wheel[1][8], 'W')  # Detached motion spark.
        self.assertEqual(wheel[7][2:6], 'CCCC')  # Horizontal energy fan.

    def test_undertow_anchor_and_stormglass_dial(self):
        palette = {key: rgb for key, rgb, _ in inks(INK_RS.read_text())}
        directory = ROOT / 'cockpit/assets/dungeon/cards'
        registry = (ROOT / 'crates/delve/src/rules/cards.rs').read_text()
        result = audit(list(directory.glob('*.card')), palette)
        for stem, expected_kind, effects in [
                ('undertow-anchor', 'spell', 'cooldown 18\ncast ring\nward 3'),
                ('stormglass-dial', 'play', 'nova 32\nward 2')]:
            target = directory / (stem + '.card')
            with self.subTest(card=stem):
                name, kind, rows = load_card(target, palette)
                self.assertEqual(kind, expected_kind)
                self.assertLessEqual(len(name), 22)
                self.assertIn(effects, target.read_text())
                self.assertIn('drop 2\nchest 3', target.read_text())
                self.assertIn(f'("{stem}", include_str!', registry)
                self.assertEqual([len(row) for row in rows], [16] * 16)
                self.assertLessEqual(len(next(line[5:] for line in target.read_text().splitlines() if line.startswith('text '))), 64)
                for signal in ('similar_silhouettes', 'translated_silhouettes'):
                    self.assertFalse(any(str(target) in pair['paths'] for pair in result[signal]))
                self.assertFalse(any(str(target) in group for group in result['exact_rendered_duplicates']))
        anchor = load_card(directory / 'undertow-anchor.card', palette)[2]
        self.assertEqual(anchor[1][7:9], '..')  # Open anchor eye.
        self.assertEqual(anchor[12][5:11], 'YYYYYY')  # Flared brass crown.
        dial = load_card(directory / 'stormglass-dial.card', palette)[2]
        self.assertEqual(dial[9][6:9], 'YYY')  # Right-angle clock hand.
        self.assertEqual(dial[1][9:12], 'CWC')  # Detached storm spark.

    def test_astral_barspoon_and_phoenix_matchbook(self):
        palette = {key: rgb for key, rgb, _ in inks(INK_RS.read_text())}
        directory = ROOT / 'cockpit/assets/dungeon/cards'
        registry = (ROOT / 'crates/delve/src/rules/cards.rs').read_text()
        result = audit(list(directory.glob('*.card')), palette)
        for stem, effects in [('astral-barspoon', 'heal 12\nward 3'),
                              ('phoenix-matchbook', 'nova 28\nheal 8')]:
            target = directory / (stem + '.card')
            with self.subTest(card=stem):
                name, kind, rows = load_card(target, palette)
                source = target.read_text()
                self.assertEqual(kind, 'play')
                self.assertLessEqual(len(name), 22)
                self.assertIn(effects, source)
                self.assertIn('drop 2\nchest 3', source)
                self.assertIn(f'("{stem}", include_str!', registry)
                self.assertEqual(len(rows), 16)
                self.assertTrue(all(len(row) == 16 for row in rows))
                self.assertLessEqual(len(next(line[5:] for line in source.splitlines() if line.startswith('text '))), 64)
                for signal in ('similar_silhouettes', 'translated_silhouettes'):
                    self.assertFalse(any(str(target) in pair['paths'] for pair in result[signal]))
                self.assertFalse(any(str(target) in group for group in result['exact_rendered_duplicates']))
        spoon = load_card(directory / 'astral-barspoon.card', palette)[2]
        self.assertEqual(spoon[5][4:6], '..')  # Open spoon bowl.
        self.assertEqual(spoon[2][10], 'C')  # Detached star.
        match = load_card(directory / 'phoenix-matchbook.card', palette)[2]
        self.assertEqual(match[1][8:11], 'OWO')  # Lit match above booklet.
        self.assertEqual(match[13][4:11], 'YhYhYhY')  # Striker teeth.

    def test_kettle_eclipse_and_corkscrew_comet(self):
        palette = {key: rgb for key, rgb, _ in inks(INK_RS.read_text())}
        directory = ROOT / 'cockpit/assets/dungeon/cards'
        registry = (ROOT / 'crates/delve/src/rules/cards.rs').read_text()
        result = audit(list(directory.glob('*.card')), palette)
        for stem, effects in [('kettle-eclipse', 'cooldown 21\ncast meteor\nward 2'),
                              ('corkscrew-comet', 'cooldown 16\ncast bolt\nblink 2')]:
            target = directory / (stem + '.card')
            with self.subTest(card=stem):
                name, kind, rows = load_card(target, palette)
                source = target.read_text()
                self.assertEqual(kind, 'spell')
                self.assertLessEqual(len(name), 22)
                self.assertIn(effects, source)
                self.assertIn('drop 2\nchest 3', source)
                self.assertIn(f'("{stem}", include_str!', registry)
                self.assertEqual(len(rows), 16)
                self.assertTrue(all(len(row) == 16 for row in rows))
                self.assertLessEqual(len(next(line[5:] for line in source.splitlines() if line.startswith('text '))), 64)
                for signal in ('similar_silhouettes', 'translated_silhouettes'):
                    self.assertFalse(any(str(target) in pair['paths'] for pair in result[signal]))
                self.assertFalse(any(str(target) in group for group in result['exact_rendered_duplicates']))
        kettle = load_card(directory / 'kettle-eclipse.card', palette)[2]
        self.assertEqual(kettle[2][6:9], 'CWC')
        self.assertEqual(kettle[9][12:16], 'hYWh')
        screw = load_card(directory / 'corkscrew-comet.card', palette)[2]
        self.assertEqual(screw[4][2:9], 'hYWWWYh')
        self.assertEqual(screw[14][5:8], 'hYh')

    def test_last_call_fork_and_mothwing_tea(self):
        palette = {key: rgb for key, rgb, _ in inks(INK_RS.read_text())}
        directory = ROOT / 'cockpit/assets/dungeon/cards'
        registry = (ROOT / 'crates/delve/src/rules/cards.rs').read_text()
        result = audit(list(directory.glob('*.card')), palette)
        for stem, effects in [('last-call-fork', 'nova 35\nward 1'),
                              ('mothwing-tea', 'heal 15\nmoth 30')]:
            target = directory / (stem + '.card')
            with self.subTest(card=stem):
                name, kind, rows = load_card(target, palette)
                source = target.read_text()
                self.assertEqual(kind, 'play')
                self.assertLessEqual(len(name), 22)
                self.assertLessEqual(len(source.split('text ', 1)[1].splitlines()[0]), 64)
                self.assertEqual([len(row) for row in rows], [16] * 16)
                self.assertIn('drop 2\nchest 3\n' + effects + '\nart\n', source)
                self.assertEqual(registry.count(f'("{stem}", include_str!'), 1)
                self.assertFalse(set(''.join(rows)) & set('0123456789wa@$c'))
                for signal in ('similar_silhouettes', 'translated_silhouettes'):
                    self.assertFalse(any(str(target) in pair['paths'] for pair in result[signal]))
                self.assertFalse(any(str(target) in group for group in result['exact_rendered_duplicates']))
        fork = load_card(directory / 'last-call-fork.card', palette)[2]
        self.assertEqual(fork[4][6:8], '..')  # open gap between prongs
        self.assertEqual(fork[7][5:9], 'WWWW')  # joined bridge
        tea = load_card(directory / 'mothwing-tea.card', palette)[2]
        self.assertEqual(tea[2][11:13], 'hh')  # detached tag
        self.assertEqual(tea[9][5:8], 'uYu')  # woven sachet

    def test_cinder_service_and_moonshine_prism(self):
        palette = {key: rgb for key, rgb, _ in inks(INK_RS.read_text())}
        directory = ROOT / 'cockpit/assets/dungeon/cards'
        result = audit(list(directory.glob('*.card')), palette)
        registry = (ROOT / 'crates/delve/src/rules/cards.rs').read_text()
        for stem, effects in [('cinder-service', 'cooldown 19\ncast meteor\nward 1'),
                              ('moonshine-prism', 'cooldown 17\ncast ring')]:
            target = directory / (stem + '.card')
            with self.subTest(card=stem):
                _, kind, rows = load_card(target, palette)
                self.assertEqual(kind, 'spell')
                self.assertEqual([len(row) for row in rows], [16] * 16)
                self.assertIn(effects + '\nart\n', target.read_text())
                self.assertIn(f'("{stem}", include_str!', registry)
                self.assertFalse(set(''.join(rows)) & set('0123456789wa@$c'))
                for signal in ('similar_silhouettes', 'translated_silhouettes'):
                    self.assertFalse(any(str(target) in pair['paths'] for pair in result[signal]))
                self.assertFalse(any(str(target) in group for group in result['exact_rendered_duplicates']))
        plate = load_card(directory / 'cinder-service.card', palette)[2]
        self.assertEqual(plate[4][6:11], 'ROWOR')
        self.assertEqual(plate[12][5:9], 'hYWh')
        prism = load_card(directory / 'moonshine-prism.card', palette)[2]
        self.assertEqual(prism[3][6], 'C')
        self.assertEqual(prism[9][3:13], 'WCCWWCCCCh')

    def test_cellar_key_and_siphon(self):
        palette = {key: rgb for key, rgb, _ in inks(INK_RS.read_text())}
        directory = ROOT / 'cockpit/assets/dungeon/cards'
        result = audit(list(directory.glob('*.card')), palette)
        registry = (ROOT / 'crates/delve/src/rules/cards.rs').read_text()
        for stem, kind, effects in [
                ('cellar-key', 'play', 'blink 5\nimmune 3'),
                ('copperhead-siphon', 'spell', 'cooldown 14\ncast bolt\nward 1')]:
            target = directory / (stem + '.card')
            _, actual_kind, rows = load_card(target, palette)
            with self.subTest(card=stem):
                self.assertEqual(kind, actual_kind)
                self.assertEqual([len(row) for row in rows], [16] * 16)
                self.assertIn(effects + '\nart\n', target.read_text())
                self.assertIn(f'("{stem}", include_str!', registry)
                self.assertFalse(set(''.join(rows)) & set('0123456789wa@$c'))
                for signal in ('similar_silhouettes', 'translated_silhouettes'):
                    self.assertFalse(any(str(target) in pair['paths'] for pair in result[signal]))
                self.assertFalse(any(str(target) in group for group in result['exact_rendered_duplicates']))
        key = load_card(directory / 'cellar-key.card', palette)[2]
        self.assertEqual(key[4][3:7], '....')  # open key bow
        self.assertEqual(key[11][4:9], 'YWWWY')  # separate ward teeth
        siphon = load_card(directory / 'copperhead-siphon.card', palette)[2]
        self.assertEqual(siphon[2][10:12], 'CC')  # detached fizz
        self.assertEqual(siphon[10][3:7], 'CWWC')  # glass reservoir

    def test_night_kitchen_drops(self):
        palette = {key: rgb for key, rgb, _ in inks(INK_RS.read_text())}
        directory = ROOT / 'cockpit/assets/dungeon/cards'
        result = audit(list(directory.glob('*.card')), palette)
        registry = (ROOT / 'crates/delve/src/rules/cards.rs').read_text()
        for stem, effects in [('last-orders-ladle', 'heal 12\nward 2'),
                              ('coatcheck-comet', 'nova 30')]:
            target = directory / (stem + '.card')
            _, kind, rows = load_card(target, palette)
            with self.subTest(card=stem):
                self.assertEqual(kind, 'play')
                self.assertEqual([len(row) for row in rows], [16] * 16)
                self.assertIn(effects + '\nart\n', target.read_text())
                self.assertIn(f'("{stem}", include_str!', registry)
                self.assertFalse(set(''.join(rows)) & set('0123456789wa@$c'))
                for signal in ('similar_silhouettes', 'translated_silhouettes'):
                    self.assertFalse(any(str(target) in pair['paths'] for pair in result[signal]))
                self.assertFalse(any(str(target) in group for group in result['exact_rendered_duplicates']))
        ladle = load_card(directory / 'last-orders-ladle.card', palette)[2]
        self.assertEqual(ladle[11][3:9], 'ORWWRO')
        comet = load_card(directory / 'coatcheck-comet.card', palette)[2]
        self.assertEqual(comet[2][3:5], '..')  # open silver hook
        self.assertEqual(comet[7][2:9], 'CWWWWWC')

    def test_ticket_and_wick_bar_drop_readability(self):
        palette = {key: rgb for key, rgb, _ in inks(INK_RS.read_text())}
        directory = ROOT / 'cockpit/assets/dungeon/cards'
        headers = {
            'velvet-rope-ticket': 'name Velvet Rope Ticket\nkind play\nrarity rare\ntext Admit one survivor: blink eight steps along your aim with four seconds free of hex, chill, web and hook.\ndrop 2\nchest 3\nblink 8\nimmune 4',
            'wayfarer-wick': 'name Wayfarer Wick\nkind play\nrarity common\ntext Light the road home: a small mercy in your pocket.\ndrop 5\nchest 3\nheal 25\nward 1',
        }
        result = audit(list(directory.glob('*.card')), palette)
        for stem, header in headers.items():
            target = directory / (stem + '.card')
            _, kind, rows = load_card(target, palette)
            with self.subTest(card=stem):
                self.assertEqual(kind, 'play')
                self.assertEqual(target.read_text().split('\nart\n')[0], header)
                self.assertEqual([len(row) for row in rows], [16] * 16)
                self.assertFalse(any(c in '0123456789wa@$c' for row in rows for c in row))
                for signal in ('similar_silhouettes', 'translated_silhouettes'):
                    self.assertFalse(any(str(target) in pair['paths'] for pair in result[signal]))
                self.assertFalse(any(str(target) in group for group in result['exact_rendered_duplicates']))
        ticket = load_card(directory / 'velvet-rope-ticket.card', palette)[2]
        self.assertEqual(ticket[2][9:12], 'CWC')  # detached blink glint
        self.assertEqual(ticket[7][7:13], '.hYYYY')  # separated torn stub
        self.assertEqual(ticket[13][4:11], 'RRRRRRR')  # velvet rope below ticket
        wick = load_card(directory / 'wayfarer-wick.card', palette)[2]
        self.assertEqual(wick[3][6:10], 'CWWC')  # tapered ghost flame
        self.assertEqual(wick[8][6:11], 'hWiWh')  # hanging wax drip
        self.assertEqual(wick[12][2:6], 'Y..Y')  # open brass finger loop

    def test_bell_and_drowned_crown_readable_materials(self):
        palette = {key: rgb for key, rgb, _ in inks(INK_RS.read_text())}
        directory = ROOT / 'cockpit/assets/dungeon/cards'
        headers = {
            'zero-hour-bell': 'name Zero Hour Bell\nkind spell\nrarity rare\ntext Ring a cryogenic foundry bell: a ring inherits your shot traits; one second of ward shelters the ringer.\ndrop 2\nchest 3\ncooldown 15\ncast ring\nward 1',
            'undertow-crown': "name Undertow Crown\nkind spell\nrarity rare\ntext Break a drowned sovereign's crown into a ring of shots and reclaim 10 life.\ndrop 2\nchest 3\ncooldown 18\ncast ring\nheal 10",
        }
        result = audit(list(directory.glob('*.card')), palette)
        for stem, header in headers.items():
            target = directory / (stem + '.card')
            _, kind, rows = load_card(target, palette)
            with self.subTest(card=stem):
                self.assertEqual(kind, 'spell')
                self.assertEqual(target.read_text().split('\nart\n')[0], header)
                self.assertEqual([len(row) for row in rows], [16] * 16)
                self.assertFalse(any(c in '0123456789wa@$c' for row in rows for c in row))
                for signal in ('similar_silhouettes', 'translated_silhouettes'):
                    self.assertFalse(any(str(target) in pair['paths'] for pair in result[signal]))
                self.assertFalse(any(str(target) in group for group in result['exact_rendered_duplicates']))
        bell = load_card(directory / 'zero-hour-bell.card', palette)[2]
        self.assertEqual(bell[2][6:10], 'Y..Y')  # open suspension loop
        self.assertEqual(bell[10][2:14], 'YWWYYYYYYYYY')  # flared brass lip
        self.assertEqual(bell[13][7:9], 'Wh')  # exposed clapper
        crown = load_card(directory / 'undertow-crown.card', palette)[2]
        self.assertEqual(crown[1].count('W'), 3)  # three distinct crown points
        self.assertEqual(crown[6][6:10], 'h..h')  # broken central opening
        self.assertEqual(crown[12][5:11], 'CCWCCi')  # detached undertow splash

    def test_watchlight_and_spindle_material_silhouettes(self):
        palette = {key: rgb for key, rgb, _ in inks(INK_RS.read_text())}
        directory = ROOT / 'cockpit/assets/dungeon/cards'
        headers = {
            'wake-lantern': 'name Wake Lantern\nkind hold\nrarity rare\ntext A drowned watchlight guides each shot home and chills what it finds.\ndrop 2\nchest 3\nhoming 1\nchill 1',
            'widows-spindle': "name Widow's Spindle\nkind hold\nrarity rare\ntext A ruby bobbin feeds on fallen foes; its barbed thread pierces one more target.\ndrop 2\nchest 3\nvamp 2\npierce 1",
        }
        result = audit(list(directory.glob('*.card')), palette)
        for stem, header in headers.items():
            target = directory / (stem + '.card')
            _, kind, rows = load_card(target, palette)
            with self.subTest(card=stem):
                self.assertEqual(kind, 'hold')
                self.assertEqual(target.read_text().split('\nart\n')[0], header)
                self.assertEqual([len(row) for row in rows], [16] * 16)
                self.assertFalse(any(c in '0123456789wa@$c' for row in rows for c in row))
                for signal in ('similar_silhouettes', 'translated_silhouettes'):
                    self.assertFalse(any(str(target) in pair['paths'] for pair in result[signal]))
                self.assertFalse(any(str(target) in group for group in result['exact_rendered_duplicates']))
        lantern = load_card(directory / 'wake-lantern.card', palette)[2]
        self.assertEqual(lantern[1][5:11], 'Y....Y')  # open carrying loop
        self.assertEqual(lantern[8][6:9], 'CWC')  # bright suspended flame
        self.assertEqual(lantern[6][5:11], '......')  # open chamber
        spindle = load_card(directory / 'widows-spindle.card', palette)[2]
        self.assertEqual(spindle[6][12:15], 'rWr')  # separate barbed thread
        self.assertEqual(spindle[3][5:8], 'WWY')  # brass flange highlight
        self.assertEqual(spindle[14][7], 'I')  # exposed iron axle

    def test_bone_totem_skulls_bound_stake_and_grave_fire(self):
        palette = {key: rgb for key, rgb, _ in inks(INK_RS.read_text())}
        target = ROOT / 'cockpit/assets/dungeon/cards/bone-totem.card'
        _, kind, rows = load_card(target, palette)
        self.assertEqual(kind.strip(), 'spell')
        self.assertEqual([len(row) for row in rows], [16] * 16)
        self.assertEqual(target.read_text().split('\nart\n')[0],
                         "# The Gravecaller's third spell (rules/minions.rs).\n"
                         'name   Bone Totem\nkind   spell\nrarity rare\n'
                         'text   A stake of skulls where you aim spits grave-fire; never moves.\n'
                         'drop   0\nchest  0\ncooldown 12\ntotem  1')
        self.assertEqual(rows[1][9:12], 'CWC')
        self.assertEqual(rows[5][4:9], 'KKWKK')
        self.assertEqual(rows[10][4:9], 'KKWKK')
        self.assertEqual(rows[13][5:9], 'YIIY')
        self.assertFalse(any(c in '0123456789wa@$c' for row in rows for c in row))
        result = audit(list(target.parent.glob('*.card')), palette)
        for signal in ('similar_silhouettes', 'translated_silhouettes'):
            self.assertFalse(any(str(target) in pair['paths'] for pair in result[signal]))
        self.assertFalse(any(str(target) in group for group in result['exact_rendered_duplicates']))

    def test_thunder_scroll_parchment_and_forked_lightning(self):
        palette = {key: rgb for key, rgb, _ in inks(INK_RS.read_text())}
        target = ROOT / 'cockpit/assets/dungeon/cards/thunder-scroll.card'
        _, kind, rows = load_card(target, palette)
        self.assertEqual(kind.strip(), 'play')
        self.assertEqual([len(row) for row in rows], [16] * 16)
        self.assertEqual(target.read_text().split('\nart\n')[0],
                         'name   Thunder Scroll\nkind   play\nrarity rare\n'
                         'text   Read it aloud: lightning strikes every monster here.\n'
                         'drop   3\nchest  2\nnova   60')
        self.assertEqual(rows[2][3:14], 'YWWWWWWWYYh')
        self.assertEqual(rows[7][6:9], 'WWW')
        self.assertEqual(rows[13][3:12], 'WWWWWWWWW')
        self.assertFalse(any(c in '0123456789wa@$c' for row in rows for c in row))
        result = audit(list(target.parent.glob('*.card')), palette)
        for signal in ('similar_silhouettes', 'translated_silhouettes'):
            self.assertFalse(any(str(target) in pair['paths'] for pair in result[signal]))
        self.assertFalse(any(str(target) in group for group in result['exact_rendered_duplicates']))

    def test_fae_dagger_wing_guard_and_teleport_echo(self):
        palette = {key: rgb for key, rgb, _ in inks(INK_RS.read_text())}
        target = ROOT / 'cockpit/assets/dungeon/cards/fae-dagger.card'
        _, kind, rows = load_card(target, palette)
        self.assertEqual(kind.strip(), 'play')
        self.assertEqual([len(row) for row in rows], [16] * 16)
        self.assertEqual(target.read_text().split('\nart\n')[0],
                         'name   Fae Dagger\nkind   play\nrarity rare\n'
                         'text   Press its number: you are where you were aiming.\n'
                         'drop   3\nchest  3\nblink  12')
        self.assertEqual(rows[6][2:6], '([(.')
        self.assertEqual(rows[9][2:7], 'YJJJY')
        self.assertEqual(rows[7][11], 'z')
        self.assertFalse(any(c in '0123456789wa@$c' for row in rows for c in row))
        result = audit(list(target.parent.glob('*.card')), palette)
        for signal in ('similar_silhouettes', 'translated_silhouettes'):
            self.assertFalse(any(str(target) in pair['paths'] for pair in result[signal]))
        self.assertFalse(any(str(target) in group for group in result['exact_rendered_duplicates']))

    def test_candle_wax_flame_and_melted_pool(self):
        palette = {key: rgb for key, rgb, _ in inks(INK_RS.read_text())}
        target = ROOT / 'cockpit/assets/dungeon/cards/wax.card'
        _, kind, rows = load_card(target, palette)
        self.assertEqual(kind.strip(), 'take')
        self.assertEqual([len(row) for row in rows], [16] * 16)
        self.assertEqual(target.read_text().split('\nart\n')[0],
                         "name   Candle-Wax\nkind   take\n"
                         "text   Drippings from the Crypt's endless candles.\n"
                         "where  crypt\ndrop   6\nchest  0\nwax    1")
        self.assertEqual(rows[3][6:10], 'YWWY')
        self.assertEqual(rows[5][7], 'h')
        self.assertEqual(rows[9][4:11], 'W7W77WW')
        self.assertEqual(rows[13][2:13], 'W77W777W77W')
        self.assertFalse(any(c in 'wa@$c' for row in rows for c in row))
        result = audit(list(target.parent.glob('*.card')), palette)
        for signal in ('similar_silhouettes', 'translated_silhouettes'):
            self.assertFalse(any(str(target) in pair['paths'] for pair in result[signal]))
        self.assertFalse(any(str(target) in group for group in result['exact_rendered_duplicates']))

    def test_bomb_iron_shell_and_lit_fuse(self):
        palette = {key: rgb for key, rgb, _ in inks(INK_RS.read_text())}
        target = ROOT / 'cockpit/assets/dungeon/cards/bomb.card'
        _, kind, rows = load_card(target, palette)
        self.assertEqual(kind.strip(), 'take')
        self.assertEqual([len(row) for row in rows], [16] * 16)
        self.assertEqual(target.read_text().split('\nart\n')[0],
                         "name   Bomb\nkind   take\nrarity common\n"
                         "text   One more charge for E: clears every shot in the room.\n"
                         "drop   14\nchest  0\nbombs  1")
        self.assertEqual(rows[1][12:15], 'YWY')
        self.assertEqual(rows[5][6:12], 'hYYYYh')
        self.assertEqual(rows[9][4:7], 'WWi')
        self.assertFalse(any(c.isdigit() or c in 'wa@$c' for row in rows for c in row))
        result = audit(list(target.parent.glob('*.card')), palette)
        for signal in ('similar_silhouettes', 'translated_silhouettes'):
            self.assertFalse(any(str(target) in pair['paths'] for pair in result[signal]))
        self.assertFalse(any(str(target) in group for group in result['exact_rendered_duplicates']))

    def test_borrowed_minute_brass_frame_and_violet_sand(self):
        palette = {key: rgb for key, rgb, _ in inks(INK_RS.read_text())}
        target = ROOT / 'cockpit/assets/dungeon/cards/borrowed-minute.card'
        _, kind, rows = load_card(target, palette)
        self.assertEqual(kind.strip(), 'spell')
        self.assertEqual([len(row) for row in rows], [16] * 16)
        self.assertEqual(target.read_text().split('\nart\n')[0],
                         "name Borrowed Minute\nkind spell\nrarity rare\n"
                         "text Turn the little glass: shelter and a measure of life.\n"
                         "drop 1\nchest 2\ncooldown 20\nward 2\nheal 14")
        self.assertEqual(rows[3][6:10], '....')
        self.assertEqual(rows[7][6:10], 'h^^h')
        self.assertEqual(rows[12][6:10], '^^^^')
        self.assertEqual(rows[2], rows[13])
        self.assertFalse(any(c.isdigit() for row in rows for c in row))
        result = audit(list(target.parent.glob('*.card')), palette)
        for signal in ('similar_silhouettes', 'translated_silhouettes'):
            self.assertFalse(any(str(target) in pair['paths'] for pair in result[signal]))

    def test_vestal_censer_chains_bowl_and_smoke(self):
        palette = {key: rgb for key, rgb, _ in inks(INK_RS.read_text())}
        target = ROOT / 'cockpit/assets/dungeon/cards/vestal-censer.card'
        _, kind, rows = load_card(target, palette)
        self.assertEqual(kind.strip(), 'hold')
        self.assertEqual([len(row) for row in rows], [16] * 16)
        self.assertEqual(target.read_text().split('\nart\n')[0],
                         "name   Vestal Censer\nkind   hold\nrarity relic\n"
                         "text   The chapel's own censer, still smoking.\n"
                         "drop   0\nchest  1\nmend   2")
        self.assertEqual(kind, 'hold')  # aligned headers normalize like single spaces
        self.assertEqual(rows[7][6:12], 'Y..Y.Y')  # open suspension
        self.assertEqual(rows[10][3:14], 'YWWYYYYWWWY')  # brass rim
        self.assertEqual(rows[12][5:10], 'ORWRO')  # incense core
        self.assertEqual(rows[1][8:11], 'CWC')  # smoke highlight
        self.assertEqual(rows[14][6:10], 'YYYh')  # tapered bowl
        self.assertFalse(any(c.isdigit() for row in rows for c in row))
        result = audit(list(target.parent.glob('*.card')), palette)
        for signal in ('similar_silhouettes', 'translated_silhouettes'):
            self.assertFalse(any(str(target) in pair['paths'] for pair in result[signal]))
        self.assertFalse(any(str(target) in group for group in result['exact_rendered_duplicates']))

    def test_rime_whistle_instrument_and_frost_moth(self):
        palette = {key: rgb for key, rgb, _ in inks(INK_RS.read_text())}
        target = ROOT / 'cockpit/assets/dungeon/cards/rime-whistle.card'
        _, kind, rows = load_card(target, palette)
        self.assertEqual(kind.strip(), 'play')
        self.assertEqual([len(row) for row in rows], [16] * 16)
        self.assertEqual(target.read_text().split('\nart\n')[0],
                         'name   Rime Whistle\nkind   play\nrarity rare\n'
                         'text   Blow it: a rime moth answers and freezes foes, one at a time.\n'
                         'drop   0\nchest  0\nmoth   45')
        self.assertEqual(rows[2][9:14], 'CWCWC')
        self.assertEqual(rows[10][3:6], 'hCh')
        self.assertEqual(rows[15][2:8], 'hYYYYh')
        self.assertFalse(any(c.isdigit() for row in rows for c in row))
        result = audit(list(target.parent.glob('*.card')), palette)
        for signal in ('similar_silhouettes', 'translated_silhouettes'):
            self.assertFalse(any(str(target) in pair['paths'] for pair in result[signal]))

    def test_axe_spin_has_two_axes_and_orbit(self):
        palette = {key: rgb for key, rgb, _ in inks(INK_RS.read_text())}
        target = ROOT / 'cockpit/assets/dungeon/cards/axe-spin.card'
        _, kind, rows = load_card(target, palette)
        self.assertEqual(kind.strip(), 'guard')
        self.assertEqual([len(row) for row in rows], [16] * 16)
        self.assertEqual(target.read_text().split('\nart\n')[0],
                         "# Guard cards put a defence on the space key. This one turns the knight.\n"
                         "name   Axe Spin\nkind   guard\nrarity rare\n"
                         "text   Space: axes spin, cutting foes and shots. Pause between spins.\n"
                         "drop   0\nchest  0\nguard  spin")
        self.assertEqual(rows[3][4:9], 'hWWWh')
        self.assertEqual(rows[12][7:12], 'hWWWh')
        self.assertEqual(rows[0][6:9], 'CCC')
        self.assertFalse(any(c.isdigit() for row in rows for c in row))
        result = audit(list(target.parent.glob('*.card')), palette)
        for signal in ('similar_silhouettes', 'translated_silhouettes'):
            self.assertFalse(any(str(target) in pair['paths'] for pair in result[signal]))
        self.assertFalse(any(str(target) in group for group in result['exact_rendered_duplicates']))

    def test_barrow_warden_is_an_armored_character(self):
        palette = {key: rgb for key, rgb, _ in inks(INK_RS.read_text())}
        target = ROOT / 'cockpit/assets/dungeon/cards/barrow-warden.card'
        _, kind, rows = load_card(target, palette)
        self.assertEqual(kind.strip(), 'spell')
        self.assertEqual([len(row) for row in rows], [16] * 16)
        self.assertEqual(target.read_text().split('\nart\n')[0],
                         "# The Gravecaller's second spell (rules/minions.rs).\n"
                         "name   Barrow Warden\nkind   spell\nrarity rare\n"
                         "text   A barrow knight raises his shield for you and roars foes on.\n"
                         "drop   0\nchest  0\ncooldown 14\nwarden 1")
        self.assertEqual(rows[2][5:8], 'CKC')  # separated cold eyes
        self.assertEqual(rows[8][2:5], 'YWh')  # bright shield face
        self.assertEqual(rows[12][6], '.')  # negative space between legs
        self.assertEqual(rows[4][13], 'S')  # separate polearm shaft
        result = audit(list(target.parent.glob('*.card')), palette)
        for signal in ('similar_silhouettes', 'translated_silhouettes'):
            self.assertFalse(any(str(target) in pair['paths'] for pair in result[signal]))
        self.assertFalse(any(str(target) in group for group in result['exact_rendered_duplicates']))

    def test_bow_and_crossbow_readable_weapon_construction(self):
        palette = {key: rgb for key, rgb, _ in inks(INK_RS.read_text())}
        directory = ROOT / 'cockpit/assets/dungeon/cards'
        headers = {
            'bow': 'name   Bow\nkind   arm\nrarity common\ntext   Yew and gut: a stream of quick arrows. Take it with E.\ndrop   3\nchest  4\narm    bow',
            'crossbow': 'name   Crossbow\nkind   arm\nrarity rare\ntext   A rail: the bolt goes through everything in its line.\ndrop   6\nchest  3\narm    crossbow',
        }
        result = audit(list(directory.glob('*.card')), palette)
        rows_by_name = {}
        for name, header in headers.items():
            with self.subTest(card=name):
                target = directory / (name + '.card')
                _, kind, rows = load_card(target, palette)
                rows_by_name[name] = rows
                self.assertEqual(kind.strip(), 'arm')
                self.assertEqual([len(row) for row in rows], [16] * 16)
                self.assertEqual(target.read_text().split('\nart\n')[0], header)
                for signal in ('similar_silhouettes', 'translated_silhouettes'):
                    self.assertFalse(any(str(target) in pair['paths'] for pair in result[signal]))
                self.assertFalse(any(str(target) in group for group in result['exact_rendered_duplicates']))
        # Open bow arc, straight string, and a separate horizontal nocked arrow.
        bow = rows_by_name['bow']
        self.assertEqual(bow[7][2:13], 'WWWWWWWWWWC')
        self.assertEqual(bow[5][4:11], 'h....Yh')
        # Crossbow's upright rail, wide transverse limbs and offset trigger loop.
        crossbow = rows_by_name['crossbow']
        self.assertEqual(crossbow[6], 'hYhhhhYWhYhhhhYh')
        self.assertEqual(crossbow[11][6:11], 'hYhYh')
        self.assertEqual(crossbow[1][6:9], 'WCW')

    def test_quiver_and_harness_materials_and_rules(self):
        palette = {key: rgb for key, rgb, _ in inks(INK_RS.read_text())}
        directory = ROOT / 'cockpit/assets/dungeon/cards'
        headers = {
            'crescent-quiver': 'name   Crescent Quiver\nkind   hold\nrarity relic\ntext   A quiver cut crescent, so every arrow flies a little wider.\ndrop   1\nchest  1\nvolley 6',
            'deeps-harness': 'name   Deeps Harness\nkind   hold\nrarity relic\ntext   Dwarf iron across the shoulders.\ndrop   1\nchest  1\narmor  1\nmax_hp 10',
        }
        result = audit(list(directory.glob('*.card')), palette)
        sprites = {}
        for name, header in headers.items():
            with self.subTest(card=name):
                target = directory / (name + '.card')
                _, _, rows = load_card(target, palette)
                sprites[name] = rows
                self.assertEqual([len(row) for row in rows], [16] * 16)
                self.assertEqual(target.read_text().split('\nart\n')[0], header)
                for signal in ('similar_silhouettes', 'translated_silhouettes'):
                    self.assertFalse(any(str(target) in pair['paths'] for pair in result[signal]))
                self.assertFalse(any(str(target) in group for group in result['exact_rendered_duplicates']))
        self.assertEqual(sprites['crescent-quiver'][1][6:11], 'W...W')  # separate arrowheads
        self.assertEqual(sprites['crescent-quiver'][8][3:5], '..')  # crescent's open interior
        self.assertEqual(sprites['deeps-harness'][3][7:9], '..')  # neck opening
        self.assertEqual(sprites['deeps-harness'][7][7:9], 'YY')  # brass clasp

    def test_elixir_and_college_orb_materials_and_rules(self):
        palette = {key: rgb for key, rgb, _ in inks(INK_RS.read_text())}
        directory = ROOT / 'cockpit/assets/dungeon/cards'
        headers = {
            'elixir': "# The Alchemist's third: the one he never sells.\nname   Elixir\nkind   spell\nrarity relic\ntext   Smash an elixir at your feet: every friend close by mends.\ndrop   0\nchest  0\ncooldown 16\nfeat 1",
            'college-orb': 'name   College Orb\nkind   hold\nrarity relic\ntext   A pale orb that remembers its lessons.\ndrop   1\nchest  1\nchain  1',
        }
        result = audit(list(directory.glob('*.card')), palette)
        sprites = {}
        for name, header in headers.items():
            with self.subTest(card=name):
                target = directory / (name + '.card')
                _, _, rows = load_card(target, palette)
                sprites[name] = rows
                self.assertEqual([len(row) for row in rows], [16] * 16)
                self.assertEqual(target.read_text().split('\nart\n')[0], header)
                for signal in ('similar_silhouettes', 'translated_silhouettes'):
                    self.assertFalse(any(str(target) in pair['paths'] for pair in result[signal]))
                self.assertFalse(any(str(target) in group for group in result['exact_rendered_duplicates']))
        self.assertEqual(sprites['elixir'][7][6:9], 'VWV')  # restorative cross
        self.assertEqual(sprites['elixir'][4][7:9], '..')  # clear glass neck
        self.assertEqual(sprites['college-orb'][11][5:10], '.....')  # open cradle
        self.assertEqual(sprites['college-orb'][5][5:7], 'CC')  # captive light

    def test_cleaver_and_storm_hammer_have_authored_materials(self):
        palette = {key: rgb for key, rgb, _ in inks(INK_RS.read_text())}
        directory = ROOT / 'cockpit/assets/dungeon/cards'
        headers = {
            'cleaver': "# The Butcher's own: a cleaver as long as a man's arm.\nname   Cleaver\nkind   arm\nrarity relic\ntext   Hold F: heavy chops up close.\ndrop   0\nchest  0\nlook   steel blade\nmelee  damage=32 reach=3.0 arc=140 every=16",
            'storm-hammer': "# The Hammerer's first: the maul's little brother, thrown.\nname   Storm Hammer\nkind   spell\nrarity relic\ntext   A thrown hammer stuns the first foe it meets, bursting round it.\ndrop   0\nchest  0\ncooldown 8\nfeat 1",
        }
        result = audit(list(directory.glob('*.card')), palette)
        sprites = {}
        for name, header in headers.items():
            with self.subTest(card=name):
                target = directory / (name + '.card')
                _, _, rows = load_card(target, palette)
                sprites[name] = rows
                self.assertEqual([len(row) for row in rows], [16] * 16)
                self.assertEqual(target.read_text().split('\nart\n')[0], header)
                for signal in ('similar_silhouettes', 'translated_silhouettes'):
                    self.assertFalse(any(str(target) in pair['paths'] for pair in result[signal]))
                self.assertFalse(any(str(target) in group for group in result['exact_rendered_duplicates']))
        self.assertEqual(sprites['cleaver'][4][4:6], '..')  # blade hanging hole
        self.assertEqual(sprites['storm-hammer'][6][7:12], 'CW.WC')  # storm rune
        self.assertEqual(sprites['storm-hammer'][12][12:14], 'CW')  # detached spark

    def test_defensive_equipment_has_distinct_materials_and_unchanged_rules(self):
        palette = {key: rgb for key, rgb, _ in inks(INK_RS.read_text())}
        directory = ROOT / 'cockpit/assets/dungeon/cards'
        expected = {
            'body-armour': ['name Body armour', 'kind hold', 'rarity common',
                'text Mail and a tabard. Protection worth bringing home.',
                'drop 3', 'chest 5', 'armor 1'],
            'iron-helm': ['name Iron helm', 'kind hold', 'rarity common',
                'text An iron cap. It turns aside a share of every hit.',
                'drop 3', 'chest 5', 'armor 1'],
            'gate-aegis': ['name   Gate Aegis', 'kind   guard', 'rarity relic',
                "text   The gate's own shape, hung on the arm.",
                'drop   1', 'chest  1', 'guard  shield'],
        }
        result = audit(list(directory.glob('*.card')), palette)
        sprites = {}
        for name, header in expected.items():
            with self.subTest(card=name):
                target = directory / (name + '.card')
                _, _, rows = load_card(target, palette)
                sprites[name] = rows
                self.assertEqual([len(row) for row in rows], [16] * 16)
                self.assertEqual(target.read_text().split('\nart\n')[0].splitlines(), header)
                for signal in ('similar_silhouettes', 'translated_silhouettes'):
                    self.assertFalse(any(str(target) in pair['paths'] for pair in result[signal]))
                self.assertFalse(any(str(target) in group for group in result['exact_rendered_duplicates']))
        self.assertEqual(sprites['body-armour'][7][6:10], 'hYYh')  # gold tabard stripe
        self.assertEqual(sprites['iron-helm'][9][4:12], '....H...')  # open visor, central nasal
        self.assertEqual(sprites['gate-aegis'][8][6:10], 'Y..Y')  # open gate within shield

    def test_horizon_rifle_has_scope_long_barrel_and_open_trigger(self):
        palette = {key: rgb for key, rgb, _ in inks(INK_RS.read_text())}
        directory = ROOT / 'cockpit/assets/dungeon/cards'
        target = directory / 'horizon-rifle.card'
        _, kind, rows = load_card(target, palette)
        self.assertEqual(kind.strip(), 'arm')
        self.assertEqual([len(row) for row in rows], [16] * 16)
        self.assertEqual(rows[2], '......hYCCYh....')  # cyan glass in brass scope
        self.assertEqual(rows[6], '.hYWWWWWWWWWWYh.')  # continuous bright barrel
        self.assertEqual(rows[8][7:9], '..')  # negative-space trigger opening
        self.assertEqual(target.read_text().split('\nart\n')[0].splitlines(), [
            'name Horizon Rifle', 'kind arm', 'rarity relic',
            'text A slow long-range rifle. Every fourth shot is a headshot.',
            'drop 0', 'chest 0', 'arm crossbow'])
        result = audit(list(directory.glob('*.card')), palette)
        for signal in ('similar_silhouettes', 'translated_silhouettes'):
            self.assertFalse(any(str(target) in pair['paths'] for pair in result[signal]))
        self.assertFalse(any(str(target) in group for group in result['exact_rendered_duplicates']))

    def test_abyssal_compass_has_open_ring_needle_and_captive_star(self):
        palette = {key: rgb for key, rgb, _ in inks(INK_RS.read_text())}
        directory = ROOT / 'cockpit/assets/dungeon/cards'
        target = directory / 'abyssal-compass.card'
        _, kind, rows = load_card(target, palette)
        self.assertEqual(kind, 'hold')
        self.assertEqual([len(row) for row in rows], [16] * 16)
        self.assertEqual(rows[6][6:9], 'CWC')  # separate captive blue star
        self.assertEqual(rows[8][8:10], 'WY')  # two-tone needle
        self.assertEqual(rows[8][2], 'W')  # west direction mark
        self.assertEqual(target.read_text().split('\nart\n')[0].splitlines(), [
            'name Abyssal Compass', 'kind hold', 'rarity rare',
            'text A captive blue star steadies the hand that carries it.',
            'drop 2', 'chest 3', 'rate 5', 'max_hp 8'])
        result = audit(list(directory.glob('*.card')), palette)
        for signal in ('similar_silhouettes', 'translated_silhouettes'):
            self.assertFalse(any(str(target) in pair['paths'] for pair in result[signal]))
        self.assertFalse(any(str(target) in group for group in result['exact_rendered_duplicates']))

    def test_arcneedle_reads_as_a_spool_with_separate_lightning(self):
        palette = {key: rgb for key, rgb, _ in inks(INK_RS.read_text())}
        directory = ROOT / 'cockpit/assets/dungeon/cards'
        target = directory / 'arcneedle-spindle.card'
        _, kind, rows = load_card(target, palette)
        self.assertEqual(kind, 'hold')
        self.assertEqual([len(row) for row in rows], [16] * 16)
        self.assertEqual(rows[2], rows[13])  # broad brass spool flanges
        self.assertEqual(rows[4], '.....hCCWCh.....')  # narrow wound core
        self.assertEqual(rows[5][2:5], 'WW.')  # detached arc, not a blade
        self.assertEqual(target.read_text().split('\nart\n')[0].splitlines(), [
            'name Arcneedle Spindle', 'kind hold', 'rarity rare',
            'text A storm-surge bobbin: shots pierce one foe and arc to one neighbor at half damage.',
            'drop 2', 'chest 3', 'pierce 1', 'chain 1'])
        result = audit(list(directory.glob('*.card')), palette)
        for signal in ('similar_silhouettes', 'translated_silhouettes'):
            self.assertFalse(any(str(target) in pair['paths'] for pair in result[signal]))
        self.assertFalse(any(str(target) in pair for pair in result['exact_rendered_duplicates']))

    def test_rebreather_has_twin_filters_and_not_a_shield_outline(self):
        palette = {key: rgb for key, rgb, _ in inks(INK_RS.read_text())}
        directory = ROOT / 'cockpit/assets/dungeon/cards'
        target = directory / 'plague-divers-rebreather.card'
        _, kind, rows = load_card(target, palette)
        self.assertEqual(kind, 'play')
        self.assertEqual([len(row) for row in rows], [16] * 16)
        # Two brass filter canisters flank a cyan mouth valve, with air gaps.
        self.assertEqual(rows[7], 'h77h.h7WW7h.h77h')
        self.assertEqual(rows[9], 'hY7Yh7YYYY7hY7Yh')
        header = target.read_text().split('\nart\n')[0].splitlines()
        for line in ('drop 3', 'chest 4', 'immune 6', 'ward 1'):
            self.assertIn(line, header)
        result = audit([directory / 'barrow-warden.card', target], palette)
        self.assertEqual(result['exact_rendered_duplicates'], [])
        self.assertEqual(result['similar_silhouettes'], [])
        self.assertEqual(result['translated_silhouettes'], [])

    def test_shifted_recolour_is_a_separate_review_hint(self):
        left = Path(self.tmp.name) / 'left.card'
        right = Path(self.tmp.name) / 'right.card'
        left.write_text('name Left\nkind spell\nart\n' + ('WWWW\n' * 4))
        right.write_text('name Right\nkind spell\nart\n......\n' + ('..BBBB\n' * 4))
        result = audit([left, right], {'W': 'ffffff', 'B': '0000ff'})
        self.assertFalse(result['approval'])
        self.assertEqual(result['similar_silhouettes'], [])
        self.assertEqual(result['exact_rendered_duplicates'], [])
        self.assertEqual(len(result['translated_silhouettes']), 1)
        self.assertTrue(result['translated_silhouettes'][0]['same_silhouette'])
        right.write_text('name Tiny\nkind spell\nart\n..BB\n')
        self.assertEqual(audit([left, right], {'W': 'ffffff', 'B': '0000ff'})[
            'translated_silhouettes'], [])

    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.path = Path(self.tmp.name) / 'sample.card'
        self.palette = {'W': 'ffffff'}

    def card(self, art, name='Sample'):
        self.path.write_text(f'name {name}\nkind spell\nart\n{art}\n', encoding='utf-8')
        return self.path

    def test_last_call_pair_has_distinct_art_and_registered_sources(self):
        palette = {key: rgb for key, rgb, _ in inks(INK_RS.read_text())}
        directory = ROOT / 'cockpit/assets/dungeon/cards'
        registry = (ROOT / 'crates/delve/src/rules/cards.rs').read_text()
        for slug, kind in [('last-call-carom', 'hold'), ('velvet-intermission', 'spell')]:
            path = directory / (slug + '.card')
            _, actual_kind, rows = load_card(path, palette)
            self.assertEqual(actual_kind, kind)
            self.assertEqual([len(row) for row in rows], [16] * 16)
            self.assertIn('cards/' + slug + '.card', registry)
            result = audit(list(directory.glob('*.card')), palette)
            self.assertFalse(any(str(path) in pair['paths'] for pair in result['similar_silhouettes']))
            self.assertFalse(any(str(path) in group for group in result['exact_rendered_duplicates']))

    def test_orrery_heart_has_satellite_tilted_orbit_and_pedestal(self):
        palette = {key: rgb for key, rgb, _ in inks(INK_RS.read_text())}
        target = ROOT / 'cockpit/assets/dungeon/cards/orrery-heart.card'
        _, kind, rows = load_card(target, palette)
        self.assertEqual(kind, 'hold')
        self.assertEqual([len(row) for row in rows], [16] * 16)
        # Detached moon above the asymmetric brass orbital instrument.
        self.assertEqual(rows[1][11:15], 'CWWC')
        self.assertEqual(rows[3][11:15], '....')
        self.assertEqual(rows[7][4:8], 'CWWC')
        self.assertEqual(rows[14][5:12], 'hhYYYhh')
        self.assertNotEqual(rows[5], rows[5][::-1])
        header = target.read_text(encoding='utf-8').split('\nart\n')[0].splitlines()
        for line in ('kind hold', 'rarity rare', 'drop 2', 'chest 3',
                     'orbit 1', 'homing 1'):
            self.assertIn(line, header)

    def test_whiteout_parasol_has_a_frost_crown_and_offset_hook(self):
        palette = {key: rgb for key, rgb, _ in inks(INK_RS.read_text())}
        directory = ROOT / 'cockpit/assets/dungeon/cards'
        target = directory / 'whiteout-parasol.card'
        rows = load_card(target, palette)[2]
        self.assertEqual(len(rows), 16)
        self.assertTrue(all(len(row) == 16 for row in rows))
        self.assertEqual(rows[1], '.......W........')  # icy canopy finial
        self.assertEqual(rows[8][8], '.')  # open space beneath scalloped canopy
        self.assertEqual(rows[14][7:13], 'hYYYYh')  # exposed hooked handle
        result = audit(list(directory.glob('*.card')), palette)
        self.assertFalse(any(str(target) in pair['paths']
                             for pair in result['similar_silhouettes']))
        self.assertFalse(any(str(target) in group
                             for group in result['exact_rendered_duplicates']))
        source = target.read_text(encoding='utf-8').split('art\n')[0].splitlines()
        for rule in ('kind spell', 'rarity rare', 'drop 2', 'chest 3',
                     'cooldown 16', 'cast ring', 'ward 2'):
            self.assertIn(rule, source)

    def test_switchglass_escapement_has_a_distinct_open_clock_frame(self):
        palette = {key: rgb for key, rgb, _ in inks(INK_RS.read_text())}
        directory = ROOT / 'cockpit/assets/dungeon/cards'
        target = directory / 'switchglass-escapement.card'
        rows = load_card(target, palette)[2]
        self.assertEqual(len(rows), 16)
        self.assertTrue(all(len(row) == 16 for row in rows))
        self.assertEqual(rows[4][5:7], '..')  # open brass frame, not a filled gem
        result = audit(list(directory.glob('*.card')), palette)
        self.assertFalse(any(str(target) in pair['paths']
                             for pair in result['similar_silhouettes']))
        self.assertFalse(any(str(target) in group
                             for group in result['exact_rendered_duplicates']))
        source = target.read_text(encoding='utf-8').split('art\n')[0]
        for line in ('kind hold', 'rarity rare', 'drop 2', 'chest 3',
                     'rate 3', 'max_hp 10'):
            self.assertIn(line, source.splitlines())

    def test_railglass_prism_has_offset_lens_beam_and_open_cradle(self):
        palette = {key: rgb for key, rgb, _ in inks(INK_RS.read_text())}
        directory = ROOT / 'cockpit/assets/dungeon/cards'
        target = directory / 'railglass-prism.card'
        _, kind, rows = load_card(target, palette)
        self.assertEqual(kind, 'hold')
        self.assertEqual(len(rows), 16)
        self.assertTrue(all(len(row) == 16 for row in rows))
        self.assertEqual(rows[1][12:14], 'CW')  # detached refracted beam
        self.assertEqual(rows[9][7:11], '....')  # open optical cradle
        source = target.read_text().split('art\n')[0].splitlines()
        for rule in ('kind hold', 'rarity rare', 'drop 2', 'chest 3',
                     'rate 4', 'max_hp 10'):
            self.assertIn(rule, source)
        result = audit(list(directory.glob('*.card')), palette)
        self.assertFalse(any(str(target) in pair['paths']
                             for pair in result['similar_silhouettes']))
        self.assertFalse(any(str(target) in group
                             for group in result['exact_rendered_duplicates']))

    def test_miasma_flask_has_escaping_vapour_and_preserves_spell(self):
        palette = {key: rgb for key, rgb, _ in inks(INK_RS.read_text())}
        directory = ROOT / 'cockpit/assets/dungeon/cards'
        target = directory / 'miasma-flask.card'
        rows = load_card(target, palette)[2]
        self.assertEqual(len(rows), 16)
        self.assertTrue(all(len(row) == 16 for row in rows))
        self.assertEqual(rows[0][10:12], 'AA')
        self.assertEqual(rows[3][7:9], 'AA')
        self.assertEqual(rows[5][3:5], '..')  # uncorked space above neck
        result = audit(list(directory.glob('*.card')), palette)
        self.assertFalse(any(str(target) in pair['paths']
                             for pair in result['similar_silhouettes']))
        self.assertFalse(any(str(target) in group
                             for group in result['exact_rendered_duplicates']))
        source = target.read_text(encoding='utf-8').split('art\n')[0]
        fields = dict(line.split(None, 1) for line in source.splitlines()
                      if line and not line.startswith('#'))
        self.assertEqual({k: fields[k] for k in
                          ('kind', 'rarity', 'drop', 'chest', 'cooldown', 'feat')},
                         dict(kind='spell', rarity='relic', drop='0', chest='0',
                              cooldown='6', feat='1'))

    def test_viper_hourglass_open_frame_and_venom_reservoir(self):
        palette = {key: rgb for key, rgb, _ in inks(INK_RS.read_text())}
        directory = ROOT / 'cockpit/assets/dungeon/cards'
        target = directory / 'vipers-hourglass.card'
        header = target.read_text().split('art\n')[0]
        self.assertEqual(header, "name Viper's Hourglass\nkind hold\nrarity rare\ntext Venom pools in the lower bulb. Shots poison and burst on impact.\ndrop 2\nchest 3\npoison 1\nburst 20\n")
        rows = load_card(target, palette)[2]
        self.assertEqual([len(row) for row in rows], [16] * 16)
        self.assertEqual(rows[3][4:12], '.i....i.')  # glass separated from brass pillars
        self.assertEqual(rows[7][7:9], 'G.')  # narrow falling venom, not a solid waist
        self.assertEqual(rows[11][6:10], 'GGGG')  # lower reservoir
        self.assertEqual(rows[12][6:10], 'WGGG')  # glass glint on pooled venom
        result = audit(list(directory.glob('*.card')), palette)
        for signal in ('similar_silhouettes', 'translated_silhouettes'):
            self.assertFalse(any(str(target) in pair['paths'] for pair in result[signal]))
        self.assertFalse(any(str(target) in group for group in result['exact_rendered_duplicates']))

    def test_small_legacy_art_is_not_stretched(self):
        self.assertEqual(load_card(self.card('W.\n.W'), self.palette)[2], ['W.', '.W'])

    def test_invalid_art_is_rejected(self):
        for art in ('', 'W' * 17, '\n'.join(['W'] * 17), 'W\n\nW', 'X'):
            with self.subTest(art=art), self.assertRaises(ValueError):
                load_card(self.card(art), self.palette)

    def test_missing_art_is_rejected(self):
        self.path.write_text('name Empty\n', encoding='utf-8')
        with self.assertRaisesRegex(ValueError, 'missing art'):
            load_card(self.path, self.palette)

    def test_native_and_enlarged_samples_have_two_backdrops_and_escape_labels(self):
        svg = gallery([self.card('W', 'A < B & C')], self.palette)
        root = ET.fromstring(svg)
        ns = {'s': 'http://www.w3.org/2000/svg'}
        groups = [g for g in root.findall('.//s:g', ns) if 'transform' in g.attrib]
        self.assertEqual(len(groups), 4)
        self.assertEqual(sum('scale(1)' in g.attrib['transform'] for g in groups), 2)
        self.assertEqual(sum('scale(8)' in g.attrib['transform'] for g in groups), 2)
        rects = root.findall('.//s:rect', ns)
        for color in ('#090e18', '#c9c3b6'):
            self.assertEqual(sum(r.get('fill') == color for r in rects), 2)
        self.assertIn('A &lt; B &amp; C', svg)

    def test_audit_uses_rendered_colours_and_reports_transparency(self):
        first = self.card('W.W')
        other = self.path.with_name('alias.card')
        other.write_text('name Alias\nkind spell\nart\nQ.Q\n', encoding='utf-8')
        blank = self.path.with_name('blank.card')
        blank.write_text('name Blank\nkind spell\nart\n...\n', encoding='utf-8')
        result = audit([first, other, blank], {'W': 'ffffff', 'Q': 'FFFFFF'})
        self.assertFalse(result['approval'])
        self.assertEqual(result['exact_rendered_duplicates'], [[str(other), str(first)]])
        by_name = {c['name']: c for c in result['cards']}
        self.assertEqual(by_name['Sample']['components_8_connected'], [1, 1])
        self.assertEqual(by_name['Blank']['review_hints'], ['fully transparent'])
        self.assertEqual(by_name['Alias']['opaque_pixels'], 2)

    def test_duplicate_detection_uses_fixed_canvas_without_moving_pixels(self):
        first = self.card('W')
        padded = self.path.with_name('padded.card')
        padded.write_text('name Padded\nkind spell\nart\nW...\n....\n', encoding='utf-8')
        shifted = self.path.with_name('shifted.card')
        shifted.write_text('name Shifted\nkind spell\nart\n.W\n', encoding='utf-8')
        result = audit([first, padded, shifted], self.palette)
        self.assertEqual(result['exact_rendered_duplicates'], [[str(padded), str(first)]])

    def test_refined_loot_silhouettes_are_distinct_from_flagged_peers(self):
        palette = {key: rgb for key, rgb, _ in inks(INK_RS.read_text())}
        directory = ROOT / 'cockpit/assets/dungeon/cards'
        for left, right in (('bottled-squall-honey', 'frostbite-ampoule'),
                            ('permafrost-flywheel', 'rimewheel-bearing')):
            with self.subTest(asset=left):
                paths = [directory / (name + '.card') for name in (left, right)]
                rows = load_card(paths[0], palette)[2]
                self.assertEqual(len(rows), 16)
                self.assertTrue(all(len(row) == 16 for row in rows))
                result = audit(paths, palette)
                self.assertEqual(result['similar_silhouettes'], [])
                self.assertEqual(result['exact_rendered_duplicates'], [])
                self.assertGreaterEqual(result['cards'][0]['colours'], 4)

    def test_stormglass_writ_is_a_distinct_illuminated_warrant(self):
        palette = {key: rgb for key, rgb, _ in inks(INK_RS.read_text())}
        directory = ROOT / 'cockpit/assets/dungeon/cards'
        target = directory / 'stormglass-writ.card'
        rows = load_card(target, palette)[2]
        self.assertEqual(len(rows), 16)
        self.assertTrue(all(len(row) == 16 for row in rows))
        result = audit(list(directory.glob('*.card')), palette)
        self.assertFalse(any(str(target) in pair['paths']
                             for pair in result['similar_silhouettes']))
        self.assertFalse(any(str(target) in group
                             for group in result['exact_rendered_duplicates']))
        source = target.read_text(encoding='utf-8').split('art\n')[0]
        for line in ('kind spell', 'rarity rare', 'drop 2', 'chest 3',
                     'cooldown 7', 'cast bolt'):
            self.assertIn(line, source.splitlines())

    def test_flask_bandolier_is_a_distinct_full_size_harness(self):
        palette = {key: rgb for key, rgb, _ in inks(INK_RS.read_text())}
        directory = ROOT / 'cockpit/assets/dungeon/cards'
        target = directory / 'flask-bandolier.card'
        _, kind, rows = load_card(target, palette)
        self.assertEqual(kind.strip(), 'arm')
        self.assertEqual(len(rows), 16)
        self.assertTrue(all(len(row) == 16 for row in rows))
        result = audit(list(directory.glob('*.card')), palette)
        self.assertFalse(any(str(target) in pair['paths']
                             for pair in result['similar_silhouettes']))
        self.assertFalse(any(str(target) in group
                             for group in result['exact_rendered_duplicates']))
        directives = target.read_text(encoding='utf-8').split('art\n')[0]
        for line in ('rarity relic', 'drop   0', 'chest  0', 'arm    stones'):
            self.assertIn(line, directives.splitlines())
        self.assertIn('Hold F: lob a lit flask where you aim; the ground burns after.', directives)

    def test_adder_hourglass_has_distinct_serpent_frame(self):
        palette = {key: rgb for key, rgb, _ in inks(INK_RS.read_text())}
        directory = ROOT / 'cockpit/assets/dungeon/cards'
        target = directory / 'adder-hourglass.card'
        rows = load_card(target, palette)[2]
        self.assertEqual(len(rows), 16)
        self.assertTrue(all(len(row) == 16 for row in rows))
        result = audit(list(directory.glob('*.card')), palette)
        self.assertFalse(any(str(target) in pair['paths']
                             for pair in result['similar_silhouettes']))
        self.assertFalse(any(str(target) in group
                             for group in result['exact_rendered_duplicates']))
        directives = target.read_text(encoding='utf-8').split('art\n')[0]
        for line in ('kind hold', 'rarity rare', 'drop 2', 'chest 3',
                     'pierce 1', 'poison 1'):
            self.assertIn(line, directives.splitlines())

    def test_choir_crumb_is_a_distinct_bird_and_broken_offering(self):
        palette = {key: rgb for key, rgb, _ in inks(INK_RS.read_text())}
        directory = ROOT / 'cockpit/assets/dungeon/cards'
        target = directory / 'choir-crumb.card'
        rows = load_card(target, palette)[2]
        self.assertEqual(len(rows), 16)
        self.assertTrue(all(len(row) == 16 for row in rows))
        self.assertEqual(rows[4][10], 'W')  # linnet eye highlight
        self.assertEqual(rows[12][6], '.')  # broken bread negative space
        result = audit(list(directory.glob('*.card')), palette)
        self.assertFalse(any(str(target) in pair['paths']
                             for pair in result['similar_silhouettes']))
        self.assertFalse(any(str(target) in group
                             for group in result['exact_rendered_duplicates']))
        directives = target.read_text(encoding='utf-8').split('art\n')[0]
        for line in ('kind   play', 'rarity rare', 'drop   0', 'chest  0', 'linnet 1'):
            self.assertIn(line, directives.splitlines())

    def test_thundercomb_has_open_teeth_and_preserves_storm_rules(self):
        palette = {key: rgb for key, rgb, _ in inks(INK_RS.read_text())}
        directory = ROOT / 'cockpit/assets/dungeon/cards'
        target = directory / 'thundercomb.card'
        _, kind, rows = load_card(target, palette)
        self.assertEqual(kind, 'hold')
        self.assertEqual(len(rows), 16)
        self.assertTrue(all(len(row) == 16 for row in rows))
        self.assertEqual(rows[8][5], '.')
        self.assertEqual(rows[8][9], '.')
        directives = target.read_text().split('art\n')[0].splitlines()
        for rule in ('rarity rare', 'drop 2', 'chest 3', 'homing 1', 'chain 1'):
            self.assertIn(rule, directives)
        result = audit(list(directory.glob('*.card')), palette)
        self.assertFalse(any(str(target) in pair['paths']
                             for pair in result['similar_silhouettes']))
        self.assertFalse(any(str(target) in group
                             for group in result['exact_rendered_duplicates']))

    def test_entire_current_library_renders_valid_xml(self):
        palette = {key: rgb for key, rgb, _ in inks(INK_RS.read_text())}
        paths = list((ROOT / 'cockpit/assets/dungeon/cards').glob('*.card'))
        self.assertTrue(paths)
        root = ET.fromstring(gallery(paths, palette))
        self.assertEqual(len(root.findall('.//{http://www.w3.org/2000/svg}title')), len(paths))


class CelestialConsumableArtTests(unittest.TestCase):
    def test_celestial_consumables_have_distinct_shapes_and_original_rules(self):
        palette = {key: rgb for key, rgb, _ in inks(INK_RS.read_text())}
        directory = ROOT / 'cockpit/assets/dungeon/cards'
        result = audit(list(directory.glob('*.card')), palette)
        expected = {
            'meteor-cordial': ('rarity rare', 'drop 2', 'chest 2', 'nova 35', 'heal 10'),
            'moonmilk-flask': ('rarity common', 'drop 4', 'chest 3', 'heal 10', 'ward 1'),
            'thunderhead-phial': ('rarity rare', 'drop 3', 'chest 4', 'nova 24', 'heal 10'),
        }
        for key, rules in expected.items():
            with self.subTest(card=key):
                target = directory / (key + '.card')
                _, kind, rows = load_card(target, palette)
                self.assertEqual(kind, 'play')
                self.assertEqual(len(rows), 16)
                self.assertTrue(all(len(row) == 16 for row in rows))
                source = target.read_text().split('art\n')[0].splitlines()
                for rule in rules:
                    self.assertIn(rule, source)
                self.assertFalse(any(str(target) in pair['paths']
                                     for pair in result['similar_silhouettes']))
                self.assertFalse(any(str(target) in group
                                     for group in result['exact_rendered_duplicates']))
        # The crescent flask has an open interior, not a round bottle body.
        moon = load_card(directory / 'moonmilk-flask.card', palette)[2]
        self.assertEqual(moon[8][7:11], '....')
        # Detached storm and diagonal meteor trail remain legible negative space.
        storm = load_card(directory / 'thunderhead-phial.card', palette)[2]
        self.assertEqual(storm[10][8], '.')
        meteor = load_card(directory / 'meteor-cordial.card', palette)[2]
        self.assertEqual(meteor[1][10:12], 'YY')



    def test_reticle_and_seed_cage_have_open_readable_centres(self):
        palette = {key: rgb for key, rgb, _ in inks(INK_RS.read_text())}
        directory = ROOT / 'cockpit/assets/dungeon/cards'
        expected = {
            'wishbone-reticle': ('Wishbone Reticle', 'An ivory sight splits a stopped shot into two hungry trajectories.', ['fork 1', 'velocity 10']),
            'widowseed-cage': ('Widowseed Cage', 'A living seed in a brass cage gives your shots venom.', ['poison 1']),
        }
        result = audit(list(directory.glob('*.card')), palette)
        for stem, (name, description, effects) in expected.items():
            with self.subTest(card=stem):
                target = directory / (stem + '.card')
                _, kind, rows = load_card(target, palette)
                self.assertEqual(kind, 'hold')
                self.assertEqual([len(row) for row in rows], [16] * 16)
                self.assertEqual(target.read_text().split('art\n')[0].splitlines(),
                                 ['name ' + name, 'kind hold', 'rarity rare',
                                  'text ' + description, 'drop 2', 'chest 3'] + effects)
                for signal in ('similar_silhouettes', 'translated_silhouettes'):
                    self.assertFalse(any(str(target) in pair['paths'] for pair in result[signal]))
                self.assertFalse(any(str(target) in group for group in result['exact_rendered_duplicates']))
        sight = load_card(directory / 'wishbone-reticle.card', palette)[2]
        self.assertEqual(sight[6][7:9], '..')
        self.assertEqual(sight[9][3:5], 'CC')
        cage = load_card(directory / 'widowseed-cage.card', palette)[2]
        self.assertEqual(cage[1][7:9], '..')
        self.assertEqual(cage[8][4:5], '.')
        self.assertEqual(cage[8][6:9], 'GWG')


class WhiteoutParasolArtTests(unittest.TestCase):
    def test_frost_canopy_has_ribs_icicles_and_hook_without_rule_changes(self):
        palette = {key: rgb for key, rgb, _ in inks(INK_RS.read_text())}
        directory = ROOT / 'cockpit/assets/dungeon/cards'
        target = directory / 'whiteout-parasol.card'
        _, kind, rows = load_card(target, palette)
        self.assertEqual(kind, 'spell')
        self.assertEqual([len(row) for row in rows], [16] * 16)
        self.assertEqual(target.read_text().split('art\n')[0].splitlines(), [
            'name Whiteout Parasol', 'kind spell', 'rarity rare',
            'text Open a warded ring of shots carrying your projectile traits.',
            'drop 2', 'chest 3', 'cooldown 16', 'cast ring', 'ward 2'])
        self.assertEqual(rows[6][5:10], '..Y..')
        self.assertEqual(rows[7][1:14], 'CWWWWWYWWWWWC')
        self.assertEqual(rows[9][2], 'i')
        self.assertEqual(rows[9][12], 'i')
        self.assertEqual(rows[14][8:12], 'YYYY')
        result = audit(list(directory.glob('*.card')), palette)
        for signal in ('similar_silhouettes', 'translated_silhouettes'):
            self.assertFalse(any(str(target) in pair['paths'] for pair in result[signal]))
        self.assertFalse(any(str(target) in group for group in result['exact_rendered_duplicates']))


if __name__ == '__main__':
    unittest.main()
