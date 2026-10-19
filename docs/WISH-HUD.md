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
--bots 3 --idle-bots --art v2`, join the browser before bots, press T, submit
`triple shot`, and confirm 1 becomes 0 only for that seat.
