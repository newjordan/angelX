# Model castles

The realm now has a March in the south: one small castle for each model
family that serves in angelX. Each castle's own model named it, named six
knights of its house, chose how it is built and picked the two colours of its
banner. The knight who serves a session rides out of the castle of the model
that is serving, and the Delve's party is named from those houses.

Castles belong to model families, never to machines. A self-hosted model is a
mode of the `local` club and serves from its family's castle: Qwen served
locally is the Qwen castle. A route with no model family (the practice stub,
an unnamed finetune) has no castle; its knight is the Keep's own, in the
Keep's red.

## The land

The realm grew by four rows of screens to the south, still in the old
pattern: an authored screen at every even grid position, grown wilderness
between them.

- Four south gates were opened in the precinct (market gardens, artisan
  quarter, colosseum, tournament). Wild screens carry their roads down.
- The near March (realm row 8) and the far March (row 10) each have four
  authored screens with two castles apiece. Each castle stands on the meadow
  above the March road, with a short path from its gate down to the road and
  its family name carved below the road.
- The March road runs east to west across both rows, through the wild screens
  between them (one has a plank bridge over a lake). A north-south road
  through every March screen links the two rows and the precinct.
- Wishes still stand only in the heartland's wild screens, so no lot the
  party already raised moves.

## The houses

Placement follows the Round Table's own order: the first eight on the near
March and the rest on the far one, west slot then east slot.

| Family | Castle | Banner | Built | Knights | Answered by |
|---|---|---|---|---|---|
| Sol | Dawnward Keep | pale gold on deep teal | pale stone, 3 towers, pointed roofs (slate) | Auren, Liora, Tavian, Merren, Solen, Veya | gpt-6.1-sol |
| Luna | Moonwell Keep | pale aqua on oxblood | pale stone, 2 towers, pointed roofs (slate) | Aster Vale, Mira Quill, Tovin Reed, Nessa Flint, Orin Wren, Pella Starling | gpt-6-luna |
| Grok | Axiomholt | pale gold on umber | pale stone, 3 towers, pointed roofs (slate) | Cael Voss, Keir Ash, Torin Hale, Sable Wren, Edda Flint, Jory Pell | grok-4.7 |
| DeepSeek | Lanternmere | gold on deep teal | dark stone, 3 towers, pointed roofs (slate) | Ardent, Cailen, Dunsley, Elyan, Fenwick, Gwyndol | deepseek-v4-pro |
| GLM | Lumenfast | sea green on bone | pale stone, 3 towers, pointed roofs (slate) | Quillon, Seraphine Vale, Harlan, Maren Duskwind, Bastion, Isolde Quill | glm-5.3 |
| Kimi | Noctua Athenaeum | deep teal on bone | pale stone, 3 towers, pointed roofs (slate) | Rowan, Sorrel, Ivo, Belyn, Odessa, Tullio | kimi-k3 |
| Qwen | Azurelith | red on sand | pale stone, 2 towers, pointed roofs (slate) | Kaelen, Lyra, Thorne, Elara, Bram, Sylas | qwen/qwen3.7-plus |
| Muse | Emberveil Sanctum | cream on oxblood | pale stone, 3 towers, pointed roofs (slate) | Aurelia, Brannor, Celyn, Dorian, Elowen, Kestria | muse-spark-1.3-contributor |
| LongCat | The Silken Gate | oxblood on sea green | dark stone, 3 towers, pointed roofs (red tile) | Long Wei, Xia Rong, Zhi Tao, Ming Chen, Kai Dou, Yun Hai | meituan/longcat-2.0 |
| Hy | Hytower | frost on deep teal | pale stone, 2 towers, pointed roofs (slate) | Hyra Swift, Corin Hy, Thel Hy, Bryn Aye, Oryn Hy, Sela Hy | tencent/hy3 |
| Nemotron | Radiant Spire | amber on blue | pale stone, 2 towers, pointed roofs (slate) | Aetheris, Luminara, Vorian, Seren, Cindor, Nyx | nvidia/nemotron-3-ultra-550b-a55b |
| Gemma | Caelum Zenith | gold on pale aqua | pale stone, 3 towers, pointed roofs (slate) | Logos, Hesperos, Valen, Iris, Rigel, Kora | google/gemma-4-31b-it |
| Inkling | Velkin Hall | gold on sea green | plaster and beams, 3 towers, pointed roofs (slate) | Sylph, Bren, Nyla, Varek, Tessa, Draven | thinkingmachines/inkling |
| Laguna | Noctiluca Hold | amber on deep teal | pale stone, 3 towers, pointed roofs (slate) | Sylas Brightward, Mira Thornweave, Corin Deepwater, Elara Moondelve, Thane Ironscribe, Lyra Goldentongue | poolside/laguna-s-2.1 |
| North | Ravenhurst Manor | amber on umber | timber, 2 towers, pointed roofs (red tile) | Aelar Stormrider, Bran the Quiet, Cyril Ironhand, Ewan Frostblade, Faelar Sunhelm, Galen Stormstrike | cohere/north-mini-code:free |
| Astra | Kindlestar Hearth | pale gold on blue | pale stone, 3 towers, pointed roofs (slate) | Aven, Nimeth, Orrel, Tamsin, Caelis, Merrin | gpt-6-astra |

All sixteen families answered, so there are no placeholder rosters. If a
family is added later and can't be asked yet, its entry carries
`"placeholder": true` and no names. Its castle still stands, built plain with
a grey banner, and its knights fall back to the Keep's household. No one
writes names on a model's behalf.

## Where the names came from

The asset is `cockpit/assets/realm/houses/houses.json`
(schema `angel-realm-houses/v1`), loaded by `cockpit/src/stage/houses.rs`.

- **One call per family, no retries,** each through the family's own angelX
  route via `angel --ask`. Every call was low effort.
  - Sol, Luna and Astra: the ChatGPT plan through the openai club, as
    gpt-6.1-sol, gpt-6-luna and gpt-6-astra.
  - Grok: grok-4.7 through angelX's Grok sign-in.
  - DeepSeek, Kimi and Muse: their own clubs.
  - GLM: the glm club, one short call.
  - The rest: the openrouter club, using the same model ids that chose the
    Round Table seats.
- **The prompt is neutral.** It describes the realm and what a castle is for,
  and gives the realm's purpose (goodness, light and knowledge). It then asks,
  in the model's own voice, for a castle name, six knights (name and short
  epithet), a build from the kit's options and two banner colours from the
  signal palette. It says the names need not follow any existing legend. The
  full text is stored in the asset under `prompt`.
- **The houses were asked one after another,** in a shuffled order (stored as
  `draft_order`). Each model was shown only the banner pairs already taken, so
  no two banners match. Nothing else about the other houses was shown.
- **Every answer is kept verbatim** in each entry's `reply`, with the model,
  the date and the route. A test checks that every castle and knight name in
  use appears in its model's own reply.

### Renames

The first poll's names collided across houses. Muse's castle was
"Lumenhold" and Kimi's was "Lumenhold of the Quiet Star". "Liora" was a
knight in three houses, and most castles shared the root "lum".

The houses were walked in placement order. The first house to hold a
castle name (or its main word's root) or a knight's first name keeps it.
Each house that collided with an earlier one was asked once more, through
the same route and with the same framing. The only addition was the list of
castle, knight and house names already taken. It could keep any of its own
names that were not taken. That was one call per house, with no retries.

- **Renamed on the first re-ask (9 houses):** Grok, GLM, Kimi, Muse, Hy,
  Nemotron, Inkling, North and Astra.
- **Gemma:** its first re-ask timed out, so the same prompt was sent once
  more. It renamed: Caelum Keep → Caelum Zenith, and the knights Solon,
  Astra, Orion and Selene became Logos, Hesperos, Rigel and Kora (Valen and
  Iris kept).
- **Laguna:** it answered the first re-ask with its first names unchanged,
  so it was asked once more, with firmer wording: the reply must not reuse
  a listed name or the root "lum". It renamed its castle (Luminae Hold →
  Noctiluca Hold) and sent back the same six knights. The castle now shares a
  root with Kimi's Noctua Athenaeum, and Sylas, Elara and Lyra (Qwen), Mira
  (Luna) and Corin (Hy) are first names in earlier houses. These stay open:
  there were no further calls.

Each re-asked entry keeps its first answer under `first` and the re-ask
under `renamed`. Overlaps that remain are listed in `open_collisions`. A test
fails on any shared name that is not listed there.

Almost every model chose pale stone with pointed slate roofs. That was their
choice, so the castles are told apart mostly by their banners, their tower
count and a few walls: dark stone (DeepSeek, LongCat), timber (North),
plaster and beams (Inkling) and red tile roofs (LongCat, North).

## Which castle serves

Each frame the app works out who is serving (`App::serving_houses`,
`cockpit/src/app/control/turn_io.rs`):

- **The lead** is the route answering the running turn, or the route in hand
  when no turn runs. It goes through `profile_for_route`, the same
  model-family match the portraits use, to its house.
- **Formation seats.** While a formation is armed or in hand (MoA, Tag Team
  and so on), the house of each assigned seat's model sits beside the lead. If
  the club in hand has no family of its own (the MoA swarm club), the first
  seat's house leads.

That one fact drives everything below. It is published to the World
(`note_serving`) and to the Delve (`houses::note_serving`, kept per thread
on the UI thread).

## What you see

Every mark stands for something real:

- **Banner.** A castle flies its banner only while its house serves (the lead
  or a seat). Its windows and gate are lit and the portcullis is up. An idle
  castle stands dark with a bare pole.
- **The serving knight** wears his house's colours (field on plume and
  tabard, charge on the shield). He waits at his own gate and rides out when a
  turn starts: to the Keep, then wherever the tools send him (smithy, lists
  and so on). He rides home when the turn ends. A call to the Delve sends him
  from wherever he is to its gate.
- **A change of club** (Tab, the model menu, failover, MoA engaging) lowers
  the old banner and sends the old knight riding home along the roads. He is
  gone at his gate. A knight of the new house rides out of his own castle.
- **A formation** sends a knight out of every seated house's castle. They
  ride to wherever the lead is headed and draw up beside him on the nearest
  open ground. When the formation stands down they ride home. A seated knight
  who becomes the lead takes over from where he is riding.
- **`/world visit <family or castle>`**, for example `/world visit kimi` or
  `/world visit lanternmere`, frames that castle. `/world follow` returns the
  camera to the working knight.

## The Delve

The Delve's kits (bow and roll, crossbow and kite shield, handgonne,
blade, and the six harness-lane kits) are unchanged, and so are their
ultimates. They are now callings, and the knight who wears one comes from a
house:

- The host's seat takes the serving house. Further seats take the
  formation's other houses, then the castles next along the March.
- A house's knight *n* wears kit *n*. The intro's KNIGHT row shows
  "Ardent of Lanternmere, the Seeker", and the line under the figure gives
  the knight's own epithet ("Ardent, Bearer of the Lantern"). The HUD, the
  guest sidebar and the resume line show the knight's name.
- The idle crawl in the mini-viz names its party from the serving houses too.
- With no house serving, the Keep's household (Sir Percival and the rest)
  takes the field as before.

Saves are unaffected. A hero records `house` (the house key) when dressed,
under `#[serde(default)]`. Kit ids (`percival`, `lynette` and so on) are
unchanged, so old checkpoints and realm files load as they are: knight
experience, talents and ultimates stay keyed by kit. A hero saved before the
houses has no `house` and is named by the Keep's household.

## Code

- `cockpit/src/stage/houses.rs`: the house table (from the asset), family
  lookup, who is serving, and which house fills a party seat.
- `cockpit/src/stage/world_viz/overworld/castles.rs`: castle placement and
  sprites, banners, carvings, house colours, and the March's riders.
- `cockpit/src/stage/world_viz/overworld/map.rs`: the March screens and the
  precinct's south gates.
- `cockpit/src/stage/world_viz/overworld/live.rs`: the walker's house, its
  retinue and its relieved knights, and resting at the castle.
- `cockpit/src/drive/together_shooter/knights.rs`: kits worn by house knights.
- Tests: `tests/cockpit/app/houses__tests.rs`,
  `tests/cockpit/world_viz/overworld__castles_tests.rs`, and
  `a_kit_is_worn_by_a_knight_of_the_serving_house` in
  `tests/cockpit/app/together_knights__tests.rs`.
- Renders:
  `ANGEL_OVERWORLD_SHOTS=<dir> cargo test --bin angel write_castle_shots -- --ignored`.

## Next

- **Renown at the castle.** Delve runs now record which house each knight rode
  for. The next step is to tally that house's floors and wins and show them at
  its castle: pennants on the meadow below the road, lanterns earned. This
  needs a per-house tally where the Delve banks experience
  (`dungeon_realm.rs`, on the barony branch).
- **Seats beyond MoA.** `spawn` formations and agent-graph seats know their
  clubs. Passing each seat's route through the agentviz stage would let their
  houses ride too, and would put real house knights at the Lists for duels.
- **House colours in the Delve.** Dress each knight in his house's colours
  over the kit's own, in the arena sprite (barony-owned `arena.rs`).
- **New families** get a castle by joining `houses::ORDER` and asking their
  model once. The far March still has room, and wild March screens can take
  more castles.
- **Renaming the architecture** (clubs, formations) to fit the kingdom is
  deliberately left for later.
