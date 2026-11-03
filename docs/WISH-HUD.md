# Wishes in the seat HUD

The count is the knight's current **phrasebook window allowance**: 1 when
`Run::can_wish(id)` is true, otherwise 0. Each room clear and Sanctuary entry
opens a window with one wish per living knight. A fight closes it. Unused
wishes do not accumulate; there is no gold cost or floor/run stockpile.

The T line submits words. The cockpit matches the phrasebook against that
knight's deck and grants its next available tier, replacing the earlier tier.
Unknown words can be taught by the cockpit; starting that lesson spends the
window immediately. A known grant spends the window only on success. A full
ladder, full spell slots, invalid card or full book cannot spend it. `grant`
itself is also used by tools and delayed lessons; interactive callers own the
`can_wish` check. The HUD does not change any of these rules.

Sanctuary custom reforging has its existing separate once-per-floor allowance
(`can_reforge`, `reforged_on`); this count does not combine it with phrasebook
wishes. Realm building wishes use the shared treasury and are also separate.

Each browser seat reads its own knight's count beside the action wells, on a
registered `dgn.hud.chrome` status plate. The cell sidebar shows the same
count and reason near health, including in compact mode. The local knight's
details come first in the sidebar. A new window produces a short earned
notice; authoritative window consumption produces a spent notice, including
learning before a card arrives. Falls, revivals and fight closure do not
produce spending notices.

No Run, save, command, protocol or simulation field changes. `Hud.wishes` is
derived locally from the existing replicated window and health, with a serde
default for older local HUD JSON. Clients observe changes rather than sending
new notice events. The development host now accepts the existing known-wish
action kinds 1/2 for browser QA; unknown phrases are refused there without
spending because it has no teaching service.

Review: `ANGEL_WISH_SHOTS=DIR cargo test --manifest-path cockpit/Cargo.toml
--bin angel write_wish_hud_review -- --ignored --test-threads=1` writes four-seat
native frames, the real terminal cell buffers as HTML, and before/after Run
snapshots. Start the dev host with the before snapshot, `--scene first
--bots 3 --idle-bots`, join the browser before bots, press T, submit
`triple shot`, and confirm 1 becomes 0 only for that seat.

## The wish scroll and the genie

A wish asked for (the T line, a picked known wish, a browser seat's words)
no longer grants at once: the host's run opens the knight's wish scroll
(`rules::genie`, `Hero::offer`) with three ways to have it, and nothing is
spent until one is picked with 1, 2 or 3 (a tap on the browser's choice
slips); Esc rolls it up. As you said; bolder (a step further up the ladder
for a third of the knight's health) or, without that step, the genie's whim
(a wish he picks) or, for a super wish, save some (one step and a plain wish
kept); and the genie's deal (the wish plus gold, or a mending and gold, which
fills none of his freedom). Only the ladder's checked tiers are granted, so
the card checker and the wish caps still decide what a wish gives. Learned
words are spent the same way (`Run::asked`), but the host sends them to a
grant runner the moment the scroll opens on them (`teach_spoken`): the model
writes while the knight chooses, and the choice only says how the entry is
granted, at once when it was written first. Rolled up, or picked for the
genie's whim, the entry is still written into the phrasebook for next time,
granted to no one, and no wish is spent on it. A known wish, the freeing
words and a knight still being made wait for the pick, as before.

Every sealed wish fills the party's freedom gauge (`Genie::freedom`, out of
100) by its grade and the tier it reached, double for a super wish. Full, a
knight with a super wish may say "I wish the genie were free": every knight
sees him freed, and from then on every floor gives three wishes. Old saves
load with an empty gauge and no scroll open.

The window and the browser draw the scroll over the knight's own view
(`render::genie`, the art team's genie from `crates/delve/assets/genie`,
cut by `scripts/delve/genie_art.py`); a terminal drawing cells draws its own
half-block scroll. Review: `ANGEL_WISH_SCROLL_SHOTS=DIR cargo test -p
angel-delve --lib write_wish_scroll_shots -- --ignored` (pixels) and the
same variable with `cargo test --bin angel write_wish_scroll_cells --
--ignored` in cockpit/ (cells).

## The wish altar and the grant runners

Wishes are made at the wish altar (`rules::altar`): a stone plinth with
the genie's lamp on it, in the first floor's start room, in every
Sanctuary and beside an open floor's arrival. A knight who holds a wish
walks up and presses E at the lamp: their wish line opens on their own
screen (the browser's text line, a terminal's scroll), the same line T
opens anywhere. Nothing is spent by rubbing; the scroll's choice spends it.

New words go to the host's grant runners (`crates/realm-host/src/together_grants.rs`):
eight threads, started and warmed when a delve begins, each taking one wish
at once. A runner asks the host's model for the phrasebook entry as a
reply (no tools, no turn of the host's own), checks it with the game's
checker, has it mended once if turned away, and hands it back; the host
appends it to the workspace's phrasebook and grants it on its next look,
mid-fight or not. No wish waits on another, nor on the host's own turns;
a knight may have three in flight at once, scrolls still being chosen
among them. While any is in flight the
lamp smokes and a curl rises over its knight's head; when one lands the
lamp flares, the knight is lit by sparks, everyone hears `wish_grant`,
and the knight's screen says what they were given.

Timing bench: `ANGEL_WISH_TIMING=<dir> [ANGEL_WISH_TIMING_REAL=1] cargo
test --bin angel wish_grant_timing -- --ignored --nocapture` in cockpit/;
`wish_choose_timing` the same way times words spoken to granted with the
knight taking `ANGEL_WISH_CHOOSE_MS` to pick.
Shots: `ANGEL_ARENA_SHOTS=<dir> cargo test -p angel-delve
write_wish_altar_shots -- --ignored`. The development host stands in for
the runners with `--grant-after SECS`.
