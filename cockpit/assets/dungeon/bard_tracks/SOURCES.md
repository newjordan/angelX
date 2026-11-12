# Bard tracks: sources

Songs that really aired on Knight Shift Radio, YuE2 instrumental trance renders. Copied read-only
(`<id>.wav` as aired, 48 kHz stereo 16-bit, and `<id>.json`, the queue's own
metadata). The live station was only read: `state/station.sqlite` (opened `mode=ro`), `state/ledger.jsonl`,
`state/sender.log`, and `queue/played/` (where a song moves once it has been played). Nothing on the station's host was stopped,
restarted, written or loaded; the copy was one rsync, niced and rate-limited.

How "aired" was checked: `station.sqlite` `tracks.status = aired` (its `updated` stamp is when the station marked the song aired,
UTC), the song sits in `queue/played/`, and `state/sender.log` has an `on air: <title>` line a few minutes before that stamp.
Several titles are reused by different songs (two "Golden Hour Coming", two "Wide Open Tomorrow", two "Next Chapter"); the ids
below are the ones copied. The ledger's `audio_sha256` is the render before the station's own processing, so it does not match the
aired file's hash; the aired file's sha256 is below.

| | Title | Id | Aired (db, UTC) | On air (sender.log, UTC) | Style line (the render's prompt) | Measured | sha256 of the aired wav |
|---|---|---|---|---|---|---|---|
| a | Wide Open Tomorrow | `de553e53477f` | 2026-11-07 04:31:34 | 2026-11-07 04:29:34 | Instrumental uplifting trance, 140 BPM, in A major | 140.004 BPM, kicks fit within 4.0 ms | `03d8625dc9890cb52adbfc71253b0901c7e6175c914fb8f4e105c63fef43ca80` |
| b | First Light | `e3937dd9b2a4` | 2026-11-07 04:43:14 | 2026-11-07 04:40:09 | Instrumental uplifting trance, 140 BPM, in F sharp minor | 140.029 BPM, kicks fit within 8.3 ms | `9718c839122d9da926ef8ef37063625d7ec15835edb0b2477d1fc2c89ed9e7fb` |
| c | Golden Hour Coming | `5dbd61b9afea` | 2026-11-07 04:37:14 | 2026-11-07 04:34:05 | Instrumental uplifting trance, 139 BPM, in C major | 137.925 BPM, kicks fit within 20.0 ms | `cc77fc5fcde87f079cddfba26edba573a60ae20d04d86962d688e0fa0817bbbe` |
| d | Brand New Morning | `eb0f002d505c` | 2026-11-06 20:49:22 | 2026-11-06 20:46:39 | Instrumental uplifting trance, 139 BPM, in A major | 139.998 BPM, kicks fit within 4.8 ms | `9905beb99ffb76a99809f6594327471821defcef8c10205d4639f2597fb89612` |
| e | Together Now | `a5c47681bcf5` | 2026-11-07 04:56:20 | 2026-11-07 04:53:09 | Instrumental uplifting trance, 139 BPM, in C sharp minor | 140.045 BPM, kicks fit within 9.0 ms | `286ddebc7a41b5e716b4785a59527d6fcbe0acd0daf1c74ff0f83a6ecf94639c` |
| f | Common Ground | `72c3e4dcce07` | 2026-11-06 21:18:17 | 2026-11-06 21:15:35 | Instrumental uplifting trance, 138 BPM, in C major | 138.066 BPM, kicks fit within 4.8 ms | `653601b82b8c8002e3a41a53ffacec2a5672c7852d70cd9bc18740b4d5373d63` |
| g | The Long Good Dawn | `08859c82903f` | 2026-11-07 05:17:28 | 2026-11-07 05:14:07 | Instrumental uplifting trance, 139 BPM, in C major | 137.99 BPM, kicks fit within 5.1 ms | `71862fd483bdf83fec6dd72698862754a77c8cd2bde8cc7c8ca318a29a792ac7` |
| h | Shoulder to Shoulder | `254e2dee8e0f` | 2026-11-07 05:10:22 | 2026-11-07 05:07:11 | Instrumental uplifting trance, 138 BPM, in C sharp minor | 138.015 BPM, kicks fit within 2.5 ms | `e5948b1aed72d2f25d006f4de0d157470eaf368b687ea0ee29a948353e049468` |

Each song is 144 to 190 s (`queue/played/<id>.wav` -> `out/station/<id>/take.wav` on the station's host).

Also pulled and measured, not used (no clean 24-bar full-band phrase with the lead in from the first bar): Siege Engine Breakfast `73da41e752b0`, Tin Halo `a06b6781fdcb`, Next Chapter `da6d3b8f8e8d`.

Why these: uplifting trance, instrumental (the render's lyrics are `[instrumental]`), a four-on-the-floor kick, a pumping or rolling bassline
and a lead synth in front (a soaring lead, a climbing supersaw arpeggio or a pluck melody), all aired in the station's trance blocks of
2026-11-06 20:30 to 05:30 UTC. The psytrance, Goa and industrial trance songs were left out: the brief asks for uplifting or progressive trance.

Stems: `separate.py` (htdemucs_6s on a GPU, about 16 s a song) gives vocals, drums, bass, other, guitar and piano.
The game's lead (the "high synth") is other + guitar + piano + the vocals stem, high-passed at 140 Hz; bass and drums are
their own stems. The vocals stem is folded in because these are instrumentals (the lyrics are `[instrumental]`, and the
ledger's `vocal_notes_aired` is 0 for every song used): on c (Golden Hour Coming), d (Brand New Morning) and h
(Shoulder to Shoulder) most of the lead is in it, and the separator took it for a voice. If a take of those three sounds
like a person, `LEAD_STEMS` in `build.py` and `analyze.py` is where to drop it.
