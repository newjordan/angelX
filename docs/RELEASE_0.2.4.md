# angelX 0.2.4

The Delve and the kingdom are redrawn in one lit, rounded hand: a knight in
cool steel, boxes and bars with rounded corners, foes and scenery with lit
materials, and 3D rooms that shade softly into black. The browser seat gains a
card sheet, wishes and a volume. A new `daybreak` seat joins the ChatGPT
account's routes, cartridges can carry slash commands, and the harness keeps
its provider connections and stops retrying what cannot succeed.

## Upgrade

```sh
angelX update
```

Or install fresh with the one line in the [README](../README.md). From a source
checkout, `git pull` and run `./bin/angelX` again; rustup adds the wasm target
the browser client is built with.

## Connecting a model

- **`/setup`.** A full-screen menu connects a model. What this computer already
  has (a model server on localhost such as Ollama, LM Studio, llama.cpp or vLLM,
  an API key in the environment, a ChatGPT or Grok login) is listed first and
  marked ready. The rest is "Sign in with ChatGPT / Grok", "Paste an API key",
  "Use a free model on this computer" and "Not now". Arrows or a number and
  Enter pick, Esc goes back, and the mouse clicks rows.
- **A recruit shows the way.** The menu is written on a torch-lit scroll, and a
  recruit in a cream gi and a bucket helm stands beside it: he waves hello,
  points at the choices, heaves a key in when one is pasted, hauls while a
  sign-in runs, stumbles over a refused key and gives a thumbs-up when it is
  done. Kitty draws him as a picture; other terminals get half-blocks.
- **Checked before it is saved.** A pasted key is checked with the provider
  before it is saved, a provider without a default model lists its models with
  type-to-search, and a plan sign-in waits for the browser (Ctrl+O reopens its
  link). A missing plan CLI can be installed from the menu.
- **No restart.** The choice is written to `~/.angelX/angel.env` as before, and
  the new model takes over once the cockpit is idle.
- **First launch.** With nothing configured, the first launch opens the menu,
  and `angelX setup` does the same; `angelX setup --text`, or input that is
  not a terminal, keeps the plain questions, and `ANGEL_SETUP=0` turns the
  first-launch menu off.
- **The installer sets the PATH.** The one-line install adds `~/.local/bin` to
  the shell's startup file itself, once, and says to open a new terminal
  window. `ANGELX_NO_MODIFY_PATH=1` prints the line instead.

## A new look for the Delve

- **The knight.** The hero is cool lit steel, seen three ways, and walks in four
  steps with what he holds and strikes with. A cool rim of sky lights the
  shadowed side of his helm and shoulder, a cream glint keys the helm, and a
  shadow sits under every knight. The classes keep their looks, each material on
  its own lit ramp, and the Swordsman wears a cape. The same steel shows at the
  lists, in the saddle, in the chariot cab and at the pit stop, and a roll tucks
  into a ball. The chariot teams are lit from above, and the hats won and lost
  are lit like the knight.
- **Boxes drawn by one hand.** Place plaques and name tags, the `E` prompts, the
  ENTER band, boards, the ballot, the privy window, the tray, meters and menu
  highlights share one rounded, softly lit frame: a lit rim along the top,
  slate falling toward black, and a small shadow down and right. The floor
  plates a knight stands on, the signs carried by props, the college's room
  titles and the tapestry's bays follow. The Delve's frames in the terminal
  round their corners too.
- **Health bars and names.** Every monster's and knight's health bar is rounded
  with a lit rim, and a boss's name sits on the same steel-rimmed plate the
  realm's names use.
- **Boards keep out of each other's way.** Fortune's dare settles under a
  folly's standings instead of on them, the tavern's song board stays clear of
  the tray, and the kingdom's door footer hides while What's New is open.
  Names, taunts and prompts stay inside each seat's own view, so a boss's name
  at the edge of a wide hall no longer runs off one player's screen.
- **Foes.** Foes have rounder forms and material depth, keep their textures,
  and keep their colours through a hit. The dragon regains its shape.
- **Effects and icons.** Bursts have a warm core, a hit tints a foe pale
  instead of flashing a white silhouette, smoke is soft, and card and item
  icons have shaded forms.
- **Smaller changes.** The Powder Room's crates are rounded and lit, Book Hop
  has one river with drifting ripples and rounded, gilt-banded books, the
  tavern floor is oak boards that each keep their own tone, and the lists'
  crowd has round heads under hair, hoods and caps.

## The kingdom's scenery and the 3D rooms

- **Trees and stones.** Trees and stones read as lit volumes. The woods at a
  room's edge are trees of different sizes, habits and hues, and the kingdom's
  trees grow in sixteen habits. Stone blocks wear round at their outer corners
  and joints.
- **Roofs and buildings.** Roofs catch the light on their west hips and round
  off at ridge and eave, village cottages break their roofs four ways, and
  buildings part from the ground on a coloured edge instead of black. The
  ranch's barn roof is a lit volume.
- **Fences, sites and stalls.** Fences are split rails, lit on top and sagging
  a little between squared posts. Building sites are staked with a cord that
  sags. The market's stalls each keep their own canvas, and an offended stall
  shutters up in planks. The hedges are clipped round, and the laundry's vats
  are coopered tubs.
- **Water and ground.** Lakes, ponds and rivers wander at their banks and round
  at their corners. Bogs are dark pools and thick rushes, and the Mines' and the
  Archive's standing water is open water with wandering banks. A crypt pit shows
  its far wall going down into the dark. Sown fields, flower fields and the
  scarecrow's crop grow in tufted rows. Roads are pebbles in loose clusters over
  trodden dust. Dungeon floors are marked in small lit chips gathered in
  patches, and ranch grass grows in lit clumps.
- **Soft light.** Light gives way between its steps in soft blots instead of a
  checker. Firelight near a flame warms stone a step lighter and never into
  soot.
- **The 3D rooms.** The realm's 3D rooms have softer light and quieter surfaces,
  flat planes carry their value in colour instead of a checker, and haze falls
  into the terminal's black instead of a lit rectangle. Rooms keep their leaded
  windows, shaped flames and green forests.
- **Races and stages.** The race halls, the duck range and the half-pipe shade
  each surface in soft steps and fade to black as they recede. The duck range's
  counter is a rounded oak lip over staggered boards, and the half-pipe's launch
  window is shaded brass and gold planks.
- **The map.** The scrying glass and the map's signs are rounded, and night
  black keeps the dusk from washing them flat.
- **Lighter to draw.** Still floor marks, hall flags, the map's buildings,
  headstones and water are drawn once and kept, and a room with no water spends
  nothing on drawing it. A seat's race frame is quicker than before.

## In the browser

- **A card sheet on Tab.** Tab shows the knight's arm, held cards, hand and
  spells, what they add up to, whether a wish is open, and every wish the scroll
  knows with this knight's level on it.
- **Wishes from the page.** T opens a line on the page; Enter sends it and Esc
  lets go. A wish the phrasebook knows is granted at once, and a new one is
  learned. A game names its text line in its controls, so other games can use
  the same line.
- **A volume.** The page has a note that mutes, a slider beside the full-screen
  button, and `-` and `=` keys to step it. The setting is kept in the browser.
- **Quieter notes.** Pop-up notes are one small line at the picture's top edge,
  off the action: 11 pixels, cut with an ellipsis, up for two and a half
  seconds, and a repeat only keeps the line up.
- **The game's slate.** The join form, buttons, status line, toasts, stats box
  and dev panel take the game's slate, in tonal greys and cream.

## Below ground

- **Three new cards.** Fork (rare) sends one shaft in and two out the other
  side: a shot forks past what it stops on. Awakened Fork (a relic) splits, and
  what it splits into splits again. Fart Bombs (rare) leave a short sulphur
  cloud where a hit lands. They are drops and chest finds, and wishes reach
  them too: a wish for forking shots grows from Fork to Awakened Fork, and a
  wish for fart clouds grows through three levels.
- **Friends and foes closer to now.** Everyone but your own knight is drawn
  50 ms behind the host instead of 100, so other knights and monsters move
  nearer to when they happen.
- **A guide.** [MINIGAME-BUILD.md](https://github.com/newjordan/angelX/blob/main/docs/MINIGAME-BUILD.md) explains how to build a
  mini-game or a folly: the feel brief first, then the verb, one room, the
  challenge, the juice and the podium.

## Seats and cartridges

- **The Daybreak seat.** The `daybreak` route serves the ChatGPT account's
  models under OpenAI's Daybreak program, each request carrying the access
  program the account's catalog lists. It uses the same ChatGPT login and startup
  catalog as the `openai` route, and it appears only when that catalog lists a
  Daybreak program for one of the account's models. It does not grant or bypass
  account eligibility.
  - Select it with `ANGEL_DRIVER=daybreak` or `--driver daybreak`. `/connect
    daybreak` explains the setup, and `/model` and `/think` work as on the
    `openai` route.
  - `ANGEL_DAYBREAK_MODEL` and `ANGEL_DAYBREAK_REASONING_EFFORT` pin the
    starting model and effort. An invalid pin is reported as the route's
    selection error and never replaced silently.
  - The seat is never part of automatic fallback.
- **Slash commands in cartridges.** A `[[commands]]` entry in a cartridge names
  a word and a stack of skills from its `skills/` folder, and
  `/<word> <task>` sends one turn with those skills, as `/skills` does. Until
  it is typed the cartridge costs nothing: no process starts, no tool joins the
  registry, and its skills stay out of the catalog. A built-in command of the
  same name wins. See [CARTRIDGES.md](CARTRIDGES.md).
- **REA as `/rea`.** [`examples/cartridges/rea`](examples/cartridges/rea/) plugs
  in [REA](https://github.com/morluto/rea) (Reverse Engineer Anything, MIT),
  which reverse engineers native binaries, JavaScript and Electron apps, .NET
  assemblies, APKs, firmware and websites. Its MCP server carries 207 tools, so
  the cartridge drives REA's command line instead and each turn stays small.
  `install.sh` copies the cartridge to `~/.angelX/cartridges/rea`, installs the
  pinned `rea-agents` 6.5.0 beside it (Node.js and npm are required) and takes
  REA's own skill from that package. `/rea <target> <what to find or build>`
  runs a turn, and removing the folder unplugs it.

## The harness

- **Connections kept.** A streamed model call hands its connection to the next
  one, so a call no longer opens a new TCP and TLS connection and a DNS lookup
  of its own. Esc still cuts a live stream at once, and a cut socket is never
  reused. The ChatGPT account, OpenAI API and Muse seats do the same. On Linux
  streamed calls reuse their connection; elsewhere each keeps its own, as
  before.
- **Idle connections.** A connection the provider closes while idle costs the
  next call nothing: the dead socket is replaced, and a request whose reused
  connection dies before a single response byte is sent once more on a fresh
  one. Esc during the resend still ends the call.
- **A quicker start.** A headless task starts its seat's window probe at
  startup, so the first model call finds the connection already open.
- **No retry for what cannot succeed.** A provider that answers an error with
  `x-should-retry: false` is not asked again, and the error shows the header
  after the status. A certificate the trust store rejects surfaces at once as a
  provider error naming the certificate problem. A bad endpoint URL, an unknown
  scheme, a plain-http request where only https is allowed, and a proxy that is
  badly specified or refuses the credentials surface after one attempt, marked
  as a transport configuration error. Refused connections, resets and DNS
  failures still retry as before.
- **`run_tests` reads the runner a model names.** It takes the runner strings
  its description lists (`npm test`, `npx jest`, `cargo test`,
  `python3 -m pytest`, `node --test`), an `args` string that repeats the runner
  command, and an absolute `dir` inside the workspace. A model that names pytest
  gets pytest with its own arguments, and on the Cargo route `args: test` is the
  subcommand.
- **A red tail keeps its first failure.** When a routed test run fails and the
  model's own `| tail -N` would cut every diagnostic line, the first failure
  block (30 lines) is kept ahead of the tail it asked for. Green runs and tails
  that already show a failure are unchanged.
- **Paths spelled from the launch directory.** With a relative workspace such as
  `--workspace ws/bowling`, file-tool paths, a leading `cd ws/bowling` in the
  shell and the verifier's `cd` resolve to the workspace, unless the workspace
  has a real entry of that name. A write spelled that way is checked where it
  landed, not reported as a missing file.
- **Cache breakpoints behind a gateway.** A model on OpenRouter that takes
  explicit cache breakpoints keeps them when a proxy or relay sits in front of
  OpenRouter, so resent prefixes no longer pay full price.
