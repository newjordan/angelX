import Base
import ./sl.bend as S
import ./math.bend as M

# The session's header slots (see rs.bend for the whole layout):
#
#   0..18     elapsed, the Dimming, the posts lit, how many sat down, the warden's
#             shrug, the bell (a flag and its tick), paid, again, and the tally
#             (a flag, then its nine numbers)
#   19..30    each knight's tally score (19 + id - 1), and a flag that it has one
#   31..35    the five numbers the host hands in each tick
#   36, 37..  how many sounds the tick made, and the sounds
#   106       how many foes there are; 108 is the sparks counter a tick keeps
#   112..117  which knights are here and awake (111 + id)
#   120..155  what each knight pressed: run, down, aim down, interact, dash, fire
#   160..189  where everyone stood at the start of a tick: five slots a knight
#   192..197  a bot's settings (bot.bend)

def elapsed(+t: S.Sl) -> U32:
  S.g(t, 0)

def set_elapsed(+t: S.Sl, +v: U32) -> S.Sl:
  S.p(t, 0, v)

def dim(+t: S.Sl) -> U32:
  S.g(t, 1)

def set_dim(+t: S.Sl, +v: U32) -> S.Sl:
  S.p(t, 1, v)

def posts(+t: S.Sl) -> U32:
  S.g(t, 2)

def set_posts(+t: S.Sl, +v: U32) -> S.Sl:
  S.p(t, 2, v)

def provisioned(+t: S.Sl) -> U32:
  S.g(t, 3)

def set_provisioned(+t: S.Sl, +v: U32) -> S.Sl:
  S.p(t, 3, v)

def warden_inv(+t: S.Sl) -> U32:
  S.g(t, 4)

def set_warden_inv(+t: S.Sl, +v: U32) -> S.Sl:
  S.p(t, 4, v)

def bell_on(+t: S.Sl) -> U32:
  S.g(t, 5)

def set_bell_on(+t: S.Sl, +v: U32) -> S.Sl:
  S.p(t, 5, v)

def bell_t(+t: S.Sl) -> U32:
  S.g(t, 6)

def set_bell_t(+t: S.Sl, +v: U32) -> S.Sl:
  S.p(t, 6, v)

def paid(+t: S.Sl) -> U32:
  S.g(t, 7)

def set_paid(+t: S.Sl, +v: U32) -> S.Sl:
  S.p(t, 7, v)

def again(+t: S.Sl) -> U32:
  S.g(t, 8)

def set_again(+t: S.Sl, +v: U32) -> S.Sl:
  S.p(t, 8, v)

def tally_on(+t: S.Sl) -> U32:
  S.g(t, 9)

def set_tally_on(+t: S.Sl, +v: U32) -> S.Sl:
  S.p(t, 9, v)

def t_at(+t: S.Sl) -> U32:
  S.g(t, 10)

def set_t_at(+t: S.Sl, +v: U32) -> S.Sl:
  S.p(t, 10, v)

def t_time(+t: S.Sl) -> U32:
  S.g(t, 11)

def set_t_time(+t: S.Sl, +v: U32) -> S.Sl:
  S.p(t, 11, v)

def t_sparks(+t: S.Sl) -> U32:
  S.g(t, 12)

def set_t_sparks(+t: S.Sl, +v: U32) -> S.Sl:
  S.p(t, 12, v)

def t_chain(+t: S.Sl) -> U32:
  S.g(t, 13)

def set_t_chain(+t: S.Sl, +v: U32) -> S.Sl:
  S.p(t, 13, v)

def t_posts(+t: S.Sl) -> U32:
  S.g(t, 14)

def set_t_posts(+t: S.Sl, +v: U32) -> S.Sl:
  S.p(t, 14, v)

def t_falls(+t: S.Sl) -> U32:
  S.g(t, 15)

def set_t_falls(+t: S.Sl, +v: U32) -> S.Sl:
  S.p(t, 15, v)

def t_points(+t: S.Sl) -> U32:
  S.g(t, 16)

def set_t_points(+t: S.Sl, +v: U32) -> S.Sl:
  S.p(t, 16, v)

def t_rank(+t: S.Sl) -> U32:
  S.g(t, 17)

def set_t_rank(+t: S.Sl, +v: U32) -> S.Sl:
  S.p(t, 17, v)

def t_best(+t: S.Sl) -> U32:
  S.g(t, 18)

def set_t_best(+t: S.Sl, +v: U32) -> S.Sl:
  S.p(t, 18, v)

# ---- the sounds a tick makes -------------------------------------------------
# Slot 36 counts them and 37.. holds them in the order they were made; a sound
# is the index of its name in `SOUNDS` (crates/delve/src/rules/lightgallop.rs).

def snd(+t: S.Sl, +code: U32) -> S.Sl:
  +n = S.g(t, 36)
  S.pks((n < 63 : U32), S.p(S.p(t, (37 + n : U32), code), 36, (n + 1 : U32)), t)

def snd_if(+t: S.Sl, +c: Bool, +code: U32) -> S.Sl:
  S.pks(c, snd(t, code), t)

def k_step_wood() -> U32:
  0

def k_rock_land() -> U32:
  1

def k_roll() -> U32:
  2

def k_shot_spark() -> U32:
  3

def k_engine_whistle() -> U32:
  4

def k_spell_fire() -> U32:
  5

def k_step_gravel() -> U32:
  6

def k_wheel_spin() -> U32:
  7

def k_knight_hit() -> U32:
  8

def k_swing() -> U32:
  9

def k_kill() -> U32:
  10

def k_gold() -> U32:
  13

def k_spoil_pickup() -> U32:
  14

def k_boss_death() -> U32:
  16

def k_boss_roar() -> U32:
  17

def k_shield_block() -> U32:
  18

def k_hero_hurt() -> U32:
  19

def k_knight_fall() -> U32:
  20

def k_potion() -> U32:
  21

def k_wall_up() -> U32:
  22

def k_vent_fire() -> U32:
  23

def k_circle_toll() -> U32:
  24

def k_shield_up() -> U32:
  25

def k_chest_open() -> U32:
  26

# The sounds of a chain of lance-locks, rising a step with each link.
def k_chain(+n: U32) -> U32:
  M.pk(11, M.pk(12, M.pk(13, M.pk(14, 15, M.eq(n, 3)), M.eq(n, 2)), M.eq(n, 1)), M.eq(n, 0))

# The five numbers the host hands in each tick.
def slope_of(+t: S.Sl) -> U32:
  S.g(t, 31)

def loop_min_of(+t: S.Sl) -> U32:
  S.g(t, 32)

def spur_cap_of(+t: S.Sl) -> U32:
  S.g(t, 33)

def lock_reach_of(+t: S.Sl) -> U32:
  S.g(t, 34)

def dimming_of(+t: S.Sl) -> U32:
  S.g(t, 35)
