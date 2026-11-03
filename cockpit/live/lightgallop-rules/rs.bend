import Base
import ./sl.bend as S

# The slots of the ride's state. The host (index.js) lays the session out in the
# same order; a mismatch here is a different game, which the parity test finds.
#
#   0..255     the session: its header, the numbers, the sounds, the keys
#   256..287   when each lantern post, spring and boost pad last fired
#   288..767   the foes: four slots each (hits left, smoke, flinch)
#   768..1919  the seats, 192 slots each, for knights 1 to 6
#   a seat:    56 named slots, then 24 slots of spark bits (as halves of u64),
#              then room for 16 loose sparks of five slots each

def x(+t: S.Sl, +b: U32) -> U32:
  S.g(t, (b + 0 : U32))

def set_x(+t: S.Sl, +b: U32, +v: U32) -> S.Sl:
  S.p(t, (b + 0 : U32), v)

def y(+t: S.Sl, +b: U32) -> U32:
  S.g(t, (b + 1 : U32))

def set_y(+t: S.Sl, +b: U32, +v: U32) -> S.Sl:
  S.p(t, (b + 1 : U32), v)

def vx(+t: S.Sl, +b: U32) -> U32:
  S.g(t, (b + 2 : U32))

def set_vx(+t: S.Sl, +b: U32, +v: U32) -> S.Sl:
  S.p(t, (b + 2 : U32), v)

def vy(+t: S.Sl, +b: U32) -> U32:
  S.g(t, (b + 3 : U32))

def set_vy(+t: S.Sl, +b: U32, +v: U32) -> S.Sl:
  S.p(t, (b + 3 : U32), v)

def ground_on(+t: S.Sl, +b: U32) -> U32:
  S.g(t, (b + 4 : U32))

def set_ground_on(+t: S.Sl, +b: U32, +v: U32) -> S.Sl:
  S.p(t, (b + 4 : U32), v)

def ground_path(+t: S.Sl, +b: U32) -> U32:
  S.g(t, (b + 5 : U32))

def set_ground_path(+t: S.Sl, +b: U32, +v: U32) -> S.Sl:
  S.p(t, (b + 5 : U32), v)

def ground_s(+t: S.Sl, +b: U32) -> U32:
  S.g(t, (b + 6 : U32))

def set_ground_s(+t: S.Sl, +b: U32, +v: U32) -> S.Sl:
  S.p(t, (b + 6 : U32), v)

def gsp(+t: S.Sl, +b: U32) -> U32:
  S.g(t, (b + 7 : U32))

def set_gsp(+t: S.Sl, +b: U32, +v: U32) -> S.Sl:
  S.p(t, (b + 7 : U32), v)

def dir(+t: S.Sl, +b: U32) -> U32:
  S.g(t, (b + 8 : U32))

def set_dir(+t: S.Sl, +b: U32, +v: U32) -> S.Sl:
  S.p(t, (b + 8 : U32), v)

def roll(+t: S.Sl, +b: U32) -> U32:
  S.g(t, (b + 9 : U32))

def set_roll(+t: S.Sl, +b: U32, +v: U32) -> S.Sl:
  S.p(t, (b + 9 : U32), v)

def ball(+t: S.Sl, +b: U32) -> U32:
  S.g(t, (b + 10 : U32))

def set_ball(+t: S.Sl, +b: U32, +v: U32) -> S.Sl:
  S.p(t, (b + 10 : U32), v)

def crouch(+t: S.Sl, +b: U32) -> U32:
  S.g(t, (b + 11 : U32))

def set_crouch(+t: S.Sl, +b: U32, +v: U32) -> S.Sl:
  S.p(t, (b + 11 : U32), v)

def skid(+t: S.Sl, +b: U32) -> U32:
  S.g(t, (b + 12 : U32))

def set_skid(+t: S.Sl, +b: U32, +v: U32) -> S.Sl:
  S.p(t, (b + 12 : U32), v)

def charge(+t: S.Sl, +b: U32) -> U32:
  S.g(t, (b + 13 : U32))

def set_charge(+t: S.Sl, +b: U32, +v: U32) -> S.Sl:
  S.p(t, (b + 13 : U32), v)

def spur(+t: S.Sl, +b: U32) -> U32:
  S.g(t, (b + 14 : U32))

def set_spur(+t: S.Sl, +b: U32, +v: U32) -> S.Sl:
  S.p(t, (b + 14 : U32), v)

def gauge(+t: S.Sl, +b: U32) -> U32:
  S.g(t, (b + 15 : U32))

def set_gauge(+t: S.Sl, +b: U32, +v: U32) -> S.Sl:
  S.p(t, (b + 15 : U32), v)

def lock_on(+t: S.Sl, +b: U32) -> U32:
  S.g(t, (b + 16 : U32))

def set_lock_on(+t: S.Sl, +b: U32, +v: U32) -> S.Sl:
  S.p(t, (b + 16 : U32), v)

def lock_target(+t: S.Sl, +b: U32) -> U32:
  S.g(t, (b + 17 : U32))

def set_lock_target(+t: S.Sl, +b: U32, +v: U32) -> S.Sl:
  S.p(t, (b + 17 : U32), v)

def lock_ticks(+t: S.Sl, +b: U32) -> U32:
  S.g(t, (b + 18 : U32))

def set_lock_ticks(+t: S.Sl, +b: U32, +v: U32) -> S.Sl:
  S.p(t, (b + 18 : U32), v)

def lock_cd(+t: S.Sl, +b: U32) -> U32:
  S.g(t, (b + 19 : U32))

def set_lock_cd(+t: S.Sl, +b: U32, +v: U32) -> S.Sl:
  S.p(t, (b + 19 : U32), v)

def spring_cd(+t: S.Sl, +b: U32) -> U32:
  S.g(t, (b + 20 : U32))

def set_spring_cd(+t: S.Sl, +b: U32, +v: U32) -> S.Sl:
  S.p(t, (b + 20 : U32), v)

def freeze(+t: S.Sl, +b: U32) -> U32:
  S.g(t, (b + 21 : U32))

def set_freeze(+t: S.Sl, +b: U32, +v: U32) -> S.Sl:
  S.p(t, (b + 21 : U32), v)

def shake(+t: S.Sl, +b: U32) -> U32:
  S.g(t, (b + 22 : U32))

def set_shake(+t: S.Sl, +b: U32, +v: U32) -> S.Sl:
  S.p(t, (b + 22 : U32), v)

def lunge(+t: S.Sl, +b: U32) -> U32:
  S.g(t, (b + 23 : U32))

def set_lunge(+t: S.Sl, +b: U32, +v: U32) -> S.Sl:
  S.p(t, (b + 23 : U32), v)

def lunged(+t: S.Sl, +b: U32) -> U32:
  S.g(t, (b + 24 : U32))

def set_lunged(+t: S.Sl, +b: U32, +v: U32) -> S.Sl:
  S.p(t, (b + 24 : U32), v)

def chain(+t: S.Sl, +b: U32) -> U32:
  S.g(t, (b + 25 : U32))

def set_chain(+t: S.Sl, +b: U32, +v: U32) -> S.Sl:
  S.p(t, (b + 25 : U32), v)

def best_chain(+t: S.Sl, +b: U32) -> U32:
  S.g(t, (b + 26 : U32))

def set_best_chain(+t: S.Sl, +b: U32, +v: U32) -> S.Sl:
  S.p(t, (b + 26 : U32), v)

def sparks(+t: S.Sl, +b: U32) -> U32:
  S.g(t, (b + 27 : U32))

def set_sparks(+t: S.Sl, +b: U32, +v: U32) -> S.Sl:
  S.p(t, (b + 27 : U32), v)

def shield(+t: S.Sl, +b: U32) -> U32:
  S.g(t, (b + 28 : U32))

def set_shield(+t: S.Sl, +b: U32, +v: U32) -> S.Sl:
  S.p(t, (b + 28 : U32), v)

def hurt(+t: S.Sl, +b: U32) -> U32:
  S.g(t, (b + 29 : U32))

def set_hurt(+t: S.Sl, +b: U32, +v: U32) -> S.Sl:
  S.p(t, (b + 29 : U32), v)

def ctrl(+t: S.Sl, +b: U32) -> U32:
  S.g(t, (b + 30 : U32))

def set_ctrl(+t: S.Sl, +b: U32, +v: U32) -> S.Sl:
  S.p(t, (b + 30 : U32), v)

def down(+t: S.Sl, +b: U32) -> U32:
  S.g(t, (b + 31 : U32))

def set_down(+t: S.Sl, +b: U32, +v: U32) -> S.Sl:
  S.p(t, (b + 31 : U32), v)

def coyote(+t: S.Sl, +b: U32) -> U32:
  S.g(t, (b + 32 : U32))

def set_coyote(+t: S.Sl, +b: U32, +v: U32) -> S.Sl:
  S.p(t, (b + 32 : U32), v)

def buffer(+t: S.Sl, +b: U32) -> U32:
  S.g(t, (b + 33 : U32))

def set_buffer(+t: S.Sl, +b: U32, +v: U32) -> S.Sl:
  S.p(t, (b + 33 : U32), v)

def jump_key(+t: S.Sl, +b: U32) -> U32:
  S.g(t, (b + 34 : U32))

def set_jump_key(+t: S.Sl, +b: U32, +v: U32) -> S.Sl:
  S.p(t, (b + 34 : U32), v)

def spur_key(+t: S.Sl, +b: U32) -> U32:
  S.g(t, (b + 35 : U32))

def set_spur_key(+t: S.Sl, +b: U32, +v: U32) -> S.Sl:
  S.p(t, (b + 35 : U32), v)

def jumping(+t: S.Sl, +b: U32) -> U32:
  S.g(t, (b + 36 : U32))

def set_jumping(+t: S.Sl, +b: U32, +v: U32) -> S.Sl:
  S.p(t, (b + 36 : U32), v)

def air(+t: S.Sl, +b: U32) -> U32:
  S.g(t, (b + 37 : U32))

def set_air(+t: S.Sl, +b: U32, +v: U32) -> S.Sl:
  S.p(t, (b + 37 : U32), v)

def draft(+t: S.Sl, +b: U32) -> U32:
  S.g(t, (b + 38 : U32))

def set_draft(+t: S.Sl, +b: U32, +v: U32) -> S.Sl:
  S.p(t, (b + 38 : U32), v)

def ang(+t: S.Sl, +b: U32) -> U32:
  S.g(t, (b + 39 : U32))

def set_ang(+t: S.Sl, +b: U32, +v: U32) -> S.Sl:
  S.p(t, (b + 39 : U32), v)

def cam_x(+t: S.Sl, +b: U32) -> U32:
  S.g(t, (b + 40 : U32))

def set_cam_x(+t: S.Sl, +b: U32, +v: U32) -> S.Sl:
  S.p(t, (b + 40 : U32), v)

def cam_y(+t: S.Sl, +b: U32) -> U32:
  S.g(t, (b + 41 : U32))

def set_cam_y(+t: S.Sl, +b: U32, +v: U32) -> S.Sl:
  S.p(t, (b + 41 : U32), v)

def zoom(+t: S.Sl, +b: U32) -> U32:
  S.g(t, (b + 42 : U32))

def set_zoom(+t: S.Sl, +b: U32, +v: U32) -> S.Sl:
  S.p(t, (b + 42 : U32), v)

def looping(+t: S.Sl, +b: U32) -> U32:
  S.g(t, (b + 43 : U32))

def set_looping(+t: S.Sl, +b: U32, +v: U32) -> S.Sl:
  S.p(t, (b + 43 : U32), v)

def want_lock(+t: S.Sl, +b: U32) -> U32:
  S.g(t, (b + 44 : U32))

def set_want_lock(+t: S.Sl, +b: U32, +v: U32) -> S.Sl:
  S.p(t, (b + 44 : U32), v)

def crates(+t: S.Sl, +b: U32) -> U32:
  S.g(t, (b + 45 : U32))

def set_crates(+t: S.Sl, +b: U32, +v: U32) -> S.Sl:
  S.p(t, (b + 45 : U32), v)

def collected(+t: S.Sl, +b: U32) -> U32:
  S.g(t, (b + 46 : U32))

def set_collected(+t: S.Sl, +b: U32, +v: U32) -> S.Sl:
  S.p(t, (b + 46 : U32), v)

def falls(+t: S.Sl, +b: U32) -> U32:
  S.g(t, (b + 47 : U32))

def set_falls(+t: S.Sl, +b: U32, +v: U32) -> S.Sl:
  S.p(t, (b + 47 : U32), v)

def hits(+t: S.Sl, +b: U32) -> U32:
  S.g(t, (b + 48 : U32))

def set_hits(+t: S.Sl, +b: U32, +v: U32) -> S.Sl:
  S.p(t, (b + 48 : U32), v)

def kills(+t: S.Sl, +b: U32) -> U32:
  S.g(t, (b + 49 : U32))

def set_kills(+t: S.Sl, +b: U32, +v: U32) -> S.Sl:
  S.p(t, (b + 49 : U32), v)

def finish_on(+t: S.Sl, +b: U32) -> U32:
  S.g(t, (b + 50 : U32))

def set_finish_on(+t: S.Sl, +b: U32, +v: U32) -> S.Sl:
  S.p(t, (b + 50 : U32), v)

def finish_at(+t: S.Sl, +b: U32) -> U32:
  S.g(t, (b + 51 : U32))

def set_finish_at(+t: S.Sl, +b: U32, +v: U32) -> S.Sl:
  S.p(t, (b + 51 : U32), v)

def score(+t: S.Sl, +b: U32) -> U32:
  S.g(t, (b + 52 : U32))

def set_score(+t: S.Sl, +b: U32, +v: U32) -> S.Sl:
  S.p(t, (b + 52 : U32), v)

def loose_n(+t: S.Sl, +b: U32) -> U32:
  S.g(t, (b + 53 : U32))

def set_loose_n(+t: S.Sl, +b: U32, +v: U32) -> S.Sl:
  S.p(t, (b + 53 : U32), v)

def id(+t: S.Sl, +b: U32) -> U32:
  S.g(t, (b + 54 : U32))

def set_id(+t: S.Sl, +b: U32, +v: U32) -> S.Sl:
  S.p(t, (b + 54 : U32), v)

def joined(+t: S.Sl, +b: U32) -> U32:
  S.g(t, (b + 55 : U32))

def set_joined(+t: S.Sl, +b: U32, +v: U32) -> S.Sl:
  S.p(t, (b + 55 : U32), v)

