# Usage images

Unedited captures of the running cockpit in Kitty. The task images come from
one live GLM-5.3 turn fixing a planted bug in a small Rust crate; each action
was approved by hand. Files under `ui/` are crops of those captures.
[Capture details and hashes](provenance.json).

| Image | Shown behavior |
|---|---|
| [Startup](intro.png) | Fresh launch: Excalibur raised, the knight on the summit. |
| [Task](task.png) | GLM-5.3 reads the crate while the knight rides to the Scriptorium. |
| [Model picker](ui/model-picker.png) | `/model` with each route's thinking level. |
| [Thinking picker](ui/think-picker.png) | `/think` for the selected route. |
| [Approval](ui/approval.png) | An action capsule waiting for approval. |
| [Edit approval](ui/edit-approval.png) | An anchored edit with its byte change. |
| [Receipts](ui/receipts.png) | The turn summary and the answer. |
| [Diff](ui/diff.png) | `/diff` after the fix. |
| [Goal](ui/goal.png) | `/goal` with criteria and a verify command. |
| [Loop workshop](ui/loop-workshop.png) | `/loop` limits for the active goal. |
| [Formations](ui/formation.png) | `/moa` formation roster. |
| [Context](ui/context.png) | `/context` window occupancy. |
| [Commands](ui/command-picker.png) | `/help`, then `/` and `Tab`. |
| [World view](world.png) | The overworld map beside `/diff`. |

## The March and the world above the Delve (0.2.1)

Frames written by the cockpit's own renderers through their shot-writer tests
(`write_castle_shots`, `write_world_shots`, `write_barony_shots`,
`write_joust_shots`): staged scenes, unedited. The castles strip is five castle
frames side by side, scaled 3× with nearest-neighbour.

| Image | Shown behavior |
|---|---|
| [The March](realm/castles.png) | Five castles of the March; the serving house's knight at its gate. |
| [The Delve's gate](realm/gate.png) | The gate courtyard: the stair down, the signpost, two knights. |
| [The King's Hall](realm/kings-hall.png) | King Brannoc on his throne, the war table and the Great Forge. |
| [The lists](realm/joust.png) | A course against Sir Kay as the lances meet. |

## The Delve and the loop's crawl (0.2.0)

Frames written by the cockpit's own renderers through their shot-writer tests:
staged scenes, unedited. The terminal draws the crawl's frames in Dotmax dots.

| Image | Shown behavior |
|---|---|
| [The Undercroft](delve/undercroft.png) | Home, built out, with Tobbin's forge offering its fourth rung. |
| [Dame Fortune's Wheel](delve/fortune-wheel.png) | The wheel mid-spin before her audience. |
| [The Fungal Deep](delve/fungal-deep.png) | Two knights in a sanctuary on the fifth floor. |
| [The Siege Perilous](delve/tavern.png) | Maud's tavern: the bar, the rumours and the chair. |
| [The Trophy Hall](delve/trophy-hall.png) | Statuettes on their plinths; one gilded. |
| [Wren's bounties](delve/bounties.png) | The bounty board read at home. |
| [Crawl: a fight](crawl/fight.png) | A skeleton archer in the Mines while the loop edits. |
| [Crawl: a chest](crawl/chest.png) | A measurement's chest, opened. |
| [Crawl: a guardian](crawl/guardian.png) | The hall's guardian in Dragon Keep. |
