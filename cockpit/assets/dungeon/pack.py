#!/usr/bin/env python3
"""Fetch the Delve's free music (CC0, OpenGameArt) and convert it to
music/<track>.mp3 at the mix level. Downloads go to $TMPDIR/audio-packs.
Existing outputs are kept; delete one to redo it. Sources and licences are
in LICENSES.md."""
import os, pathlib, subprocess, urllib.request

HERE = pathlib.Path(__file__).resolve().parent
TMP = pathlib.Path(os.environ.get('TMPDIR', '/tmp')) / 'audio-packs'
OGA = 'https://opengameart.org/sites/default/files/'

# track: (download, file inside a zip or None, seamless loop?)
TRACKS = {
    'crypt': (OGA + 'qubodup-yd-DarkShrineLoop-OpenGameArt.ogg', None, True),
    'mines': (OGA + '5%20Action%20Chiptunes%20By%20Juhani%20Junkala.zip',
              'Juhani Junkala [Retro Game Music Pack] Level 2.wav', False),
    'keep': (OGA + '5%20Action%20Chiptunes%20By%20Juhani%20Junkala.zip',
             'Juhani Junkala [Retro Game Music Pack] Level 3.wav', False),
    'boss': (OGA + 'Juhani%20Junkala%20-%20Epic%20Boss%20Battle%20%5BSeamlessly%20Looping%5D.wav', None, True),
    'menu': (OGA + 'Dark%20Intro_0.ogg', None, False),
    # 'sanctuary' was generated with ElevenLabs music (sound.py).
}


def fetch(url):
    TMP.mkdir(parents=True, exist_ok=True)
    dest = TMP / urllib.request.unquote(url.rsplit('/', 1)[1])
    if not dest.exists():
        urllib.request.urlretrieve(url, dest)
    return dest


def main():
    (HERE / 'music').mkdir(exist_ok=True)
    for track, (url, inner, seamless) in TRACKS.items():
        out = HERE / 'music' / f'{track}.mp3'
        if out.exists():
            continue
        src = fetch(url)
        if inner:
            unpacked = TMP / src.stem
            subprocess.run(['unzip', '-o', '-q', str(src), '-d', str(unpacked)], check=True)
            src = next(p for p in unpacked.rglob("*") if p.name == inner)
        # Seamless loops keep their ends; the rest get a soft seam.
        fades = [] if seamless else ['afade=t=in:d=0.3', 'areverse', 'afade=t=in:d=1.5', 'areverse']
        chain = ','.join(fades + ['loudnorm=I=-20:TP=-2:LRA=11'])
        subprocess.run(['ffmpeg', '-v', 'error', '-y', '-i', str(src), '-af', chain,
                        '-ac', '1', '-ar', '44100', '-b:a', '96k', str(out)], check=True)
        print('music', track)


if __name__ == '__main__':
    main()
