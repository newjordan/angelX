# crates/delve/src/rules/griffinwing/rail.rs — the rail Griffinwing flies: three
# beats laid end to end. The sim reads only the beats' lengths and origins; the
# canyon's pose (Pose, Rail, trig.rs's deterministic sine, cosine and arctangent,
# f64 arithmetic) is the stage's, drawing only, and is not ported.

# How long the first two beats fly, in ticks: they run to the clock.
def griffinwing.rail.LEN(+i: U32) -> U32:
  Bool.pick(U32, U32.is_eq(i, 0), 1800, 2400)

# Where the Wyrm rises in the third beat: its checkpoint.
def griffinwing.rail.BOSS_AT() -> U32:
  480

# A beat's first point on the rail, as fixed point: [0, 1800 * PACE, (1800 + 2400) * PACE].
def griffinwing.rail.ORIGIN(+i: U32) -> U32:
  Bool.pick(U32, U32.is_eq(i, 0), 0, Bool.pick(U32, U32.is_eq(i, 1), (1800 * griffinwing.types.PACE() : U32), (4200 * griffinwing.types.PACE() : U32)))

# The beat a point on the rail falls in.
def griffinwing.rail.beat_of(+s: U32) -> U32:
  Bool.pick(U32, gw.ge(s, griffinwing.rail.ORIGIN(2)), 2, Bool.pick(U32, gw.ge(s, griffinwing.rail.ORIGIN(1)), 1, 0))

# The third beat's canyon is long enough that its fight never runs out of it.
def griffinwing.rail.FIGHT_LEN() -> U32:
  (3 * 60 * 30 : U32)
