# The first session

A new friend opens the link in their browser, types a name, presses Play,
and is in the Sozzled Griffin with the party. What they see in the next ten
minutes decides whether they come back. This page is how that is measured,
what the measuring found on 2026-10-29, and what was changed.

## The beginner bot

`angel_delve::first_session` (`crates/delve/tests/first_session__bot.rs`,
built for tests and the `testing` feature) plays a friend's seat the way the
live box starts a delve: `Run::at_home`, a naked start, open floors, the
host's own place empty (`ANGEL_DELVE_HOST_KNIGHT=0`), the party in the
tavern, the friend seated as knight 2 in the company's colours
(`live_start`).

It plays like a beginner, and every number it plays by is a knob
(`Beginner`):

- it looks at the screen every 8 ticks and holds its keys in between; it
  turns on a foe only once it has been awake and in sight half a second;
- its aim is off by up to 35 degrees either way, it wanders on one look in
  eight, and it side-steps a coming shot one time in four;
- it stands still two seconds in every new place, eight seconds to type a
  wish (the wish line holds the keys, as the page's does) and three to read
  the scroll's offers, and makes one wish at a time, half a minute apart;
- it goes for what lies near labelled E TAKE (the cache's weapon, a helm or
  mail a kill dropped), and gives up on what will not come;
- it knows only the walk keys. Every other key it learns from the words on
  its own screen: a line in the room's view once it has been up a second
  and a half; a line in the bands (the HUD) only at a glance, three percent
  a second; a toast the host sent. Or it tries keys when nothing it knows
  works (Space, E, the arrows, Q, T, three seconds apart) and keeps the one
  that did something.

What a seat's screen says is read off the renderer itself: `ink::said`
records every line `Img::text` writes while a reading is open (with where,
and on how big a picture, so a line on a room's whole canvas counts only
where the camera's crop shows it), and every board `notices::board_from` puts
up over the view. The screen is drawn as the client draws it (the view, the
presence marks, the scroll, the first steps' hints, the bands), without the
host's empty place, which the wire never sends.

### Running it

    FS_SEEDS=1,2,3 FS_SECS=600 FS_OUT=<scratch>/fs FS_FILM=<scratch>/fs/film \
      cargo test --manifest-path crates/Cargo.toml -p angel-delve --profile fast \
      --lib first_session_sim -- --ignored --nocapture

(under `systemd-run`, as the build rules say). Per seed it prints seconds
from the link to the first move, fire, weapon, hit, kill and wish, to floor 1
and out of it; falls, and how many in the first three chambers fought; runs
lost; kills; idle seconds; seconds a board stood over a fight. Then, for the
first seed, every key learned and how, every place come into, and every line
the screen showed (when first up, how long, when read). `FS_OUT` writes all of
it as JSON per seed; `FS_FILM` writes the first seed's screen every
`FS_FILM_EVERY` ticks as raw RGB (`film.rgb`, `film.txt`) to cut GIFs from.
`FS_TUNE=clear_rest=0,...` moves server knobs and `FS_PRIMER=0` turns the
first steps' hints off, to measure one change alone. `FS_TRACE=1` prints the
beginner's every third of a second, and every blow it takes.

Seeds run one after another: the renderer's caches are shared, and threads
drawing at once wait on each other far longer than they save. A ten-minute
session takes four to eight seconds.

### In a browser

    ANGEL_DELVE_OPEN=1 delve-dev-host --bundle <dist> --scene live --newbie 1

puts the same beginner at the first seat of a live-box start (knight 1, no
empty place beside it); a browser that joins that seat watches the
beginner's screen as its own, and the beginner's clock starts when it joins.
`<scratch>/first-session/film.mjs <link> <dir> <secs>` films that page in
headless Chromium (the join form, Play, then the game in the page's stage
presenter).

## What it found (2026-10-29, twelve seeds, ten minutes each)

| | before | after |
|---|---:|---:|
| first move | 2 s | 2 s |
| floor 1 reached | 24 s | 24 s |
| the floor's wish sign read on first arrival | 0 of 12 | 12 of 12 |
| first wish made | 53 s | 37 s |
| first kill | 43 s | 46 s |
| falls in ten minutes | 5.8 | 2.4 |
| falls in the first three chambers | 3.8 | 1.3 |
| runs lost (back to the tavern) | 5.7 | 2.4 |
| floor 1 left within ten minutes | 0 of 12 | 5 of 12 (at 381 s) |
| idle | 271 s | 180 s |
| boards over the view in a fight | 30 s of 197 s | 19 s of 168 s |

The worst three frictions, and what was done:

1. **The first rooms killed the newcomer.** A naked knight takes about 31 of
   100 health from any hit on the first floor; a beginner who dodges one shot
   in four walked into the next chamber on a fifth of their health and fell
   there, then walked back from the tavern (25 s) and fell again. **A
   breather** (`Run::breather`, `rules/depths.rs`): a chamber's company put
   down on an open first floor (as the live box lays every floor) mends every
   knight standing 30% of their health, as a rest, with the potion's sound and
   a line on their own screen.
   `Tune::clear_rest`, `clear_rest_floors` (dev panel, first floors); 0 is
   the game as it was. Measured alone it takes the falls from 5.8 to 2.4.
2. **The first wish's sign was spent under the floor's title card.** THREE
   WISHES / T TO MAKE THEM started on the tick the floor began, the same tick
   as the title card (1.8 s) and Fortune's dare: 1.2 of its 3 seconds showed,
   under the dare, and the bot never read it on a first arrival. The sign
   now waits for the card, and the dare waits behind both (`Tune::dare_after`,
   5 s) and comes in on the shared motion. Read 12 of 12; the first wish
   comes 16 s sooner.
3. **Nothing names the fire key or the wish key in the view.** The tray's
   keycaps (F, SPC) are the only words for the fire key, and the wish key's
   only words were the sign above. **First steps' hints** (`arena::primer`):
   ARROWS: FIRE (PUNCH, bare-handed) when a foe has been awake a moment near a
   knight who has not fought on this floor, and T: MAKE A WISH in a calm
   moment on the first floor while a knight holding wishes has made none;
   each leaves the moment it is used and never comes back on that seat. They
   are the client's own (the minimap's kind: the run and the wire never hear
   of them), every timing a client knob (dev panel, first steps). Measured,
   they show in every session but change no number: with the sign fixed it
   teaches T first, and this beginner finds fire from the tray or by trying
   keys before its first fight. They stand as the net under a player who
   reads neither.

## What is still weak

- **The first kill came 3 s later**: a beginner who knows T earlier wishes in
  the arrival chamber (eleven seconds standing still) before its first fight.
- **The floor's exit**: 7 of 12 still do not leave floor 1 in ten minutes;
  the guardian is the wall for a lone beginner.
- **Two ways to wish, told differently**: the sign and the HUD say T, the
  page's toast and the lamp say E at the genie's lamp. Both work; a newcomer
  is told two things at once.
- **The bot's own limits**: it learns a key by trying keys more freely than a
  shy player might (it found T by chance in eight seeds of twelve), it never
  reads the page's own text (the join form, the wish line's placeholder), and
  it plays alone, where a new friend usually joins a party.
