# Kingdom Works: the Frontier

Knights Wish now has a persistent construction and production game. The founding
camp begins with six workers, four road carriers, and limited wood, stone and
food. Workshops produce into local buffers. Carriers take goods along actual
roads to other workshops or the camp. Adventurers secure the rare resource
claims that make metallurgy, enchanted equipment and a castle possible.

![The working waterwheel village](images/kingdom-works/village/waterwheel.png)

## Playing

Open `/world walk`, then press **4** on the surface to open Kingdom Works.

| Key | Build page |
|---|---|
| WASD | Move the plot cursor; scroll the map |
| Q / arrow keys | Cycle plans; road is first, shafts and leylines are later plans |
| E | Pay for construction; hold while moving to paint roads, walls or connectors |
| X | Dismantle a module or road; with a connector selected, remove only that network |
| F | Pause or resume the selected workshop |
| 1 | Cycle that workshop's recipes |
| 2 | Assign a secured, unassigned claim to a mine |
| 3 | Toggle expedition supply |
| P | Cycle Build, Charters, Armory, Claims, Ores, Art, People, Land and Works |
| Space | Toggle close and wide map views |
| C | Bring the company into the constructed frontier |
| 4 | Close the desk |

On Charters and Armory, WASD or Q selects an entry and E funds research or
issues/equips the selected item. On the frontier, WASD walks between the actual
buildings. E at the southern gate returns to the campaign surface. Each player
has their own desk cursor and selection; the kingdom economy is shared.

Build a **forester** near woodland and a **quarry** near rock, beside the founding road. Add a **farmstead** to feed residents, then a sawmill and homes. A clay pit near orange clay feeds a brick kiln; its bricks build cottages, modular walls and passable archways. Wait for carriers to deliver their output. Research the
**Stonecraft charter** to unlock the mason. Housing and storage must also be
connected and completed before they contribute capacity. The objective strip
on the desk explains the next stage.

Rare claims come from delves. The Ore Guide identifies the resource, its routes,
its defenders and their counterplay before you leave. Floor-entry warnings
report the current ore and the actual next floor's resource. Approach a vein
marker to discover it and its two ore sentries. Clear the defenders and the
room, then press E at the seam to challenge its guardian. Win the fight and
press E at the seam again to secure it. Clearing the ordinary room or leaving
and returning does not defeat the guardian. A secured claim persists and may
supply one mine outpost at a time. Claims grant no ore immediately: the mine
must be built, connected, staffed and assigned the claim with **2**.

The Claims ledger has 64 slots. Select a linked claim and press **2** to unlink its outpost; buffered and travelling ore remains. Press **X** twice, with separate key presses, to release an unlinked claim. This frees a slot while preserving earned resource evidence for land charters. Live encounters cannot be retired, and their completed death records remain to prevent duplicate rewards.

| Delve source | Resource | Claim guardian |
|---|---|---|
| Crypt | Coal | Cindermaw |
| Mines, odd depth | Iron ore | Ironroot Colossus |
| Mines, even depth | Moon crystal | Amethyst-crowned Wyrm |
| Dragon Keep | Living ember | Cindermaw |
| Archive, Fungal Deep, Unknown | Crystal | Amethyst-crowned Wyrm |

The sentries are dangerous material-specific enemies with distinct painted
silhouettes and combat behavior:

| Ore enemy | Warning and counterplay |
|---|---|
| Ironplate sentinel | Frontal plate turns shots; flank it or break its guard, then punish the bash recovery |
| Blast crawler | Blinking kegs and marked bomb landings; leave fuse circles and shoot kegs from range |
| Prism stalker | A flash precedes the copied shot; sidestep or raise a barrier after firing |
| Cinderhound | A locked charge lane becomes a burning trail; sidestep and punish its recovery |

Sentries have a finite encounter ledger, so retreat and checkpoint restore do
not create fresh rewards or bypass the guardian requirement. The Ore Guide uses
the same resource mapping and enemy descriptions as the encounters.

The guardians use the game's real boss combat: warning shapes, recovery
windows, damage, phase changes and normal death settlement. Ironroot charges
and slams; Cindermaw leaps and inhales; the Wyrm uses beams and rings. Their
painted rest and wind-up poses appear in combat.

## Building a kingdom

The main chain is **iron claim → outpost → smelter + charcoal → ingots → royal
forge → equipment**. Stone becomes blocks and wood becomes planks. Moon crystal
feeds the mana conduit and enchanter. Food feeds the trail kitchen.

Five charters unlock the next tier: stonecraft, metallurgy, caravans, leycraft,
and sovereignty. Their costs are paid from delivered camp stock. Caravanserais
add four carriers; warehouses add storage. Cottages add beds. Fed residents arrive over time when beds and provisions are available; empty homes do not instantly add workers. A completed castle keep adds sixteen beds and two hundred storage per resource.

The forge initially makes an Iron Oathblade. Press **1** at the forge to choose
other recipes, including generic expedition arms. The enchanter initially
makes a Hearth Wardstone. Finished equipment must be delivered to the camp,
then issued through the Armory. An owned item can be equipped again without
consuming another manufactured copy. Loadouts belong to persistent profiles
and are restored for future expeditions.

| Crafted item | Combat behavior |
|---|---|
| Iron Oathblade | Broad melee sweep |
| Prospectors Pick | Narrow, heavy melee strike |
| Ember Edge | Melee sweep and returning ranged blade |
| Runebound Aegis | Held mana barrier; empowers allied shots |
| Mooncrystal Staff | Three-shard ranged fan |
| Hearth Wardstone | Reusable heal and brief ward, with cooldown |

When supply is enabled, each present knight beginning a new expedition also
consumes available generic arms, rations and ward stock: respectively +2 armour,
1 bomb and ten seconds of arrival protection. Ordinary floor changes do not
issue those supplies again.

## The Delve is the quarry

The hillside above has field stone and rubble only. Everything worth building
a castle from is won below, seam by seam, through the same sentries, guardians
and charters as the rare ores (`rules/geology.rs`). Each part of the Delve has
its own rock, and the rock a party wins sets what its castle looks like.

| Delve | Quarry stone | Also yields | Castle look |
|---|---|---|---|
| The Mines | limestone | iron ore (odd floors) or moon crystal (even), galena from floor 3 | pale ashlar |
| The Crypt | flint | coal, bone | flint banding |
| Dragon Keep | basalt | ember, sulphur | a dark keep |
| The Drowned Archive | slate | freestone, moon crystal | slate roofs, chevron work |
| The Fungal Deep | tufa | saltpetre, moon crystal | light vaulting stone |
| The Swamp of Horrors | clay | peat, bog iron, reeds | brick and thatch |
| The Unknown | voidstone | moon crystal | wished-for walls |

Deeper rock is purer and cut faster: a mine on a seam cuts 2 stone a shift on
floors 1-2, 3 on floors 3-4 and 4 on floors 5-6. A claim carries `yields`, a
map of resource to rate, and a linked mine makes all of it. The older ores are
still won where they were, so every land charter keeps its route. A claim from
before geology has no `yields` and gives its one resource at its one rate.

**Cutting stone below.** Before any machinery a knight can swing a sword at a
claimed face. Three swings part a rough block, which goes on the knight's
shoulders: it is heavy (a knight walks at 62% of their pace), a friend beside
them takes it with E so a party can chain one along, and a face gives a dozen
blocks an expedition. A knight who reaches the Delve gate's stockyard sets the
block down there.

**The stockyard and the Great Shaft.** The stockyard is a store on the realm's
`Home`, saved with it and sent to guests. The shaft over it is hand carry, a
windlass, a horse gin, a water hoist and a ley lift, lifting 3, 10, 24, 60 and
140 blocks a minute (`Tier::throughput`, the number the production game reads)
and holding 16, 40, 96, 200 and 400. The winch beside the gate shows the ledger
on E and raises the shaft on a second E, paid from the camp's stock.

**The Deep Stirs.** Cutting, lifting and working mines raise a stir (0 to 100)
that settles on its own. From 30 up, on the third floor and deeper, a secured
claim the party leaves unguarded for ninety seconds has its sentries sent back
once an expedition. They are the ordinary sentries; beating them takes 15 off
the stir.

The Delve's banked gold is also counted as the kingdom's coin (`coin` in the
camp's stock, held at the stockyard until the camp has room).

**One store of rough stone.** The stockyard is where all rough stone stands:
the blocks a party cuts below, and the rubble the Keep's hillside outcrop and
its hired quarrymen add (`Run::stock_rough`). The Keep's banker masons cut
from it (`rules/hands.rs`): a quarry block makes a stone, it takes two pieces
of rubble to make one, so the outcrop is a poor early source. A block cut below
comes up on the knight's back when the party's run ends: the host starts the
next run at home with it still on their shoulders (`GameCommand::CarryBlock`),
and it is set down where the stockyard stands: on the blank-slate map, in the
pocket of open ground by the gate rock's west face (three tiles west of the
gate's stand, with the winch at its far end), open from the first stone to the
last; two tiles south of the stand on the old map. The shaft head on the gate's
crown is drawn at the tier the run has raised it to, flat and in 3D
(`world3d::lots`). The hillside's rubble is held to eight pieces in the yard so
it never fills the room the Delve's blocks are set down in.

## The Keep, on the production game

On the blank-slate map the realm's industry is the hillside kingdom
(`Kingdom::found_hillside`), and the Keep is the realm's long project: the
great tower, begun, which every run starts with 1,500 stones up. The ladder
from motte to shell keep to tower belongs to the house camps later. The
game's wall (`Run.sites`) and the sim's site (`Kingdom.sites`, plan
`great-tower`, its stages and bills) are one wall: the sim's is staked out
from the game's `laid` (`Kingdom::found_keep`) and follows it; every stone
set afterwards moves both (`rules/works_seam.rs`, `kingdom/src/seam.rs`).

- **Hired setters** are the lodges' crews: once a ten-second shift the sim
  sets what the Keep's yard can pay for (`Kingdom::work_sites`), and those
  stones stand in the game's wall under the realm's mark. The masons wait
  while a knight is bedding a stone, and when the next bay of falsework is not
  raised (the carpenters' bays are still the game's own).
- **A knight's set stone** is paid by the same bill from the same yard
  (`Kingdom::site_hand_set`), laid in order with the masons'. The set does not
  begin if the yard could not pay for it: the words say what is short.
- **Hired hands by trade** are the sim's people at their posts
  (`Run::hired_hands`, from `Kingdom::staffed_by_trade`).
- **Mortar** at the tub is the yard's own, which goes off in its shelf's time.
- **Rock** a knight brings up is set down at the yard and goes into the camp's
  stock (the hillside's field stone is the sim's `stone`); the sim's carts take
  it to the bankers. A knight at a bench dresses camp rock into cut stone in the
  yard (`Kingdom::hand_dress`): a block to a stone, three of field stone.
- **The legs between the works are carts.** Rock goes from the camp to the
  bankers, ashlar from the bankers to the Keep's yard, lime, sand and water to
  the mortar mill, poles and joists to the yard, all by the carts of the
  carter's yard (`Command::Carts`), a tonne a load, on the roads the sim keeps
  (`first_chain.rs` buys eight). With no carts bought nothing reaches the yard
  and no stone is set (`seam_tests`: 1,500 s, no trips, nothing laid).
- **The Great Shaft is one scale.** The sim's lift plans lift 3, 10, 24, 60 and 140
  blocks a minute (hand, windlass, gin, water hoist, ley lift; the last two are the
  powered figures), the numbers the stockyard's tiers always had. The stockyard's tier
  is the lift the camp has built (`Run::shaft_lift`), the winch's second E pays that
  plan's own price and raises it in place (`Command::Upgrade`), and nothing is lifted
  that a staffed lift does not raise.
- **The first chain**, as a player's script (`kingdom/src/first_chain.rs`),
  from a won seam to stones standing by commands alone; its test and the
  game's run of it (`works_seam__tests.rs`) print what holds the masons up.

Not on the production game yet: the knight's own bench (it dresses rock out
of the camp's stock directly; only the bankers' rock is carted to them) and the
templates a Master Mason draws by hand (the sim's `template` is the tracing
floor's).

The sim and the game are one place: the Keep stands where the Works' site does
(centre (133, 81), the rect `(119, 67, 28, 28)`), the Delve's hoist and the road to
it through the Keep's gate are its fixtures (`hillside::shaft_road`), and the Keep's
ground is the Keep's from the first day (nothing else is built on it). The falsework
is one system: the bays the carpenters raise ahead of the masons are the poles in the
Keep's yard (`works_seam::derive_bays`), and a knight carpenter stands the camp's
poles there (`Kingdom::stand_poles`).

## Building where you stand (2026-11-09)

Key 4 puts the builder in a knight's hand on the hillside: they keep walking, and a
ghost of the tool stands on the ground ahead with its price and whether the land
suits it, in the flat view and in 3D (`rules::hillside`, `arena::hillside_art`,
`world3d::hillside`). Each seat has its own cursor, tool and line in the making;
everyone shares the economy.

| Key | In the builder |
|---|---|
| 1, 2, 3 | Take slot 1 to 3 of the hotbar's bank in hand (nine slots, three banks; the slot in hand holds whatever tool is in hand) |
| Up, down | Another bank of the hotbar |
| Q, left, right | Step through every tool: road, every work (the swape among them), ropeway, chute, barge, the rollerway pieces (hand-chain, roller, splitter, merge, priority gate, trestle), rail, bay, route, the patterns, the copies kept, then what stands against the Deep (palisade, timber gate, watchtower, brazier) and **MEND** (E on a fire beats it out; on a cut line or a burnt works sets it right at once at the real price; on a battered wall mends it) |
| E | Lay it: now if the camp can pay, else as a plan. Hold E walking to lay a row (road, wall, shaft, rollerway, rail, any work); on a rail bay or route tool, E at a bay, then E at the bay the wagons unload at; on a line tool press at the first building, walk the way it goes holding E, let go at the second |
| , | Plan mode: everything laid is a plan for the camp's hands, paid for or not |
| X | Cancel the plan under the ghost, else take down what is under it (or drop a line or a copy half made) |
| F | Pause the work under the ghost, or set a plan's urgency (usual, urgent, low) |
| Z | Take back the last thing laid (a row or a stamp is one step) with its price |
| G | Take in hand what the cursor is on: the work, the plan, the road, the line |
| M | Mark one corner of a patch, walk, mark the other: the works in it are copied into a pattern, kept with the realm and in hand |
| R, P | Turn what is in hand a quarter (a pattern, a piece of rollerway, a swape); on a piece or swape already laid under the cursor, turn that one |
| H | The card of what the cursor points at |
| . ; ' | Change the work's recipe (at a carter's yard, the carts it keeps; at a swape, the one good it lifts), link a claim, the Delve supply (were 1, 2, 3). With the route tool in hand, `.` sends another wagon out on the route that serves the bay under the cursor and `;` sells one |
| Space | The flow overlay: arrows as thick as the throughput, red where something starves, amber where it backs up, the Clerk of Works' line |
| C | The ledger over the world (Charters, Claims, People, Works, Goals, Output, Chokes; P turns the pages, C closes it) |
| 4 | Put the builder away |

Rollerway pieces, rail, bays and routes are laid at once, tile by tile, and come back with Z
like any step (`Laid::Roller`, `Laid::Rail`); only works are laid as plans, and a swape planned and
turned (R or P) is raised facing the way it was turned (`Ghost.face`). In 3D the rail network
stands as the tramway did, sleepers on ballast with its wagons running on it, and now has its own pieces: a switch has its lever and point blade, thrown toward the way the wagon at it or coming to it goes on; a bay is a plank platform on the side of its building with a board marked by its name, and a lantern lit while a wagon works it; a wagon held up for the track carries an amber tag, a jammed one a blinking red. Rollerways are in 3D too: a tile is blocks (rollers with a pale lug riding each log, the hand-chain's trodden way between staked ropes with its labourer, the house of a splitter, merge or gate, the trestle with its second line under it), baked a chunk of the kingdom at a time like the roads (`world3d::hillside::chunk_of_belts`: four steps of the rollers' turn a chunk, one when the line stands), and each good riding them is a small shared mesh moved to its slot, so hundreds of tiles are a few draws and only the goods grow with the line (a view of more than `GOODS_SOLO` goods shows the lead good of each tile). A swape is a 3D building too: the post and its pivot arm, turned to the way it faces, swinging between what it lifts from and what it lifts to. A mule train has its own look in both views (a trodden way with hoof-prints, a grey mule with panniers), and the hods and ladders, the great crane, the vineyard, the wine press and the aqueduct are drawn (`works_kit::gaps`).

**The Keep's yard in 3D is the sim's.** What the bailey's yard shows is the stock of the sim's castle stockyard (`HillView::keep_yard`): the stack of dressed blocks, the heap of rubble and the pile of poles are drawn from what it holds (a block of the stack a score of units, `works_yard::PER_BLOCK`). The crane works while the yard can pay for another stone (`Kingdom::site_allowance`); when it cannot (the supply stopped, the yard dry) it stands idle, the jib parked over the cradle, the wheel still, the hook up and empty, the lantern out, the ladder bare. The carts bound for the yard that are within `RELAY` tiles of it (the sim's own, loaded, waiting for a berth, or unloading) are drawn in the bailey on the lane to the crane's foot at the place their distance gives them, standing in a queue at the berths, two hodmen carrying a cart's load to the stack for as long as the sim has it unloading, and are not drawn on the road too. Without the sim (the old map) the yard is the Works' own stock and its carts the loop they were.

The mouse sets the cursor and lays with the left button in the flat view
(`BuilderAction::Plot`); held and dragged it lays a row as holding E and walking does
(`Hold`, `Let`), a right click points at a thing for its card, and a click on the strip's
hotbar takes the slot (`Slot`). Works have the sizes they are drawn at (`Plan.span` and
`Plan.depth`: a sawpit is three tiles by two; the hillside only, the valley's are a
tile), their tile is their top-left corner and a road on any side joins them; a
line runs from a tile of one to a tile of the other. A post waiting on goods keeps
the people who were at it standing idle there (`Citizen.waiting`), where they were
counted to be and still free for any post that can work. A valley village saved from
before the hillside comes up onto it round the camp when the blank-slate map stands
(`Kingdom::migrate_to_hillside`). In 3D each building near the view is dyed in what it
is made of (timber, stone, brick, thatch, iron, whitewash), the look's zone budget
being sixteen (`world3d::works_look`).

## Plans, the hands, and the numbers (2026-11-09)

**Plans.** Anything the land takes can be laid as a plan whether the camp can pay for it or not
(`kingdom::ghosts`, `Command::Plan`): E lays what the camp can pay for at once and a plan
for the rest; the comma key makes everything a plan. Plans are the party's (every seat sees
and may rank or cancel them) and drawn on the ground in blue, amber while the camp lacks the
goods, teal while a crew is at them, the urgent ones marked. The camp's own hands, a crew
of a labourer who carries and a carpenter who raises (one more for each carpenter's shop that
stands, four at most), take the most urgent plan the camp can pay for, the nearest of those, take
its price from the stock as they set out, walk it there at two tiles a second, raise it in half
the building's seconds and walk back; a plan cancelled with a crew out brings the goods home.
Nothing is spent twice and nothing is raised from nothing. A stamped pattern as plans is all or
nothing; its lines are laid by hand once the works stand.

**Taking back** (`Command::Retract`): a road or a work nobody has used yet comes down with its
whole price returned, a worked one as a demolition (half); a held-E row, a dragged row and a stamp are
one step; each seat keeps twelve. **Copies** (`Command::Clip`): the works, roads, shafts and lines in a
patch (forty tiles at most) become a pattern in one of eight places the realm keeps (`Kingdom::copies`,
saved with it), taken in hand as `COPY A`... and stamped as any pattern is.

**Numbers on demand** (`kingdom::books`, `kingdom::numbers`). The books keep, on the host and
never saved or sent, each good's made, used and held for every minute of the last hour (what is
used is what is left of what was held, made and brought in over what is held at the end), and each
works' batches, seconds at work and seconds held up for the last five minutes. A card (H, or a right click) says
what a works makes, in and out a minute over the last few, stock, power, crew and what it waits for (the
Clerk's walk); a line's, a store's and a plan's have their own. The ledger's Output page graphs a good at a
time (bars made and used, a line for stock), and the Chokes page ranks what holds the works up from the Clerk's
structured causes (`Cause`, `causes_for`), the Keep's chain counting for more, and E sends the view to it (the
camera looks there until the knight walks). All of it is made on the host for the seat that asked and sent in
that seat's own board only (`wire::seat_tree` takes it from every other seat's).

**Milestones** (`kingdom::goals`). One next goal is on the view's top right at all times with how and why: win a
seam, fund Stonecraft and the First charter, raise a quarry, a carter's yard with carts (`.` at the yard keeps
more), a banker, the lime charter, mortar, a lodge, two cottages, then the Keep's own stages. The Goals page of
the ledger is the whole tree: every charter and stage, what each opens, done or next. The hillside's Charters
page lists the base charters (Stonecraft...) the hillside's works stand on, which it could not before.

## The surface's HUD, and trying it on a browser seat (2026-11-09)

The surface keeps its words in a few fixed places, and nothing is laid over another
(`the_hud_never_overlaps_at_any_frame`): the realm's label and, near the Keep, its progress
readout on one plate in the top left (`stand_back::corner`; the "hold N to stand back" hint
rides it for the first twelve seconds a seat sees it, and goes for good once N is used); the
knight's trade plate at the foot, and the builder's strip in its place while the builder is
out, with the Clerk's lines (up to three, ending in two dots when the sentence is longer)
over it; the ledger, which covers the view and stands every other mark down. The goal's
arrow and its name slide along clear of the corner, the signs of the ways down and the flow
overlay's marks. A knight carrying a block up is sent to the stockyard.

A browser seat joins a development host (`crates/delve/dev-host`) from the page the web
builder makes (`crates/web/build.sh web/delve delve`; `WEB_PROFILE=web-fast` builds it without fat LTO,
in a fraction of the time). Scenes for it: `delve-dev-host --scene works` (a
fresh realm at the spawn in the bailey, nothing in stock), `works-gate` (a step from the Delve's
door), `works-face` (a won limestone face below: Q swings, three chips part a block),
`works-block` (a block on the knight's back, the stockyard to find), `works-small`,
`works-big` (seventy buildings), `works-castle` (the Keep 9,000 stones up) and `works-old` (a
village saved on the old valley meadow, brought up onto the hillside in the first second).
Hold N in the page to stand back: the page draws it through the same path as the window.

## Logistics and simulation

Carriers advance one road tile per simulation second. Workshops advance on
ten-second production shifts. Paused solo play and offline time produce
nothing; work continues while the company is delving. Builders can reach construction over owned dry land, allowing continuous walls and courtyards. Completed productive sites and occupied homes need roads. Roads connect orthogonally to adjacent loading
docks. Removing a road can strand cargo; restoring a route resumes delivery.
Demolishing a destination redirects its cargo to camp. Material stays counted
in camp stock, machine buffers or shipments throughout transit.

Machines consume only their local input buffer when a batch completes.
Carrier dispatch prefers direct producer-to-consumer deliveries. Player priorities order requests; equally prioritized workshops rotate fairly. A caravan carries up to four units. Outputs queue
locally up to 24 units per resource; a full buffer stops production without
consuming its inputs. The selected workshop shows its status, recipe progress,
local inputs and waiting outputs. Construction, disconnection, missing workers,
missing inputs and full output buffers are distinct states.

The Works ledger controls low/normal/high job and delivery priority (**E**), production stock targets (**2**), food reserve (**3**), powered drives (**1**) and pause (**F**). Production targets count camp stock, local buffers and travelling cargo. Pause a workshop to release labour or save its inputs for research. Research and construction spend delivered
camp materials. Changing a recipe discards its unfinished progress and recovers
unused local inputs. Dismantling recovers local materials plus half construction
cost within the current storage limit. Storage is a per-resource capacity.

Residents have persistent names, homes, jobs, hunger, rest and morale. Each eats one harvest every 90 simulation seconds. Newcomers need spare beds and saved food for 120 seconds. The default reserve protects meals and newcomer provisions before the trail kitchen makes rations; setting it to zero explicitly permits the kitchen to take those supplies. Shortages reduce work efficiency and recover when supplied. Residents walk visible paths and prefer roads, but travel time does not yet gate production or construction credit. Relationships and a day/night schedule are not implemented.

The landscape is 72 by 54 plots in nine parcels. A new settlement owns the first 24 by 18 parcel. The Land ledger shows evidence and delivered supplies needed to survey the others; some require a secured ore claim. **2** pins a parcel's resource objective for the next expedition. Shallow fords accept roads; deep water requires paid timber bridges. Old saves retain their original buildable meadow.

There are 26 toolbar plans (24 physical buildings plus two network connectors), the founding camp, 23 resources, 19 recipes and five research charters. Limits are 160 buildings and 64 discovered claims. Claims remain continuous sources. Carriers have limited capacity and physical paths, with no road collision, fluid pipes or conveyor lanes.

## Workshop power

A river waterwheel needs a dry riverbank, a road and one resident. After Stonecraft, lay drive shafts between it and workshops. Shafts may share roads and paid bridges but cannot pass through an unrelated building. In Works, **1** purchases the selected workshop's optional drive; powered operation doubles work rate while preserving recipe costs. A fitted drive can switch back to manual work without paying again.

Mechanical and arcane networks are separate even when they cross on one plot. The waterwheel supplies eight mechanical units. The ley generator supplies ten arcane units and receives bound mana through normal carriers, consuming one local mana per 30 seconds of active charge. Higher-priority consumers receive power first; a machine gets its whole demand or stops. The ledger distinguishes missing roads, labour, fuel, inputs, source connections and insufficient power. Cut a connector and its downstream drive stops without consuming unfinished recipe inputs; repair the network to resume. Dismantling a bridge first requires removing the connectors it carries.

Browser pointer commands select tabs and rows, cycle plans, confirm the main action and place modules directly. Left click places on an empty plot; right click inspects. The camera stays fixed between mouse placements, while WASD resumes cursor panning. Native mouse dispatch is being integrated; keyboard controls work in both clients.

## Modular implementation

`crates/kingdom` is the independent `angel-kingdom` crate. It depends on serde,
with no renderer, host, clock, files, network or randomness. `Catalog::parse`
validates resources, recipes, building plans and acyclic research prerequisites.
`Kingdom::apply` applies atomic commands. `advance_second` runs transport,
construction and production; `work` is a ten-second convenience adapter.

The catalogue is `delve/kingdom/frontier.json`: costs, inputs, outputs, build
time, workers, housing, storage, carriers, landscape siting, power demand/capacity/fuel, connector costs, prerequisites and menu order are data.
Another world can supply another catalogue and encounter/render adapters.
Stable IDs are saved; renaming or removing in-use content needs a save migration.
Incompatible catalogues are not hot-swapped into an existing save.

The Delve adapters live in `rules/industry.rs`, `ore_threats.rs`, `claim_bosses.rs` and
`kingdom_gear.rs`. `Home::industry` and the equipment ledger participate in
revision-based settlement, observation, checkpoint restore and host sync.
An older delayed save cannot replace newer factory work or loadout changes.
Old homes default to an unstarted economy. Resource-qualified deposit IDs keep
Crypt and Mines claims separate, while matching legacy IDs remain readable.
Fresh expeditions retire unsecured sightings and their encounter records;
secured claims and assigned mines persist. Checkpoint restoration and normal
floor transitions retain the active encounter. The finite campaign through
the Unknown currently has ore encounters; infinite Deep floors need their own
adapter and do not spawn them. The host owns the simulation clock;
clients receive state over the existing game wire.

The surface uses the common flat renderer in native and browser clients,
including when the rest of the game is in 3D mode. Collision is regenerated
from the constructed buildings. The campaign's authored castle doors and
interiors remain reachable on the original surface. Browser builders and live
system watchers include the economy crate, catalogue and generated art inputs.

## Art and verification

Painted architecture, terrain, carrier walk poses, monsters and equipment are
retained under `cockpit/assets/kingdom` with prompts and manifests. Build-time
cropping and resizing produce small runtime atlases, combat poses and larger
Armory portraits. Working factories have smoke, furnace sparks and arcane
motion. Three of the six monster concepts are implemented as claim bosses;
Verdigris Mantis, Mossback Briarhorn and Chalkwing Gargoyle remain library
concepts. Copper is an extension resource without a production chain yet.

```sh
CARGO_TARGET_DIR=/tmp/angelx-economy-target cargo test --manifest-path crates/Cargo.toml -p angel-kingdom --offline
# This large engine's test harness builds much smaller without debug symbols.
export CARGO_TARGET_DIR=/tmp/angelx-final-test-target
export CARGO_INCREMENTAL=0 CARGO_PROFILE_TEST_DEBUG=0
cargo test --manifest-path crates/Cargo.toml -p angel-delve --offline --config 'profile.test.package.angel-delve.opt-level=0' --lib industry
cargo test --manifest-path crates/Cargo.toml -p angel-delve --offline --config 'profile.test.package.angel-delve.opt-level=0' --lib ore_threat
cargo test --manifest-path crates/Cargo.toml -p angel-delve --offline --config 'profile.test.package.angel-delve.opt-level=0' --lib claim_boss
cargo test --manifest-path crates/Cargo.toml -p angel-delve --offline --config 'profile.test.package.angel-delve.opt-level=0' --lib kingdom_gear
cargo test --manifest-path crates/Cargo.toml -p angel-delve --offline --config 'profile.test.package.angel-delve.opt-level=0' --lib write_frontier_review -- --ignored --nocapture
cargo test --manifest-path crates/Cargo.toml -p angel-delve --offline --config 'profile.test.package.angel-delve.opt-level=0' --lib write_ore_threat_review -- --ignored --nocapture
```

The economy tests complete a progression session from founding stock through
caravans, research, iron, named equipment, magic and castle construction without
injected intermediate materials. Additional tests cover cargo conservation,
road interruptions, demolition recovery, worker limits, backpressure, catalogue
validation and atomic failures. The ignored visual review writes eight actual
game screens; its two claims are labelled encounter fixtures, while separate
combat tests verify the real guardian gates. Normal-input starter-kit tests
fight the ore sentries and guardians; the Ironroot test then walks to the seam,
secures it, constructs a mine and observes carrier delivery.

A separate real-worker protocol check runs 950 client-input ticks, constructs
a forester, verifies delivered timber, changes desk pages, restores the runtime
checkpoint and compares its economy with the local simulation. Native cockpit
integration and browser WASM checks pass; the full native client and embedded
browser bundle are also built. [Retained screenshots](images/kingdom-works/README.md)
distinguish actual game captures from isolated art fixtures.

## The hillside works (2026-11-08)

`found_hillside` founds a second kind of world beside the valley: the realm's whole 240 x 176 slope, open from the first day, with field stone, woods, a clay bank, sand along a river and a windy ridge, and the camp below the Keep's gate. Old valley saves load unchanged and keep their 72 x 54 meadow. `delve/kingdom/frontier.json` carries the hillside's economy beside the valley's (the valley's menu is untouched; the hillside's is `hillside_menu`). The design is `ECONOMY.md`: start at the wall and ask what it needs.

- **Good stone is won below.** A quarry charter works a claim whose `yields` (rock and what lies with it) pass up the Great Shaft, a lift you raise in place from hand to windlass, horse gin, water hoist and ley lift (`Command::Upgrade`). Bankers cut the rock to ashlar, quoins and voussoirs (templates from the tracing floor) and throw off the rubble the wall's core needs.
- **Mortar goes off.** Limestone and fuel make quicklime, water slakes it to putty, sand makes mortar; mortar loses a share (one ninetieth) of what a store holds every shift, so the mill stands near the wall. (Pass 3 found the loss had been rounded up to a whole unit a shift in every store, which threw away more than half of all the mortar made; it is now the share, to the unit.)
- **Sand lies near the works.** A hollow of sharp sand beside the Keep's east wall (`hillside::SAND_HOLLOW`, a dozen tiles from the stockyard) takes the first pits; the river banks forty tiles west still serve.
- **Woods: coppice.** A woodsman's camp can fell (4 timber a shift, takes it from the wood, which stops regrowing under a quarter) or cut coppice (3 a shift, takes nothing, keeps the wood for ever). Felling is the choice for the opening hour or for new ground; coppice is the camp that sustains the wood demand.
- **Roads are a shared network.** A carter's yard keeps carts and oxen and serves every road it touches; no cart belongs to a pair of works. Each cart takes a tonne (every good has a weight: ten ashlar, twenty sacks, hundreds of nails) and mixed loads; the planner writes orders from what each building lacks and what each store holds, the nearest free cart takes one, and the cart that has just unloaded is the one that takes the next load nearby, so the way back is not empty. A road tile takes one cart at a time; past that the cart entering is slowed and the jam is remembered, so routes bend round it. A second road or a tramway is the fix (a line that stands idle, with nothing to carry, no longer shuts the roads out of the work it feeds), and the Clerk says which street is jammed ("the east road is jammed") or that "the carter's yard has no free carts". `Command::Carts` buys carts for a yard; `Kingdom::road_report` reads the yards, free carts, orders left over and jammed tiles.
- **Lifting at the wall climbs in three tiers, in place.** Hods and ladders (labourers, timber and poles, no charter; +18 percent on the masons in range), the treadwheel crane (+40), the great crane (+80, the `greatcrane` charter); each `Command::Upgrade`s the one before.
- **Pack mules.** `LineKind::Mule`: a string of mules (reared at the pasture, a rope each) on a trodden way: no road, no track, small loads (2 a mule). The `muleteers` charter.
- **Aqueducts.** An arcade of dressed stone and timber centring that gives 8 power wherever it stands (the `aqueducts` charter); with a drive of shafts it turns the water hoist, which lifts three times its twelve a shift, so the Great Shaft's tier above the gin no longer needs the river. (A lift is crewed and powered like any consumer; before pass 3 a powered hoist lifted nothing.)
- **Wine.** A vineyard and a press (`viticulture`); masters drink it.
- **Warehouses are real stores.** Makers push to the nearest warehouse with room and works pull from the nearest that holds the good (the camp is the first warehouse and keeps fixed levels), so a warehouse beside a far maker lets many carts fetch at once.
- **Ropeways, chutes, barges and mule trains stay point to point.** They are what you lay for the long haul when a road's flow outgrows it: a fleet, a berth at each end and a buffer, `Kingdom::line_report` for which is starved and which is backed up. Oxen and horses eat fodder. (The tramway is the rail network below; a save's old tramways come onto it by themselves.)
- **Rollerways and swapes: the continuous line** (`rollers.rs`). A rollerway is laid tile by tile, each piece with a way it faces, and goods stream along it three to a tile, one slot a beat, four beats a second. Nothing hops: a good moves on only when the slot ahead is empty, so a line that cannot unload backs up from its end, tile by tile, where it can be seen, and a splitter, a priority gate or a second place to unload clears it. The pieces: the **hand-chain** (a labourer a tile, free to lay, one slot a second: the cheap first line), the **roller** (two slots a second; the `rollerways` charter), the **splitter house** (deals goods to its front, left and right in turn), the **merge** (three ways in, each in its turn), the **priority gate** (front first, to the sides only while the front is full) and the **low trestle** (one line over another). A **swape** is a work, not a piece: the pivot arm that lifts one good at a time from what stands behind it to what stands in front, facing the way the goods go (a bench or store to a line, a line to a bench, store or the camp, a line to a line, a building to a building). A boy works it (a lift every two seconds, thirty a minute); fitted to a drive shaft it lifts three times as fast. An unloading swape takes only what its building will take, and can be set to one good (`Command::Filter`); a loading one set to a good keeps the line free of what the far end cannot use, which is what a jam of the wrong good needs. `Kingdom::roller_report` names each line, how full it is, goods a minute lifted off it, and the head of any jam ("the swape cannot put sawn planks into the warehouse"). Real rates, all integers and deterministic; five hundred tiles and fourteen hundred goods cost a few milliseconds a second.
- **Rails are a network** (`rails.rs`). Rail is laid tile by tile and joins every rail tile it touches; a tile with three or four neighbours is a switch, which the wagons set themselves, each by its own route. A **bay** is a named rail tile beside a building, where wagons load and unload; a **route** is a list of stops (a bay and load, unload or both, and optionally one good); wagons, each with its horse (the old tramway's price, load, speed and fodder), run it round and round. **Blocks** keep them apart: a plain tile of track is a lane each way, a switch, a bay or a dead end holds one wagon, so wagons meeting at a switch take turns and a wagon working a bay holds it while the ones behind queue. A wagon held up looks for another way round; **a jam is real** when there is none (`Kingdom::rail_jams` says which wagon waits for which). The cure is a bypass round the bay, a bay on a spur, a second track. `Command::Rail`, `Bay`, `Route`, `Wagons`, `Unroute`. A point-to-point tramway is gone from the game: old saves' tramways migrate to rail, two bays and a route when they next tick, and a new one cannot be laid by `Command::Line`. **A railway in one gesture** is `Command::Railway { from, to, path, wagons, good }`: the rail from beside one work to beside the other (the shortest clear way when `path` is empty, rail already laid counting for less, so it joins the network it passes), a bay at each end named for its work (a bay already there is shared), a route that loads at the first and unloads at the second (one good, or whatever there is) and the wagons: all of it or none, and a railway the camp cannot pay for, that has no clear way or whose ends do not stand beside the two works changes nothing. `Kingdom::plan_rail_path` answers where it would go.
- **What each way of moving goods is for** (`transport_bench`: `cargo test -p angel-kingdom --lib zz_transport_table -- --ignored --nocapture` prints the same heavy flow, stone, over six to forty-eight tiles by each way). The **hand-chain** is the first automation: a few tiles, a labourer each, nothing to buy. The **cart** is the road network: any two works on it, mixed light loads, best inside a dozen tiles (four carts carry fifty stones a minute over six tiles and eight a minute over forty-eight), a carter for a dozen carts, and oxen and harness that take a tannery to make. The **rollerway** is a fixed line of thirty a minute whatever its length (two swapes, a boy each, a timber and a nail a tile): the continuous line for a heavy flow ten to forty tiles long, where the roads would need five carts and the fleet is not there yet; fitted to a drive it carries ninety. The **railway** is the trunk: eight wagons carry thirty-five stones a minute over twenty-four tiles and twenty-four over forty-eight, no hands, a beam and two nails a tile and a horse to a wagon; it passes eight carts from about twenty tiles. (A line is limited by its bays, which hold one wagon at a time: twelve wagons carry no more than eight, and a second bay at each work, or a second railway, is the way to more. A wagon takes eight units whatever they weigh, so the rail is for heavy goods: eight stones are 800 kilograms, eight loaves 160, where a cart takes fifty.) The **ropeway** carries the most (twenty-four buckets: a hundred and thirty a minute over six tiles, forty-eight over twenty-four) over any ground in a straight line, for a pole and a rope a tile. The **barge** is the river's, and only the river's.
- **Trades and tools.** Each building names a trade and a level; one qualified person oversees four posts and the rest learn beside them (`Command::Teach` for coin). Tools wear out and go back to a smith to be re-steeled; a site without a tool or a smith stops.
- **Castle sites.** `FoundSite` stakes a baked `angel-castle/v1` plan; `site_allowance`, `site_set` and `work_sites` are the game's three calls, and `clerk` says in one sentence what is holding the build up.
- **The Clerk reads out causes.** `Kingdom::clerk` returns the sentence and the `causes` behind it (a maker working flat out, a good nothing makes, a crew missing, the carts); `causes_for(good)` and `walk_for(good, consumer)` answer the same for any good, so a player or a script can build another of what is the limit.
- **Checks.** `design::check` proves every good has a maker and a use, every building can be raised from founding stock, and every line of every castle's bill has a chain; `design::solve` counts the machines, people, carts and carter's yards (loads by weight over a typical haul) and shaft for N stones a minute. `cargo run -p angel-kingdom --example economy_tree` prints both as JSON.
- **The Deep Stirs.** `Kingdom::pressure` computes smoke, noise and digging against the calm of castles that stand (`pressure_with` adds what the party's cutting below has stirred and, given a tile, only the calm that reaches it: a castle calms `calm_radius(laid)` tiles round, 16 plus a tile for every 150 stones laid, to 90).
- **The Deep comes up** (`kingdom::defence`, `rules::deep_raid`). The realm's day is 600 sim seconds and its night the last 200. At dusk, with a knight on the hillside, the Deep is judged once: the stir at the nearest works to the shaft, less the calm there. From *uneasy* it sends a raid, one a night, bounded: two raiders at *uneasy*, four at *stirring*, six at *raiding*, never more than eight, **the first raid always two**, one more every third raid. The raids teach in turn: imps with torches (fire), thieves, sappers, warboars, round again. The raiders are ordinary Delve foes; they climb the Great Shaft three seconds apart, march a field of cheapest ways to the nearest smoke (a working smoky works or a lit brazier; boars go for the Keep's wall), fight any knight within six tiles like any foe, and go back down at dawn or after 170 seconds. A party beaten on the hillside is knocked down, not killed: the raiders take their due from the camp (a few goods, never the last of the repair goods) and go.
  - *What they do.* Imps set a works alight (`ignite`: it does no work, its stores burn, and it burns down in 40 seconds unless a well within ten tiles or a knight's hands take the heat out; burnt down, a plan of it is laid urgent); thieves rob a store of four goods and run for the shaft (felled, they drop it); sappers cut a road, a tile of rollerway or of rail (`cut_near`: the goods on it are lost, a plan of exactly the piece is laid urgent); boars wreck a cart (never a carter's yard's last) and batter the Keep's lift in hand. Anything of theirs in the way is battered. The camp, the Great Shaft's lift and a castle's yard are never struck.
  - *What stands against them.* New plans: the **palisade** (timber, 40 hold), the **timber gate** (a way through a wall for carts and knights; the raiders' field takes a gap when it is cheaper than breaking), the **watchtower** (150 hold, an archer's post that asks the roads and rollerways for a quiver of 24 **arrows**, looses one at the nearest raider within 14 tiles every two seconds for 14 damage), the **brazier** (burns a timber every two minutes; lit, it draws the raiders and adds 2 to the stir, so a brazier set out before the tower is a lure). The existing castle wall (120) and brick wall (100) hold too. Arrows are fletched at the carpenter's shop (a plank and two nails make twelve) and the first tower's quiver comes from the founding stock (24). A raider's field costs a floor tile one step and a defence what it has left, so a short wall is walked round and a long one broken through.
  - *Repair is real work.* Everything lost is laid out as a plan, urgent and the Deep's own (`by` 0), for the camp's hands: a destroyed or burnt works as itself, a cut road as a road, a cut roller as the same piece facing the same way (`mend-roller:<piece>`), a cut rail as rail (`mend-rail`). What stands but is battered is patched by the hired hands (4 hold every three seconds) once the blows have stopped for thirty seconds, and by a knight's hands at a price: the builder's **MEND** tool (last in the Q order) beats at a fire (three presses put one out), sets a cut line or a burnt works to rights at once at the real price, or mends twenty hold of a battered defence for its share of the cost.
  - *The siege.* A boar that reaches the Keep batters the stones of the lift in hand that it can reach (up to 4.8 m, three tiles). A stone parts after two blows, a true one (`Site.trues`) after five; what rested on it comes down with it by Blocks' `rests` (the one-pass majority rule, `world::works_siege`): the Keep's baked castle is from a v1 export and has no rows, so they are derived from its stones by Blocks' own rule (on the first 2,500 stones of a Blocks great tower the derived rows are identical to Blocks' own for 84 percent of stones, agree on the ground for all of them, and a lost stone brings down the same stones in 28 of 29 trials). The lost stones are `Site.lost`: the wall's height does not change, there are holes, a tile closed by a lost stone is open (a breach), and the masons and the knights lay the lost stones again before any new one. A first raid brings down at most 6 stones, any raid at most 24, and never more than 24 stones are out at once.
  - *Walls and towers, measured* (`deep_raid` tests). Three imps break a stone wall of 120 hold twenty-two seconds after their first blow; the same ring under two watchtowers with full quivers loses 2 hold and the raid is broken. The raiders walk a field of cheapest ways: a floor tile costs a step, a defence the hold it has left twice over, so a palisade is broken through when going round is about eight tiles longer, a stone wall when it is about twenty-four, and a short wall is walked round; a gap or a timber gate in a wall is the way in.
  - *Calm.* A castle that stands calms the land round it, and a raid is judged at the works it comes for: the same stir, less the castle's calm there. A works inside the castle's calm is not among the raid's targets; beyond it, it is.
  - *The night.* The top left of the view says how the day stands and, while a raid is on, what it is and how many stand.

## Latest source checkpoint — 2026-11-05

The village and power source passes 67 pure kingdom tests, 56 targeted game/input/encounter checks and the normal-input 519-second village power scenario. Verified screenshots are in [the village capture index](images/kingdom-works/village/README.md). The production native app and installed worker have not yet been rebuilt for this latest source. The preview browser bundle compiled, but the local preview host requires a rebuild with `angel-delve/bend-engine` before it can launch. Native mouse dispatch and frontier hero stride/facing remain pending. See [the paused model-switch handoff](VILLAGE-PRODUCTION.md#model-switch-handoff--2026-11-05-2154-utc) for exact restart commands and remaining work.
