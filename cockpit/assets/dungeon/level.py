#!/usr/bin/env python3
"""Bring the Delve's sounds to their mix levels, offline (no API calls).

  sfx/*.mp3:   common body level (RMS -20 dB), peaks limited to -2 dBFS.
  music/*.mp3: trailing silence trimmed so loops come round without a gap
               (seamless loops are left alone), integrated -20 LUFS.

Runs in place; safe to repeat. sound.py and pack.py call the same steps
for new files."""
import pathlib, re, subprocess, sys, tempfile

HERE = pathlib.Path(__file__).resolve().parent
SFX_RMS_DB = -20.0
PEAK = 0.79  # -2 dBFS
SEAMLESS = {'boss', 'crypt'}


def rms_db(path):
    out = subprocess.run(['ffmpeg', '-hide_banner', '-i', str(path), '-af', 'astats', '-f', 'null', '-'],
                         capture_output=True, text=True).stderr
    vals = re.findall(r'RMS level dB: (-?[\d.]+)', out)
    return float(vals[-1]) if vals else None


def rewrite(path, chain):
    with tempfile.TemporaryDirectory() as tmp:
        out = pathlib.Path(tmp) / path.name
        subprocess.run(['ffmpeg', '-v', 'error', '-y', '-i', str(path), '-af', chain,
                        '-ac', '1', '-ar', '44100', '-b:a', '96k', str(out)], check=True)
        out.replace(path)


def level_sfx(path):
    rms = rms_db(path)
    if rms is None:
        return
    gain = SFX_RMS_DB - rms
    rewrite(path, f'volume={gain:.2f}dB,alimiter=limit={PEAK}:attack=1:release=20:level=false')


def level_music(path):
    trim = '' if path.stem in SEAMLESS else 'areverse,silenceremove=start_periods=1:start_threshold=-50dB,areverse,'
    rewrite(path, f'{trim}loudnorm=I=-20:TP=-2:LRA=11')


def main():
    names = set(sys.argv[1:])
    for path in sorted((HERE / 'sfx').glob('*.mp3')):
        if not names or path.stem in names:
            level_sfx(path)
    for path in sorted((HERE / 'music').glob('*.mp3')):
        if not names or path.stem in names:
            level_music(path)


if __name__ == '__main__':
    main()
