# The Delve: task list

Written 2026-10-12, after the playtest verdict: **"it feels like E.T."**
Everything below works toward one thing. The Delve has to feel like a single
world you live in, not a pile of features you stumble over.

Working rules (each one comes from the operator):

- Every seated knight plays at once. No volunteers, no spectators.
- Real stakes: gold, horses, marks, the barony. Never score alone.
- Instructions are two or three plain words. No walls of text, no
  slash-separated lists.
- Each knight has their own screen. Never design around a shared camera.
- E interacts, F fires, spells are on 1 to 4.
- Co-op too easy means more pressure. Never lower monster health or damage.
- One fixed number per limit, picked from play. No formulas.
- Finished-grade only: open every render before it counts as done.
- The privy is the model for a good mini-game: short, funny, and tied to
  the rest of the game (the health penalty, the Great Unwashed feat), with
  every stage readable at a glance.

Status marks: `[ ]` open, `[~]` in progress, `[x]` done (with commit id).

---

## 0. Bring everything into one tree

- [x] (c1f8b71f) Merge `delve/integration-20261012` into the night branch. It holds
      17 branches: school-of-magic, helios-ranch, new-bosses, privy-overhaul,
      wish-looks, feel-pressure, foe-patterns, controls-ui, floor-plates,
      kingdom-scroll, exits-21, no-zombies, vote-ballot, grok-polish, and the
      Muse chariot and GLM night snapshots. Most of the ~24 conflicts are the
      same chariot/risen files carried twice; keep the newer of each hunk.
- [x] (39c121cf) Merge `delve/naked-start` (the first two floors teach before they test).
- [x] (d06cf42f) Merge `delve/art-v2-proto` (black space and shared braille art).
      It has an add/add conflict in `cabbage-storm.card`.
- [x] (9d426642) Leftovers from review/fixes: #11 home cue, #24 leech wall, #25 shield
      booms cap, #26 skills in the pit stop, #29 guest leftovers.
      (#30 warnings: done with the line below.)
- [x] (433131be) The full Delve test suite is green (1158 pass), and the Delve
      builds without a warning.

## 1. Feel: the E.T. problem

"E.T." means wandering with no idea what to do, falling into things, and
nothing feeling good. Fix that first.

- [x] (36664df0) **Rate of fire is too high by default.** Bow: 10-tick
      rest and 24 damage (3 shots a second, same 72 a second). Crossbow and
      handgonne were already slow and heavy. feel-pressure set the bow to
      a 5-tick rest (6 shots a second). Pick one lower fixed number for each
      arm by playing it, and keep the faster shot speed.
- [x] (d19a7ef5) **Always a goal.** A gold arrow with two or three words. In
      the kingdom: "The Delve" until the party has been down, then the
      nearest place not yet entered (edge arrow off-screen, bobbing over the
      door on-screen). Below: once a room is clear, the doorway toward "To the
      boss", "Find the key" or "Down the stairs".
- [x] (00346ac9, 84dd8721) **Short paths.** Roads are a trot (twice the walking
      pace), and a coach round of six posts (Keep, Village, Horse Ranch,
      Colosseum, Labyrinth College, the Delve) rides you to the next post on
      one press of E.
- [x] (00346ac9, 5a5ad775) **Signposts.** Every place in view wears a name
      plaque (overlaps skipped) led by a small picture of what happens there.
- [x] (from feel-pressure) **Hits land.** Hit-freeze, knockback, sparks, a burst on every death,
      and a payoff when a room's last foe falls (DELVE-NEXT Part 1).

## 2. Every mini-game, finished-grade

Each game gets a redesign before any polish: all seated knights play at
once, real stakes, two- or three-word instructions, real sprites (no letter
glyphs), and a funny payoff in the privy's spirit. A finished game:

- has a reason to replay;
- pays out into the shared economy;
- shows a render that has been looked at.

- [x] (104acaa4) **Everyone plays at once, in every game.** Stepping on a ring
      starts a heat: each knight in the room gets their own round, sees it on
      their own screen, and a standings board shows the race. The first home
      wins: a gold crown on the winner, the game's booby prize (chamber pot,
      cabbage, rubber duck, trencher, seaweed, dunce cap) on everyone else
      until they win one back.
- [x] (104acaa4) **Three-word boards.** Every game's board is its name and clock,
      how to play in two or three words, progress, and one live cue where a
      game has a moment. Only the game being played shows its board.

- [x] (8e25fc67) **Privy** (the model): checked the animated window; every
      stage is two words and each knight's stop is personal. The way-on
      arrow no longer covers its PARTY BEST board.
- [x] (104acaa4) **Chamber-Pot Put** (Strand): everyone putts at once, and the
      pot lands on the loser's head.
- [x] (104acaa4, 3575a9a7) **Trencher Toss** (Goose Green): every knight throws at
      once (heats); lines shortened. A plate-stealing goose was left out: it
      would be a random hazard, against the fixed-numbers rule.
- [x] (0c3de013) **Cabbage chariot** (Hippodrome pad C): kept as its own game
      (the chariot handoff keeps the two apart on purpose), now plainly
      told apart: "CABBAGE RACE / STEP ON C" beside "HORSE RACE / HOLD E:
      BOARD". Everyone plays the cabbage race at once (heats).
- [x] (d26117d8) **Horse race** (Hippodrome coach): every knight in the room
      races — friends drive the rival teams with their own keys and see the
      race from their own chariot; best place is crowned, the rest wear a
      cabbage. Purse pays per finish; travel has its own plate. Still open:
      wagers on herd horses.
- [x] (22c1683d) **Duck shoot** (Fairground): everyone at once (heats), and a
      far-rail duck counts double.
- [x] (2426c392) **Half-pipe** (Mummers): the camera was pitched up at the
      backdrop; it now looks down into the channel with the gold launch
      window in view, and the rider's shadow keeps to the wall. The whole
      party rides (heats).
- [x] (104acaa4) **Surf** (Millpond): the whole party at once (heats).
- [x] (104acaa4) **Cart / wheelbarrow** (Timberyard): the whole party at once (heats).
- [x] (104acaa4) **Belfry volley**: the whole party at once (heats).
- [x] (1ff489a9) **Longbow Green**: finished from Codex's parked files.
      The **Popinjay Shoot** (pad A): a painted wooden popinjay slides on a
      rope before the butts; every knight in the room shoots at once
      (heats), toeing the line: walk to lead it, hold F to draw (a meter
      fills gold at full draw), let go to loose; a gold caret shows where
      the arrow will land, wind and all, and a pennant shows the wind,
      which shifts after each hit. Three hits in six arrows; the best
      archer is crowned, the rest wear an apple with an arrow through it;
      the first win pays the realm 25 gold once, and three for three is a
      feat ("Down Goes the Popinjay"). Board: "WALK, DRAW, LOOSE".
- [x] (22c1683d) **Firefly Grove**: all eight lanterns pay 40 gold and 2 wax
      once; the grove's song pays 40 gold once.
- [x] (1651e1cb) **Hedge Maze**: a golden acorn behind the long hedge; the first
      knight to it each outing wears the crown; the realm is paid 30 gold once.
- [x] **Joust** and **Fortune wheel**: checked; the joust key legend and the
      wheel's mode lines are already short.
- [x] (3a9f5ec4) **Two-knight joust** (the Lists): friends ride each other
      down the tilt, best of three passes, the gallery wagers real gold, and
      a lone knight meets a champion. Design calls in section 7.
- [x] **Errands**: every errand was "stand on a spot, press E" three
      times. Give each region its own silly errand that the whole party plays:
  - [x] (047af79f) **The coast**: a crab stole the beacon's lamp oil. On the
        Strand every knight chases it at once; it scuttles sideways from the
        nearest knight, digs in at high tide, and whoever grabs it gets the
        oil and the crown.
  - [x] (710f946b) **The woods**: the badger cook's Boot, Hat and Beard
        mushrooms all grow in the hedge maze at once, so the party splits up.
        The Boot kicks the first hand; pickers carry theirs home overhead; the
        best picker is Deputy Stirrer (the crown).
  - [x] (03a0d0db) **The wetland**: Brother Bucket's three sluice valves are
        sprung (six seconds open, then SPROING); the water drains only with
        all three open at once. Whoever fishes out the bell wears the crown.
  - [x] (afa0df7e) **The high roads**: pigeon post. A pigeon circles each room
        with a lost tax paper; a shot or a swing bonks it and the paper drops
        (E to take). Pigeons poop on whoever stands beneath. The best bonker
        is crowned when the minutes go in.
  - [x] (3004bcba) **Wind country**: a goose heist. Grab the picnic sail and run it
        off Goose Green; the goose (faster than a knight, after a startled
        honk) pecks the thief and takes it back; a friend's honk draws it off.

## 3. Classes you can change in the kingdom

Today the kit (weapon, guard, hand, ultimate) is chosen once, on the intro
screen.

- [x] (28363858) Five class halls, each a banner stone beside its building
      in the kingdom: **Archer** (Rookery, bow), **Shieldmaiden** (Gatehouse,
      crossbow and kite shield), **Gunner** (Smithy, handgonne and four
      bombs), **Swordsman** (Round Table, blade and more health),
      **Berserker** (Mines, twin axes and the spin — and you are a dwarf).
      Walk up, E, done: free, at once, same share of health. Each knight
      changes on their own screen with their own key.
- [x] (68c156f8) **Classes are obvious in the hero look, not everyone is
      a knight.** Every class is a whole figure of its own, not a badge:
      the **Mage** in a slate robe, pointed hat and staff with a pale orb;
      the **Healer** in a cowled grey habit with a scapular, swinging a
      brass censer; the **Archer** hooded and cloaked in forest green with a
      longbow and a quiver; the **Swordsman** in plate and plumed helm with
      his blade (no shield); the **Shieldmaiden** in mail, winged helm and
      braids behind a steel-rimmed kite shield; the **Gunner** in a wide
      iron kettle hat and buff coat with his handgonne; the **Berserker**,
      the dwarf, horned cap, ginger beard and two axes. Decisions:
  - Drawn in `arena/class_looks.rs` (art in `class_looks/art.rs`) as ink
    rows: each class toward the viewer, across (flipped for left) and
    away; a body and its held thing, the held thing redrawn for the
    strike (staff raised, bow drawn, censer swung with smoke, gun levelled
    with a flash, blade thrust, axes up). Walking moves the legs on the
    stride (or swings a robe's hem); across, the passing step rides a
    pixel higher. Strike shows while a shot or swing is fresh
    (`fire_cooldown`, `swing`). The canvas is 20 wide so a held thing
    never shifts the feet.
  - The seat's own colour is the trim (`7 8 5`, recoloured per seat like
    the knight's): hat band and hem, scapular, feather, belt and
    fletching, plume and tabard, shield field, neckerchief, the dwarf's
    belt. A class wears the seat's colour, not its kit's.
  - The posse kits that are no class (Composer, Dispatcher, Money,
    Competition) stay the realm's knight, helmet and tabard stages and
    all. A class keeps its look as health fails (nothing flies off); the
    health bar tells it. Avatars (a player's own picture) still win.
  - Drawn everywhere the hero is: the dungeon, side-on ledge halls,
    follies, the countryside rooms and the kingdom (all go through
    `knights::figure_with`), the phantoms of the ultimate and the stage
    bezel (`knights::portrait`), and the intro's chosen-knight portrait
    (a class kit shows its class). Each hall's banner stone now has its
    class on a stand beside it, in the onlooker's own trim. The cockpit's
    mini realm shows the harness's house knight, never a Delve hero, so it
    is unchanged.
  - Art v2: a class is drawn from the same figure, brought up by Scale3x,
    lit from the upper left and shaded to the lower right inside a dark
    coloured edge; the gate now lets every class through (dress, kit
    colours, the dwarf and a raised shield no longer fall back to v1).
  - Renders: `/work/artifacts/delve-world-20261013/classes/` (lineup.png
    every class in every facing and pose and the four seats; lineup-1x.png;
    lineup-v2.png; classes-v1/ and classes-v2/ fights, kingdom, folly and a
    ledge hall, each with its 192-column cells). Writers:
    `write_class_lineup`, `write_class_play_shots`.
  - The pit stop's strip-and-dress keeps the knight: it is about his
    armour.
- [x] **Each class shoots like itself, and rides like itself.**
  - The Mage throws an arcane bolt (a pale orb with a wavering cold tail
    and turning motes) and the Healer a holy spark (a warm star that
    turns between a cross and a saltire, trailing embers), where their
    kits' bows loosed arrows. Each has its own sound, made offline by
    `synth_sfx.py` (`shot_arcane`, a rising glassy zap; `shot_spark`, a
    swung whoosh, a small bell and embers), and its hits sound as magic.
    Look and sound only: the shot is the bow's (kind, damage, speed,
    range, pierce and rest unchanged, tested). The class shot is a shot
    look (`classes::shot_look`), so a card's dressing still wins over it
    and it never sticks in stone as an arrow.
  - Checked: the Gunner's balls read as iron shot with the muzzle flash on
    his strike frame; the Swordsman's thrown blade spins out and back. The
    Berserker's twin axes are melee only (no throw in her kit), so she has
    no thrown axe to draw; her strike frame raises both axes.
  - Riders: at the lists a classed knight rides in their class's head
    and coat over the seat's shield (`class_looks::dress_rider`, one call
    in `joust.rs`); in the chariot race each seated driver wears the
    class's headgear, toward, across and away
    (`class_looks::dress_driver`), and in art v2 the same head, brought
    up and lit, sits on the cart study's driver.
- [x] (a01626c8) A **Mage** at the Labyrinth College (Cabbage Storm and Hedge
      Maze in the spell slots, a thunder scroll and a rime whistle in hand)
      and a **Healer** at the Chapel (censer, choir-crumb, holy water). A
      class's spells go with the class when the knight changes.
- [x] (61588a53) Changing class below ground, for gold: each Sanctuary flies
      a field banner in the next class's colour; E there takes it up for 20
      of the gold the knight carries ("E: GUNNER 20G").

## 4. Cohesion: one world

- [x] (4f727f5f) **One economy.** Every surface game's first win pays the realm
      25 gold once (bounded: no farm); errands and the grove pay once; the
      crown and booby prize carry into the delve. Horses and the barony
      already drew on the realm's treasury; the horse market was only a
      typed command, so today's four horses now stand in pens at the
      Stables (hold E to buy, paid from the treasury at the save) (d03b4daa).
- [x] (0c0a5b75) **The map makes sense.** The layout pass: every countryside
      door is built in its own region (`overworld::kingdom::REGIONS`), not
      by its old home-floor cell. The Woods by the Dark Forest; The High
      Roads in the hills past the Dragon Keep (that wild screen is rock now,
      not lava); The Coast on the north-east shore; The Wetland in the
      western marsh; The Fair between the Village and the Artisans; Wind
      Country on the downs below the Homecoming fields; The Tilt-Yards
      between the Colosseum and the Tournament (the Horse Ranch with them).
      Each region's name is lettered in grey, no box, on a cleared row under
      its places or on open water; lots and doors keep off it. Every door
      gets a lane to the nearest road, drawn as road. Castle Town, The
      Mountain, The Wastes and The March are lettered too. Plaques: the
      Colosseum, Artisans and Tournament show only their carved sign (no
      second box), a plaque sits just over its own roof, and the place a
      knight stands at is named first and may step aside, so the Village
      shows its own name. **Open plot** for the King's court: tiles 33 to 37
      across, 35 to 39 down (`OPEN_PLOT`), between the Keep and the Village
      beside the north road; cleared and walkable, nothing else builds there.
- [x] (e2f8feb9) **Landmarks you can read on the map.** Every countryside lot draws
      its own landmark (`overworld/landmarks.rs`, built from kit helpers and the
      plain inks) matching its room and plaque: lighthouse, sluice gate, pigeon
      loft, kite shop with kites, archery butts, scarecrow, steaming bathhouse,
      duck-shoot booth, mummers' stage, wash vats, hedge maze, bothy with its
      cauldron, log yard, apple orchard, snowy crag, open belfry, aqueduct
      arches, salt pans, bathing huts, watermill, flooded abbey, quay, causeway,
      toll bridge, flower beds, cheese downs, three loud flags, caravan tents,
      wheelwright's wheels and a hippodrome track; an anvil and smoke stand at
      the Smithy. Chimney smoke now rises only from landmarks with chimneys.
- [x] (735537d7) **People send you places.** The town crier by the Keep cries
      an open errand (a relic's clue, a lantern to light, the surveyor, the
      troll's lunch, the weather clerk), and the gold arrow points there
      until the party arrives. The dwarf king's Mines mission is the crier's
      first errand once he is met (13d15fdb). Merlin names an open errand
      and Maud gossips about an unwon game; both set the arrow, which now
      finds the road to countryside places off the map (41b2ae83).
- [~] (a65436c9) **One look.** One UI rule everywhere: "E: VERB" appears only
      near the thing (kingdom, countryside, trinkets, banners); always-on
      letter markers (E, X, K, SITE) are gone; shut ways are planked, open
      ones arrowed; plaques and labels never overlap. The countryside
      rooms' sprite and material pass is done (see section 10).
  - [x] (176bc0ea) The Hippodrome bookie booth crowds its signs and the B pad under the
        knights.
  - [x] (176bc0ea) Signal Hill shows no board while a knight stands on a flag.
  - [x] (176bc0ea) The Chapel's plaque drops out beside the Keep and the Rookery.
- [x] (9b9d9357) **The surface leads down.** The crown brings an extra bomb
      into the delve; a booby prize takes one blow whole, then flies off.
      And up: the countryside's build plots (bakery, beacon, harbour, ...)
      cost ore, wax and bone, which come up from below.

## 5. Little interactions everywhere

Lots of small, funny things you can do, so the world feels alive and you
feel part of it. Each one is press E, see something happen, maybe get a
tiny reward.

- [x] (4948a29a) Ring the chapel bell: it tolls for everyone and the ground shakes.
- [x] (7f800f90) Pet Tallow (E beside her): she purrs, hearts float, +2 health.
- [x] Feed the horses at the stable: already there — the Stables' tend plate
      (hold E) brushes and waters the horse for a steadier next bout.
- [x] (4948a29a) Kick a bucket (Village, Washbucket Quay): it flies, and someone yells.
- [x] (4948a29a) Sit on a bench (Keep, Village, Artisan Quarter): a sigh and a slow mend; a step stands you up.
- [x] (4948a29a) Toss a coin in the village well for a one-line fortune.
- [x] Drink at the tavern: already there — Maud's bar serves a round that
      takes effect at the next delve.
- [ ] Knock on a door; somebody answers (parked: the village houses are
      painted into the map, with no doors of their own to stand at).
- [x] (5421a605) Honk at the goose on the picnic green, and it chases you down and pecks.
- [x] (5421a605) Lock yourself in the village stocks; friends' shots are tomatoes (no harm, a splat each).
- [x] (7f800f90) Beat a Training Yard quintain hard (150 in a second) and it thwacks you a pace back.
- [x] (4948a29a) Bow to a friend (E near them); a bow back is a HUZZAH.
- [x] (4ffa6e2a) Graffiti on the privy wall: each finished stop scratches a line
      ("BORS SAT HERE AND PONDERED", "HONK - GARETH"); the next knight reads it
      while sitting tight. The wall keeps its last eight, saved with the home.
- [x] (e7b092c2) Clay pots below: up to four along the walls of ordinary rooms,
      placed from the seed. A shot or a swing shatters one, every other one
      has a coin in it, and broken pots stay broken for the floor.

## 6. Ideas waiting (DELVE-NEXT)

- [~] (8cfe6f52) Follies: rooms that play like another game. **The Wax Maze**
      is in: one hall a floor turns into a crypt maze of candle wax with four
      wraiths (Hound chases, Cutter heads you off, Flanker comes round,
      Drifter wanders); holy water turns them pale; everyone munches at
      once, antes a tenth of their carried gold, and the most wax takes the
      pot and the crown; a gold kept for every five drops. 75 seconds.
      **The Belfry Shoot** (636704b6) is the even floors' folly: the hall fills with
      bats, a flock every two seconds for a minute; the most harm dealt takes
      the pot and the crown; a gold kept per hundred harm. **The Keg Gantry** (7a8afbcc): the floor's side-on hall (floor one has one)
      becomes a three-girder scaffold; a drunk ogre rolls a keg every 1.5 s;
      everyone climbs through the planks; a keg bowls you back to the
      bottom unhurt, a keg cleared in the air scores; first on the top
      girder (or the highest when the minute is up) takes the pot and the
      crown. **The Powder Room** (b5cb604c), a Bomberman nod: every third floor's hall
      becomes a pillared store packed with crates; the bomb key drops a keg
      (two at a time) that blows a cross two tiles long; each crate broken
      is yours, a blast sends you sooty to your corner and costs three; most
      crates takes the pot and the crown. Halls rotate maze, belfry, powder.
      Friends on their own angelX see it all (6e428b21). **Book Hop** (3f65da9a), a
      Frogger nod: every other Powder Room floor (6, 12, ...) floods its hall
      with lanes of drifting books; hop across, the ink sends you back, most
      crossings takes the pot and the crown. The three 3D follies (the
      Shaft Run, the Ore Cart, the Griffin Run) are in: see the race halls
      below.
- [x] (596a54ed) **3D at frame rate.** world3d's raster clips and projects
      each triangle once into a reused list (frustum-culled), then fills and
      shades the frame in bands of rows on scoped threads (at least 32 rows
      a band, at most 8 bands), with its buffers kept between frames; the
      bytes are the same as the old single pass (test
      `banded_frames_are_the_same_bytes_as_one_pass`). Several meshes render
      in one frame without merging (a fixed course plus what moves), with
      the depth buffer handed back for sprites, and a dark backdrop for
      places with a roof. The ride glass drops its six-tick step and moves
      every tick. Release timings on the 16-core host, machine busy with
      other builds (`raster_frame_bench`, `race3d_frame_bench`):

      | Frame | Before | After | After, 4 cores |
      |---|---|---|---|
      | world3d court, 384x224 | 5.33 ms | 1.1 ms | 4.1 ms |
      | world3d court, 768x448 | 21.9 ms | 3.9 ms | 13.9 ms |
      | a seat's Shaft Run frame, 4 knights, 384x224 / 768x448 | (no race) | 2.1 / 2.8 ms | 6.2 / 6.9 ms |
      | a seat's Ore Cart frame | | 3.3 / 3.8 ms | 6.9 / 6.6 ms |
      | a seat's Griffin Run frame | | 2.5 / 3.2 ms | 5.5 / 5.9 ms |

      A seat's race frame is the whole arena frame (3D, dots, sprites, HUD).
      Four seats at 30 fps want 33 ms between them; even on four cores they
      take about 27.
- [x] (c950e989, f78040c3) **The race halls.** From the second floor down,
      one hall a floor (not the folly's, not the gantry's, never a boss's
      camp) is a race hall; walking in starts the race, the doors hold until
      the board, and the knights stand still in the hall while their keys
      drive the race. Floors take the races in turn: 2 the Shaft Run, 3 the
      Griffin Run (under Dragon Keep), 4 the Ore Cart, and round again. Every
      seated knight races at once (a knight who sits down mid-race starts
      from the top), each behind their own chase camera on their own screen
      (host, friends' angelX and browser seats alike); each antes a tenth of
      their carried gold; the first home takes the pot and the laurel, and
      everyone keeps two gold for each thing picked up. 90 seconds at most;
      whoever is still out is ranked by how far down they got. The board
      (shared) shows place, time, haul and gold for six seconds, then the
      hall comes back cleared. Title card: the name, two or three words and
      the two keys, then 3, 2, 1, GO. Crashes get a two- or three-word joke
      (WHEEEE!, MIND THE GAP, BONK!, BUFFER STOP, TIMBER!, BIRD DOWN, ...).
      The view is the 3D frame as ordered dots on black paper in the
      realm's inks, with the knights as their own sprites in it.
- [x] (c950e989, f78040c3) **The Shaft Run** (ROLL. DON'T FALL. F hop, E or
      back to brake). Each knight in a cage ball (bronze, silver, white
      enamel, crystal by seat), down 486 tiles of winding mineshaft:
      timbered ramps, two plank bridges with no rails (roped in gold so the
      edge reads), two crossings where ore carts trundle across (a bonk
      knocks you back up the shaft, then lets you through), two lodestone
      rails that pull the ball to an open edge and speed it along the rail,
      two sluices that wash you side to side, two collapsing drifts whose
      rotten planks go a second after the first ball passes (the leader
      gets a clean run, the field must hop the holes), and Brannoc's
      ore-sorter at the bottom. Off an edge is down the shaft; a rope hauls
      you back six tiles short of the post you fell from. Balls bump and
      trade their pace; bends throw a fast ball wide. 23 gem shards on the
      risky lines, first to each takes it. A steady test knight is home in
      57 to 58 s with two crashes.
- [x] (c950e989, f78040c3) **The Ore Cart** (JUMP. SWITCH. F jump, E switch
      toward the arrow held, forward pumps, back brakes). Carts on three
      lines through 484 tiles of galleries and trestles over chasms. Lines
      end at buffer stops (stay on and you go over the stop), rails break
      (be in the air), timbers lie across lines (clear them or leave the
      line), gold nuggets sit low and high (some only a jump reaches). A cart
      that runs into another shunts it on and stops dead. The start is three
      abreast and one behind. Steady test knight: 56 s, no crashes.
- [x] (c950e989, f78040c3) **The Griffin Run** (FLY. SHOOT. F looses a bolt,
      held for one a third of a second; E barrel-rolls; arrows rein across
      and up). Griffins down 560 tiles of gorge under Dragon Keep to its
      gate: 15 gold rings (each a burst and a haul for every griffin through
      it), 9 gargoyles on the wing (shoot one for a burst, fly into one and
      tumble), spires up from the river and old ramparts across the gorge
      (beside or over, under or over). A bolt in a rival's feathers slows
      them a second, unless they are rolling. A tumble throws you off the
      stone and costs your pace, never your place on the course. Steady test
      knight: 60 to 61 s with one tumble.
- [x] (7349d865) **Racers ride as their class.** In a ball, a tub or on a
      griffin, a knight of a class is that class's own figure seen from
      behind (the Mage's hat and staff, the Berserker's horns, the Healer's
      hood), running inside a ball and standing elsewhere, in the seat's
      trim; the finish board shows each class's portrait. A rival just
      behind or beside your ride (between it and your camera) is left out
      of your view until they pull ahead or drop back.
- [x] (7349d865) **The Griffin Eyrie**, a way in from the kingdom: a new
      countryside place on the Mountain (its plaque a griffin, its landmark a
      crag with a nest, a perched griffin and the master's hut). In the yard,
      E by the perch saddles a griffin for every knight there for a free
      practice flight of the Griffin Run: no antes, no pot, no laurel, again
      as often as you like. Until somebody wins a race (a race hall's, or
      the Eyrie's flight), Maud's gossip names it ("NOBODY'S WON A RACE
      YET. THE GRIFFINS ARE BORED.") and the gold arrow points to it once
      the party has been down; the first win settles into the realm
      (`race_won`). The Eyrie's only door is its kingdom door, so the home
      floor's road network is unchanged.
- [x] (7349d865) **Browser seats during a race** (`browser_seat_race_bench`,
      release, four knights racing, four seats). A 3D race changes nearly
      every 16-pixel tile each frame, so every picture goes out whole:

      | Seat frame | Painting all four seats, a tick | Encoding a seat | A picture | A seat at 30 fps |
      |---|---|---|---|---|
      | v1, 384x224 (default) | 8.4 to 11.4 ms (p95 12.2) | 3.1 to 3.5 ms | 5 to 6.5 KB | 150 to 195 KB/s |
      | art v2, 768x448 | 11.7 to 15.1 ms (p95 17.5) | 3.1 ms | 130 to 166 KB | 3.9 to 5.0 MB/s |

      The painter's budget is 33 ms a tick (it paints twice a tick for
      near links, still under it), so a v1 seat holds a steady 30 fps on
      any link. An art-v2 seat paints in time too, but its fast-PNG
      pictures of the doubled dot frame are 25 times the size: fine on a
      LAN, too much for a home uplink. Left for the transport (not this
      stream): send race pictures at native size, or encode them with the
      indexed, best-compression path the v1 frames use.
- [ ] Holdings: buy a cleared dungeon's floors and build them out (parked:
      DELVE-NEXT Part 3 is a whole management game, the King's court tree,
      charters, stewards and a ten-room cross-section per floor; its choices
      want the operator's call before a night spends itself on it).
- [x] Holdings: the operator's call came ("whole management game, yes");
      see section 7.
- [x] Valour meter and tempering (arrived with feel-pressure and
      controls-ui in the integration merge).

## 7. The two-knight joust (stream world/joust)

- [x] (3a9f5ec4) Holding E on the mount plate puts that knight in the near
      saddle and the nearest friend at the lists in the far one; everyone
      else at the lists sits in the gallery. Alone at the lists (friends
      elsewhere, at vigil or down), a champion rides as before (Kay,
      Palamedes, Lancelot).
- Design calls:
  - Keys: A or D picks a horse at your end before the first pass, F spurs,
    W or S sets the lance (helm or chest), E swings the shield (up or
    down), F strikes. The Space brace is gone (E's shield does its work);
    holding Space at the end still withdraws. Lance and shield lock at the
    couch, 12 ticks before the meeting; until then both show, so the game
    is the late switch. A champion's shield and lance waver until your
    horse's tell, as before.
  - Hits: on the shield is a point (a lance broken on it, two); round the
    shield is two (HELM high, CHEST low); at the very moment, one more.
    The better lance takes the pass; two passes or an unhorsing take the
    bout; three passes level go to points, then a draw.
  - Horses: the knight who mounts rides the saddled mount, the friend the
    next stall along; either can change before the first pass. A horse's
    charge is its speed: two horses meet at the time their two speeds give,
    and further down the lists from the slower one's end. Balance is its
    heart; tending still adds a knock. Herd horses stay at the Hippodrome.
  - The flag: with two knights it drops once both have spurred, or after
    10 seconds before the first pass (time for the wagers) and 5 after.
  - Gold: knights carry none on the surface, so the lists use the realm's
    treasury and bank every win under the winner's name. Entry 20 a rider,
    the Crown adds 20; the winner banks 40 (the purse of 60 less their own
    entry), the loser's entry is taken at the save. The gallery backs a
    rider with A or D before the first spur, F raises by 10 up to 30,
    paid at evens. A draw hands every stake back. A treasury that cannot
    cover the entries makes it a bout for honour; wagers stop at what the
    treasury still holds. Lost gold rides the mark `lists:paid`.
  - Stakes worn: the winner gets the laurel; a friend beaten wears the
    dunce's cap (the hobbyhorse booby) until they win one back. Feat:
    Laurel of the Lists (beat a friend); Over the Tail and Champion of the
    Lists stay with the champions.
  - Leaving the lists mid-bout: from the gallery voids the wager; from the
    saddle after a pass is a forfeit; before the first pass, the friend
    left in the saddle rides on against the champion.
  - Look: a painted tilt on posts, the pass counter big over the stands
    with pass shields and balance pips, each knight's own keys at the foot
    of their screen, STRIKE! when the window opens, the hit words big under
    the lists, UNHORSED! with the helm bouncing off and birds going round,
    pennants and stakes over the gallery, and a result board with every
    wager. Renders: /work/artifacts/delve-world-20261013/joust/.
- [ ] Playtest: the 12-tick couch and the 10-second first flag are first
      picks; change them from play, not by formula.

## 8. Holdings

The operator: "whole management game, yes. Integrate it into the kingdom,
where castles and dungeon floors can be bought and managed; link everything
together from angelX internal operations presented as game systems."

Design calls (DELVE-NEXT Part 3, cut to what plays):

- **Where.** The Court of Deeds (the Land Office) stands on the open plot in the town core, between the Keep and the Village, its
  own door on the kingdom's map. Its great board shows one deed at a
  time (arrows turn it: six floors, then the sixteen model castles), the
  works ledger on the west wall shows what angelX's work does, and every
  plate is hold E like a station's. A castle can also be bought at its own
  hall's deed plate.
- **The court tree, two branches.** Claim outright (DELVE-NEXT's deed
  price) or hold in fief (40%, and the King takes 30 gold a rent day). The
  guild grant and leaving it wild were left out: a grant is free money with
  no play, and "wild" is just not buying.
- **What is for sale.** A floor once the realm has cleared it (the
  treasury's record); a castle once its model house has served the realm
  (its yard in the stable has calls). Placeholder houses that never served
  stay off the market.
- **Ten bays, twelve rooms.** Four families of three rooms (Tavern, Forge,
  Chapel, Trade), built in order within a family, so a holding is what its
  owners left out. Small-room prices from DELVE-NEXT (400, 450, 500). No
  quality multiplier (no formulas): each deed adds a fixed rent per room
  instead (deeper floors more, castles 5).
- **Stewards.** Five folk, one way each: Wynn the greedy (rent +30, stirs
  unrest), Brother Aled the pious (calms unrest), Gwen the brave (walls
  hold), Old Hob the sot (taverns +10), Mistress Vell the shrewd (builds
  twice as fast). Free to hire, a sack of grain a rent day.
- **Stars are the charter.** Two to five stars: a steward, five rooms, ten
  rooms, full walls. Each costs gold and one scroll from the scriptorium,
  and raises the rent.
- **Rent day.** Every floor the party clears in a delve. Rent goes into
  the holding's coffer; a knight collects it at the Court's strongbox, a
  floor's steward below, or a castle's deed plate.
- **Walk it.** A held floor has the steward's hall off its first hall
  (like the King's forge-hall), its ten bays as built; each standing room
  gives the whole party its boon once a floor (ale, a bomb, mail, spells,
  raise, ward, gold, grain). The floor's first hall flies the realm's
  banner, and so does every castle the realm holds, on the map.
- **angelX's work keeps the holdings, never pays them** (the settlement
  rule: coding cannot mint Delve gold). One fixed number each, every one
  on the Court's ledger as two short words:
  cache hits fill the granary (a sack per 250,000 cached tokens); fresh
  tokens pay wages from it (a sack per 50,000); a passing test run mends a
  course of the most worn walls; a commit lays three of a room's six
  courses; a compaction copies a scroll; a subagent is a retainer who
  garrisons a holding for a rent day; a loop iteration is a patrol that
  walks off a point of unrest; a failed or slow (3 minute) turn stirs one.
  Unrest at 5 halves rents, at 10 stops them. Walls wear a course a rent
  day; at 0 the holding is breached and pays nothing.
- **Second wave (the parked parts, built).**
  - *Raids.* Walls at 4 or below bring raiders. While they hold a
    holding it pays no rent, and each rent day they sack a room (back to
    half built, for labour or commits to raise again). The party fights
    them in the holding's hall: a floor's steward's hall below, a castle's
    hall in the kingdom. Every seated knight fights at once. Raiders come
    4, 6 or 8 by how long they have held it. The holding fights too: each
    trade room buys off one raider, the forge arms the party (mail from the
    smithy and armory, a bomb from the fletcher), the chapel mends every
    three seconds, the tavern hurls mutton bones and the archery range looses
    arrows. A win frees the holding, mends 3 walls and pays the raiders'
    purse (120, 200, 300). A floor wipe is a wipe. In a castle's hall at
    home the party is beaten, not killed: the raiders sack a room and stay.
    Raiders keep their own company (like the Wanted hall), outside the
    floor's boss provision.
  - *Neighbours.* A room feels the room in the bay beside it in the same
    row of five: brewed, thirsty, noisy, blessed, rowdy, stocked, supplied,
    each 10 rent. Arrows on the bays, the words on the readouts, and the
    Court says what a new room would do before it is paid for.
  - *The King's favour.* One number from -5 to 5. A fief raises it by 1, a
    claim lowers it by 1, and the cards move it. At 3 his masons build every
    room for 100 less; at -3 his men tithe claimed holdings too.
  - *His cards.* One after a rent day, eight in rotation (royal visit,
    tithe demand, dwarf masons, brewers' guild, pilgrims, monks' offer,
    sellswords, border baron). The herald stands by the Land Office between
    two answer posts. Hold E at a post to answer (2-3 words and a price).
    The way-on arrow leads to him while a card waits.
  - *Boons that go with the party.* The taproom sends a tavern brawler and
    the kennels a hound. Each takes the party's ally place for the rest of
    the delve and fights through every room; Beaumains, once hired, keeps
    the place. A fifth family, the Barracks (barracks, kennels, archery
    range), joins the four. A standing barracks keeps a holding's walls
    from wearing.

- [x] (52f70c44) The holdings' rules: deeds, terms, bays, stewards, charters,
      rent days, coffers, saved in `Home` beside the treasury (a bad
      record resets alone). `together_shooter/holdings.rs`.
- [x] (52f70c44) The cockpit's half: marks settle at the save (buy, build,
      steward, charter, collect, labour, grain), rent days in
      `settle_realm`, the realm's record (cleared floors, served houses).
- [x] (52f70c44) angelX's work fed in through the harness event path: tool
      calls (commit, test, compaction, spawn), turn spend (cache and fresh
      tokens, slow turns), failed turns, compaction notices and background
      compactions, loop iterations. `App::note_ops_*`, `holdings::feed`.
- [x] (52f70c44) The Court of Deeds: a room on the home floor with its door by
      the Keep, the great board with the cross-section, the works ledger,
      eleven plates, the clerk, the strongbox.
- [x] (52f70c44) A held floor's steward's hall below, its bays' boons once a
      floor (mirrored to friends), the realm's banners in its first hall.
- [x] (52f70c44) Castles: bought and collected at their own hall, a tapestry
      cross-section, the realm's pennon over them on the map.
- [x] (52f70c44) Tests: buy, fief, uncleared and unserved, short purse, rooms
      and labour, stewards, charters, rent, unrest, breaches, every
      steward's way, every op's fixed number, the herald's classification,
      labels, save and reset, the Court's plates, the steward's hall, the
      mirror, a castle; the map's door in and out, and angelX's work
      carried through the cockpit to a saved realm (531d13ee). Renders looked at in
      `/work/artifacts/delve-world-20261013/holdings/holdings/`.
- [x] (bf91b5f8) Raids fought in the steward's hall and a castle's hall, with
      the bays as defences; neighbour effects; the King's favour and his
      cards with the herald in the kingdom; the brawler and the hound;
      the Barracks family. Tests for each, and renders in
      `/work/artifacts/delve-world-20261013/holdings/raids/`.

## 9. New rooms (world/rooms)

Each room has its own door in the kingdom (or an old map door now opens
on it), tests for its interaction and reachability, and renders in
`/work/artifacts/delve-world-20261013/rooms/`.

- [x] (1ff489a9) **Longbow country**, north of the flags: Longbow Green
      (cell -2,-9), the Fletchery (-1,-8) and the Scarecrow's Field
      (-3,-9), joined by roads to Signal Hill, the Kitewright and the
      Timberyard, each with its own kingdom door and plaque picture
      (target, arrow, scarecrow). **The scarecrow's wardrobe**: he is ON
      STRIKE in his smallclothes; E to ask, then the crows' TIN HAT (atop
      the green's pole), FEATHER COAT (by the goose pen) and LEFT BOOT (in
      the wheat); dressed, he pays 65 gold and 3 bone once and the realm
      may build **the Quiver Coach** (fletchery to Broadsheet Row). The
      town crier cries the errand and each missing piece, so the gold
      arrow sends the party there; Maud gossips about the unwon shoot.
- [x] (979acb5c) **Haggler's Square**, behind the Artisan Quarter's door:
      Magda buys ore, the Chandler wax, Soup Ned bone. Hold E and the offer
      climbs a gold every fifth of a second; let go to sell one from the
      treasury; hold past the top for a second and the keeper goes red,
      steams, and slams the shutters (OUT) for three seconds. Sales settle
      with the realm's save.
- [x] (979acb5c) **The Royal Soak** (bathhouse, own kingdom door): four
      tubs, 5 treasury gold a soak, three seconds with a rubber duck while
      Brother Suds scrubs; the knight comes out scrubbed, and the next
      floor left without the privy costs them nothing (the privy joke's
      other half). One knight a tub.
- [x] (979acb5c) **The Dented Anvil**, behind the Smithy's door (the
      Gunner's banner stays outside): Old Dint tempers the arm in hand a
      step for 40 gold, and mends a missing helm, mail and shield for 25,
      then tests his work by hitting the helmet (stars). Paid from the
      treasury the way the surface peddler is; a purchase lapses if the
      knight walks out before it is paid.
- Design calls: the town's three cells are (3,-5), (4,-5), (5,-5), south
  of the quay; the Smithy and Artisan Quarter map doors, which led to the
  generic Great Hall, now open on their rooms. Holdings, the King's court,
  class halls, the horse market and the hippodrome are untouched.
- [x] (9e4a9f0b) **Nooks below ground**: every floor of a delve (not the wheel's
      horde or gauntlet) hangs one lived-in dead end off a room, without
      dice and without taking a monster away; each pack keeps two, by
      depth. Each is played by the whole party at once and raises monsters
      when it goes wrong (at most four a nook):
  - **Grubb's Kitchen** (Cavern, Fungal): "E: SCOOP STEW" while the goblin
    cook stirs; when his ears twitch he turns, and whoever moves gets the
    ladle and a goblin cousin comes. 12 scoops, 3 gold each, crown to the
    best scooper.
  - **The Quiet Room** (Archive, Unknown): "E: READ" eight lecterns, 4 gold
    and a terrible title a page; three books (the floor's choice) are
    mimics that bite and fight. A mimic breathes now and then: the tell.
  - **The Great Bellows** (Cavern, Hellforge): "PUMP IN TURN", west, east;
    half hot wakes two imps, full hot tempers every knight's arm a step.
  - **The Bone Dominoes** (Crypt, Unknown): "E: TOPPLE", then "FILL THE
    GAPS": a knight standing in each of three chalked gaps falls over in
    the line's place; an empty gap stalls it and raises two skeletons. The
    coffer pays 8 gold each, crown to who fell most.
  - **Mother Mort's Still** (Hellforge, Fungal): "PICK TWO JARS" from the
    spoils a knight carries: bone and wax a potion, wax and ore a thunder
    scroll, bone and ore a bomb, the same jar twice a frog. Two brews a
    knight a floor; every third brew burps a slime.
  - **The Bell Loft** (Crypt, Archive): the ghost rings a tune, the party
    rings it back ("RING THE TUNE"); a wrong bell calls a wraith. Three
    tunes and the hymn mends everyone whole, 6 gold each.

## 10. Art style (stream world/art, 2026-10-13)

The operator: "do a countryside braille pass... the braille art is still too
simple, we haven't solved our art style yet." The style is now written down
as one system in `docs/ART-V2.md` ("The material system") and built as shared
helpers in `overworld/material.rs`, used by the default renderer.

- [x] (74874f7a) **The system.** Black paper, lit masses; one north-west
      light; per-material ramps inside the realm palette, no black outlines;
      flat ground laid out in braille-sized two-by-four dot cells by a patchy
      density field; grouped texture (leaf clumps, grain, courses, ripples)
      from fixed seeds; dusk dithers only between two steps so texture
      survives. Calls made: kept the 384x224 v1 canvas and its palette (the
      operator plays v1; v2 stays opt-in), no signal inks in material, tonal
      greys for every "white".
- [x] (74874f7a, 79719cd8) **Countryside pass.** Every countryside room:
      overlapping lit woods (oak, pine, copper by region), clipped hedges in
      the maze, boulders on the mountain; water tiles are one body of open
      water with depth, swells, drifting glints, earth lip and foam; reeds on
      the banks, shells on the beaches; undergrowth at the wood's foot;
      meadow, sand, snow, mud and scree ground; trodden roads with ruts.
      Every set piece redrawn with the helpers (houses with hipped roofs and
      chimneys, mill and turning sails, ruins with fallen crenels and moss,
      apple trees, priory, bird office, observatory dome, lighthouse tower,
      grandstands and rutted track, log stacks, laundry vats, players' stage
      and painted flat, caravan, toll houses, signal hut, dovecote, bell
      towers, aqueduct). Blocks that were drawn as nothing (Goose Green,
      Kitewright, Signal Hill, Wheelwright, Toll Bridge, Hippodrome) now show
      what stops you. Joke lettering stands on sign boards.
- [x] (ae56d2be) Pebbles, clover, toadstools, fallen leaves and puddles on
      the open ground.
- [x] (0b75ba30, 22ea7d2b) **Dungeon.** Walls are coursed masonry (rough
      rock in caves) with rubble caps, lit lips and a contact shadow, in each
      pack's stone; floors are dressed with flag joints along the walls that
      wear away into the dark. Doors, pots, chests, tombs and foes keep their
      sprites and hitboxes.
- [x] (ae56d2be) **Kingdom.** The realm map's woods grow from the same
      material trees (eight varieties, crowns overlapping south over north),
      so the kingdom and the countryside read as one place; the King's Hall
      walls match the dungeon's.
- [x] **Readability rework** after review ("muddier, not richer"): a value
      hierarchy now holds in every countryside room. Ground is darkest and
      calm (tufts of three or four pixels in close tones, bare paper
      between, never lone-pixel speckle); trodden roads are a step lighter,
      laid in two-by-two clods with a crisp verge; standing things are
      lifted a step above the dusk (`dusk_with`, world rooms) so props and
      focal objects read first; the edge wood is dark and quiet; orchard
      trees are focal with red apples and trunks; the cliffs end at a lit lip
      over a shadowed face. The new rooms from the integration merge
      (Longbow Green, Fletchery, Scarecrow's Field, Market, Bathhouse,
      Smithy, Land Office) take the same edges, ground and dressed walls.
- [x] (06190872) **Braille.** The scryglass crawl (what a loop shows
      while agents work) was an even mesh of dots: its faces sat in the dark
      tones braille cannot separate. Its walls, floors and vault are now
      drawn in the band where dots fill, with black joints, rough rock in the
      mines and caves and a ribbed vault; painted niches, shelves and doors
      are lifted into the same band. The arena in braille was checked the
      same way (trees, houses and roads hold). The scryglass vista, ride and
      interiors already read and are unchanged.
- [x] **Braille encoder rebuilt** (structure-aware, both encoders): wall
      edges, doorways, floor joints, silhouettes and figures read crisply at
      96x30; true black stays black; colour carries value. Two ride guard
      bands (vista and interior ink coverage) were recalibrated because a
      lit fill is now drawn solid instead of as a Bayer screen.
- [x] (06190872) **Caves.** Cavern and fungal walls are packed boulders
      and fissures (moss-toned in the fungal caves), not mortared courses.
- [x] (e6504d36) **Kingdom buildings.** The kit's stone (Keep, towers,
      cottages, Colosseum, village, and the layout agent's landmarks built
      from it) takes the material's per-block tones and lit courses; kit
      roofs are the material's hipped shingle, slate and thatch. The map's
      dithered dusk was approved by the operator and is left as it was.
- [x] (e6504d36) **Monsters.** Every creature is shaded in the room's
      light (lit top edge, shaded underside); sizes and hitboxes unchanged
      (`sizes.txt` from the lineup is identical before and after).
- [ ] Not done: the v2 opt-in layers do not yet
      call the material helpers; hero figures belong to the class-looks
      stream.

## Foundation (stream world/foundation)

The operator: "we clearly have the wrong foundation for presentation and
multiplayer setups". Frame rate is never again a reason to park a feature.
Five stages, each keeping the game playable and the suite green. The design
and the numbers live in `docs/DELVE-FOUNDATION.md`.

- [x] (98cd1fda) **Sim off the terminal's thread.** `drive/together_sim.rs`:
      a `delve-sim` thread owns the 30 Hz accumulator (a quarter second of
      catch-up after a stall, dropped steps counted), takes stamped controls
      from one channel (keyboard and friends alike), and publishes an
      immutable snapshot after every tick and edit, the tick before kept beside
      it. Friends' publish, feel ledger and checkpoint run after the tick, off
      the terminal; sounds and cues reach the cockpit as events. The cockpit
      reads the snapshot latched for the frame and edits the run under the
      sim's lock (`as_mut`). Tests pump the same core by hand
      (`ANGEL_DELVE_SIM_THREAD=0` does it in the cockpit too).
- [x] (c49955ff, ceb9198b) **Presentation pipeline.** The host's picture is
      blended, drawn, scaled and encoded on the `delve-render` worker
      (`ui/game_pipeline.rs`); frames pace at 60 or `ANGEL_DELVE_FPS`; browser
      seats paint in parallel on resident painters; images are packed RGBA
      with a per-thread sprite atlas; scenes paint from a layered draw list
      (`arena/draw_list.rs`), the crowd in ranks (guardians, monsters,
      knights), each from the back of the room to the front. Every shot set
      compared pixel by pixel with integration: identical but for monster or
      knight overlaps in fights.
- [x] (f7369361) **Netcode.** `GET /net` (delve-net/1, binary): per-tick
      deltas of a generated value tree against the acknowledged tick (no
      `Live` plumbing for new fields), lists diffed by id, per-seat interest,
      the newest three controls per client tick with sequences, prediction
      with reconciliation for the joined angelX and a tick-and-a-half
      interpolation buffer for everyone else. Everything that moves slides
      between ticks (races included). Heavy browser links get half-size
      pictures. About 1 KB a tick a seat, from 14 to 23 KB of JSON.
- [x] (cb74b875, 2ed6e75b) **Activities.** `together_shooter/activity.rs`:
      every room game an `Activity` (on, ride, tick, end, settle, fields) in
      five ordered lists (rides, follies, Undercroft rooms, the world's rooms,
      the realm's places); the step's reins chain, `tick_home`'s match,
      `tick_world`, `clear_activity` and the realm's settle calls walk them.
      Shakes and chain sparks are `fx` events, shown by `fade`, `present` and
      the step's end (every still shot matches integration). The rules read
      the map only through `map.rs`, and no longer warm the renderer's atlas.
- [x] (07e065ff) **Browser seats: one prediction.** The host stamps each
      picture with the seat's last taken control, its knight's place and
      pace; the page slides only until a picture acknowledges the key, then
      not at all; the host leads by the round trip from the tick's place, and
      a friend's heading changes move their knight from the moment they
      arrived (sub-tick). Proven at 40, 80 and 150 ms: never back more than a
      pixel on start, stop, turn, diagonal or tap.
- [ ] **The browser as a real client.** The renderer and the wire in wasm,
      fed the binary deltas over the page's socket, with the native friend's
      prediction; pictures stay as the fallback. Plan in
      `docs/DELVE-FOUNDATION.md`.
- [ ] **Measure.** Frame and tick telemetry before and after, and a headless
      four-seat bench of ms per tick, ms per frame and bytes per tick.
