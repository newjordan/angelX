#!/usr/bin/env python3
"""Record the Delve chorus with ElevenLabs: every line in chorus.txt without a
voice file gets one, in its character's voice, at <who>/<id>.mp3 (mono, 64k,
loudness-matched). Needs ELEVENLABS_API_KEY. Re-run after adding lines;
existing files are kept (delete one to re-record it). Arguments limit the run
to those characters or line ids.

The Shoggoth is two voices layered and pitched into a chorus; the guardians
are treated to sound like they come out of the stone."""
import json, os, pathlib, subprocess, sys, tempfile, urllib.request

HERE = pathlib.Path(__file__).resolve().parent
MODEL = 'eleven_v4'
CAST = {
    # The Herald keeps the operator's calibrated announcer settings.
    'herald': [('YOq2y2Up4RgXP2HyXjE5', {'stability': 0.71, 'similarity_boost': 1.0, 'speed': 1.07, 'use_speaker_boost': True})],
    'blaise': [('qxePw1S1QmBgjlU3GIy5', {'stability': 0.55, 'similarity_boost': 0.8, 'speed': 0.92})],
    'wren': [('cgSgspJ2msm6clMCkdW9', {'stability': 0.4, 'similarity_boost': 0.8, 'speed': 1.06})],
    'tobbin': [('yZ3w1DL4ZAxIdOscv2t8', {'stability': 0.45, 'similarity_boost': 0.8, 'speed': 1.0})],
    'shoggoth': [('weA4Q36twV5kwSaTEL0Q', {'stability': 0.8, 'similarity_boost': 0.7, 'speed': 0.85}),
                 ('nPczCjzI2devNBz1zQrb', {'stability': 0.8, 'similarity_boost': 0.7, 'speed': 0.85})],
    'warden': [('pFZP5JQG7iQjIQuC4Bku', {'stability': 0.6, 'similarity_boost': 0.8, 'speed': 0.82})],
    'cinderjaw': [('SOYHLrjzK2X1ezoPC6cr', {'stability': 0.35, 'similarity_boost': 0.8, 'speed': 0.9})],
}
# The rest of the chorus reuses these calibrated voices. Keep the aliases
# pointed at the original presets so settings stay in one place.
CAST.update({
    'anselm': CAST['blaise'],
    'ector': CAST['blaise'],
    'kay': CAST['blaise'],
    'merlin': CAST['blaise'],
    'fortune': CAST['wren'],
    'mabel': CAST['wren'],
    'maud': CAST['wren'],
    'pip': CAST['wren'],
    'dinadan': CAST['wren'],
    'tallow': CAST['wren'],
    'beaumains': CAST['tobbin'],
    'grubbins': CAST['tobbin'],
    'snibbet': CAST['tobbin'],
    'leviathan': CAST['warden'],
})
# ffmpeg filters per character (after the raw takes).
TREAT = {
    # One clear lead (Ava, a little lower) with the deep voice and a high
    # ghost underneath: many voices, but the words stay readable.
    'shoggoth': ('[0:a]asetrate=44100*0.93,aresample=44100,atempo=1.075[a];'
                 '[1:a]asetrate=44100*0.72,aresample=44100,atempo=1.39,adelay=70|70,volume=0.55[b];'
                 '[0:a]asetrate=44100*1.16,aresample=44100,atempo=0.86,adelay=140|140,volume=0.22[c];'
                 '[a][b][c]amix=inputs=3:normalize=0,aecho=0.8:0.6:110|230:0.25|0.12,lowpass=f=6500'),
    'warden': '[0:a]aecho=0.8:0.85:320|610:0.4|0.25,lowpass=f=6500',
    'cinderjaw': '[0:a]asetrate=44100*0.78,aresample=44100,atempo=1.15,acrusher=bits=10:mix=0.25,aecho=0.7:0.6:60:0.3',
}

def lines():
    for row in (HERE / 'chorus.txt').read_text().splitlines():
        row = row.strip()
        if not row or row.startswith('#'):
            continue
        who, cue, lid, words = [p.strip() for p in row.split('|', 3)]
        yield who, cue, lid, words

def tts(voice, settings, text, out):
    body = json.dumps({'text': text, 'model_id': MODEL, 'voice_settings': settings}).encode()
    req = urllib.request.Request(f'https://api.elevenlabs.io/v1/text-to-speech/{voice}?output_format=mp3_44100_128', data=body,
                                 headers={'xi-api-key': os.environ['ELEVENLABS_API_KEY'], 'Content-Type': 'application/json'})
    with urllib.request.urlopen(req, timeout=180) as r:
        out.write_bytes(r.read())

def main():
    only = set(sys.argv[1:])
    for who, cue, lid, words in lines():
        if only and who not in only and lid not in only:
            continue
        out = HERE / who / f'{lid}.mp3'
        if out.exists():
            continue
        out.parent.mkdir(exist_ok=True)
        with tempfile.TemporaryDirectory() as tmp:
            takes = []
            for i, (voice, settings) in enumerate(CAST[who]):
                take = pathlib.Path(tmp) / f'take{i}.mp3'
                tts(voice, settings, words, take)
                takes.append(take)
            chain = TREAT.get(who, '[0:a]anull')
            cmd = ['ffmpeg', '-v', 'error', '-y']
            for take in takes:
                cmd += ['-i', str(take)]
            cmd += ['-filter_complex', chain + ',loudnorm=I=-16:TP=-1.5:LRA=11,silenceremove=start_periods=1:start_threshold=-50dB',
                    '-ac', '1', '-b:a', '64k', str(out)]
            subprocess.run(cmd, check=True)
        print('voiced', who, lid)

if __name__ == '__main__':
    main()
