# crates/delve/src/rules/tune.rs — the knobs. `Run::tune` is written only once a
# knob has moved (`Tune::is_default`), so `tune` below is the run's `tune`
# value: JNull while default (every knob at the code's constant), else the
# whole struct. Read a knob through these, never `J.get(.., "tune")` alone:
# an absent key is the Rust default, not zero.

def tune.Tune.get_u.if(none: Bool, +v: J.Val, +d: U32) -> U32:
  match none:
    case True{}:
      d
    case False{}:
      J.u32(v)

def tune.Tune.get_u(+t: J.Val, +k: String, +d: U32) -> U32:
  tune.Tune.get_u.if(J.is_null(J.get(t, k)), J.get(t, k), d)

def tune.Tune.get_f.if(none: Bool, +v: J.Val, +d: F32) -> F32:
  match none:
    case True{}:
      d
    case False{}:
      J.f32(v)

def tune.Tune.get_f(+t: J.Val, +k: String, +d: F32) -> F32:
  tune.Tune.get_f.if(J.is_null(J.get(t, k)), J.get(t, k), d)

def tune.Tune.hitstop(+t: J.Val) -> U32:
  tune.Tune.get_u(t, "hitstop", 1)

def tune.Tune.hitstop_boss_kill(+t: J.Val) -> U32:
  tune.Tune.get_u(t, "hitstop_boss_kill", 6)

def tune.Tune.hitstop_elite_kill(+t: J.Val) -> U32:
  tune.Tune.get_u(t, "hitstop_elite_kill", 3)

def tune.Tune.hitstop_phase(+t: J.Val) -> U32:
  tune.Tune.get_u(t, "hitstop_phase", 4)

def tune.Tune.hitstop_heavy(+t: J.Val) -> U32:
  tune.Tune.get_u(t, "hitstop_heavy", 2)

def tune.Tune.hitstop_cooldown(+t: J.Val) -> U32:
  tune.Tune.get_u(t, "hitstop_cooldown", 14)

def tune.Tune.hitstop_finisher(+t: J.Val) -> U32:
  tune.Tune.get_u(t, "hitstop_finisher", 0)

def tune.Tune.cinder_fall(+t: J.Val) -> U32:
  tune.Tune.get_u(t, "cinder_fall", 20)

def tune.Tune.cinder_slide(+t: J.Val) -> U32:
  tune.Tune.get_u(t, "cinder_slide", 5)

def tune.Tune.cinder_lock(+t: J.Val) -> U32:
  tune.Tune.get_u(t, "cinder_lock", 10)

def tune.Tune.cinder_tell(+t: J.Val) -> U32:
  tune.Tune.get_u(t, "cinder_tell", 27)

def tune.Tune.cinder_attack(+t: J.Val) -> U32:
  tune.Tune.get_u(t, "cinder_attack", 165)

def tune.Tune.pennon_gravity(+t: J.Val) -> U32:
  tune.Tune.get_u(t, "pennon_gravity", 112)

def tune.Tune.pennon_jump(+t: J.Val) -> U32:
  tune.Tune.get_u(t, "pennon_jump", 1536)

def tune.Tune.pennon_run(+t: J.Val) -> U32:
  tune.Tune.get_u(t, "pennon_run", 640)

def tune.Tune.pennon_coyote(+t: J.Val) -> U32:
  tune.Tune.get_u(t, "pennon_coyote", 4)

def tune.Tune.pennon_buffer(+t: J.Val) -> U32:
  tune.Tune.get_u(t, "pennon_buffer", 5)

def tune.Tune.rafter_gravity(+t: J.Val) -> U32:
  tune.Tune.get_u(t, "rafter_gravity", 112)

def tune.Tune.rafter_jump(+t: J.Val) -> U32:
  tune.Tune.get_u(t, "rafter_jump", 1536)

def tune.Tune.rafter_run(+t: J.Val) -> U32:
  tune.Tune.get_u(t, "rafter_run", 640)

def tune.Tune.rafter_coyote(+t: J.Val) -> U32:
  tune.Tune.get_u(t, "rafter_coyote", 4)

def tune.Tune.rafter_buffer(+t: J.Val) -> U32:
  tune.Tune.get_u(t, "rafter_buffer", 5)

def tune.Tune.brass_stride(+t: J.Val) -> U32:
  tune.Tune.get_u(t, "brass_stride", 3)

def tune.Tune.brass_combo(+t: J.Val) -> U32:
  tune.Tune.get_u(t, "brass_combo", 12)

def tune.Tune.brass_tell(+t: J.Val) -> U32:
  tune.Tune.get_u(t, "brass_tell", 27)

def tune.Tune.brass_shove(+t: J.Val) -> U32:
  tune.Tune.get_u(t, "brass_shove", 12)

def tune.Tune.brass_stop(+t: J.Val) -> U32:
  tune.Tune.get_u(t, "brass_stop", 2)

def tune.Tune.latch_tell(+t: J.Val) -> U32:
  tune.Tune.get_u(t, "latch_tell", 36)

def tune.Tune.latch_window(+t: J.Val) -> U32:
  tune.Tune.get_u(t, "latch_window", 7)

def tune.Tune.latch_rest(+t: J.Val) -> U32:
  tune.Tune.get_u(t, "latch_rest", 24)

def tune.Tune.latch_recover(+t: J.Val) -> U32:
  tune.Tune.get_u(t, "latch_recover", 18)

def tune.Tune.latch_stop(+t: J.Val) -> U32:
  tune.Tune.get_u(t, "latch_stop", 4)

def tune.Tune.moss_launch(+t: J.Val) -> U32:
  tune.Tune.get_u(t, "moss_launch", 1450)

def tune.Tune.moss_nudge(+t: J.Val) -> U32:
  tune.Tune.get_u(t, "moss_nudge", 180)

def tune.Tune.moss_bounce(+t: J.Val) -> U32:
  tune.Tune.get_u(t, "moss_bounce", 85)

def tune.Tune.moss_gravity(+t: J.Val) -> U32:
  tune.Tune.get_u(t, "moss_gravity", 28)

def tune.Tune.moss_flipper(+t: J.Val) -> U32:
  tune.Tune.get_u(t, "moss_flipper", 1250)

def tune.Tune.fire_rate(+t: J.Val) -> U32:
  tune.Tune.get_u(t, "fire_rate", 100)

def tune.Tune.foe_hit(+t: J.Val) -> U32:
  tune.Tune.get_u(t, "foe_hit", 100)

def tune.Tune.foe_health(+t: J.Val) -> U32:
  tune.Tune.get_u(t, "foe_health", 100)

def tune.Tune.foe_count(+t: J.Val) -> U32:
  tune.Tune.get_u(t, "foe_count", 100)

def tune.Tune.hero_speed(+t: J.Val) -> F32:
  tune.Tune.get_f(t, "hero_speed", 13.0)

def tune.Tune.road_pace(+t: J.Val) -> F32:
  tune.Tune.get_f(t, "road_pace", 2.0)

def tune.Tune.roll_speed(+t: J.Val) -> F32:
  tune.Tune.get_f(t, "roll_speed", 27.0)

def tune.Tune.shot_speed(+t: J.Val) -> F32:
  tune.Tune.get_f(t, "shot_speed", 1.0)

def tune.Tune.foe_pace(+t: J.Val) -> F32:
  tune.Tune.get_f(t, "foe_pace", 1.0)

def tune.Tune.camera_follow(+t: J.Val) -> F32:
  tune.Tune.get_f(t, "camera_follow", 0.22)

def tune.Tune.hit_ward(+t: J.Val) -> F32:
  tune.Tune.get_f(t, "hit_ward", 1.5)

# `Tune::cooldown`: a cooldown of `ticks` at this attack rate.
def tune.Tune.cooldown(+t: J.Val, +ticks: U32) -> U32:
  tune.Tune.cooldown.go(tune.Tune.fire_rate(t), ticks)

def tune.Tune.cooldown.go(+rate: U32, +ticks: U32) -> U32:
  Bool.pick(U32, Bool.or(U32.is_eq(rate, 100), U32.is_zero(ticks)), ticks, U32.max(U32.div((ticks * 100 : U32), rate), 1))

# `Tune::percent`: a share in percent scaled by a percent knob.
def tune.Tune.percent(+share: U32, +knob: U32) -> U32:
  Bool.pick(U32, U32.is_eq(knob, 100), share, U32.div((share * knob : U32), 100))
