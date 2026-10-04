#!/usr/bin/env python3
"""Generate the Delve's sound effects and music with ElevenLabs.

  sfx/<name>.mp3    — ElevenLabs sound-effects API, trimmed to under 1.5 s,
                      mono 96k, peak-normalised then loudness-matched.
  music/<track>.mp3 — ElevenLabs music API (composition plans), mono 96k,
                      loudness-normalised, ends faded into the start so the
                      loop seam is soft.

Needs ELEVENLABS_API_KEY. Existing files are kept: delete one to regenerate
it. Arguments limit the run to those names (e.g. `sound.py shot_bow crypt`).

Credits are limited: the shipped SFX and the sanctuary track came from here;
the other music is CC0 from OpenGameArt (pack.py). Prefer free packs for new
sounds, and run level.py after any change.

Levels (see sfx/README.md): music files sit at -20 LUFS, SFX at about
-18 LUFS short-term with -2 dBTP peaks, voices (voice.py) stay at -16 LUFS.
The runtime mixer then plays music at 0.40, SFX at 0.55 and voices at 0.30.
"""
import json, os, pathlib, subprocess, sys, tempfile, urllib.request

HERE = pathlib.Path(__file__).resolve().parent
KEY = os.environ.get('ELEVENLABS_API_KEY', '')

# Short, dry, readable game sounds. A black-paper pixel knight game: crisp,
# a little retro, never muddy or long.
SFX = {
    'shot_bow': 'a single short wooden bow twang releasing an arrow, crisp, close, dry, game sound effect',
    'shot_crossbow': 'a heavy crossbow trigger clack and bolt release thunk, short, punchy, dry, game sound effect',
    'shot_handgonne': 'a small medieval hand cannon blast, short boom with a crackle, punchy, no echo, game sound effect',
    'shot_bolt': 'a short magical energy bolt zap, bright shimmering whoosh, retro fantasy game sound effect',
    'swing': 'a quick sword swing whoosh through the air, short and sharp, game sound effect',
    'hit': 'a short meaty impact thud of an arrow hitting a monster, punchy, game hit sound',
    'kill': 'a small monster defeated: short crunchy pop and bone rattle, satisfying, retro game sound effect',
    'hero_hurt': 'a knight in armour takes a hit: short metal clank and grunt-free impact, game sound effect',
    'roll': 'a quick dodge roll in chainmail: short cloth whoosh and soft armour rustle, game sound effect',
    'shield_block': 'a projectile glancing off a steel shield: short bright metallic ping, game sound effect',
    'bomb': 'a magical bomb blast that clears the screen: short deep whump with a bright shimmer tail, game sound effect',
    'card_pickup': 'picking up a magic card: short bright paper flick with a soft sparkle chime, game sound effect',
    'spoil_pickup': 'picking up treasure: two quick soft coin clinks, game pickup sound',
    'door_seal': 'heavy iron portcullis slamming down shut, short metallic clang, dungeon game sound effect',
    'door_open': 'iron portcullis grinding up open, short chain rattle, dungeon game sound effect',
    'wave_gate': 'a monster doorway igniting with fire: short rising whoosh of flame and a low growl, game sound effect',
    'spike': 'floor spike trap shooting up: short sharp metallic shing, game sound effect',
    'rock_land': 'a boulder falling and crashing onto stone floor: short heavy thud with gravel, game sound effect',
    'vent_fire': 'a wall vent breathing a burst of fire: short roaring whoosh, game sound effect',
    'chest_open': 'a wooden treasure chest creaking open with a small magical chime, short, game sound effect',
    'mimic': 'a treasure chest monster snapping its jaws: short wooden chomp with a hiss, game sound effect',
    'boss_rise': 'a dungeon boss appears: short deep ominous rumble with a rising dark swell, game sound effect',
    'boss_fall': 'a dungeon boss is defeated: short big crumbling explosion with a triumphant low boom, game sound effect',
    'descend': 'walking down stone stairs into a deeper dungeon: short echoing footsteps and a low magical whoosh, game sound effect',
    'play_card': 'casting a spell from a magic card: short bright rising shimmer, game sound effect',
    'wall_up': 'a glowing magic barrier wall raising: short humming energy shield power up, game sound effect',
    'mana_empty': 'out of magic power: short dull descending fizzle, game sound effect',
}

# Composition plans for loops: 60–90 s, no vocals. Fantasy meets a little
# synth: the realm is a kingdom of knights in a world still being computed.
COMMON_NEG = ['vocals', 'singing', 'choir', 'spoken word', 'pop', 'EDM drop']


def plan(sections, model='music_v2_5'):
    return {
        'composition_plan': {
            'chunks': [
                {
                    'text': text,
                    'duration_ms': ms,
                    'positive_styles': pos,
                    'negative_styles': COMMON_NEG + neg,
                    'context_adherence': 'high',
                    'conditioning_ref': None,
                    'condition_strength': None,
                }
                for text, ms, pos, neg in sections
            ]
        },
        'model_id': model,
    }


DARK = ['dark fantasy dungeon game soundtrack', 'medieval instruments meeting soft analog synth',
        'pixel-art adventure game mood', 'loopable']
MUSIC = {
    'crypt': plan([
        ('[Crypt - Candles in the Dark]', 36000, DARK + ['84 BPM', 'D minor', 'low cello drone', 'plucked lute arpeggio',
         'distant bells', 'soft analog pad', 'quiet frame drum pulse', 'eerie but calm, exploring a tomb'], ['loud drums']),
        ('[Crypt - Deeper]', 36000, DARK + ['84 BPM', 'D minor', 'lute and hurdy-gurdy motif', 'subtle synth arpeggio',
         'steady low pulse', 'mysterious, tense'], ['loud drums']),
    ]),
    'mines': plan([
        ('[Mines - Iron Under the Hill]', 36000, DARK + ['96 BPM', 'E minor', 'anvil and metal percussion in rhythm',
         'low bowed bass', 'hammered dulcimer ostinato', 'warm analog synth bass', 'driving, industrious, adventurous'], []),
        ('[Mines - The Seam]', 36000, DARK + ['96 BPM', 'E minor', 'dulcimer and fiddle motif', 'syncopated hand percussion',
         'synth pulse', 'adventurous, determined'], []),
    ]),
    'keep': plan([
        ('[Dragon Keep - Embers]', 36000, DARK + ['100 BPM', 'C minor', 'deep war drums', 'low brass swells',
         'distorted synth bass drone', 'crackling, heat, menace and courage'], []),
        ('[Dragon Keep - The Climb]', 36000, DARK + ['100 BPM', 'C minor', 'heroic horn motif over drums', 'arpeggiated synth',
         'tense, rising'], []),
    ]),
    'boss': plan([
        ('[Guardian - Battle]', 32000, DARK + ['140 BPM', 'A minor', 'driving taiko and frame drums', 'aggressive low strings ostinato',
         'distorted analog synth bass', 'brass stabs', 'intense boss battle, heroic, relentless'], []),
        ('[Guardian - Battle II]', 32000, DARK + ['140 BPM', 'A minor', 'pounding drums', 'fast fiddle and synth arpeggio interplay',
         'epic boss battle climax energy'], []),
    ]),
    'sanctuary': plan([
        ('[Sanctuary - A Safe Room]', 36000, ['calm medieval fantasy game safe-room music', '70 BPM', 'F major',
         'gentle harp and lute', 'soft warm synth pad', 'candlelight, rest, reflection, hope', 'loopable', 'no percussion'], ['drums']),
        ('[Sanctuary - Breath]', 36000, ['calm medieval fantasy game safe-room music', '70 BPM', 'F major',
         'recorder melody over harp', 'soft pad', 'peaceful, kind', 'loopable'], ['drums']),
    ]),
    'menu': plan([
        ('[The Old Delve - Title]', 30000, DARK + ['88 BPM', 'D minor', 'solemn horn call', 'low strings', 'lute arpeggio',
         'soft analog synth shimmer', 'a quest beginning, grail legend, wonder and resolve'], ['fast drums']),
        ('[The Old Delve - Title II]', 30000, DARK + ['88 BPM', 'D minor', 'the horn theme returns over harp and pad',
         'gentle frame drum', 'hopeful, legendary'], []),
    ]),
}


def post(url, body, accept='audio/mpeg'):
    req = urllib.request.Request(url, data=json.dumps(body).encode(),
                                 headers={'xi-api-key': KEY, 'Content-Type': 'application/json', 'Accept': accept})
    with urllib.request.urlopen(req, timeout=600) as r:
        return r.read()


def ffmpeg(*args):
    subprocess.run(['ffmpeg', '-v', 'error', '-y', *args], check=True)


def make_sfx(name, prompt):
    out = HERE / 'sfx' / f'{name}.mp3'
    if out.exists():
        return
    out.parent.mkdir(exist_ok=True)
    raw = post('https://api.elevenlabs.io/v1/sound-generation?output_format=mp3_44100_128',
               {'text': prompt, 'duration_seconds': 1.0, 'prompt_influence': 0.6})
    with tempfile.TemporaryDirectory() as tmp:
        src = pathlib.Path(tmp) / 'raw.mp3'
        src.write_bytes(raw)
        # Trim leading silence, cap at 1.4 s with a short fade, mono, then
        # bring peaks to -2 dBTP and the body to about -18 LUFS.
        ffmpeg('-i', str(src), '-af',
               'silenceremove=start_periods=1:start_threshold=-45dB,'
               'areverse,silenceremove=start_periods=1:start_threshold=-50dB,areverse,'
               'atrim=0:1.4,afade=t=out:st=1.25:d=0.15,'
               'loudnorm=I=-18:TP=-2:LRA=7',
               '-ac', '1', '-ar', '44100', '-b:a', '96k', str(out))
    print('sfx', name)


def make_music(name, body):
    out = HERE / 'music' / f'{name}.mp3'
    if out.exists():
        return
    out.parent.mkdir(exist_ok=True)
    raw = post('https://api.elevenlabs.io/v1/music?output_format=mp3_44100_128', body)
    with tempfile.TemporaryDirectory() as tmp:
        src = pathlib.Path(tmp) / 'raw.mp3'
        src.write_bytes(raw)
        dur = float(subprocess.run(['ffprobe', '-v', 'error', '-show_entries', 'format=duration', '-of', 'csv=p=0', str(src)],
                                   capture_output=True, text=True).stdout.strip())
        # A soft seam: fade the last 2.5 s and the first 0.4 s.
        ffmpeg('-i', str(src), '-af',
               f'afade=t=in:d=0.4,afade=t=out:st={max(dur - 2.5, 0):.2f}:d=2.5,loudnorm=I=-20:TP=-2:LRA=11',
               '-ac', '1', '-ar', '44100', '-b:a', '96k', str(out))
    print('music', name)


def main():
    if not KEY:
        sys.exit('ELEVENLABS_API_KEY is not set')
    only = set(sys.argv[1:])
    for name, prompt in SFX.items():
        if not only or name in only:
            make_sfx(name, prompt)
    for name, body in MUSIC.items():
        if not only or name in only:
            make_music(name, body)


if __name__ == '__main__':
    main()
