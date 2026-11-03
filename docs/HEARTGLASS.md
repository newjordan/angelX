# The Heartglass: the Virtual Embryo campaign as a holo-panel

The operator's words (2026-10-20): "make a visualization and tracking system with a glass layer, high tech as fuck".

The campaign predicts how the mouse embryo's heart develops, from single-cell and MERFISH data (the Virtual Embryo Challenge). The `vec` cartridge keeps its state in one folder. The Heartglass is a floating holo-panel in angelX's window that shows that state: the heart itself, turning and beating, and where we stand on each board. Its modules float over the window by default; the leaned plate remains an explicit alternate look. It is in the Orrery's family (docs/FOURTH_WALL.md) and has its own thread. Its heart is an `angel-gpu` scene (docs/GLASS_CORE.md).

## What it shows

Top to bottom:

- **The title strip.**
  - HEARTGLASS.
  - A heart monitor's trace: a P wave, the spike on each beat, a T wave. It scrolls left as time goes and flares with a new score.
  - The campaign's real progress from the board's `quest` counters: proxy runs, official scores, heart boards in the top 10.
  - The phase (`P2`) and a countdown to `phase.collab_ends`, ticking each second.
- **The heart.** The cartridge's cloud of cells, one tiny gem a cell, in an `angel-gpu` scene with depth.
  - Cardiac cells (myocardium red, endocardium cyan, epicardium gold, progenitors violet: the palette in the file) are lit but mostly shine by themselves, each with a soft translucent shell as its glow.
  - The other cells are small, dim and greyed toward slate, so the heart's own lineages carry the picture.
  - It turns once every 26 seconds over a projector ring and swells on each beat (a lub and a softer dub, 57 a minute).
  - It cycles E8.25, E8.75, E9.5, then each predicted stage in the file. A stage holds 7.5 seconds. The next is built in 2.6 seconds by a scan: a ring of light rides up through the heart, the old stage's slabs narrow away as it passes and the new stage's grow in.
  - Each stage's frame is its own (the data's stages are not registered to each other), so the scan rebuilds rather than morphs.
  - Our predicted stage is marked as ours: its title is gold, reads OUR PREDICTION and names its board (`E10.5*`, `HX`), and the ring and its place on the stage rail turn gold.
  - Top left, the stage's label, cell count and cardiac count. Top right, the lineage key from the file's palette.
- **The stage rail.** Every stage in the file. A glow slides to the one in front, and a line under it fills as its hold runs.
- **Five board tracks**, on one score scale shared by all five (the floor and everything known, rounded out to fives, at least 45 to 80). On each:
  - A dashed tick at the floor (50).
  - Our best official score, a solid bar from the floor with a live ember on its head. Below the floor it runs left in a darker red.
  - Our best proxy score, a ghost bar with a ring.
  - Above the rail, the VE leader (gold ▼) and the VE 10th place (silver ▽). Below it, the partner team's own best (`ve_partner`, violet ◆, "team") and the Yukon frontier (cyan ▲).
  - The three heart boards (heart interp, heart extrap, Gata4) have a rose tag and faintly warm rows.
  - Over the rail, the gap to 10th place ("5.4 to 10th", or gold "+0.4 over 10th").
  - On the right, our best official score: gold at or above the 10th place, white above the floor, grey at or under it. Under it, the day's budget as pips (lit pips are submissions left), and a sparkline of the board's history (official scores joined, proxy scores faint dots, the floor a faint line).
- **The legend**, and when the budget next resets.
- **The ticker**: the newest event, coloured by kind, and how long ago it happened.

A new official score (a board's record gains one) flares the heart: it swells, its aura burns, sparks fly, the board's row lights, the rim brightens, and a banner reads NEW OFFICIAL SCORE · HX 64.7. It fades over eight seconds. A score that landed while the panel was hidden, after it had been shown, flares when it is shown again.

With no board yet, or a malformed one, the plate reads NO CAMPAIGN YET and names the folder it watches. With a board but no cloud, or a malformed cloud, it reads NO CLOUD YET over the idle ring, and the boards still show.

## Turning it on

- `ANGEL_GLASS_HEART=1` or `=float` with `ANGEL_GLASS=1` shows it from the start, floating over the whole window as a heads-up display of modules you can drag (below, "Floating over the window"). `ANGEL_GLASS_HEART=plate` explicitly shows the leaned plate instead.
- `/heart float` and `/heart plate` switch between the two looks and show it. `/heart reset` puts every floating module back where it was designed, shows any that were hidden, and shows the floating look.
- Its print is set in Share Tech Mono (SIL Open Font License), carried in the binary and written once to `~/.angelX/fonts/`. `ANGEL_GLASS_HEART_FONT` names another font, as a family or a file. The panel lays its print on a monospace grid, so another font should be monospaced too.
- `/heart` shows or hides it. Showing it uses the floating HUD unless this session explicitly selected `/heart plate` or launched with `ANGEL_GLASS_HEART=plate`. It is a built-in command, so a cartridge command of the same name is never reached (`ui::input::cartridge_command`).
- It is off by default.
- In a terminal session, `/heart` only says that the panel lives in angelX's window. Nothing else changes.
- `ANGEL_VEC_HOME` names the folder. The default is `~/.angelX/vec/`.
- **Demo mode.** `ANGEL_GLASS_HEART=demo` or `/heart demo` shows a synthetic
  campaign made in code (`heartglass/demo.rs`): invented scores on five
  boards and a generated four-stage heart, deterministic every run. The
  phase chip reads `DEMO` and the ticker says it carries no VE data; the
  cartridge's folder is never read while it is up. This is the mode to
  screen-record and screenshot anywhere. `/heart float` or `/heart plate`
  leaves it for the real campaign.

## Where the plate stands

- Anchored at the top right, two columns in and below the header's rows: where the Orrery stands.
  - It is 40% of the window's width, from 36 to 48 columns.
  - It is as tall as the window allows above the composer. The heart's viewport takes what the boards leave, from 150 px to 380 px.
- Its lean is the Orrery's way, half as far (−0.025 rad). The plate is three times the Orrery's height and carries text to read.
  - It stands further in from the right edge by its lean's reach, so the lower corner does not swing out of the window.
- With the Orrery up (`ANGEL_GLASS_ORRERY=1`), it stands four columns to the Orrery's left, its top level with it, or below it if there is no room to the left.
- In a window too small to float it over the text (under 80 columns or 24 rows, or without 150 px for the heart over the boards), it is not shown.

## Floating over the window

The operator's words (2026-10-22): "because it wasnt styled and laid out over the whole UI, and then when we do, after we do, we apply subtle gradients/drop shadow style blurs to showcase the over, but not so much we cant read below if we focus", and "make each element draggable, modlable".

Floating, the panel has no plate. Its parts are six modules spread over the window (`heartglass/hud.rs`), each on a soft shade of its own.

- **The modules.**
  - **The heartbeat strip** (HEARTGLASS, the trace, the counters, the phase and the countdown). In a window 1180 px or wider it rides in the header's empty middle, right of the menus and clear of the workspace's name. In a narrower one it runs across the top under the header.
  - **The heart**, with its stage title and lineage key, at the right under the header. In a window too short for the heart over the boards, it stands to the boards' left.
  - **The stage rail**, under the heart.
  - **The boards**, kept to the lower right over the far ends of the transcript's lines, and **the legend** under them.
  - **The ticker**, along the bottom left on the rule of dots over the composer, so the transcript's newest line stays clear. In a narrow window it takes the whole bottom, under the legend.
  - The window's own cell size sets the header's and composer's clearance. Under 640 x 420 px the HUD is not shown.
- **The shade.** Each module's print is its own picture, and under it a halo drawn from that print: every mark swollen two pixels and softened (a band of shade under each line of print), swollen further and feathered wide (the bloom), a wash over the module's content that fades a little downward, and for the heart a soft oval behind the cells. It is a deep blue near black, at most 84% where the print is thickest and lighter in a module's open spaces, so the text under it stays readable when looked for.
  - The parts that move (the trace, the countdown and the clock's other pictures, the stage title) are in the bloom, not the tight band, so no box shows round them.
  - Floating, the print's dim greys and faint rules are lifted toward its lights (a gamma of 0.68), and text under 18 px is set in the window's own font, which holds better over text than thin Share Tech Mono. The plate is unchanged.
  - The heart's scanlines fade toward the edges of its viewport, and its sweep is a soft glow, so no rectangle shows over the text.
- **The pointer.**
  - Over a module, a hairline outline with brighter corners shows its extent, a tab over it carries its name and a hide button (×), and a grip sits on its lower right corner. Nothing shows when the pointer is elsewhere.
  - Drag a module's body (or its tab) to move it. It settles on the window's margins when it comes within 10 px of them, and its content stays inside the window (only its shade may hang over an edge).
  - Drag its grip, or its body with Shift or Control held, to resize it from its top left corner, from 0.6 to 2.2 times its designed size and never past the window. Its pictures stretch while you drag and are drawn afresh at the new size once you let go.
  - Click the × to hide a module.
  - A click on a module's body or tab that does not drag goes through to the cockpit under it, as if the module were not there (the header's model menu opened through the ticker). Presses anywhere else, right and middle buttons, the wheel and every move are the cockpit's, as before.
- **The saved layout.** `~/.angelX/heartglass-hud.json`, written aside and renamed in when a drag, a resize or a hide ends, and read when the panel's thread starts.
  - It names only the modules you moved, resized or hid: where each stands (its anchor to the window's left, middle or right and top, middle or bottom, by the third its middle is in, and pixels from there), its scale, and whether it is hidden.
  - A module it does not name, a field it leaves out, an anchor or offset out of bounds, a file that is not JSON or is over 64 KiB, or no file at all: the designed layout. A scale out of bounds is clamped.
  - A module kept to the right edge stays by it when the window widens; one in the middle stays in the middle.
- **How it reaches the window** (`crates/glass/src/zones.rs`). The panel hands the window pointer zones (`Host::set_pointer_zones`): the hovered module's hide button and tab, then every module's grip, then every module's body, the first holding the pointer winning. A left press in a zone, its moves and its release go to the panel (`Host::take_pointer_events`), never to the cockpit. A press and release in a body zone that moved no more than 4 px are then handed to the cockpit as a click where they happened. The window shows a grab hand over a body, a resize arrow over a grip and a pointer over the ×, and publishes where the pointer is (`Host::pointer`) for the hover chrome.

## The contract it reads

The cartridge's `CONTRACT.md` (schema `vec.board/1`, cloud `VECCLD01`) is the source. The panel is a reader: it never writes in that folder. Everything there is untrusted input.

- **Polling.** One thread looks at `board.json`'s modification time and length at most once a second. It reads the board only when they change, and never while the panel is hidden.
- **The board** (`campaign::parse_board`).
  - Over 1 MiB, not JSON, another schema, or no boards with a key: no campaign yet.
  - A missing or `null` field is unknown. Fields the contract does not name are ignored. Unknown quest counters are ignored, and a missing counter is 0.
  - Lists are bounded: eight boards, the newest 64 history entries a board, the newest 20 events, and 160 characters of any text.
  - Times are ISO 8601. Scores must be finite numbers.
- **The cloud** is read again only when the board's `cloud.sha256` names a different file. Its bytes must hash to that digest.
  - A file that does not match (being replaced, or a board that is behind) keeps the shown cloud and is looked at again in ten seconds.
  - A malformed file the board names is "no cloud yet". It is not read again until the board names another.
- **The cloud's reader** (`campaign::decode_cloud`) holds the contract's bounds and refuses the file whole, never in part, as the reference decoder (`vec/lib/vec/cloud.py`, `decode`) does:
  - over 16 MiB, or a magic that is not `VECCLD01`;
  - more than 64 groups, 16 stages, or 20,000 points in a stage;
  - a group index out of range;
  - a name that is not UTF-8;
  - a count that runs past the end, or bytes after the last stage.
  - It is stricter in two places the renderer needs: a stage's kind must be 0 or 1, and every coordinate must be finite.
- **Strays.** Cells further than 2.6 from the middle (the cloud is normalized to a 95th-percentile radius of 1) are left out of the meshes.

**The data rule.** The VE data is unpublished and must not be redistributed. Nothing derived from it is in the repository: the tests make their clouds and boards in code, and screenshots of the real cloud stay in `<artifacts>/heartglass/`.

## How it is built

- **Where the code is.**
  - `cockpit/src/ui/heartglass.rs`: the layout, `/heart` and the launch.
  - `heartglass/campaign.rs`: the reader and the watcher.
  - `heartglass/heart.rs`: the meshes, the stage cycle, the scene, the beat, the trace and the flare.
  - `heartglass/plate.rs`: the printed plate, and each floating module's print (glass only).
  - `heartglass/layers.rs`: the thread and the plate's layers (glass only).
  - `heartglass/hud.rs`: the floating layout, the saved places, the hit-test and what a drag does (pure).
  - `heartglass/layers/float.rs`: the floating modules' pictures, halos, layers and hover chrome (glass only).
- **Pure parts.** Everything but the thread and the font is a pure function of its inputs: the reader, the layout, the meshes, the schedule, the scene, the plate's drawing (with text through a `Typeset`) and the clock's strings. They are tested without a window.
- **The heart.**
  - Each stage is cut into 14 horizontal slabs, one uploaded mesh each. A gem is an octahedron of six vertices.
  - A slab's mesh has up to three groups: cardiac cores (lit, emissive 0.82), the other cells' cores (lit, emissive 0.32, dimmed) and cardiac glow shells (unlit, blended at alpha 0.11).
  - The meshes go up once a cloud (a stage of 8,000 cells is about 5 MB on the GPU). They go up again when the panel is shown after hiding, in case the window's budget dropped them.
  - The scan only moves slab nodes, and the beat only scales the root, so each frame sends the scene's nodes and no geometry.
  - The scene is drawn into a picture twice the viewport's size, with a transparent clear, and placed by a leaned layer.
- **The plate.** Everything printed on the plate is one picture, drawn on the CPU at twice its shown size in the window's own font (`angel_glass::page::Pager`), with the chart's canvas (`ui::showcase_chart::Canvas`). It is drawn again only when the board, the cloud or the window's size changes.
  - While the window is being sized, the old plate stretches until the size has held for 250 ms.
  - Text whose font failed to load (fontconfig can fail under load) is drawn again three seconds later, rather than left out for the session.
- **What moves with the clock.**
  - The countdown, the budget's reset and the ticker's age are three small pictures, each sent when its text changes.
  - The stage titles are one picture a stage.
  - Everything that glows, beats or moves is a layer over the plate: the aura, the projector light, scanlines and a sweep, the stage glow, the embers, the trace, the brackets and the rim, motes, and the flare.
  - The layer group is "HART" (`0x48415254`), and the pictures and meshes take ids from `0x4845_0000` in their bands.
- **The thread** describes a frame 30 times a second while the panel shows, as the Orrery's does, and sleeps 150 ms at a time while it is hidden.
- **Floating, the cost** is the plate's. A module's print and halo are drawn on the CPU only when the board, the cloud or the module's size changes (a new size once it has held for 250 ms, or the drag has ended), the halo at one pixel per shown pixel. Moving a module only moves its two pictures. Nothing is blurred per frame.

## Proof

Built on `glass/heartglass` from `dev` at `d0d909766`. RTX 3090, Vulkan, the nested seat (`<artifacts>/glass-test-kit`, window 1576 x 880).

- **Tests.** 32 Heartglass tests in the cockpit (`cargo test --bin angel heartglass`). The plate and GPU modules need the `glass` feature, which is on by default.
  - The reader:
    - a cloud round-trips;
    - each bound is refused at one past it: 65 groups, 17 stages, 20,001 points, a group index of 9 of 9, a kind of 2, a NaN, bad UTF-8, a trailing byte and 16 MiB + 1;
    - every proper prefix of a good file is refused;
    - 6,000 mutated files (flipped bytes, extremes, cuts, inserted and removed runs, lying counts) are all refused or read within every bound, and each accepted one re-encodes to its own length (3,204 accepted, 2,796 refused).
  - The board:
    - the contract's example parses, `ve_partner` included;
    - a board with only a key, nulls, wrong types and a best with no score all read as unknown;
    - nine kinds of non-board and every cut of a real one are no campaign;
    - over-long lists keep their newest;
    - a new official score is noticed once, and a proxy score is not official.
  - The watcher, on a temporary folder: nothing, then a campaign and its cloud; a cloud file changed under an unchanged digest is not read; a board that changes but names the same cloud keeps it; a new digest reads the new cloud; a digest that does not match keeps the shown cloud; a malformed cloud is no cloud and is not read again; a malformed or missing board is no campaign.
  - The layout, in six window sizes from 1100 x 1300 to 3840 x 2160:
    - the leaned plate with its brackets stays inside the window, below the header's rows and above the composer;
    - every part is inside the plate, stacked without overlap;
    - small windows (including 1280 x 720) float none;
    - beside the Orrery the two leaned panels do not touch.
  - The heart:
    - every slab mesh validates, with one core a cell and one glow a cardiac cell;
    - cardiac gems are exactly their lineage's colour and the others dim;
    - strays are left out;
    - the cycle builds the first stage, then scans from each to the next;
    - a scan trades each slab, lower ones first;
    - the scene draws only the stage in front when held, and both while scanning;
    - the trace's spike falls on the beat;
    - a flare's first frame is dark but not over, and it runs eight seconds.
  - The plate, with a stand-in typesetter: its picture is the panel at twice the size; our official bar is solid rose between the floor and our score; the proxy past it is a ghost; the leader is gold above the rail and the team violet below; one budget pip is lit and one hollow; the score range; the countdown ("3d 04:05:06", "ended", "budget resets 2h07m"); text cut to fit with an ellipsis; a predicted stage names its board's tag.
  - `/heart` parses as the built-in ahead of any cartridge's, `/hear` keeps its meaning, and outside the window the toggle changes nothing.
  - The input module's catalog test (`slash_catalog_is_unique_and_help_documented`) passes with `/heart` in `/help`.
- **On the GPU.** `heartglass_gpu__tests.rs` makes its own device, renders through `Core::draw_scene` and reads back. It skips cleanly with no adapter. On the RTX 3090:
  - **Depth.** A red myocardium cell and a cyan endocardium cell sit on one line of sight from the eye. Whichever is listed first, the pixel is the near red (239, 60, 83, 255). The far cyan alone on that pixel is (60, 209, 237, 255). Without depth, one of the two orders would show cyan.
  - **Colours.** An epicardium cell shines (240, 185, 60). A neural cell beside it is there but dim (26, 33, 50). Four pixels from the gold core its glow is a translucent gold (28, 22, 7, 28 premultiplied). The corners stay clear.
  - **A whole synthetic heart.** Held, it covers 10,701 pixels, 446 of them myocardium red. Mid-scan it covers 10,115. The idle ring alone covers 1,204, and its near edge is in view.
- **Screenshots, opened and looked at** (`<artifacts>/heartglass/`). The `final-*` set is from the release build at this commit's code; the rest are from debug builds earlier on this branch.
  - Synthetic board and cloud (made by `tools/synth_vec.py`, numbers invented): `final-synth-held.png` (E8.75 held), `final-synth-ours.png` (our predicted stage, gold), `final-synth-flare.png` (a new official score: the burst, the sparks, the lit HX row, the banner, the ticker), and `final-synth-sheet.png` (the three side by side). Also `synth-with-orrery.png` (beside the Orrery), `state-no-campaign.png` and `state-no-cloud.png`.
  - The real local campaign, read only from `~/.angelX/vec/`, never committed. No real score or leaderboard number is written in this doc or the tests.
    - `final-real-e95.png`: observed E9.5.
    - `final-real-ours.png`: the cartridge's own predicted Gata4 stage, E8.75*. The real HX and G4 official scores, both under the floor, run left from it.
    - `final-real-sheet.png`: the two side by side.
    - `real-scan-sheet.png`: scans between stages.
  - The command in the real app, pasted and entered through the nested seat's own keyboard: `toggle-0-before.png`; `toggle-1-on.png` ("heart · the Heartglass is up"); `toggle-2-off.png` ("heart · the Heartglass is down", the panel gone).
- **Frame time** (`ANGEL_GLASS_STATS=1`).
  - The setup: the practice cockpit and the real board, the same binary off and on, alternating, two rounds of about 26 s each. The first two 2-second windows of each run are dropped. Ranges are over the rest, and n counts 2-second windows.
  - These are the window's own in-process numbers, on a shared machine (load average 12 to 16 from other builds and runners) with a second nested seat running beside this one. Treat them as indicative.
  - Against the idle cockpit:

    | Build | Panel | fps | render p50, median (range) | render p95, median | CPU build, median |
    |---|---|---|---|---|---|
    | release | off | 4.9 | 1.20 ms (0.99–1.32), n=22 | 1.91 ms | 0.09 ms |
    | release | on | 64 | 1.33 ms (1.02–1.50), n=22 | 2.36 ms | 0.41 ms |
    | debug | off | 4.1 | 3.34 ms (2.02–6.80), n=16 | 3.97 ms | 0.99 ms |
    | debug | on | 62 | 3.76 ms (2.61–8.76), n=22 | 5.06 ms | 2.14 ms |

  - An idle window draws only on change, so it is not a like-for-like baseline. With the Orrery up both ways, the window draws every display frame in both, and the Heartglass's own cost shows:

    | Build | Panels | fps | render p50, median (range) | render p95, median | CPU build | present |
    |---|---|---|---|---|---|---|
    | release | Orrery | 64 | 1.01 ms (0.81–1.59) | 2.61 ms | 0.10 ms | 0.98 ms |
    | release | Orrery and Heartglass | 64–72 | 1.96 ms (0.82–2.36) | 3.45 ms | 0.32 ms | 1.54 ms |

  - So the panel costs about 0.2 to 0.3 ms of the window's CPU a frame, and about 1 ms of p50 in all, most of it in present. That is probably the GPU drawing the heart, but it is not measured.
  - It stays under the 4 ms glass p50 reference.
  - With the panel up, the window draws every display frame instead of only on change, as with the Orrery: the panel's layers animate.
  - Picture uploads are the same either way: about 7.6 MB per 2 s, the realm's own. The panel's steady state sends only the clock's small pictures.
- **The reader against the reference decoder.** This is a one-off check, not in the repository, because it runs the cartridge's own Python.
  - 4,000 mutated copies of a synthetic cloud were decoded by both `vec/lib/vec/cloud.py`'s `decode` and `campaign::decode_cloud`.
  - 3,398 got the same verdict, and each accepted file the same group, stage and point counts.
  - 602 were refused only by the Rust reader's two stricter checks (a kind over 1, a coordinate that is not finite).
  - None disagreed.
  - The script and the result are in `<artifacts>/heartglass/tools/diff_fuzz.py` and `differential.txt`.
- **The floating HUD** (built on `glass/heartglass-hud` from `dev` at `a61196ddf`, `--profile fast`, the nested seat, the real local campaign read from a copy of `~/.angelX/vec/`).
  - Tests: 13 more in the cockpit (`heartglass_hud__tests.rs`, and `/heart reset` in `heartglass__tests.rs`) and two in `angel-glass` (`zones.rs`).
    - The designed layout in six windows from 941 x 640 to 3840 x 2160: every module's content in the window, under the header (the strip excepted in a wide window) and over the composer (the ticker excepted, never its input row), no two modules' contents overlapping, the strip in the header's middle when wide.
    - The saved layout: no file, an empty one, JSON of another shape, unknown modules and fields, nulls, a file grown past 64 KiB all give the design; a file naming one field of one module moves only that; an anchor of 7, an offset of 1e30 and a scale of 99 or −4 fall back or are clamped; a layout kept to a file reads back the same, and only what moved is written.
    - The hit-test: each module's body, grip and (once hovered) hide button and tab; nobody's in the transcript's middle; the zones' order.
    - A drag moves only its module, stops at the window's edge and settles on its margin; a click without a drag changes nothing; the grip and Shift resize from the top left corner between their bounds; the hide button hides, and a press on it let go elsewhere does not.
    - The halo is darkest behind the ink, a lighter wash elsewhere on the content, clear in the margin's corners and never over 84%; a module's print is its size at its scale.
    - The plate's three pictures (no campaign, a campaign, a campaign with a cloud) hash exactly as they did before the plate's drawing was split into parts (`the_plate_draws_exactly_as_it_did`).
  - In the window, pasted and clicked through the nested seat's own keyboard and pointer (screenshots in `<artifacts>/heartglass/hud/`, real data, not committed): the designed HUD over a busy transcript at 1600 x 1000 (`final-1600-default.png`); the boards dragged over the transcript, the hover chrome showing (`final-1600-drag.png`); the 941 px narrow window (`final-941-narrow.png`); the heart resized from its grip and its chrome (`hover-chrome-heart.png`); the ticker resized with Shift (`shift-resized-ticker.png`); a click on the ticker, dragged over the header, opening the model menu under it (`click-through-model-menu.png`); the layout restored after the window was closed and opened again (`relaunch-restored.png`); `/heart plate` from the floating look, the plate as it was (`final-plate.png`). `/heart reset` put every module back and removed the file.
  - The halo's strength was set by eye against the transcript. Measured on those two screenshots (the same transcript with the boards away and over it): the text under the boards keeps a median 40% of its brightness, and 26% to 30% next to the HUD's own print, so it reads when looked for.
- **Clippy.** Clean on the new files. The warnings left in `orrery.rs` (`contains` unused, two clamp patterns) were there before.

## Not done

- **GPU time is not measured.** The render times above are the window's CPU side and present.
  - The heart is about 100,000 to 200,000 triangles a frame, a quarter of them blended glow shells. They are drawn into a picture of about 1100 x 650 on every display frame, although the scene changes only 30 times a second.
  - The 1 ms of p50 it adds beside the Orrery is most likely that.
  - Drawing a scene again only when it changed belongs in `angel-gpu`, and is not done.
  - Nothing here was measured on the Mac mini.
- **Power.** The window redraws at the display's rate while the panel shows (4.9 fps idle without it). A panel that redraws only on change (no window-animated layers) would cost less and look less alive.
- **No browser seat.** The browser has no way to receive a scene yet (docs/GLASS_CORE.md), so the Heartglass is the window's only.
- **No board opens.** The floating modules move, resize and hide under the pointer, but a click on a board does nothing, and nothing opens a board's history in full. The plate takes no pointer at all.
- **A hidden module comes back only with `/heart reset`,** which also puts every other module back where it was designed.
- **Cells do not morph between stages.** The stages are unregistered frames of different embryos, so cells have no partners across stages.
- **Predicted colours are the cartridge's.** A predicted stage's lineage colours are the cartridge's nearest-neighbour typing against a reference stage, not measured types.
- **Sizes are in window pixels.** Like the Orrery's, its sizes assume a cell of about 12 x 25 px. On a display with much larger cells, it shows small.
- **The real-data screenshots exist only on this machine.**
- **A cloud that gains a stage can jump the cycle once.** The cycle is a function of the clock and the number of stages.
