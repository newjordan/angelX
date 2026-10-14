# The Undernetwork — persistent frontier proposal

**Status: direction approved by the operator. The full Undernetwork is not implemented,
balanced, scheduled, or release-qualified. Native encounter candidates are reviewed
separately; this atlas is not a claim of 100 shipped strata.**

[Architecture index](README.md) · [Current Delve](../DELVE.md) ·
[Connected settlement](connected-settlement.md) · [Stables / practice tournament](stables-tournament.md)

## Intent and council provenance

Operator direction: a Dwarf Fortress-like underground that spreads sideways through
unexcavated rock and develops downward without a final hell layer; an initial 100
strata, eventually a community frontier potentially far beyond them. Mercenary
guilds, friendly companions, mineral economies, boss pockets, persistent building,
and Hades/MegaMan-inspired temporary powers should make repeated expeditions matter.
The suggested depth 1,250,000 is a long-term horizon, **not a player-count estimate**.

Grok, DeepSeek V4 Pro and Muse Spark returned design perspectives and short
follow-ups. The requested **Sol 6.1** route was not available in this cockpit;
no response or endorsement is attributed to it. This document is the driver's
synthesis, not unanimous council approval. Model advice is not verification.

- **Grok:** geological bands and lateral discontinuities, local boss/social
  pockets, logistics instead of a succession of sealed arenas.
- **DeepSeek:** sparse deterministic chunks plus persistent edits, authoritative
  shared transactions, explicit resource sinks, no token-spend-to-power conversion.
- **Muse:** enjoyable short expeditions, companion relationships, small-crew
  viability, encounter-based gem wear and humane retreat.
- **Driver decisions:** no declining interestingness with distance from spawn;
  no obligatory large-guild membership; no unearned rewards for generating map
  coordinates; no live-world regeneration that silently moves existing geology;
  no assumption that hashes or unspecified zero-knowledge proofs certify useful work.

Existing game code has a six-floor route, including a dragon milestone at three,
plus home upgrades, rescues, bosses, a hireling, spells and ultimates. Much of that
is poorly surfaced at entry. The Undernetwork must make a stronger connected loop,
not merely advertise those systems as an already persistent infinite world.

## The central loop

```text
HALL / GUILD
  choose objective + crew + finite provisions + temporary gem loadout
       |
       v
SUPPORTED SHAFT / FRONTIER DEPOT
  use an established route, or carry supplies to establish the next foothold
       |
       v
PROSPECT -> EXCAVATE -> EXPLORE
  rock / veins / caverns / creatures / rivals / ruins / optional boss pockets
       |                     |
       |                     +--> fight / negotiate / rescue / bypass
       v
EXTRACT / RETREAT
  bring material, discoveries, survivors and spent gems home
       |
       v
REFINE / RECRUIT / REST / BUILD
  personal kit <-> guild facilities <-> shared route infrastructure
       |
       +--> inhabited settlements, easier returns, new methods of access
       +------------------------------------------------------------> repeat
```

A deeper frontier is difficult to **establish**, not forbidden to small groups
once supported. A small crew should own useful projects every session. The first
100 are a civilization-scale development arc, not 100 rooms two players clear in
an evening. Do not hard-code that two determined players can never reach 100.
Skill, knowledge, alliance and eventually self-built logistics should all count.

## What a layer means

A stratum is an addressable vertical slice of a connected world, not a generated
room that vanishes on the next stair. It contains solid rock, veins, open cavities,
water/gas zones, settlements and shafts. Excavation creates traversable space.
A shaft has an actual horizontal position and a destination beneath it; passing
between strata does not silently reroll the whole map.

Every stratum extends through procedural regions and chunks laterally. There is
no ordinary game rule saying “you reached the level's last room.” The implementation
still needs finite coordinate ranges, disk quotas, active-chunk budgets and
server capacity limits, visibly distinguished from in-world boundaries.

The layer profile is a **distribution**, not a uniform paint job. Faults, intrusions,
aquifers, old excavations and migration corridors cross neighboring strata.
An exceptional pocket can introduce an adjacent biome without erasing the layer's
identity. Prospecting signals should let players choose rather than repeatedly
roll empty rooms. Good discoveries need extraction/access work; infinite address
space must not mean instantly collectable infinite ore.

## Boss ecology and political networks — approved direction

**Bosses are inhabitants with material support and relationships, not a fixed
quota per stratum.** A layer can have many competing chiefs, one remote hive mind,
an old predator outside any society, or no dominant leader. The atlas below names
possible pockets, not an ordered kill list. Different regions within the same
stratum can support different political structures.

```text
geology + food/energy + access routes
                 |
                 v
      resource sites + carrying capacity
                 |
                 v
   populations + organizations + territories
                 |
                 v
 leaders / rivals / dependents / protected threats
                 |
     player and bounded world events
                 |
                 v
 control loss / succession / supply cuts / encounter access
                 |
                 +--> changed support and future encounters
```

Examples for the future persistent world:

| Organization | Population and leaders | Nonlinear encounters and consequences |
|---|---|---|
| Brood hive | Many small creatures supported by brood beds; very few command organisms | Outer broods can be avoided or disrupted independently. A buried mind may remain inaccessible until its protective organism falls. Cutting food changes future brood support, not the boss's health alone. |
| Orc mining gangs | Several chiefs control separate seams, smelters or haul roads | Arrive between Iron Tusk and Red Pick claims. Explore either side; defeating a chief may strengthen a surviving rival, not automatically pacify the district. No universal "king of the floor." |
| Tribute stronghold | Fewer elite troops; supply houses provision a protected ruler | Some independent houses are true prerequisites for the sealed court; a free captain need not be killed. Supply cuts weaken future escorts. |
| Migratory predator | A rare individual follows prey, not a faction's throne rules | Its arrival depends on supported prey and access; it need not block descent or answer to any chief. |

A particular protected encounter can require **ALL** specified defeats, **ANY**
of several routes, or an explicitly authored world condition. Do not turn every
society into two wardens followed by an apex. Dependencies must have reachable
solutions, readable clues and no cycles. Finding a sealed site early leaves a
retreat route; its unavailable boss does not spawn, take damage, grant rewards or
accidentally count as defeated. Ordinary kills, visits and loot possession are
not proof that a named prerequisite died.

Political aftermath must be causal and bounded. A defeated chief loses control;
survivors may inherit a limited share of unused reserves. Already committed
fighters do not magically disappear. Persistent succession in the full world is
a separate named event with costs and history, not the same defeated boss and
loot silently respawning. Summons and descendants consume finite provisioned
allowances; an instantaneous enemy cap alone does not stop farming over time.
Offline time must not generate unlimited armies or ruin an unattended guild.

**First native slice versus destination:** the current encounter work uses the
existing six-floor layouts and measured terrain as a coarse support proxy. It
implements different leader distributions, finite camp provisioning, protected
encounters and leader-loss aftermath. It does not yet implement mineable resource
stocks, trade, diplomacy, NPC-on-NPC gang fighting, cross-stratum factions, or
persistent shared-world succession. Those must be built before claiming the
player has arrived in a fully simulated gang war. Full strata should mix regional
organizations rather than permanently assigning one society to each biome.

## First 100 strata: initial content atlas

These are authoring briefs, not promised unique maps. Each stratum combines its
row's geological/economic emphasis with seeded terrain and several pocket templates.
The named encounter is an **example regional pocket**, not a mandatory floor boss.
Independent sites can contain variants; killed individuals do not respawn because
another player requests the same chunk. Trade, rescue and bypass are alternatives.

Every band introduces one main challenge, one economic specialization and one
new reason to improve a settlement. Earlier resources retain uses at later depths.

### 1–10: The Rootbound March — learn to make a home

| Depth / identity | Terrain and useful material | Example pocket / decision |
|---|---|---|
| 1 · Lantern Footings | Soft limestone, rubble, clay | Rescue a trapped surveyor; choose a bunk or first storage bay. |
| 2 · Copper Burrows | Root-laced copper lenses | Burrowers defend an ore face; distract, fight or reroute the cut. |
| 3 · The Old Workings | Timber supports, abandoned tools | Recover a miner's kit without taking the last roof support. |
| 4 · Rootwell | Fresh water and damp clay | Divert a seep to a cistern or preserve a glowcap nursery. |
| 5 · Glowcap Hollows | Edible fungi, fiber | Befriend a forager camp; harvest without stripping its food. |
| 6 · Tin Steps | Tin pockets, narrow ledges | Build a little bridge to turn awkward ore into a reliable haul. |
| 7 · Broken Foundry | Salvage iron and firebrick | Repair or disassemble a dormant smelter guarded by slag vermin. |
| 8 · Chalk Warrens | Chalk, lime, small geodes | A deceptive vein crosses a beast den; take a scout or safer route. |
| 9 · The First Cistern | Water channels, pump scrap | Reopen a flooded shortcut; test bulkheads before pumping. |
| 10 · Molecourt | Mixed seams and roomy caverns | A Matriarch's nesting pocket or burrowfolk parley; establish a depot. |

**Building payoff:** bunks, stores, modest forge, training yard and a supported
lift. The first session should deliver a rescue, a useful power and a visible
home improvement—not a long compulsory mining tutorial.

### 11–20: The Ironfold — turn finds into industry

| Depth / identity | Terrain and useful material | Example pocket / decision |
|---|---|---|
| 11 · Ochre Galleries | Ochre, hematite bands | Pigment traders offer maps for safe caravan passage. |
| 12 · Rust Warren | Iron nodules, corroded scrap | Rust-mites prefer exposed stockpiles; cover or bait them. |
| 13 · Quartz Rails | Quartz veins, hard chert | A forgotten rail spur shortens a difficult pack route. |
| 14 · Furnace Pockets | Fireclay, heat-retaining stone | Claim a warm workshop site without waking its furnace keeper. |
| 15 · Splinter Chert | Silica, brittle edges | Cutting method matters: preserve gem fragments or blast for speed. |
| 16 · Foremen's Camp | Worked seams, abandoned records | Hire skilled but distrustful laborers; fulfill a supply contract. |
| 17 · Bellows Rift | Air passages, iron-rich walls | Reconnect airflow before bringing an extraction crew. |
| 18 · Redwater | Iron-laden water, settlement sites | Choose filtration and a trade post instead of draining everything. |
| 19 · Armory Grave | Hardened metal, ruined defenses | A sentry convoy guards useful designs and multiple exits. |
| 20 · Hammerdeep | Dense iron and spacious vaults | An Ore-Wight nests on an industrial hub; defeat, placate or bypass. |

**Building payoff:** smithing specialization, repair station, ore carts and
better guild equipment. Iron remains necessary for supports far below this band.

### 21–30: The Karst Choir — learn structure and cooperation

| Depth / identity | Terrain and useful material | Example pocket / decision |
|---|---|---|
| 21 · White Steps | Marble terraces | Carve access without dropping stone onto a downstream camp. |
| 22 · Hollow Pillars | Calcite and load-bearing columns | A rich seam tempts players to remove a critical brace. |
| 23 · Hanging Gardens | Cave fiber, herbal growth | Build scaffolds; gardeners exchange medicine for protection. |
| 24 · Echo Reservoir | Fresh water, resonant crystal | Sound reveals passages and attracts blind hunters. |
| 25 · Bell Market | Dry gallery junction | A neutral market supports contracts rather than a required battle. |
| 26 · Blindwater | Fish, silt, submerged shelves | Dive for a cache or commission a pump route. |
| 27 · Saintless Crypt | Worked marble, old mechanisms | Recover a seal from a haunted side ruin without disturbing graves. |
| 28 · Broken Cathedral | Vaults, spanning arches | Shore an unstable chamber to create a large workshop plot. |
| 29 · Lantern Swarm | Luminescent insects, resin | Preserve a light habitat or harvest portable lantern material. |
| 30 · Choir of Stone | Singing formations, broad caverns | A resonant guardian challenges noisy crews; silence is a strategy. |

**Building payoff:** bridges, masonry, medical rooms and defensible communal spaces.

### 31–40: The Brine Principalities — trade and fluid boundaries

| Depth / identity | Terrain and useful material | Example pocket / decision |
|---|---|---|
| 31 · Salt Stair | Salt beds | Preservation supplies make longer expeditions economical. |
| 32 · Bitter Wells | Sulfur and brackish water | Seal a toxic seep or distill it in a controlled works. |
| 33 · Glasspan | Clear salt and brittle crust | Reflective terrain hides shallow cavities and scavengers. |
| 34 · White Rust | Corrosive runoff, salvage | Shelter metal transport; improvise protected carts. |
| 35 · Brine Quay | Salt docks, floating stores | An underground ferry guild sells passage, not permanent ownership. |
| 36 · Preserved Streets | Encased architecture | Cut into a salt-buried neighborhood containing survivors and threats. |
| 37 · Needle Beds | Mineral spires | A ranged ambush pocket rewards careful cover removal. |
| 38 · Sunken Tollhouse | Pump metal, brine channels | Negotiate a toll, clear an alternate path or restore the crossing. |
| 39 · Pale Nursery | Medicinal salts, hatch pools | A creature nursery complicates an apparently easy salt operation. |
| 40 · The Brine Court | Pressure basins, precious salts | A regional baron's court offers diplomacy or an optional siege. |

**Building payoff:** preservation, corrosion-resistant fittings, ferries and
bulkheads. Good transport can beat simply buying stronger weapons.

### 41–50: The Ember Measures — air, fuel and controlled risk

| Depth / identity | Terrain and useful material | Example pocket / decision |
|---|---|---|
| 41 · Coal Lace | Thin coal seams | Choose narrow selective mining or broad dusty cuts. |
| 42 · Soot Gardens | Fuel, fertile soot pockets | Protect growers while bringing useful fuel to upper settlements. |
| 43 · Firedamp Hollows | Gas pockets and clay | Detect and vent before lighting a tunnel. |
| 44 · Kiln Rows | Firebrick and ceramic molds | Repair a ceramic workshop with a hostile resident kiln spirit. |
| 45 · Lantern Union | Dense mining camp | Improve crew safety to obtain guides and shared ventilation plans. |
| 46 · Ash Aquifer | Warm water, mineral ash | Pumping here changes both temperature and gas pathways. |
| 47 · Cinder Orchard | Resin, charwood | Harvest living fuel without creating a spreading fire pocket. |
| 48 · The Hot Seam | Rich coal near thermal fissures | Establish a safe work shift rather than send bots indefinitely. |
| 49 · Bellows City | Old ducts, machine housings | Reconnect a local air network to awaken workshops and old defenses. |
| 50 · Furnace Heart | Fuel nexus, refractory rock | A furnace-beast pocket tests vents, cover and coordinated extraction. |

**Building payoff:** ventilation rooms, ceramic gear and dependable fuel logistics.
No routine chore should destroy a settlement merely because its owners logged off.

### 51–60: The Verdant Pressure — cultivate instead of only conquer

| Depth / identity | Terrain and useful material | Example pocket / decision |
|---|---|---|
| 51 · Jade Roots | Jade, greenstone | Rooted stone resists ordinary excavation but opens to another tool. |
| 52 · Spore Curtain | Filter fiber, mushroom groves | Set a clean-air approach through a bloom. |
| 53 · Nickel Knots | Nickel seams in tough rock | Metal-eating organisms compete with excavation crews. |
| 54 · Moldwater | Medicinal culture pools | Create a contained clinic supply rather than sterilize the chamber. |
| 55 · Mycelial Commons | Interconnected fungal settlement | Negotiate access and propagation limits with resident keepers. |
| 56 · Chrome Fold | Chromium and folded faults | Prospecting distinguishes useful veins from hazardous dust. |
| 57 · Sleeping Orchard | Slow-growing organic materials | A dormant giant shelters a valuable renewable habitat. |
| 58 · Wandering Groves | Migration corridors | Leave a wildlife passage or spend on a protected reroute. |
| 59 · Green Lung | Air-purifying ecology | A living settlement utility needs habitat space, not a fuel meter. |
| 60 · The Root Parliament | Broad living wall and chambers | A territorial organism has several negotiable fronts, not one HP gate. |

**Building payoff:** filters, greenhouses, companion medicine and habitat stewardship.

### 61–70: The Prism Reach — strange powers with clear costs

| Depth / identity | Terrain and useful material | Example pocket / decision |
|---|---|---|
| 61 · Granite Anchorage | Granite, heat channels | Found a cooled waystation before reaching exposed crystal chambers. |
| 62 · Pegmatite Fans | Large crystal pockets | Extraction technique affects socket-grade yield. |
| 63 · Tinlight | Tin, luminous inclusions | Follow readable vein signals through a light-sensitive predator zone. |
| 64 · Mirror Grotto | Reflective crystal | Beam paths change with mined cover and placed shields. |
| 65 · The Lapidary Lodge | Cut gems, trade tools | Learn a gem-family pattern through service or a local trial. |
| 66 · Singing Lenses | Resonant gem clusters | A coordinated tuning encounter can replace brute-force destruction. |
| 67 · Cold Furnace | Thermal exchange minerals | Heat and cold pockets create useful processing sites. |
| 68 · Split Spectrum | Mixed gem families | Choose a specialized power or versatile low-charge combination. |
| 69 · Wyrm Galleries | Crystal-lined tubes | Track a regional wyrm; raid its cache, drive it off or clear a route. |
| 70 · The Prism Engine | Ancient sockets and conduit rock | An optional engine guardian yields a new play pattern, not endless stats. |

**Building payoff:** gem cutting, bounded attunement choices and heat exchange.
Gem families should start appearing much earlier; this band deepens them rather
than withholding the fun until stratum 61.

### 71–80: Blackwater Roads — make distance traversable

| Depth / identity | Terrain and useful material | Example pocket / decision |
|---|---|---|
| 71 · Basalt Docks | Basalt, copper | A sheltered basin invites an expedition harbor. |
| 72 · Pressure Wells | Confined water, dense ore | Vent pressure safely before opening a haul route. |
| 73 · Eel Lanterns | Bioluminescent habitat | Hunters circle lamps; use decoys rather than switch off every light. |
| 74 · Copper Arteries | Long copper seams | A vein offers both wealth and a route through dangerous country. |
| 75 · Pumpwright Crossing | Pump workshop, disputed water rights | Assist one of several engineering camps without permanent faction lockout. |
| 76 · The Drowned Archive | Records, mechanisms, sealed books | Recover old route knowledge under limited expedition air. |
| 77 · Hollow Keels | Ruined transports, salvage | Restore a carrier or strip it for infrastructure materials. |
| 78 · Undertow Rift | Fast currents, deep ledges | Stage a rescue with ropes, platforms or an alternate dry approach. |
| 79 · Nine Locks | Connected basins and bulkheads | Open a sequence locally; never allow distant grief flooding by default. |
| 80 · Blackwater Leviathan | Large navigable reservoir | A migrating creature creates convoy, hunting and avoidance objectives. |

**Building payoff:** bulkhead control, waterways, rescue facilities and protected depots.

### 81–90: The Fault Kingdoms — competing claims and moving ground

| Depth / identity | Terrain and useful material | Example pocket / decision |
|---|---|---|
| 81 · Gold Stitch | Gold-filled fractures | Profitable veins require careful supports, not a universal power tier. |
| 82 · Silver Shatter | Silver, fragmented rock | Salvage a collapsed team route and recover stranded equipment. |
| 83 · The Listening Shelf | Tremor-sensitive formations | Surveyors predict a local event with enough time to act. |
| 84 · Migrant Road | Mixed geology, traveling camps | Escort a community carrying rare crafting knowledge. |
| 85 · Claim Court | Neutral junction, disputed workings | Resolve borders with contracts; do not force player-versus-player combat. |
| 86 · Counterweight Works | Heavy machinery, reinforcing alloys | Repair a lift whose route connects several valuable regions. |
| 87 · The Tilted City | Displaced rooms, salvage | Stabilize sections selectively and choose what to preserve. |
| 88 · Glass Fault | Quenched volcanic glass | A fast, brittle battlefield makes cover and movement matter. |
| 89 · Wardens' Span | Defensive works, toll stations | Earn passage, defeat hostile wardens or construct a demanding bypass. |
| 90 · The Walking Fault | Active local fracture zone | A mobile colossus encounter changes routes without wiping distant towns. |

**Building payoff:** shock bracing, route redundancy, surveying and regional coordination.

### 91–100: The Unwritten Deeps — become settlers, not finishers

| Depth / identity | Terrain and useful material | Example pocket / decision |
|---|---|---|
| 91 · Old Metamorph | Mixed recrystallized seams | Prior mining assumptions need fresh survey rather than stronger numbers. |
| 92 · Pale Current | Cool channels crossing hot rock | Build thermal refuges along a long, exposed route. |
| 93 · Fossil Machines | Ancient housings, unusual alloys | Restore a single function without waking every defense. |
| 94 · The Quiet Sea | Broad dry cavern or local sea | Establish a frontier harbor, observatory or caravan base. |
| 95 · Concordance | Contact between several ecologies | A rare neutral settlement offers knowledge from different routes. |
| 96 · Voidstone Fold | Unusual load-bearing material | Test a new support method under controlled local conditions. |
| 97 · The Lost Company | Abandoned deep camp | Rescue remnants or learn why its expedition failed. |
| 98 · Unlit Roads | Old routes beyond current maps | Build navigation landmarks; darkness is not an arbitrary damage multiplier. |
| 99 · Threshold Engine | Deep transport infrastructure | Repair a staging hub through multiple material and social routes. |
| 100 · The Unfinished City | Habitable caverns and unsurveyed shafts | Found a deep settlement and discover access to 101—not a final boss screen. |

**Building payoff:** a durable community foothold and new directions of discovery.
The first-100 atlas is a content plan to prototype and revise, not 100 launch promises.

## Beyond 100: endless frontier, finite content grammar

Rotate geological processes, ecology combinations, ruins, societies and tactical
conditions—not a repeating ten-floor skin and not a linear temperature ramp.
There is no obligation to approach Earth's mantle accurately at imaginary millionth
depths. The fiction can involve folded strata and ancient underworld transport.

Use a stable authored grammar plus unusual intersections: a cold brine river
through a fungal ruin, a peaceful camp inside a dormant guardian's territory,
or a crystal fault breaking into an old industrial zone. Template identity alone
must not guarantee the same solution. Terrain, access, faction state and ecology
supply the variation. New grammar/content can expand the frontier over time.

Do not promise that combinatorics generates a million genuinely novel experiences.
Measure repetition, abandoned expeditions and player-chosen objectives. Add content
because play needs it, rather than increase numeric depth for a leaderboard.

Reaching 1,250,000 in ten years averages roughly **342 new strata a day** along the
deepest route. That cannot simultaneously mean every stratum is a long bespoke
boss campaign. This is a horizon metaphor unless transport later opens major
new regions or the meaning of depth becomes finer-grained. Keep inhabited depth,
surveyed depth and merely visited depth distinct; a naked rush is not settlement.

## Economy: sources, transformations and sinks

| Source / earned activity | Transformation / choice | Sink / payoff |
|---|---|---|
| Finite local mineral deposits | Refine ore, cut stone, forge parts | Tools, supports, stations, route repair and equipment sidegrades |
| Managed renewable habitats | Grow food/fiber, prepare medicine | Expedition provisions and bounded crew recovery |
| Risky caches / real encounters | Identify gems, salvage equipment | Temporary power charges, socket choices, worn-gem reclamation |
| Rescues / negotiated contracts | Recruit people, earn local trust | New crew roles, maps, room functions and trade access |
| Boss pocket victories / alternatives | Learn a pattern or recover a unique component | New tactics, facilities or route options—not universal mandatory keys |
| Player-funded work orders | Move existing materials between players | Paid building, escort or prospecting services; no duplicated money creation |

Start with a small resource vocabulary: structural material, metal, provisions,
and a few depth-specific reagents. Distinct named minerals can map to these useful
categories until specialized recipes justify extra inventory. Avoid dozens of
currencies and compulsory daily upkeep. Construction and expeditions should be
the dominant sinks; maintenance is local, predictable and repairable.

Depth difficulty combines local hazards, enemies with counterplay, extraction
risk, and distance from **working support infrastructure**. Don't compute damage,
HP or costs as an exponential function of an unbounded depth integer. Use bounded
regional encounter tiers and additional adaptation requirements. A depot is useful
but does not magically erase every downstream logistics cost.

Upper layers stay relevant as farms, workshops, recruitment centers, trade routes
and safe projects for new players. Deep materials add capabilities rather than
make all shallow goods and labor worthless. Rich lateral deposits are finite per
site, but no one guild should monopolize every viable route. Alternate sources,
substitutions and geographically independent starts are necessary.

## The mercenary guild: friends and institutions

Begin with a small active squad and a larger persistent roster. Roles should change
how an expedition plays: a **Scout** detects nearby signals, a **Warden** holds a
choke, a **Mender** stabilizes a downed ally, a **Porter** helps extraction, and an
**Engineer** builds temporary supports. Their competencies come from rescued,
trained or contracted individuals, not arbitrary paid model tiers.

Give companions names, role growth, preferences, a few remembered milestones and
clear assignments. A saved rescue, first boss victory or brave retreat can become
a guild-hall memento. Ordinary AI uses bounded local behavior trees/state machines;
an optional language model can supply flavor, never expensive per-frame decisions
or authoritative rewards. Do not promise personality depth from generated chatter alone.

Guild construction provides barracks, infirmary, equipment room, mess hall,
training yard and dispatch board. Not every room is a combat-stat increase.
Recruiting, resting, equipping and assigning the crew supplies the Domina-like
management half; adventuring with them makes them more than production counters.

Offline work, if implemented, is a **pre-funded finite order** in previously
secured territory. It consumes already-owned supplies and has capped output and
duration. No offline frontier clearing, boss victories, rare-gem farming or
unbounded compounded production. Orders can finish while absent; simply taking
time off cannot kill recruits or destroy the home. A solo-friendly rescue bot
and meaningful parallel building jobs matter more than compulsory raid sizes.

## Powers: temporary gems, permanent learning

Combine three ideas without copying their economies wholesale:

- **Hades:** choose a small set of expedition modifiers that form a build.
- **MegaMan:** beating a distinctive encounter teaches a new tactical pattern,
  useful elsewhere—not simply +20% damage.
- **Skill gems:** socket that pattern into a limited slot, with clearly displayed
  charges or encounter durability. Spent gems remain salvage or keepsakes.

Prefer exact **casts/charges** for activated powers and explicit **encounters
remaining** for passive gems. Pausing to code, logging out, using a menu, or waiting
for a model does not wear a gem. Training dummies and trivial infinitely spawned
enemies must not create free refills or farmable progression. One authoritative
encounter-complete event handles passive wear once.

Example: a three-charge Faultstep lets you dash through a thin fractured barrier;
a two-encounter Echo Ward helps protect a retreat. The known blueprint can remain
available after the expedition, but another charged copy costs materials or a
new encounter reward. Permanent equipment still provides a viable baseline when
a gem expires. Telegraph the last charge before commitment, never surprise-delete
an entire build during a paused session.

## Death, retreat, and player-owned places

Retreat is a valid strategic success, not a failed verification. Let crews bank
finds at supported depots. A wipe can risk a clearly identified portion of unbanked
haul, consume preparation and create a rescue/recovery opportunity. Do not delete
long-term buildings or ordinary recruited companions for a first mistake.
Permanent companion death, if ever offered, belongs in explicit optional rules.

Owner plots, public routes and wilderness need different permissions. Hosts can
start as invite-only. Shared treasuries need role limits and approval for large
spends; public shafts need protection against trivial blocking or remote flooding.
Allow abandonment/reclamation rules with notice, not silent seizure. Guild conflict
can be an opt-in mode, not the price of enjoying a settlement with two friends.

## Angel work and tokens: no spend-to-power

**Do not convert provider tokens, dollars, tool calls, loop iterations or local GPU
hours into ore, damage, access, rare drops or governance votes.** All are farmable,
provider-dependent or directly pay-to-win. Adding a daily cap does not fix that
incentive. Artificially burning tokens must never become an optimal game action.

The useful connection is **authorship and world memory**: a real report can become
a source-linked library exhibit; a reviewed community contribution can receive a
plaque or attribution. Research can inspire a blueprint or public event after an
explicit content-review process, not automatically certify economic value.

The existing private loop settlement is a local visual progression system. If it
connects to a future shared economy, keep its loop-derived construction/nontradeable
memorabilia separate from shared inventory. No automatic conversion of local work
receipts or offline modified saves into public ore or construction credit.

Equal rules apply to local models, paid models and manual work. Share only explicitly
approved artifact content/metadata; raw prompts, spend logs and private reports
stay private. A digest establishes content identity, not truth or useful-work proof.
A cryptographic proof scheme is not a substitute for specifying what is proved.

## Runtime and repository schematic (proposed, not existing modules)

Keep the new simulation outside giant App/control and renderer files. These are
responsibility boundaries, not approval to create every subsystem now:

```text
world definition (seed + immutable grammar versions + stratum profiles)
    -> generation (coordinate-keyed geology, pockets and compatible borders)
    -> chunks (base generation + persistent edit overlays)
    -> simulation (excavation, encounters, ecology, crews)
    -> transactions (inventory/build/claim/reward state changes)
    -> local or shared authority + saves/event journal
    -> read-only projection
         -> existing Delve renderer / UI
         -> mini-world and settlement display

input/commands -> intents -> validation -> atomic transaction -> persisted result
                                          -> same result to all projections
```

Host-local authority first. A later shared server owns terrain edits, inventory,
claims and encounter rewards; clients submit intentions, not trusted result totals.
Deterministic rendering and report text cannot award resources. Local modded/offline
worlds remain valid fun, but their economies do not automatically merge publicly.

Coordinate-keyed generation must be independent of discovery order. Shared border
samples and region-owned feature placement keep adjacent chunks and vertical shafts
consistent. Freeze generated region versions; upgrades must not silently reroll
inhabited areas. Introduce new rules in new regions or explicit reviewed migrations.

Store only discovered/edited state and important entities; cache a bounded active
neighborhood. Quiescent distant areas use coarse bounded rules, not per-frame
creature simulation. Store optional design/content tables separately from native
simulation, with validation and versioning; don't promise hotloading from merely
moving files. Tests belong next to the existing subsystem test maps, with source
anchors and small runtime adapters.

### Minimum authority invariants

1. Mining transfers a deposit's remaining material exactly once; competing digs
   cannot both collect it. Inventory and terrain update atomically.
2. Boss rewards, encounter wear and work receipts have authoritative unique IDs;
   retries, reconnects and replay cannot double-apply them.
3. Generation must not be a free reward faucet. The client cannot choose a fresh
   seed and import its finds into a different shared world.
4. Build orders reserve/spend existing resources; cancellation has an explicit
   refund rule. Interrupted commits cannot create free buildings or lose both
   material and output.
5. Bounds apply to coordinates, quantities, pathfinding, generation effort,
   active entities and storage. Capacity exhaustion fails clearly without
   corrupting a world or pretending mathematical infinity.
6. A new depth needs a playable extraction/retreat path, reachable spawn, readable
   hazards and valid connections. Generated novelty must pass these checks.

## First buildable slice and acceptance

Do **not** implement a million-floor generator before the 20-minute loop is fun.
Preserve classic Delve and prototype an explicit expedition mode:

1. Three persistent connected test strata with lateral chunk generation, mineable
   rock, one ore family, one ecological hazard and deterministic revisit behavior.
2. One hall/guild board, two complementary companion roles, one material-funded
   building with an immediately visible benefit, and a supported return route.
3. Two optional boss-pocket families and one noncombat alternative; more than one
   horizontal objective, with no compulsory full-map clearance.
4. Two temporary gem patterns with explicit wear and an ordinary attack fallback.
5. A visible entry screen: **crew, objective, route, supplies, risk and planned
   upgrade**. A return screen: **what was banked, who needs rest, what this buys**.

Then extend the same slice to strata 1–10. Only after its save safety, costs and
player rhythm work should 11–100 be expanded band by band. Do not turn the old
six-floor arrays into a 100-element array and call it a persistent world.

**Acceptance:** two friends can set a goal, explore/dig, meet a companion or
encounter, bring something home, build a meaningful improvement, and choose a
second expedition within a short session. Reloading preserves excavation and
construction; replaying a transaction does not duplicate loot; pausing never
expires gems; empty inventory, crew defeat, invalid terrain, disk errors and
disconnects leave a recoverable state. Guest views disclose no private research.

**Open decisions for the operator:** invite-only shared world versus separate
player worlds; appetite for terrain excavation controls versus action combat;
which one upgrade makes the very first return satisfying. Economy quantities,
depth pacing and multiplayer governance need play evidence, not confident guesses.
