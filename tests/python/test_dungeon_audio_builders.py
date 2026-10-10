"""Offline builder coverage for the Delve's voice and music assets."""
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
VOICE = load_script("dungeon_voice_test", ROOT / "cockpit/assets/dungeon/voices/voice.py")
VOICE_SOURCE_DIR = ROOT / "cockpit/assets/dungeon/voices"


class DungeonAudioBuilderTests(unittest.TestCase):
    def test_voice_builder_records_each_chorus_character_and_keeps_existing_takes(self):
        with mock.patch.object(VOICE, "HERE", VOICE_SOURCE_DIR):
            source_rows = list(VOICE.lines())
        first_by_character = {}
        for row in source_rows:
            first_by_character.setdefault(row[0], row)
        rows = list(first_by_character.values())

        with tempfile.TemporaryDirectory(prefix="angel-dungeon-voices-") as tmp:
            root = Path(tmp)
            (root / "chorus.txt").write_text(
                "\n".join(" | ".join(row) for row in rows) + "\n"
            )
            preserved = rows[0]
            kept = root / preserved[0] / f"{preserved[2]}.mp3"
            kept.parent.mkdir(parents=True)
            kept.write_bytes(b"authored take")
            commands = []

            def fake_tts(_voice, _settings, text, output):
                output.write_bytes(text.encode())

            def fake_ffmpeg(command, **_kwargs):
                commands.append(command)
                Path(command[-1]).write_bytes(b"rendered take")

            with mock.patch.object(VOICE, "HERE", root):
                with mock.patch.object(VOICE, "tts", side_effect=fake_tts) as tts:
                    with mock.patch.object(
                        VOICE.subprocess, "run", side_effect=fake_ffmpeg
                    ):
                        with mock.patch.object(sys, "argv", ["voice.py"]):
                            VOICE.main()

            self.assertEqual(kept.read_bytes(), b"authored take")
            for who, _cue, lid, _words in rows:
                target = root / who / f"{lid}.mp3"
                self.assertTrue(target.is_file(), f"missing generated take: {who}/{lid}")
                if target != kept:
                    self.assertEqual(target.read_bytes(), b"rendered take")
            self.assertEqual(len(commands), len(rows) - 1)
            self.assertTrue(tts.called)
            for command in commands:
                filters = command[command.index("-filter_complex") + 1]
                self.assertIn("loudnorm=I=-16", filters)
                self.assertIn("silenceremove", filters)

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
