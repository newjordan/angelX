# Motion

How things move in the Delve: one foundation every effect is timed on, the
rules of thumb it is used by, and how to film a moment and look at it.

The Delve steps at 30 ticks a second (`rules::HZ`) and every picture is a pure
function of the run. An effect never keeps state between frames: it reads an
age off the run (ticks since a hit, a drop, a grant, a spawn) and turns it into
a pose. That is what lets the host, a friend's mirror and a browser guest draw
the same pixels from the same run, and it is the constraint everything below
is shaped by.

## The foundation: `crate::motion`

`crates/delve/src/render/motion.rs`, mounted at the crate root as
`crate::motion` (it builds without the renderer, so the rules may time by it
too) and re-exported as `render::motion`. The magic kit's `pfx::ease` and
`pfx::Beats` are this module's.

- **Eases** (`ease::*`, or `Ease` by name): `linear`, `inward` (cubic in),
  `out` (cubic out), `both` (smoothstep), `in_quad`, `out_quad`, `back` (an
  eighth past the mark and back; `back_by` with your own overshoot),
  `anticipate` (a tenth the other way first), `spring` (rings once), `elastic`
  (a quarter past, a little under, still), `bounce` (three hops), `bump` (up and
  down), `Ease::Step` (a cut). All clamp their input; all start at 0 and end
  at 1.
- **Beats** (`Beats::new(anticipation, action, follow, settle)`, in ticks):
  an effect's beat sheet as data. `at(age)` says which beat and how far
  through it; `action_at`, `follow_at`, `settle_at` are the marks;
  `since_action` counts from the action; `size`, `power` and `pose` are the
  stock curves (drawn in, thrown past full, settled; bright, held, gone;
  pulled back, thrown past, home).
- **Tracks** (`track(&[key(at, value, ease), ...], age)`): a value keyed
  through time, each key reached along its own ease. Two keys with the same
  value are a hold.
- **Springs** (`Spring { hz, damping }`, `Spring::SNAPPY/SOFT/WOBBLY`):
  stepped four times a tick from a known start, so `at(age, from, to, v0)` is
  the same pose on every seat at every age. `kick(age, v)` is a body at rest
  struck: a jiggle, a sway.
- **Shape and step**: `squash(k)` (area kept: above 0 wide and low, below 0
  tall and thin), `arc(a, b, height, t)` (a thrown arc), `steps(t, n)` and
  `on(age, every)` (pixel art's held poses).
- **Shake and impact**: `shake(tick, left, full, reach)` (the camera's, from
  its countdown: trauma squared, two waves an axis), `impact(age, hold, fall)`
  (a hit's weight: held, then falling away), `flash(age, hold, blink)`.

It is the same bits on every machine: only `+ - * /`, `sqrt`, `floor` and
`round`, and its own `math::sin`, `math::cos`, `math::exp` in place of the
platform's (which may differ in the last place between a native host and a
wasm guest). `motion::tests::the_same_bits_on_every_machine` pins a hash of
every value the module makes; if it fails on a new machine, that machine
computes motion differently and its pictures will not match the host's.

## Rules of thumb

- **Time it in ticks, as beats.** Write the beat sheet first (`Beats`, or a
  `track` of keys) and make every layer start and end on a beat. A tick is
  33 ms; a pose a player reads needs two or more.
- **Fit the effect inside what the run keeps.** An effect is drawn only while
  the run remembers its moment: a boon for `HZ` ticks, an impact for
  `IMPACT_TICKS`, a glint for `GLINT_TICKS`. Design the beats to end there; an
  effect longer than its record is cut off mid-motion (the wish grant was, at
  half its length, before 2026-10-29).
- **Anticipation where the rules allow it, impact where they do not.** Never
  delay a hit the rules have already landed: a blow's first frame is its
  widest, and the hit-stop holds on it. Anticipation belongs to what the rules
  telegraph anyway (a wind-up, a wish being made, a guardian coming in).
- **Overshoot, then settle.** Openings, drops and pop-ins go past their mark
  and back (`back`, `spring`, a `track` with a key past the rest). Nothing
  arrives at full size on its first frame unless it is an impact.
- **Exits are motion too.** Lift and dither away (`notices::board_in`, the
  parry's word); never vanish on a frame, never blink to leave.
- **Hold to read.** A word, a name, a number holds still long enough to read
  (half a second and up) between its entrance and its exit.
- **Squash and stretch keep area,** and only on bodies the rules do not
  measure by their picture: a monster's sprite squashes on a blow
  (`figures::hit_squash`), its feet planted; where it stands and what can
  strike it do not move.
- **Secondary motion follows the primary by a tick or two:** an echo ring a
  beat behind the ring, a name typed after its banner unrolls, a key line
  after its board.
- **Readable at 1x.** Look at it at native size before at three times. Thin
  lines dither on and off; fills read; single pixels flicker.
- **Picture only.** Motion never moves a hitbox, a countdown or anything on
  the wire. If a moment needs a clock the run does not keep (a room's entry,
  a run's winning), it waits for the rules to keep one; it does not invent
  one.

## Filming a moment

The motion sheet films every moment that moves, a tick at a time, as a seat
sees it:

    ANGEL_MOTION_SHEET=<scratch>/motion cargo test --manifest-path crates/Cargo.toml \
        -p angel-delve --profile fast write_motion_sheet -- --ignored

(Run it under `systemd-run` as the build rules say.) Each moment writes
`<name>.gif`, three times native size at 30 ticks a second (a hit-stop shows
as frames that repeat), and `<name>.png`, a contact sheet at twice native size
with each frame's tick under the press. `ANGEL_MOTION_ONLY=swing,pickup` films
only the moments whose names start so.

The moments are in `crates/delve/tests/render/arena__motion_sheet.rs`,
`moments()`: each is a room set up for it (`room(kit)`, `foe`, `witness`), a
`roll` of ticks through a `Lens` (a window fixed to the room, the whole view,
or the whole seat screen with its bands) and the presses or changes on each
tick. To add one, push a name and a closure that sets the scene and rolls it.

Film before you change anything, change, film again to a second folder, and
look at both: every frame, at 1x and zoomed, before calling it done.

## The moments rebuilt on 2026-10-29

Filmed before and after with the sheet (`<scratch>/motion-system/{before,after}`
that night): the ten weakest of 27, each rebuilt on the foundation.

1. **A wish granted** (`flourishes::flourishes`): one second's beats inside the
   boon's own second — a breath of light drawn in, a white core, rays past
   their reach, a ring overshooting with an echo inside it, twinkles floating
   off — where a sixty-tick ring used to be cut off at thirty.
2. **A room's signs** (A WISH, SANCTUARY, `notices::board_in`): dropped in past
   their rest and dithered on, the key line a beat behind coming up from under,
   lifted and dithered away at the end, where they used to pop on and cut off.
3. **A take's word** (TOOK ..., a friend's name, `marks::takes`): pops up into
   place past its line and back, holds, and lifts away through the dither.
4. **The parry's word** (`melee::word`): the letters spring up in a quick wave
   past their line, drift, and lift away through the dither, where the word
   used to sit still and blink out.
5. **A find dropped** (`figures::drop_lift`): thrown up, down, one small hop,
   still, its shadow on the floor, where it used to appear lying.
6. **A find taken** (`vfx::glints`): the find flies up off the floor into the
   knight along an arc, turning edge-on, and a ring snaps out round them.
7. **A guardian's entrance** (`wanted::entrance`, `notices::boss_title`): its
   banner unrolls, its name types on, its bar fills from empty with a lit
   edge, and GUARDIAN draws out over the view with its name and lifts away,
   where it used to simply stand there.
8. **A shot's hit** (`vfx::impacts`, `figures::hit_squash`): the star held a
   second tick, the ring eased open, sparks thrown further, and the monster
   squashed on its feet by the blow.
9. **A blow on a knight, in the HUD** (`party::struck_card`): the health left
   flashes white, the card's rim burns red and cools, where the bar used to
   just change.
10. **A monster coming in** (`figures::telegraph`): the spawn ring closes in
    as a countdown, lit at its end, and is thrown out as the monster wakes,
    where it used to be a still ring that vanished.

Moved onto the shared eases as straight swaps: the camera's shake, the take
label's spring, the guardian's floor rings, the floor results' count, gore's
stains, the Deep's folk and a spray's drag.

## What the sheet shows is still weak

- **A room's door** is a cut: the run keeps no tick of a room's entry, so a
  transition has nothing to key on. It wants `Run::entered` (a tick, on the
  wire) before it can have an iris or a wipe.
- **A finished run's verdict** (VICTORY, FALLEN) stands still: a wiped party's
  clock stops, and a won run keeps no tick of its winning. It now stands clear
  of the knights and the bones; its entrance waits on a tick to key on.
- **A knight falling** turns to bones in one frame (the bones then come apart
  well), and **a struck knight's** ward blinks the whole figure off and on.
  Both are the knight's own drawing (the knights' rig lane's).
- **A sword's slash** is at its widest on its first frame by design (the
  frame a hit-stop holds); its last tick leaves a sliver low on the sweep.
- **A level gained** has no moment in the run yet (the reward-moments lane).
