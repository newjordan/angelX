"""Offline builder coverage for the Delve's knight voices and music."""
import importlib.util
from pathlib import Path
import sys
import tempfile
import unittest
from unittest import mock


ROOT = Path(__file__).resolve().parents[2]


def load_script(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


LEVEL = load_script("dungeon_level_test", ROOT / "cockpit/assets/dungeon/level.py")
KNIGHTS = load_script("dungeon_knights_test", ROOT / "cockpit/assets/dungeon/knights.py")


class DungeonAudioBuilderTests(unittest.TestCase):
    def test_knight_builder_records_every_clip_and_keeps_existing_takes(self):
        with tempfile.TemporaryDirectory(prefix="angel-dungeon-knights-") as tmp:
            root = Path(tmp)
            out = root / "sfx" / "knights"
            out.mkdir(parents=True)
            kept = out / "a_cheer_1.mp3"
            kept.write_bytes(b"authored take")
            spoken, commands = [], []

            def fake_tts(voice_id, text, output):
                spoken.append((voice_id, text))
                output.write_bytes(text.encode())

            def fake_run(command, **_kwargs):
                commands.append(command)
                if "-af" in command and "astats=metadata=0" in command:
                    return mock.Mock(stderr="Overall\n RMS level dB: -30.0\n")
                Path(command[-1]).write_bytes(b"rendered take")
                return mock.Mock(stderr="")

            with mock.patch.object(KNIGHTS, "HERE", root), \
                    mock.patch.object(KNIGHTS, "OUT", out), \
                    mock.patch.object(KNIGHTS, "tts", side_effect=fake_tts), \
                    mock.patch.object(KNIGHTS.subprocess, "run", side_effect=fake_run), \
                    mock.patch.object(sys, "argv", ["knights.py"]):
                KNIGHTS.main()

            clips = [*KNIGHTS.CHATTER, *KNIGHTS.GRUNTS]
            self.assertEqual(kept.read_bytes(), b"authored take")
            for voice in KNIGHTS.VOICES:
                for clip in clips:
                    for take in (1, 2):
                        target = out / f"{voice}_{clip}_{take}.mp3"
                        self.assertTrue(target.is_file(), f"missing {target.name}")
            self.assertEqual(len(spoken), len(KNIGHTS.VOICES) * len(clips) * 2 - 1)
            # Two knights never share both lines of a moment.
            for clip in KNIGHTS.CHATTER:
                self.assertNotEqual(KNIGHTS.takes("a", clip), KNIGHTS.takes("b", clip))
            shaping = [c for c in commands if "-af" in c and "silenceremove" in c[c.index("-af") + 1]]
            self.assertTrue(shaping, "takes are trimmed")
            levelling = [c for c in commands if "-af" in c and "alimiter" in c[c.index("-af") + 1]]
            self.assertTrue(levelling, "takes are levelled and limited")

    def test_music_builder_preserves_seamless_crypt_and_boss_tracks(self):
        with tempfile.TemporaryDirectory(prefix="angel-dungeon-music-") as tmp:
            root = Path(tmp)
            paths = {name: root / f"{name}.mp3" for name in ("boss", "crypt", "mines")}
            for path in paths.values():
                path.write_bytes(b"source")
            commands = []

            def fake_ffmpeg(command, **_kwargs):
                commands.append(command)
                Path(command[-1]).write_bytes(b"leveled")

            with mock.patch.object(LEVEL.subprocess, "run", side_effect=fake_ffmpeg):
                for path in paths.values():
                    LEVEL.level_music(path)

            filters = {
                Path(command[command.index("-i") + 1]).stem:
                command[command.index("-af") + 1]
                for command in commands
            }
            for name in ("boss", "crypt"):
                self.assertNotIn("silenceremove", filters[name])
                self.assertIn("loudnorm=I=-20", filters[name])
            self.assertIn("silenceremove", filters["mines"])
            self.assertIn("loudnorm=I=-20", filters["mines"])
            self.assertTrue(all(path.read_bytes() == b"leveled" for path in paths.values()))


if __name__ == "__main__":
    unittest.main()
