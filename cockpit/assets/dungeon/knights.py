#!/usr/bin/env python3
"""Record the knights' voices with ElevenLabs: a few words of made-up knightly
chatter for the big moments, and little grunts for rolls, hits, falls and
ultimates. Nobody narrates; the knights just make their noises.

Each knight voice (a, b, c, d) gets two takes of every clip, written to
sfx/knights/<voice>_<clip>_<take>.mp3 (mono, 44.1 kHz, 96 kbit/s, like the
other effects). The takes are trimmed, lifted a touch in pitch, and levelled
quieter than the effects: chatter at RMS -23 dB, grunts at -26 dB, so they
sit under the fight. Needs ELEVENLABS_API_KEY. Existing files are kept
(delete one to re-record it); arguments limit the run to those voices or clips.

  python3 knights.py            record what is missing
  python3 knights.py a hup      just voice a, and every voice's hup
"""
import json, os, pathlib, re, subprocess, sys, tempfile, urllib.request

HERE = pathlib.Path(__file__).resolve().parent
OUT = HERE / 'sfx' / 'knights'
MODEL = 'eleven_v4'
SETTINGS = {'stability': 0.3, 'similarity_boost': 0.8}
# A touch higher and quicker reads small and cute without going chipmunk.
LIFT = 1.06

VOICES = {
    'a': 'WtA85syCrJwasGeHGH2p',  # Ember: bright, game-hero energy
    'b': '8ZYhGJrsDOe4C8yzEEhP',  # Guy: soft, earnest
    'c': 'cgSgspJ2msm6clMCkdW9',  # Jessica: playful, sunny
    'd': 'N2lVS1w4EtoT3dr4eOWO',  # Callum: husky trickster
}

# Chatter: the knights' own made-up tongue, never English. Each voice takes
# two of the four lines, so two knights rarely say the same thing.
CHATTER = {
    'cheer': ['[excited] Wahoo-ba! Fweeba!', '[excited] Hoo-hoo! Zibbadee!',
              '[excited] Yaa-hoo! Zinga-zoo!', '[excited] Wheee! Tibbaloo!'],
    'ooh': ['[curious] Ooooh... shibbaloo!', '[delighted] Ooh! Lumma-lumma!',
            '[amazed] Woooah... neebla!', '[delighted] Oooh, fizzibah!'],
    'uhoh': ['[nervous] Eep! Wagga-wagga, no-no!', '[nervous] Uh-oh... bobbadoo?',
             '[alarmed] Yikes-a! Grumbo-wah!', '[nervous] Oh-oh! Zubba-zubba!'],
    'aww': ['[sad] Awww... mubbo.', '[sad] Ohhh, noo-wah...',
            '[sad] Hmmm... boo-loo.', '[sad] Awww, fibba...'],
    'thanks': ['[warmly] Fanka-loo!', '[warmly] Tibbi-tibbi, shanka!',
               '[grateful] Ahh! Nooba-shee!', '[warmly] Shanka-shanka!'],
    'go': ['[determined] Hup-hup, zibba!', '[cheerful] Allo-ma, gwah-go!',
           '[determined] Onwa-bah!', '[cheerful] Zoom-zoom, peeba!'],
    'hello': ['[cheerful] Heebo-lay!', '[cheerful] Bibba-boo!',
              '[cheerful] Yoo-hoobla!', '[cheerful] Oy-oy, zubbalee!'],
    'hehe': ['[giggles] Hehe! Tibbly-wee!', '[laughs] Ha-hoo! Pibbly!',
             '[giggles] Hee-hee, wobba!', '[laughs] Hoho! Dibbadoo!'],
}
# Grunts: one sound each, the same two for every voice.
GRUNTS = {
    'hup': ['Hup!', 'Hyup!'],
    'oof': ['Oof!', 'Ow!'],
    'hyah': ['Hyah!', 'Hai-yah!'],
    'waah': ['[surprised] Waaah!', '[surprised] Whoa-oh!'],
}
CHATTER_DB, GRUNT_DB, PEAK_DB = -23.0, -26.0, -3.0


def takes(voice, clip):
    """The two lines voice `voice` records for `clip`."""
    if clip in GRUNTS:
        return GRUNTS[clip]
    pool, first = CHATTER[clip], 'abcd'.index(voice)
    return [pool[first % 4], pool[(first + 1) % 4]]


def tts(voice_id, text, out):
    body = json.dumps({'text': text, 'model_id': MODEL, 'voice_settings': SETTINGS}).encode()
    req = urllib.request.Request(
        f'https://api.elevenlabs.io/v1/text-to-speech/{voice_id}?output_format=mp3_44100_128',
        data=body, headers={'xi-api-key': os.environ['ELEVENLABS_API_KEY'],
                            'Content-Type': 'application/json'})
    with urllib.request.urlopen(req, timeout=180) as r:
        out.write_bytes(r.read())


def rms_db(path):
    err = subprocess.run(['ffmpeg', '-v', 'info', '-i', str(path), '-af', 'astats=metadata=0',
                          '-f', 'null', '-'], capture_output=True, text=True).stderr
    levels = re.findall(r'Overall.*?RMS level dB: (-?[\d.]+)', err, re.S)
    if not levels:
        levels = re.findall(r'RMS level dB: (-?[\d.]+)', err)
    return float(levels[-1])


def finish(raw, out, target_db):
    """Trim the silence, lift the pitch, then level and limit."""
    shaped = raw.with_suffix('.shaped.wav')
    trim = ('silenceremove=start_periods=1:start_threshold=-45dB:start_silence=0.02,'
            'areverse,silenceremove=start_periods=1:start_threshold=-45dB:start_silence=0.06,areverse')
    subprocess.run(['ffmpeg', '-v', 'error', '-y', '-i', str(raw), '-ac', '1', '-af',
                    f'{trim},asetrate=44100*{LIFT},aresample=44100,'
                    'afade=t=in:d=0.005,areverse,afade=t=in:d=0.04,areverse',
                    str(shaped)], check=True)
    gain = target_db - rms_db(shaped)
    peak = 10 ** (PEAK_DB / 20)
    subprocess.run(['ffmpeg', '-v', 'error', '-y', '-i', str(shaped), '-af',
                    f'volume={gain:.2f}dB,alimiter=limit={peak:.3f}:level=disabled',
                    '-ar', '44100', '-ac', '1', '-b:a', '96k', str(out)], check=True)


def main():
    only = set(sys.argv[1:])
    OUT.mkdir(parents=True, exist_ok=True)
    for voice, voice_id in VOICES.items():
        for clip in [*CHATTER, *GRUNTS]:
            if only and voice not in only and clip not in only:
                continue
            for n, text in enumerate(takes(voice, clip), 1):
                out = OUT / f'{voice}_{clip}_{n}.mp3'
                if out.exists():
                    continue
                with tempfile.TemporaryDirectory() as tmp:
                    raw = pathlib.Path(tmp) / 'raw.mp3'
                    tts(voice_id, text, raw)
                    finish(raw, out, GRUNT_DB if clip in GRUNTS else CHATTER_DB)
                print(out.relative_to(HERE), '|', text, flush=True)


if __name__ == '__main__':
    main()
