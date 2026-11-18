import Base
import ./math.bend as M
import ./sl.bend as S
import ./hs.bend as H
import ./rs.bend as R
import ./terrain.bend as T
import ./road.bend as O
import ./level.bend as L

def top() -> U32:
  1280

def grounded(+st: S.Sl, +b: U32) -> Bool:
  M.nz(R.ground_on(st, b))

def locked_on(+st: S.Sl, +b: U32) -> Bool:
  M.nz(R.lock_on(st, b))

def finished(+st: S.Sl, +b: U32) -> Bool:
  M.nz(R.finish_on(st, b))

def attacking(+st: S.Sl, +b: U32) -> Bool:
  Bool.or(Bool.or(M.nz(R.roll(st, b)), M.nz(R.ball(st, b))), Bool.or(locked_on(st, b), M.nz(R.lunge(st, b))))

def speed(+st: S.Sl, +b: U32) -> U32:
  M.pk(M.abs(R.gsp(st, b)), M.hyp(R.vx(st, b), R.vy(st, b)), grounded(st, b))

def body_h(+st: S.Sl, +b: U32) -> U32:
  M.pk(14, 28, Bool.or(Bool.or(M.nz(R.roll(st, b)), M.nz(R.ball(st, b))), Bool.or(M.nz(R.crouch(st, b)), locked_on(st, b))))

def path_of(+rd: L.Road, +st: S.Sl, +b: U32) -> T.Path:
  T.path_at(O.ro_paths(rd), R.ground_path(st, b))

def under(+rd: L.Road, +st: S.Sl, +b: U32) -> T.Sm:
  T.sample(path_of(rd, st, b), R.ground_s(st, b))

type Cn is Data:
  Cn{x: U32, y: U32}

def cn_x(+c: Cn) -> U32:
  match c:
    case Cn{x, y}:
      x

def cn_y(+c: Cn) -> U32:
  match c:
    case Cn{x, y}:
      y

def centre(+rd: L.Road, +st: S.Sl, +b: U32) -> Cn:
  +h = (body_h(st, b) * 128 : U32)
  +on = under(rd, st, b)
  Bool.pick(Cn, grounded(st, b),
    Cn{(R.x(st, b) + M.md(T.sm_ty(on), h, 4096) : U32), (R.y(st, b) - M.md(T.sm_tx(on), h, 4096) : U32)},
    Cn{R.x(st, b), (R.y(st, b) - h : U32)})

def camera(+st: S.Sl, +b: U32) -> S.Sl:
  +px = M.div(R.x(st, b), 256)
  +py = M.div(R.y(st, b), 256)
  +sp = speed(st, b)
  +want = (256 - M.div((M.clamp((sp - 1200 : U32), 0, 800) * 64 : U32), 800) : U32)
  +z1 = (R.zoom(st, b) + M.clamp(M.div((want - R.zoom(st, b) : U32), 8), M.neg(4), 4) : U32)
  +near = Bool.or(M.eq(z1, want), M.lt(M.abs((want - z1 : U32)), 8))
  +zoom = M.pk((z1 + M.sgn((want - z1 : U32)) : U32), z1, near)
  +vw = M.div(98304, zoom)
  +vh = M.div(57344, zoom)
  +lead = M.clamp(M.div(((R.dir(st, b) * sp : U32) * 22 : U32), 256), M.neg(120), 120)
  +tx = (px + lead - M.div(vw, 2) : U32)
  +cx1 = (R.cam_x(st, b) + M.clamp(M.div((tx - R.cam_x(st, b) : U32), 6), M.neg(24), 24) : U32)
  +cx = M.pk(tx, cx1, M.gt(M.abs((tx - cx1 : U32)), vw))
  +ty = (py - M.div((vh * 60 : U32), 100) : U32)
  +d = (ty - R.cam_y(st, b) : U32)
  +follow = M.pk(14, 5, M.nz(R.looping(st, b)))
  +cy = M.pk((R.cam_y(st, b) + M.clamp(M.div((d - (M.sgn(d) * 22 : U32) : U32), follow), M.neg(20), 20) : U32), R.cam_y(st, b), M.gt(M.abs(d), 22))
  R.set_cam_y(R.set_cam_x(R.set_zoom(st, b, zoom), b, cx), b, cy)

def after(+rd: L.Road, +st: S.Sl, +b: U32) -> S.Sl:
  +on = under(rd, st, b)
  +to_ground = M.atan2(T.sm_ty(on), T.sm_tx(on))
  +to_lock = M.atan2(R.vy(st, b), (M.max(M.abs(R.vx(st, b)), 1) * R.dir(st, b) : U32))
  +target = M.pk(to_ground, M.pk(to_lock, 0, locked_on(st, b)), grounded(st, b))
  +ease = M.pk(2, M.pk(3, 5, locked_on(st, b)), grounded(st, b))
  +slew = M.pk(40, M.pk(30, 16, locked_on(st, b)), grounded(st, b))
  camera(R.set_ang(st, b, M.turn_toward(R.ang(st, b), target, ease, slew)), b)

def snap_camera(+st: S.Sl, +b: U32, +px: U32, +py: U32) -> S.Sl:
  R.set_cam_y(R.set_cam_x(R.set_zoom(st, b, 256), b, (px - 192 + 40 : U32)), b, (py - 134 : U32))

def pull(+st: S.Sl, +b: U32, +ty: U32) -> U32:
  +base = M.md((28 * H.slope_of(st) : U32), ty, 409600)
  +plain = Bool.or(Bool.not(M.nz(R.roll(st, b))), Bool.or(M.eq(base, 0), M.eq(R.gsp(st, b), 0)))
  +downhill = Bool.not(Bool.xor(M.gt(base, 0), M.gt(R.gsp(st, b), 0)))
  M.pk(base, M.div((base * M.pk(250, 60, downhill) : U32), 100), plain)

def toward_zero(+v: U32, +by: U32) -> U32:
  M.pk(M.max((v - by : U32), 0), M.min((v + by : U32), 0), M.gt(v, 0))

def g_crouch(+st: S.Sl, +b: U32, +down: Bool, +slow: Bool, +jump_edge: Bool) -> S.Sl:
  +crouch = Bool.and(Bool.not(M.nz(R.roll(st, b))), Bool.and(down, slow))
  +s1 = R.set_crouch(st, b, 1)
  +s2 = H.snd(R.set_buffer(R.set_charge(s1, b, M.min((R.charge(st, b) + 320 : U32), 1280)), b, 0), H.k_shot_spark())
  S.pks(Bool.and(crouch, jump_edge), s2, S.pks(crouch, s1, st))

def g_launch(+st: S.Sl, +b: U32, +down: Bool, +slow: Bool) -> S.Sl:
  +ends = Bool.and(M.nz(R.crouch(st, b)), Bool.or(Bool.not(down), Bool.not(slow)))
  +s1 = R.set_crouch(st, b, 0)
  +s2 = H.snd(R.set_charge(R.set_roll(R.set_gsp(s1, b, (R.dir(st, b) * (768 + R.charge(st, b)) : U32)), b, 1), b, 0), H.k_engine_whistle())
  S.pks(Bool.and(ends, M.gt(R.charge(st, b), 0)), s2, S.pks(ends, s1, st))

def g_charge(+st: S.Sl, +b: U32) -> S.Sl:
  S.pks(M.nz(R.crouch(st, b)), R.set_charge(st, b, (R.charge(st, b) - U32.div(R.charge(st, b), 32) : U32)), R.set_charge(st, b, 0))

def g_roll(+st: S.Sl, +b: U32, +down: Bool, +slow: Bool, +ty: U32) -> S.Sl:
  +tuck = Bool.and(down, Bool.and(Bool.not(M.nz(R.crouch(st, b))), Bool.and(Bool.not(M.nz(R.roll(st, b))), Bool.not(slow))))
  +s1 = H.snd_if(R.set_roll(st, b, M.b2u(Bool.or(tuck, M.nz(R.roll(st, b))))), tuck, H.k_roll())
  +stops = Bool.and(M.nz(R.roll(s1, b)), Bool.and(M.lt(M.abs(R.gsp(s1, b)), 80), M.lt(M.abs(pull(s1, b, ty)), 12)))
  +s2 = R.set_roll(s1, b, M.b2u(Bool.and(M.nz(R.roll(s1, b)), Bool.not(stops))))
  +uncurls = Bool.and(M.nz(R.roll(s2, b)), Bool.and(Bool.not(down), M.lt(M.abs(R.gsp(s2, b)), 640)))
  R.set_roll(s2, b, M.b2u(Bool.and(M.nz(R.roll(s2, b)), Bool.not(uncurls))))

def g_spur(+st: S.Sl, +b: U32, +key: Bool, +locked: Bool, +spur_edge: Bool) -> S.Sl:
  +spurring = Bool.and(key, Bool.and(M.gt(R.gauge(st, b), 0), Bool.and(Bool.not(M.nz(R.crouch(st, b))), Bool.not(locked))))
  +cap = H.spur_cap_of(st)
  +s1 = R.set_gauge(R.set_spur(st, b, M.b2u(spurring)), b, M.pk(M.max((R.gauge(st, b) - 12 : U32), 0), R.gauge(st, b), spurring))
  +kick = Bool.and(spurring, Bool.and(spur_edge, M.lt((R.gsp(st, b) * R.dir(st, b) : U32), cap)))
  S.pks(kick, H.snd(R.set_gsp(s1, b, (R.gsp(st, b) + R.dir(st, b) * 256 : U32)), H.k_spell_fire()), s1)

def cap_of(+st: S.Sl, +b: U32) -> U32:
  M.pk(H.spur_cap_of(st), (top() + M.pk(128, 0, M.nz(R.draft(st, b))) : U32), M.nz(R.spur(st, b)))

def g_leap(+st: S.Sl, +b: U32, +on: T.Sm) -> S.Sl:
  +gsp = R.gsp(st, b)
  +s = R.set_vx(st, b, M.div(((gsp * T.sm_tx(on) : U32) + (1560 * T.sm_ty(on) : U32) : U32), 4096))
  +s = R.set_vy(s, b, M.div(((gsp * T.sm_ty(on) : U32) - (1560 * T.sm_tx(on) : U32) : U32), 4096))
  +s = R.set_ground_on(s, b, 0)
  +s = R.set_buffer(s, b, 0)
  +s = R.set_coyote(s, b, 0)
  +s = R.set_jumping(s, b, 1)
  +s = R.set_ball(s, b, 1)
  +s = R.set_roll(s, b, 0)
  +s = R.set_air(s, b, 0)
  +s = R.set_looping(s, b, 0)
  H.snd(s, H.k_step_wood())

type Nm is Data:
  Nm{g: U32, d: U32, sk: U32}

def nm_g(+n: Nm) -> U32:
  match n:
    case Nm{g, d, sk}:
      g

def nm_d(+n: Nm) -> U32:
  match n:
    case Nm{g, d, sk}:
      d

def nm_sk(+n: Nm) -> U32:
  match n:
    case Nm{g, d, sk}:
      sk

def normal(+gsp0: U32, +dir: U32, +kx: U32, +spurring: Bool, +cap: U32, +acc: U32, +grip: U32, +draft: Bool, +slope: U32) -> Nm:
  +a = M.pk(M.clamp((gsp0 + dir * M.div((96 * M.max(grip, 2048) : U32), 4096) : U32), M.neg(cap), cap), gsp0,
            Bool.and(spurring, M.lt((gsp0 * dir : U32), cap)))
  +moving = M.nz(kx)
  +rev = Bool.and(moving, M.lt((a * kx : U32), 0))
  +flip = (a + kx * 130 : U32)
  +turned = M.gt((flip * kx : U32), 0)
  +g_rev = M.pk((kx * M.max(acc, 1) : U32), flip, turned)
  +d_rev = M.pk(kx, dir, turned)
  +pushed = (a + kx * acc : U32)
  +g_push = M.pk((kx * cap : U32), pushed, M.gt((pushed * kx : U32), cap))
  +g_drag = M.pk((a - kx * M.min(6, (a * kx - cap : U32)) : U32), a, Bool.not(spurring))
  +g_same = M.pk(g_push, g_drag, M.lt((a * kx : U32), cap))
  +wake = Bool.and(draft, Bool.and(Bool.not(spurring), Bool.and(M.gt((g_same * kx : U32), 0), M.lt((g_same * kx : U32), cap))))
  +g_wake = M.pk((g_same + M.div((kx * 26 * grip : U32), 4096) : U32), g_same, wake)
  +settled = toward_zero(a, M.div((12 * slope : U32), 100))
  +g_still = M.pk(settled, M.pk((a + dir * 26 : U32), a, draft), Bool.not(spurring))
  +d_still = M.pk(dir, M.pk(M.sgn(settled), dir, M.gt(M.abs(settled), 512)), spurring)
  Nm{M.pk(g_rev, M.pk(g_wake, g_still, moving), rev),
     M.pk(d_rev, M.pk(kx, d_still, moving), rev),
     M.b2u(Bool.and(rev, M.ge(M.abs(a), 768)))}

def g_forces(+st: S.Sl, +b: U32, +on: T.Sm, +kx: U32) -> S.Sl:
  +grip = M.pk(0, M.max((4096 - U32.div((M.abs(T.sm_ty(on)) * 3 : U32), 2) : U32), 0), M.lt(T.sm_tx(on), 0))
  +acc = M.div((26 * grip : U32), 4096)
  +gsp0 = (R.gsp(st, b) + pull(st, b, T.sm_ty(on)) : U32)
  +dir = R.dir(st, b)
  +fin = M.nz(R.finish_on(st, b))
  +rolling = M.nz(R.roll(st, b))
  +plain = Bool.not(Bool.or(rolling, Bool.or(M.nz(R.crouch(st, b)), fin)))
  +n = normal(gsp0, dir, kx, M.nz(R.spur(st, b)), cap_of(st, b), acc, grip, M.nz(R.draft(st, b)), H.slope_of(st))
  +g_roll = toward_zero(M.pk((gsp0 + kx * 40 : U32), gsp0, Bool.and(M.nz(kx), M.lt((kx * gsp0 : U32), 0))), M.div((6 * H.slope_of(st) : U32), 100))
  +g_brake = toward_zero(gsp0, M.pk(65, 130, fin))
  +gsp = M.clamp(M.pk(g_roll, M.pk(nm_g(n), g_brake, plain), rolling), M.neg(4096), 4096)
  +s = R.set_gsp(st, b, gsp)
  +s = R.set_dir(s, b, M.pk(nm_d(n), dir, Bool.and(plain, Bool.not(rolling))))
  +skid = Bool.and(M.nz(nm_sk(n)), Bool.and(plain, Bool.not(rolling)))
  H.snd_if(R.set_skid(s, b, M.b2u(skid)), skid, H.k_step_gravel())

def g_move(+rd: L.Road, +st: S.Sl, +b: U32) -> S.Sl:
  +av = T.advance(O.ro_paths(rd), R.ground_path(st, b), R.ground_s(st, b), R.gsp(st, b))
  +gsp = M.pk(0, R.gsp(st, b), U32.is_eq(T.av_leave(av), 2))
  +path = T.path_at(O.ro_paths(rd), T.av_path(av))
  +at = T.sample(path, T.av_s(av))
  +was_ring = U32.is_eq(T.pa_kind(path_of(rd, st, b)), 2)
  +on_ring = U32.is_eq(T.pa_kind(path), 2)
  +vx = M.md(gsp, T.sm_tx(at), 4096)
  +vy = M.md(gsp, T.sm_ty(at), 4096)
  +off = U32.is_eq(T.av_leave(av), 1)
  +fell = Bool.and(Bool.not(off), Bool.and(M.lt(T.sm_tx(at), 0), M.lt(M.abs(gsp), H.loop_min_of(st))))
  +s = R.set_gsp(st, b, gsp)
  +s = R.set_x(s, b, T.sm_x(at))
  +s = R.set_y(s, b, T.sm_y(at))
  +flying = Bool.or(off, fell)
  +fly = R.set_ground_on(R.set_air(R.set_vy(R.set_vx(s, b, vx), b, vy), b, 0), b, 0)
  +fly = R.set_roll(R.set_coyote(fly, b, M.pk(4, 0, off)), b, 0)
  +fall = R.set_ctrl(R.set_ball(fly, b, 0), b, M.max(R.ctrl(st, b), 12))
  +fall = H.snd(fall, H.k_knight_hit())
  +ok = R.set_looping(R.set_ground_on(R.set_ground_s(R.set_ground_path(s, b, T.av_path(av)), b, T.av_s(av)), b, 1), b, M.b2u(on_ring))
  +ok = H.snd_if(ok, Bool.and(on_ring, Bool.not(was_ring)), H.k_wheel_spin())
  S.pks(off, fly, S.pks(fell, fall, ok))

def g_rest(+rd: L.Road, +st: S.Sl, +b: U32, +on: T.Sm, +kx: U32) -> S.Sl:
  +s = R.set_buffer(st, b, M.sat1(R.buffer(st, b)))
  g_move(rd, g_forces(s, b, on, kx), b)

def g_dispatch(+leap: Bool, +rd: L.Road, +st: S.Sl, +b: U32, +on: T.Sm, +kx: U32) -> S.Sl:
  match leap:
    case True{}:
      g_leap(st, b, on)
    case False{}:
      g_rest(rd, st, b, on, kx)

def ground_tick(+rd: L.Road, +st: S.Sl, +b: U32, +k_x: U32, +k_down: U32, +k_spur: U32, +jump_edge: Bool, +spur_edge: Bool) -> S.Sl:
  +on = under(rd, st, b)
  +s = R.set_air(R.set_coyote(st, b, 4), b, 0)
  +locked = Bool.or(M.gt(R.ctrl(s, b), 0), finished(s, b))
  +kx = M.pk(0, k_x, locked)
  +down = Bool.and(M.nz(k_down), Bool.not(locked))
  +slow = M.lt(M.abs(R.gsp(s, b)), 192)
  +s = g_crouch(s, b, down, slow, jump_edge)
  +s = g_launch(s, b, down, slow)
  +s = g_charge(s, b)
  +s = g_roll(s, b, down, slow, T.sm_ty(on))
  +s = g_spur(s, b, M.nz(k_spur), locked, spur_edge)
  +leap = Bool.and(M.gt(R.buffer(s, b), 0), Bool.and(Bool.not(M.nz(R.crouch(s, b))), Bool.not(locked)))
  g_dispatch(leap, rd, s, b, on, kx)

def land(+rd: L.Road, +st: S.Sl, +b: U32, +h: T.Hit, +k_down: U32) -> S.Sl:
  +on = T.sample(T.path_at(O.ro_paths(rd), T.hit_path(h)), T.hit_s(h))
  +vx = R.vx(st, b)
  +vy = R.vy(st, b)
  +along = M.div(((vx * T.sm_tx(on) : U32) + (vy * T.sm_ty(on) : U32) : U32), 4096)
  +normal = M.abs(M.div(((vx * T.sm_ty(on) : U32) - (vy * T.sm_tx(on) : U32) : U32), 4096))
  +s = R.set_gsp(st, b, along)
  +s = R.set_ground_path(R.set_ground_on(s, b, 1), b, T.hit_path(h))
  +s = R.set_ground_s(s, b, T.hit_s(h))
  +s = R.set_y(R.set_x(s, b, T.hit_x(h)), b, T.hit_y(h))
  +s = R.set_vy(R.set_vx(s, b, 0), b, 0)
  +s = R.set_ball(R.set_jumping(s, b, 0), b, 0)
  +s = R.set_lunged(R.set_lunge(R.set_lock_on(s, b, 0), b, 0), b, 0)
  +s = R.set_coyote(R.set_air(R.set_chain(s, b, 0), b, 0), b, 4)
  +s = R.set_roll(s, b, M.b2u(Bool.and(M.nz(k_down), M.ge(M.abs(along), 192))))
  +s = R.set_shake(s, b, M.pk(M.max(R.shake(s, b), 4), R.shake(s, b), M.gt(normal, 2600)))
  H.snd_if(s, M.gt(normal, 1500), H.k_rock_land())

def a_land(+rd: L.Road, +st: S.Sl, +b: U32, +h: T.Hit, +k_down: U32, +tx: U32, +ty: U32) -> S.Sl:
  match h:
    case T.Hit{ok, path, s, x, y}:
      Bool.pick(S.Sl, M.nz(ok), land(rd, st, b, h, k_down), R.set_y(R.set_x(st, b, tx), b, ty))

def a_walls(+rd: L.Road, +st: S.Sl, +b: U32, +k_down: U32) -> S.Sl:
  +tx0 = (R.x(st, b) + R.vx(st, b) : U32)
  +ty = (R.y(st, b) + R.vy(st, b) : U32)
  +out = Bool.or(M.lt(tx0, 3072), M.gt(tx0, 5117952))
  +tx = M.clamp(tx0, 3072, 5117952)
  +vx = M.pk(M.pk(M.max(R.vx(st, b), 0), M.min(R.vx(st, b), 0), M.eq(tx, 3072)), R.vx(st, b), out)
  a_land(rd, R.set_vx(st, b, vx), b, T.land(O.ro_paths(rd), R.x(st, b), R.y(st, b), tx, ty), k_down, tx, ty)

def a_speeds(+st: S.Sl, +b: U32, +kx: U32, +k_jump: U32, +spurring: Bool) -> S.Sl:
  +cap = cap_of(st, b)
  +dir = R.dir(st, b)
  +vx = R.vx(st, b)
  +vy = R.vy(st, b)
  +lunging = M.gt(R.lunge(st, b), 0)
  +steer = M.nz(kx)
  +dir_f = M.pk(kx, dir, steer)
  +t = (vx + kx * 60 : U32)
  +vx_a = M.pk(M.pk((kx * cap : U32), t, M.gt((t * kx : U32), cap)), vx, M.lt((vx * kx : U32), cap))
  +vx_b = M.pk((vx + dir * 48 : U32), vx, Bool.and(spurring, M.lt((vx * dir : U32), cap)))
  +vx_c = M.pk(vx_a, vx_b, steer)
  +vx_d = M.pk((vx_c + M.div((dir_f * 26 : U32), 2) : U32), vx_c, Bool.and(M.nz(R.draft(st, b)), M.gt((vx_c * dir_f : U32), 0)))
  +vx1 = M.pk((dir * 2304 : U32), vx_d, lunging)
  +vy1 = M.pk(M.min((vy + 33 : U32), 256), M.min((vy + 100 : U32), 4096), lunging)
  +cut = Bool.and(M.nz(R.jumping(st, b)), Bool.and(Bool.not(M.nz(k_jump)), M.lt(vy1, M.neg(1024))))
  +vy2 = M.pk(M.neg(1024), vy1, cut)
  +s = R.set_vy(R.set_vx(st, b, vx1), b, vy2)
  +s = R.set_dir(s, b, M.pk(dir, dir_f, lunging))
  +s = R.set_lunge(s, b, M.pk(M.sat1(R.lunge(st, b)), R.lunge(st, b), lunging))
  +s = R.set_jumping(s, b, M.b2u(Bool.and(M.nz(R.jumping(st, b)), M.lt(vy2, 0))))
  S.pks(locked_on(st, b), st, s)

def a_coyote(+st: S.Sl, +b: U32, +locked: Bool) -> S.Sl:
  +hop = Bool.and(M.gt(R.buffer(st, b), 0), Bool.and(M.gt(R.coyote(st, b), 0), Bool.and(Bool.not(locked), Bool.not(M.nz(R.jumping(st, b))))))
  +s = R.set_vy(st, b, M.neg(1560))
  +s = R.set_ball(R.set_jumping(R.set_coyote(R.set_buffer(s, b, 0), b, 0), b, 1), b, 1)
  S.pks(hop, H.snd(s, H.k_step_wood()), st)

def air_tick(+rd: L.Road, +st: S.Sl, +b: U32, +k_x: U32, +k_down: U32, +k_jump: U32, +k_spur: U32, +jump_edge: Bool, +spur_edge: Bool) -> S.Sl:
  +s = R.set_charge(R.set_crouch(R.set_coyote(R.set_air(st, b, (R.air(st, b) + 1 : U32)), b, M.sat1(R.coyote(st, b))), b, 0), b, 0)
  +locked = Bool.or(M.gt(R.ctrl(s, b), 0), finished(s, b))
  +kx = M.pk(0, k_x, locked)
  +s = a_coyote(s, b, locked)
  +s = R.set_want_lock(s, b, M.b2u(Bool.and(jump_edge, Bool.and(M.gt(R.buffer(s, b), 0), Bool.and(Bool.not(locked_on(s, b)), Bool.not(locked))))))
  +s = R.set_buffer(s, b, M.sat1(R.buffer(s, b)))
  +spurring = Bool.and(M.nz(k_spur), Bool.and(M.gt(R.gauge(s, b), 0), Bool.not(locked)))
  +s = R.set_gauge(R.set_spur(s, b, M.b2u(spurring)), b, M.pk(M.max((R.gauge(s, b) - 12 : U32), 0), R.gauge(s, b), spurring))
  +s = H.snd_if(s, Bool.and(spurring, spur_edge), H.k_spell_fire())
  +s = a_speeds(s, b, kx, k_jump, spurring)
  a_walls(rd, R.set_vx(s, b, M.clamp(R.vx(s, b), M.neg(4096), 4096)), b, k_down)

def move_on(+grounded: Bool, +rd: L.Road, +st: S.Sl, +b: U32, +k_x: U32, +k_down: U32, +k_jump: U32, +k_spur: U32, +jump_edge: Bool, +spur_edge: Bool) -> S.Sl:
  match grounded:
    case True{}:
      ground_tick(rd, st, b, k_x, k_down, k_spur, jump_edge, spur_edge)
    case False{}:
      air_tick(rd, st, b, k_x, k_down, k_jump, k_spur, jump_edge, spur_edge)

def step_live(+rd: L.Road, +st: S.Sl, +b: U32, +k_x: U32, +k_down: U32, +k_jump: U32, +k_spur: U32) -> S.Sl:
  +jump_edge = Bool.and(M.nz(k_jump), Bool.not(M.nz(R.jump_key(st, b))))
  +spur_edge = Bool.and(M.nz(k_spur), Bool.not(M.nz(R.spur_key(st, b))))
  +s = R.set_spur_key(R.set_jump_key(st, b, k_jump), b, k_spur)
  +s = R.set_buffer(s, b, M.pk(5, R.buffer(s, b), jump_edge))
  +fin = finished(s, b)
  after(rd, move_on(grounded(s, b), rd, s, b, M.pk(0, k_x, fin), M.pk(0, k_down, fin), M.pk(0, k_jump, fin), M.pk(0, k_spur, fin),
                    Bool.and(jump_edge, Bool.not(fin)), Bool.and(spur_edge, Bool.not(fin))), b)

def step_frozen(+rd: L.Road, +st: S.Sl, +b: U32, +k_jump: U32, +k_spur: U32) -> S.Sl:
  +edge = Bool.and(M.nz(k_jump), Bool.not(M.nz(R.jump_key(st, b))))
  +s = R.set_freeze(st, b, M.sat1(R.freeze(st, b)))
  +s = R.set_buffer(s, b, M.pk(5, M.sat1(R.buffer(s, b)), edge))
  after(rd, R.set_spur_key(R.set_jump_key(s, b, k_jump), b, k_spur), b)

def step_pick(+frozen: Bool, +rd: L.Road, +st: S.Sl, +b: U32, +k_x: U32, +k_down: U32, +k_jump: U32, +k_spur: U32) -> S.Sl:
  match frozen:
    case True{}:
      step_frozen(rd, st, b, k_jump, k_spur)
    case False{}:
      step_live(rd, st, b, k_x, k_down, k_jump, k_spur)

def step(+rd: L.Road, +st: S.Sl, +b: U32, +k_x: U32, +k_down: U32, +k_jump: U32, +k_spur: U32) -> S.Sl:
  +s = R.set_hurt(st, b, M.sat1(R.hurt(st, b)))
  +s = R.set_ctrl(s, b, M.sat1(R.ctrl(s, b)))
  +s = R.set_lock_cd(s, b, M.sat1(R.lock_cd(s, b)))
  +s = R.set_spring_cd(s, b, M.sat1(R.spring_cd(s, b)))
  +s = R.set_skid(R.set_shake(s, b, M.sat1(R.shake(s, b))), b, 0)
  step_pick(M.gt(R.freeze(s, b), 0), rd, s, b, k_x, k_down, k_jump, k_spur)

def link(+st: S.Sl, +b: U32) -> Bool:
  Bool.or(locked_on(st, b), Bool.not(grounded(st, b)))

def strike(+st: S.Sl, +b: U32) -> S.Sl:
  +s = R.set_kills(st, b, (R.kills(st, b) + 1 : U32))
  +locked = locked_on(st, b)
  +air = Bool.not(grounded(st, b))
  +chain = (R.chain(st, b) + 1 : U32)
  +s = R.set_chain(s, b, M.pk(chain, R.chain(s, b), Bool.or(locked, air)))
  +s = R.set_best_chain(s, b, M.pk(M.max(R.best_chain(s, b), R.chain(s, b)), R.best_chain(s, b), Bool.or(locked, air)))
  +lock = R.set_lunged(R.set_lunge(R.set_lock_cd(R.set_lock_on(R.set_vx(R.set_vy(s, b, M.neg(1280)), b, M.clamp(M.div(R.vx(s, b), 2), M.neg(1536), 1536)), b, 0), b, 2), b, 0), b, 0)
  +flight = R.set_lunge(R.set_jumping(R.set_vy(s, b, M.neg((1100 + M.pk(320, 0, M.nz(R.jump_key(s, b))) : U32))), b, 0), b, 0)
  S.pks(locked, lock, S.pks(air, flight, s))

def put_loose(+st: S.Sl, +b: U32, +i: U32, +x: U32, +y: U32, +vx: U32, +vy: U32, +age: U32) -> S.Sl:
  +o = (b + 80 + i * 5 : U32)
  S.p(S.p(S.p(S.p(S.p(st, o, x), (o + 1 : U32), y), (o + 2 : U32), vx), (o + 3 : U32), vy), (o + 4 : U32), age)

def scatter_go(+fuel: Nat, +i: U32, +st: S.Sl, +b: U32, +cx: U32, +cy: U32) -> S.Sl:
  match fuel:
    case 0n:
      st
    case 1n+k:
      +a = (M.neg(96) + U32.mod((i * 37 : U32), 64) + U32.mod(i, 2) * 3 : U32)
      +pace = (1100 + U32.mod((i * 97 : U32), 900) : U32)
      scatter_go(k, (i + 1 : U32), put_loose(st, b, i, cx, cy, M.md(M.cos(a), pace, 4096), M.md(M.sin(a), pace, 4096), 0), b, cx, cy)

def scatter(+st: S.Sl, +b: U32, +n: U32) -> S.Sl:
  scatter_go(U32.to_nat(n), 0, R.set_loose_n(st, b, n), b, R.x(st, b), (R.y(st, b) - 3584 : U32))

def blow(+st: S.Sl, +b: U32) -> U32:
  M.pk(0, M.pk(1, M.pk(2, 3, M.gt(R.sparks(st, b), 0)), M.nz(R.shield(st, b))), Bool.or(M.gt(R.hurt(st, b), 0), M.gt(R.down(st, b), 0)))

def knock(+st: S.Sl, +b: U32, +from: U32) -> S.Sl:
  +kind = blow(st, b)
  +away = M.pk(1, M.neg(1), M.ge(R.x(st, b), from))
  +s = R.set_hits(st, b, (R.hits(st, b) + 1 : U32))
  +s = R.set_roll(R.set_ground_on(s, b, 0), b, 0)
  +s = R.set_crouch(R.set_ball(s, b, 0), b, 0)
  +s = R.set_lunge(R.set_lock_on(R.set_charge(s, b, 0), b, 0), b, 0)
  +s = R.set_vy(R.set_vx(s, b, (away * 640 : U32)), b, M.neg(1280))
  +s = R.set_jumping(R.set_ctrl(s, b, 18), b, 0)
  +s = R.set_shake(R.set_freeze(R.set_chain(s, b, 0), b, 3), b, 8)
  +shielded = R.set_hurt(R.set_shield(s, b, 0), b, 90)
  +n = M.min(R.sparks(s, b), 16)
  +flown = scatter(R.set_hurt(R.set_sparks(s, b, 0), b, 90), b, n)
  +down = R.set_vy(R.set_falls(R.set_down(s, b, 24), b, (R.falls(s, b) + 1 : U32)), b, M.neg(1600))
  S.pks(U32.is_eq(kind, 0), st, S.pks(U32.is_eq(kind, 1), shielded, S.pks(U32.is_eq(kind, 2), flown, down)))

def tumble(+st: S.Sl, +b: U32) -> S.Sl:
  +vy = M.min((R.vy(st, b) + 100 : U32), 4096)
  +s = R.set_vy(st, b, vy)
  +s = R.set_x(s, b, (R.x(st, b) + R.vx(st, b) : U32))
  +s = R.set_y(s, b, (R.y(st, b) + vy : U32))
  R.set_zoom(R.set_down(s, b, M.sat1(R.down(st, b))), b, M.min((R.zoom(st, b) + 8 : U32), 256))

def loose_one(+rd: L.Road, +st: S.Sl, +b: U32, +i: U32) -> S.Sl:
  +o = (b + 80 + i * 5 : U32)
  +x = S.g(st, o)
  +y = S.g(st, (o + 1 : U32))
  +vx = S.g(st, (o + 2 : U32))
  +vy = M.min((S.g(st, (o + 3 : U32)) + 100 : U32), 4096)
  +age = (S.g(st, (o + 4 : U32)) + 1 : U32)
  +tx = (x + vx : U32)
  +ty = (y + vy : U32)
  +h = T.land(O.ro_paths(rd), x, y, tx, ty)
  +hit = M.nz(T.hit_ok(h))
  put_loose(st, b, i, M.pk(T.hit_x(h), tx, hit), M.pk((T.hit_y(h) - 128 : U32), ty, hit),
            M.pk(M.div((vx * 9 : U32), 10), vx, hit), M.pk(M.neg(M.max(M.div((vy * 3 : U32), 4), 300)), vy, hit), age)

def loose_age(+fuel: Nat, +i: U32, +rd: L.Road, +st: S.Sl, +b: U32) -> S.Sl:
  match fuel:
    case 0n:
      st
    case 1n+k:
      loose_age(k, (i + 1 : U32), rd, loose_one(rd, st, b, i), b)

def loose_pack(+fuel: Nat, +i: U32, +m: U32, +st: S.Sl, +b: U32, +keep: U32) -> S.Sl:
  match fuel:
    case 0n:
      R.set_loose_n(st, b, m)
    case 1n+k:
      +o = (b + 80 + i * 5 : U32)
      +kept = M.nz(U32.and(U32.shrn(keep, U32.to_nat(i)), 1))
      +moved = put_loose(st, b, m, S.g(st, o), S.g(st, (o + 1 : U32)), S.g(st, (o + 2 : U32)), S.g(st, (o + 3 : U32)), S.g(st, (o + 4 : U32)))
      loose_pack(k, (i + 1 : U32), (m + M.b2u(kept) : U32), S.pks(kept, moved, st), b, keep)

def loose_young(+fuel: Nat, +i: U32, +st: S.Sl, +b: U32, +acc: U32) -> U32:
  match fuel:
    case 0n:
      acc
    case 1n+k:
      +young = (S.g(st, (b + 84 + i * 5 : U32)) < 120 : U32)
      loose_young(k, (i + 1 : U32), st, b, (acc + U32.shln(M.b2u(young), U32.to_nat(i)) : U32))

def tick_loose(+rd: L.Road, +st: S.Sl, +b: U32) -> S.Sl:
  +n = U32.to_nat(R.loose_n(st, b))
  +s = loose_age(n, 0, rd, st, b)
  loose_pack(n, 0, 0, s, b, loose_young(n, 0, s, b, 0))

def reset_body(+rd: L.Road, +st: S.Sl, +b: U32, +path: U32, +s: U32) -> S.Sl:
  +at = T.sample(T.path_at(O.ro_paths(rd), path), s)
  +r = R.set_x(st, b, T.sm_x(at))
  +r = R.set_y(r, b, T.sm_y(at))
  +r = R.set_vy(R.set_vx(r, b, 0), b, 0)
  +r = R.set_ground_s(R.set_ground_path(R.set_ground_on(r, b, 1), b, path), b, s)
  +r = R.set_dir(R.set_gsp(r, b, 0), b, 1)
  +r = R.set_crouch(R.set_ball(R.set_roll(r, b, 0), b, 0), b, 0)
  +r = R.set_charge(R.set_skid(r, b, 0), b, 0)
  +r = R.set_gauge(R.set_spur(r, b, 0), b, 500)
  +r = R.set_lock_ticks(R.set_lock_target(R.set_lock_on(r, b, 0), b, 0), b, 0)
  +r = R.set_spring_cd(R.set_lock_cd(r, b, 0), b, 0)
  +r = R.set_shake(R.set_freeze(r, b, 0), b, 0)
  +r = R.set_lunged(R.set_lunge(r, b, 0), b, 0)
  +r = R.set_sparks(R.set_chain(r, b, 0), b, 0)
  +r = R.set_hurt(R.set_shield(r, b, 0), b, 0)
  +r = R.set_down(R.set_ctrl(r, b, 0), b, 0)
  +r = R.set_buffer(R.set_coyote(r, b, 4), b, 0)
  +r = R.set_spur_key(R.set_jump_key(r, b, 0), b, 0)
  +r = R.set_air(R.set_jumping(r, b, 0), b, 0)
  +r = R.set_ang(R.set_draft(r, b, 0), b, 0)
  +r = R.set_cam_y(R.set_cam_x(r, b, 0), b, 0)
  +r = R.set_looping(R.set_zoom(r, b, 256), b, 0)
  +r = R.set_loose_n(R.set_want_lock(r, b, 0), b, 0)
  R.set_score(r, b, 0)

def zero_taken(+fuel: Nat, +i: U32, +st: S.Sl, +b: U32) -> S.Sl:
  match fuel:
    case 0n:
      st
    case 1n+k:
      zero_taken(k, (i + 1 : U32), S.p(st, (b + 56 + i : U32), 0), b)

def new_on(+rd: L.Road, +st: S.Sl, +b: U32, +id: U32, +joined: U32, +path: U32, +s: U32) -> S.Sl:
  +r = reset_body(rd, st, b, path, s)
  +r = R.set_kills(R.set_hits(R.set_falls(R.set_collected(r, b, 0), b, 0), b, 0), b, 0)
  +r = R.set_crates(R.set_best_chain(r, b, 0), b, 0)
  +r = R.set_finish_at(R.set_finish_on(r, b, 0), b, 0)
  +r = R.set_joined(R.set_id(zero_taken(24n, 0, r, b), b, id), b, joined)
  snap_camera(r, b, M.div(R.x(r, b), 256), M.div(R.y(r, b), 256))

def place(+rd: L.Road, +st: S.Sl, +b: U32, +path: U32, +s: U32) -> S.Sl:
  +r = reset_body(rd, st, b, path, s)
  +r = R.set_gauge(r, b, M.max(R.gauge(st, b), 300))
  +r = R.set_hurt(r, b, 90)
  snap_camera(r, b, M.div(R.x(r, b), 256), M.div(R.y(r, b), 256))
