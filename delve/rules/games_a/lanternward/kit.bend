# crates/delve/src/rules/lanternward/kit.rs — the knight: a body, a lock-on, a
# sword, a shield, a lantern and a hop.
#
# Seat J (serde): {id, joined, x, y, z, face:[x,y], hp, max_hp, gems, bombs,
# flame, taper, act, t, n, mv:[x,y], push:[x,y], pusht, step, chain, queued,
# charge, shield, guard_cd, lock, lock_t, unlock_t, relock, inv, ward, freeze,
# swing, lamp, lamp_cd, carry, ctx, read, aim:[i8,i8], held, safe:[x,y],
# cam:[x,y], rev, beep, item}. Act and Carry are their names; Lock is null,
# {"Foe": id}, {"Prop": i} or "Boss". The seat being stepped is `s["$k"]`.

# The seven buttons the adventure reads, as one byte.
def lanternward.kit.buttons(+i: J.Val) -> U32:
  (Bool.to_u32(ga.b(i, "ult")) * 1 + Bool.to_u32(ga.b(i, "dash")) * 2 + Bool.to_u32(ga.b(i, "swing")) * 4
    + Bool.to_u32(ga.b(i, "interact")) * 8 + Bool.to_u32(ga.b(i, "fire")) * 16 + Bool.to_u32(ga.b(i, "bomb")) * 32 : U32)

def lanternward.kit.Seat.new(+id: U32, +joined: U32, +x: U32, +y: U32) -> J.Val:
  +k0 = ga.su(ga.su(ga.su(J.obj(), "id", U32.and(id, 255)), "joined", joined), "z", 0)
  +k1 = ga.si(ga.si(k0, "x", x), "y", y)
  +k2 = ga.si(ga.sv(k1, "face", ga.pair_i(0, 256)), "z", 0)
  +k3 = ga.su(ga.su(ga.su(ga.su(ga.su(ga.su(k2, "hp", 20), "max_hp", 20), "gems", 0), "bombs", 0), "flame", 0), "taper", 0)
  +k4 = ga.sv(ga.su(ga.su(k3, "t", 0), "n", 0), "act", J.JS{"Free"})
  +k5 = ga.sv(ga.sv(k4, "mv", ga.pair_i(0, 0)), "push", ga.pair_i(0, 0))
  +k6 = ga.sb(ga.su(ga.su(ga.su(ga.su(ga.su(k5, "pusht", 0), "step", 0), "chain", 0), "charge", 0), "shield", 0), "queued", False{})
  +k7 = ga.su(ga.su(ga.su(ga.su(ga.sv(ga.su(k6, "guard_cd", 0), "lock", J.JNull{}), "lock_t", 0), "unlock_t", 0), "relock", 0), "inv", 60)
  +k8 = ga.su(ga.su(ga.su(ga.su(ga.su(k7, "ward", 0), "freeze", 0), "swing", 0), "lamp", 0), "lamp_cd", 0)
  +k9 = ga.sv(ga.sv(ga.sv(k8, "carry", J.JS{"Nothing"}), "ctx", J.JS{"None"}), "read", J.JNull{})
  +k10 = ga.su(ga.sv(k9, "aim", ga.pair_i(0, 0)), "held", 0)
  ga.sv(ga.su(ga.su(ga.sv(ga.sv(k10, "safe", ga.pair_i(x, y)), "cam", ga.pair_i(x, y)), "rev", 0), "beep", 0), "item", J.JNull{})

# Whether every number is one a save or the wire may carry.
def lanternward.kit.Seat.valid(+k: J.Val, +id: U32, +elapsed: U32, +w: U32, +h: U32) -> Bool:
  Bool.and(
    Bool.and(lanternward.kit.Seat.valid.a(k, id, elapsed, w, h), lanternward.kit.Seat.valid.b(k)),
    Bool.and(lanternward.kit.Seat.valid.c(k, w, h), lanternward.kit.Seat.valid.d(k)))

def lanternward.kit.Seat.valid.a(+k: J.Val, +id: U32, +elapsed: U32, +w: U32, +h: U32) -> Bool:
  Bool.and(
    Bool.and(U32.is_eq(ga.u(k, "id"), id), U32.is_le(ga.u(k, "joined"), elapsed)),
    Bool.and(lanternward.in_range(ga.u(k, "x"), 0, w), lanternward.in_range(ga.u(k, "y"), 0, h)))

def lanternward.kit.Seat.valid.b(+k: J.Val) -> Bool:
  +face = ga.v(k, "face")
  Bool.and(
    Bool.and(lanternward.in_range(ga.u(k, "z"), 0, 4096), RS.i_le(lanternward.world.dist(ga.nu(face, 0), ga.nu(face, 1)), 264)),
    Bool.and(U32.is_le(ga.u(k, "max_hp"), 48), U32.is_le(ga.u(k, "hp"), ga.u(k, "max_hp"))))

def lanternward.kit.Seat.valid.c(+k: J.Val, +w: U32, +h: U32) -> Bool:
  +mv = ga.v(k, "mv")
  +push = ga.v(k, "push")
  +cam = ga.v(k, "cam")
  +safe = ga.v(k, "safe")
  Bool.and(
    Bool.and(
      Bool.and(lanternward.in_range(ga.nu(mv, 0), 4294965248, 2048), lanternward.in_range(ga.nu(mv, 1), 4294965248, 2048)),
      Bool.and(lanternward.in_range(ga.nu(push, 0), 4294965248, 2048), lanternward.in_range(ga.nu(push, 1), 4294965248, 2048))),
    Bool.and(
      Bool.and(
        Bool.and(lanternward.in_range(ga.nu(cam, 0), 0, w), lanternward.in_range(ga.nu(cam, 1), 0, h)),
        Bool.and(lanternward.in_range(ga.nu(safe, 0), 0, w), lanternward.in_range(ga.nu(safe, 1), 0, h))),
      lanternward.kit.Seat.valid.read(ga.v(k, "read"))))

def lanternward.kit.Seat.valid.read(+r: J.Val) -> Bool:
  Bool.or(J.is_null(r), U32.is_le(ga.nu(r, 1), 120))

def lanternward.kit.Seat.valid.d(+k: J.Val) -> Bool:
  Bool.and(
    Bool.and(
      Bool.and(
        Bool.and(U32.is_le(ga.u(k, "gems"), 100000), U32.is_le(ga.u(k, "bombs"), 10)),
        Bool.and(U32.is_le(ga.u(k, "flame"), 300), U32.is_le(ga.u(k, "taper"), 420))),
      Bool.and(
        Bool.and(U32.is_le(ga.u(k, "t"), 4096), U32.is_le(ga.u(k, "n"), 3)),
        Bool.and(U32.is_le(ga.u(k, "step"), 2), U32.is_le(ga.u(k, "chain"), 64)))),
    Bool.and(
      Bool.and(
        Bool.and(U32.is_le(ga.u(k, "charge"), 255), U32.is_le(ga.u(k, "shield"), 255)),
        Bool.and(U32.is_le(ga.u(k, "guard_cd"), 64), U32.is_le(ga.u(k, "lock_t"), 255))),
      Bool.and(
        Bool.and(U32.is_le(ga.u(k, "unlock_t"), 64), U32.is_le(ga.u(k, "relock"), 16)),
        Bool.and(
          Bool.and(U32.is_le(ga.u(k, "inv"), 128), U32.is_le(ga.u(k, "ward"), 36)),
          Bool.and(
            Bool.and(U32.is_le(ga.u(k, "freeze"), 16), U32.is_le(ga.u(k, "lamp"), 255)),
            Bool.and(
              Bool.and(U32.is_le(ga.u(k, "lamp_cd"), 64), U32.is_le(ga.u(k, "rev"), 128)),
              U32.is_le(ga.u(k, "pusht"), 16)))))))

def lanternward.kit.Seat.charged(+k: J.Val) -> Bool:
  U32.is_ge(ga.u(k, "charge"), 22)

def lanternward.kit.Seat.airborne(+k: J.Val) -> Bool:
  RS.i_gt(ga.u(k, "z"), 0)

def lanternward.kit.Session.lantern_owned(+s: J.Val) -> Bool:
  ga.b(ga.v(s, "inv"), "lantern")

# ---- lock-on targets ----

# Where a lock holds: null, or [x, y].
def lanternward.kit.Session.lock_point(+s: J.Val, +l: J.Val) -> J.Val:
  lanternward.kit.lock_point.tag(lanternward.tag(l), J.u32(lanternward.payload(l)), s)

def lanternward.kit.lock_point.tag(+tag: String, +n: U32, +s: J.Val) -> J.Val:
  Bool.pick(J.Val, String.eq(tag, "Foe"), lanternward.kit.lock_point.foe(ga.items(s, "foes"), n),
    Bool.pick(J.Val, String.eq(tag, "Boss"), lanternward.kit.lock_point.boss(J.get(s, "boss")),
      Bool.pick(J.Val, String.eq(tag, "Prop"), lanternward.kit.lock_point.prop(lanternward.el(s, "props", n)),
        J.JNull{})))

def lanternward.kit.lock_point.foe(+fs: List<&2, J.Val>, +id: U32) -> J.Val:
  match fs:
    case Nil{}:
      J.JNull{}
    case Con{+f, +t}:
      lanternward.kit.lock_point.foe.hit(
        Bool.and(U32.is_eq(ga.u(f, "id"), id), lanternward.foes.Foe.alive(f)), f, t, id)

def lanternward.kit.lock_point.foe.hit(hit: Bool, +f: J.Val, +t: List<&2, J.Val>, +id: U32) -> J.Val:
  match hit:
    case True{}:
      ga.pair_i(ga.u(f, "x"), ga.u(f, "y"))
    case False{}:
      lanternward.kit.lock_point.foe(t, id)

def lanternward.kit.lock_point.boss(+w: J.Val) -> J.Val:
  match w:
    case J.JNull{}:
      J.JNull{}
    case _:
      lanternward.kit.lock_point.boss.alive(U32.is_gt(ga.u(w, "hp"), 0), w)

def lanternward.kit.lock_point.boss.alive(alive: Bool, +w: J.Val) -> J.Val:
  match alive:
    case True{}:
      ga.pair_i(lanternward.boss.Warden.heart.x(w), lanternward.boss.Warden.heart.y(w))
    case False{}:
      J.JNull{}

def lanternward.kit.lock_point.prop(+p: J.Val) -> J.Val:
  match p:
    case J.JNull{}:
      J.JNull{}
    case _:
      lanternward.kit.lock_point.prop.target(lanternward.props.Prop.target(p), p)

def lanternward.kit.lock_point.prop.target(t: Bool, +p: J.Val) -> J.Val:
  match t:
    case True{}:
      ga.pair_i(ga.u(p, "x"), ga.u(p, "y"))
    case False{}:
      J.JNull{}

# Everything a lock may hold: [lock, x, y, hostile] in order: foes, the
# Warden's heart, then the props that can be targeted.
def lanternward.kit.targets.foes(+fs: List<&2, J.Val>) -> List<&2, J.Val>:
  match fs:
    case Nil{}:
      Nil{}
    case Con{+f, +t}:
      Bool.pick(List<&2, J.Val>, lanternward.foes.Foe.alive(f),
        Con{J.JA{[lanternward.variant("Foe", J.JU{ga.u(f, "id")}), J.of_i32(ga.u(f, "x")), J.of_i32(ga.u(f, "y")), J.JBool{True{}}]},
          lanternward.kit.targets.foes(t)},
        lanternward.kit.targets.foes(t))

def lanternward.kit.targets.props(+ps: List<&2, J.Val>, +i: U32) -> List<&2, J.Val>:
  match ps:
    case Nil{}:
      Nil{}
    case Con{+p, +t}:
      Bool.pick(List<&2, J.Val>, lanternward.props.Prop.target(p),
        Con{J.JA{[lanternward.variant("Prop", J.JU{U32.and(i, 255)}), J.of_i32(ga.u(p, "x")), J.of_i32(ga.u(p, "y")), J.JBool{False{}}]},
          lanternward.kit.targets.props(t, (i + 1 : U32))},
        lanternward.kit.targets.props(t, (i + 1 : U32)))

def lanternward.kit.targets.boss(+w: J.Val) -> List<&2, J.Val>:
  match w:
    case J.JNull{}:
      Nil{}
    case _:
      Bool.pick(List<&2, J.Val>, U32.is_gt(ga.u(w, "hp"), 0),
        [J.JA{[J.JS{"Boss"}, J.of_i32(lanternward.boss.Warden.heart.x(w)), J.of_i32(lanternward.boss.Warden.heart.y(w)), J.JBool{True{}}]}],
        Nil{})

def lanternward.kit.Session.targets(+s: J.Val) -> List<&2, J.Val>:
  List.append(&2, J.Val, lanternward.kit.targets.foes(ga.items(s, "foes")),
    List.append(&2, J.Val, lanternward.kit.targets.boss(J.get(s, "boss")), lanternward.kit.targets.props(ga.items(s, "props"), 0)))

# Whether the knight's lock is of something still there and in reach.
def lanternward.kit.Session.lock_holds(+s: J.Val, +k: J.Val, +l: J.Val) -> Bool:
  lanternward.kit.lock_holds.pt(lanternward.kit.Session.lock_point(s, l), k, lanternward.R(s, "lock"))

def lanternward.kit.lock_holds.pt(+p: J.Val, +k: J.Val, +lock: U32) -> Bool:
  match p:
    case J.JNull{}:
      False{}
    case _:
      RS.i_le(lanternward.world.dist((ga.nu(p, 0) - ga.u(k, "x") : U32), (ga.nu(p, 1) - ga.u(k, "y") : U32)), RS.i_div((lock * 3 : U32), 2))

# The nearest thing in front of the knight, a foe before a switch; with
# nothing in front, the nearest at all. The key is (not hostile, not front, d).
def lanternward.kit.pick_target.go(+ts: List<&2, J.Val>, +s: J.Val, +k: J.Val, +skip: J.Val, +bk: J.Val, +bl: J.Val) -> J.Val:
  match ts:
    case Nil{}:
      bl
    case Con{+t, +rest}:
      lanternward.kit.pick_target.cand(
        Bool.not(J.eq(J.nth(t, 0), skip)),
        t, rest, s, k, skip, bk, bl)

def lanternward.kit.pick_target.cand(ok: Bool, +t: J.Val, +rest: List<&2, J.Val>, +s: J.Val, +k: J.Val, +skip: J.Val, +bk: J.Val, +bl: J.Val) -> J.Val:
  match ok:
    case False{}:
      lanternward.kit.pick_target.go(rest, s, k, skip, bk, bl)
    case True{}:
      lanternward.kit.pick_target.dist(
        lanternward.world.dist((ga.nu(t, 1) - ga.u(k, "x") : U32), (ga.nu(t, 2) - ga.u(k, "y") : U32)),
        t, rest, s, k, skip, bk, bl)

def lanternward.kit.pick_target.dist(+d: U32, +t: J.Val, +rest: List<&2, J.Val>, +s: J.Val, +k: J.Val, +skip: J.Val, +bk: J.Val, +bl: J.Val) -> J.Val:
  lanternward.kit.pick_target.reach(
    Bool.and(RS.i_le(d, lanternward.R(s, "lock")),
      lanternward.props.Session.line_open(s, ga.u(k, "x"), ga.u(k, "y"), ga.nu(t, 1), ga.nu(t, 2))),
    d, t, rest, s, k, skip, bk, bl)

def lanternward.kit.pick_target.reach(ok: Bool, +d: U32, +t: J.Val, +rest: List<&2, J.Val>, +s: J.Val, +k: J.Val, +skip: J.Val, +bk: J.Val, +bl: J.Val) -> J.Val:
  match ok:
    case False{}:
      lanternward.kit.pick_target.go(rest, s, k, skip, bk, bl)
    case True{}:
      lanternward.kit.pick_target.key(
        Bool.to_u32(Bool.not(J.bool(J.nth(t, 3)))),
        Bool.to_u32(RS.i_lt(lanternward.world.dot_unit(ga.nu(ga.v(k, "face"), 0), ga.nu(ga.v(k, "face"), 1), (ga.nu(t, 1) - ga.u(k, "x") : U32), (ga.nu(t, 2) - ga.u(k, "y") : U32)), 0)),
        d, t, rest, s, k, skip, bk, bl)

# Lexicographic key (a, b, d) < best.
def lanternward.kit.pick_target.key(+a: U32, +b: U32, +d: U32, +t: J.Val, +rest: List<&2, J.Val>, +s: J.Val, +k: J.Val, +skip: J.Val, +bk: J.Val, +bl: J.Val) -> J.Val:
  +kk = {J.JA{[J.JU{a}, J.JU{b}, J.of_i32(d)]} : J.Val}
  +better = Bool.or(J.is_null(bl), lanternward.kit.key_lt(a, b, d, bk))
  lanternward.kit.pick_target.go(rest, s, k, skip, Bool.pick(J.Val, better, kk, bk), Bool.pick(J.Val, better, J.nth(t, 0), bl))

def lanternward.kit.key_lt(+a: U32, +b: U32, +d: U32, +bk: J.Val) -> Bool:
  Bool.or(
    U32.is_lt(a, ga.nu(bk, 0)),
    Bool.and(U32.is_eq(a, ga.nu(bk, 0)),
      Bool.or(U32.is_lt(b, ga.nu(bk, 1)), Bool.and(U32.is_eq(b, ga.nu(bk, 1)), RS.i_lt(d, ga.nu(bk, 2))))))

def lanternward.kit.Session.pick_target(+s: J.Val, +k: J.Val, +skip: J.Val) -> J.Val:
  lanternward.kit.pick_target.go(lanternward.kit.Session.targets(s), s, k, skip, J.JNull{}, J.JNull{})

# The next target in the direction the knight pressed, as seen from the
# current one.
def lanternward.kit.cycle.go(+ts: List<&2, J.Val>, +s: J.Val, +k: J.Val, +cx: U32, +cy: U32, +wx: U32, +wy: U32, +bd: U32, +bl: J.Val) -> J.Val:
  match ts:
    case Nil{}:
      bl
    case Con{+t, +rest}:
      lanternward.kit.cycle.cand(
        Bool.and(
          Bool.not(J.eq(J.nth(t, 0), ga.v(k, "lock"))),
          Bool.and(
            RS.i_le(lanternward.world.dist((ga.nu(t, 1) - ga.u(k, "x") : U32), (ga.nu(t, 2) - ga.u(k, "y") : U32)), lanternward.R(s, "lock")),
            lanternward.props.Session.line_open(s, ga.u(k, "x"), ga.u(k, "y"), ga.nu(t, 1), ga.nu(t, 2)))),
        t, rest, s, k, cx, cy, wx, wy, bd, bl)

def lanternward.kit.cycle.cand(ok: Bool, +t: J.Val, +rest: List<&2, J.Val>, +s: J.Val, +k: J.Val, +cx: U32, +cy: U32, +wx: U32, +wy: U32, +bd: U32, +bl: J.Val) -> J.Val:
  match ok:
    case False{}:
      lanternward.kit.cycle.go(rest, s, k, cx, cy, wx, wy, bd, bl)
    case True{}:
      lanternward.kit.cycle.take(
        lanternward.world.dist((ga.nu(t, 1) - cx : U32), (ga.nu(t, 2) - cy : U32)),
        t, rest, s, k, cx, cy, wx, wy, bd, bl)

def lanternward.kit.cycle.take(+d: U32, +t: J.Val, +rest: List<&2, J.Val>, +s: J.Val, +k: J.Val, +cx: U32, +cy: U32, +wx: U32, +wy: U32, +bd: U32, +bl: J.Val) -> J.Val:
  +good = Bool.and(
    Bool.and(RS.i_gt(d, 0), RS.i_gt(lanternward.kit.cycle.dot((ga.nu(t, 1) - cx : U32), (ga.nu(t, 2) - cy : U32), wx, wy), 128)),
    Bool.or(J.is_null(bl), RS.i_lt(d, bd)))
  lanternward.kit.cycle.go(rest, s, k, cx, cy, wx, wy, Bool.pick(U32, good, d, bd), Bool.pick(J.Val, good, J.nth(t, 0), bl))

def lanternward.kit.cycle.dot(+vx: U32, +vy: U32, +wx: U32, +wy: U32) -> U32:
  lanternward.world.dot_unit(wx, wy, vx, vy)

def lanternward.kit.Session.cycle_target(+s: J.Val, +k: J.Val, +dx: U32, +dy: U32) -> J.Val:
  lanternward.kit.cycle_target.cur(lanternward.kit.Session.lock_point(s, ga.v(k, "lock")), s, k, dx, dy)

def lanternward.kit.cycle_target.cur(+cur: J.Val, +s: J.Val, +k: J.Val, +dx: U32, +dy: U32) -> J.Val:
  match cur:
    case J.JNull{}:
      J.JNull{}
    case _:
      lanternward.kit.cycle_target.want(lanternward.world.unit(dx, dy), cur, s, k)

def lanternward.kit.cycle_target.want(want: U32 & U32, +cur: J.Val, +s: J.Val, +k: J.Val) -> J.Val:
  (+wx, +wy) = want
  lanternward.kit.cycle.go(lanternward.kit.Session.targets(s), s, k, ga.nu(cur, 0), ga.nu(cur, 1), wx, wy, 0, J.JNull{})

# ---- one tick of one knight ----

def lanternward.kit.act_code(+a: String) -> U32:
  Bool.pick(U32, String.eq(a, "Free"), 0,
    Bool.pick(U32, String.eq(a, "Slash"), 1,
      Bool.pick(U32, String.eq(a, "Lunge"), 2,
        Bool.pick(U32, String.eq(a, "Spin"), 3,
          Bool.pick(U32, String.eq(a, "Roll"), 4,
            Bool.pick(U32, String.eq(a, "Hop"), 5,
              Bool.pick(U32, String.eq(a, "Plunge"), 6,
                Bool.pick(U32, String.eq(a, "Hurt"), 7,
                  Bool.pick(U32, String.eq(a, "Item"), 8,
                    Bool.pick(U32, String.eq(a, "Down"), 9, 10))))))))))

# Put the knight in `$k` back at the table.
def lanternward.kit.put_k(+s: J.Val, +id: U32) -> J.Val:
  J.del(ga.sv(s, "seats", ga.put_at(J.get(s, "seats"), id, J.get(s, "$k"))), "$k")

# One tick of one knight.
def lanternward.kit.Session.step_seat(+s: J.Val, +id: U32, +inp: J.Val) -> J.Val:
  lanternward.kit.step_seat.has(ga.has_key(J.get(s, "seats"), id), s, id, inp)

def lanternward.kit.step_seat.has(has: Bool, +s: J.Val, +id: U32, +inp: J.Val) -> J.Val:
  match has:
    case False{}:
      s
    case True{}:
      lanternward.kit.step_seat.take(
        ga.sv(ga.sv(s, "$k", ga.at(J.get(s, "seats"), id)), "seats", J.del(J.get(s, "seats"), U32.show(id))),
        id, inp, lanternward.kit.buttons(inp))

def lanternward.kit.step_seat.take(+s: J.Val, +id: U32, +inp: J.Val, +mask: U32) -> J.Val:
  lanternward.kit.step_seat.pre(
    lanternward.ks(s, "held", mask), id, inp, mask, U32.and(mask, U32.not(lanternward.ku(s, "held"))))

def lanternward.kit.step_seat.pre(+s: J.Val, +id: U32, +inp: J.Val, +mask: U32, +press: U32) -> J.Val:
  +s1 = lanternward.ks(lanternward.ks(lanternward.ks(lanternward.ks(lanternward.ks(lanternward.ks(s,
    "inv", RS.u_sat_sub(lanternward.ku(s, "inv"), 1)),
    "ward", RS.u_sat_sub(lanternward.ku(s, "ward"), 1)),
    "taper", RS.u_sat_sub(lanternward.ku(s, "taper"), 1)),
    "guard_cd", RS.u_sat_sub(lanternward.ku(s, "guard_cd"), 1)),
    "lamp_cd", RS.u_sat_sub(lanternward.ku(s, "lamp_cd"), 1)),
    "chain", RS.u_sat_sub(lanternward.ku(s, "chain"), 1))
  +s2 = lanternward.kit.step_seat.unchain(Bool.and(U32.is_eq(lanternward.ku(s1, "chain"), 0), Bool.not(lanternward.kb(s1, "queued"))), s1)
  +s3 = lanternward.ks(lanternward.ks(s2, "unlock_t", RS.u_sat_sub(lanternward.ku(s2, "unlock_t"), 1)), "lock_t", RS.u_min((lanternward.ku(s2, "lock_t") + 1 : U32), 255))
  lanternward.kit.step_seat.freeze(
    lanternward.kit.Session.refill(lanternward.kit.step_seat.read(J.is_null(lanternward.kv(s3, "read")), s3)),
    id, inp, mask, press)

def lanternward.kit.step_seat.unchain(yes: Bool, +s: J.Val) -> J.Val:
  match yes:
    case True{}:
      lanternward.ks(s, "step", 0)
    case False{}:
      s

# A sign being read counts down and is put away at zero.
def lanternward.kit.step_seat.read(none: Bool, +s: J.Val) -> J.Val:
  match none:
    case True{}:
      s
    case False{}:
      lanternward.kit.step_seat.read.go(RS.u_sat_sub(ga.nu(lanternward.kv(s, "read"), 1), 1), s)

def lanternward.kit.step_seat.read.go(+t: U32, +s: J.Val) -> J.Val:
  lanternward.ksv(s, "read",
    Bool.pick(J.Val, U32.is_eq(t, 0), J.JNull{}, ga.pair(J.nth(lanternward.kv(s, "read"), 0), J.JU{t})))

# A blade that landed holds its owner a moment.
def lanternward.kit.step_seat.freeze(+s: J.Val, +id: U32, +inp: J.Val, +mask: U32, +press: U32) -> J.Val:
  lanternward.kit.step_seat.freeze.go(U32.is_gt(lanternward.ku(s, "freeze"), 0), s, id, inp, mask, press)

def lanternward.kit.step_seat.freeze.go(held: Bool, +s: J.Val, +id: U32, +inp: J.Val, +mask: U32, +press: U32) -> J.Val:
  match held:
    case True{}:
      lanternward.kit.put_k(lanternward.kit.Session.camera(lanternward.ks(s, "freeze", (lanternward.ku(s, "freeze") - 1 : U32))), id)
    case False{}:
      lanternward.kit.step_seat.act(lanternward.kit.act_code(ga.s(J.get(s, "$k"), "act")), s, id, inp, mask, press)

def lanternward.kit.step_seat.act(+code: U32, +s: J.Val, +id: U32, +inp: J.Val, +mask: U32, +press: U32) -> J.Val:
  lanternward.kit.step_seat.fin(
    lanternward.kit.step_seat.act.go(code, s, id, inp, mask, press), id)

def lanternward.kit.step_seat.act.go(+code: U32, +s: J.Val, +id: U32, +inp: J.Val, +mask: U32, +press: U32) -> J.Val:
  match code:
    case 9:
      lanternward.ksv(lanternward.ks(lanternward.ks(s, "t", RS.u_min((lanternward.ku(s, "t") + 1 : U32), 4000)), "shield", 0), "lock", J.JNull{})
    case 8:
      lanternward.kit.step_seat.item(U32.is_ge((lanternward.ku(s, "t") + 1 : U32), 60), lanternward.ks(s, "t", (lanternward.ku(s, "t") + 1 : U32)))
    case 10:
      lanternward.kit.Session.falling(s)
    case 7:
      lanternward.kit.Session.hurting(s)
    case _:
      lanternward.kit.Session.verbs(
        lanternward.kit.Session.face_and_move(lanternward.kit.Session.lock_on(s, id, mask, press, inp), inp),
        id, mask, press, inp)

# Holding an item overhead, then putting it down.
def lanternward.kit.step_seat.item(done: Bool, +s: J.Val) -> J.Val:
  match done:
    case False{}:
      s
    case True{}:
      lanternward.ks(lanternward.ksv(lanternward.ksn(s, "act", "Free"), "item", J.JNull{}), "t", 0)

# Things that go on whatever the knight is doing, then the camera, then back
# to the table.
def lanternward.kit.step_seat.fin(+s: J.Val, +id: U32) -> J.Val:
  lanternward.kit.put_k(lanternward.kit.Session.camera(lanternward.kit.Session.upkeep(s)), id)

# The lantern's flame comes back: slowly anywhere, quickly at a resting
# brazier or beside a friend's flare.
def lanternward.kit.refill.rest(+ps: List<&2, J.Val>, +kx: U32, +ky: U32) -> Bool:
  match ps:
    case Nil{}:
      False{}
    case Con{+p, +t}:
      Bool.or(
        Bool.and(String.eq(ga.s(p, "kind"), "Rest"),
          RS.i_lt(lanternward.world.dist((ga.u(p, "x") - kx : U32), (ga.u(p, "y") - ky : U32)), 16384)),
        lanternward.kit.refill.rest(t, kx, ky))

def lanternward.kit.refill.kindled(+ids: List<&2, U32>, +seats: J.Val, +kid: U32, +kx: U32, +ky: U32) -> Bool:
  match ids:
    case Nil{}:
      False{}
    case Con{+id, +t}:
      Bool.or(
        Bool.and(
          Bool.and(U32.is_ne(ga.u(ga.at(seats, id), "id"), kid), lanternward.kit.Seat.flaring(ga.at(seats, id))),
          RS.i_lt(lanternward.world.dist((ga.u(ga.at(seats, id), "x") - kx : U32), (ga.u(ga.at(seats, id), "y") - ky : U32)), 7168)),
        lanternward.kit.refill.kindled(t, seats, kid, kx, ky))

def lanternward.kit.Session.refill(+s: J.Val) -> J.Val:
  lanternward.kit.refill.owned(lanternward.kit.Session.lantern_owned(s), s)

def lanternward.kit.refill.owned(owned: Bool, +s: J.Val) -> J.Val:
  match owned:
    case False{}:
      s
    case True{}:
      lanternward.kit.refill.cap(
        lanternward.ks(s, "flame", RS.u_min(lanternward.ku(s, "flame"), lanternward.R(s, "flame"))))

def lanternward.kit.refill.cap(+s: J.Val) -> J.Val:
  lanternward.kit.refill.flaring(lanternward.kit.Seat.flaring(J.get(s, "$k")), s)

def lanternward.kit.refill.flaring(flaring: Bool, +s: J.Val) -> J.Val:
  match flaring:
    case True{}:
      s
    case False{}:
      lanternward.ks(
        s, "flame",
        RS.u_min(
          (lanternward.ku(s, "flame") + Bool.pick(U32, lanternward.kit.refill.rest(ga.items(s, "props"), lanternward.ku(s, "x"), lanternward.ku(s, "y")), 3,
            Bool.pick(U32, lanternward.kit.refill.kindled(ga.ids(J.get(s, "seats")), J.get(s, "seats"), lanternward.ku(s, "id"), lanternward.ku(s, "x"), lanternward.ku(s, "y")), 2,
              Bool.pick(U32, U32.is_eq(U32.mod(ga.u(s, "elapsed"), 2), 0), 1, 0))) : U32),
          lanternward.R(s, "flame")))

def lanternward.kit.Session.upkeep(+s: J.Val) -> J.Val:
  lanternward.kit.upkeep.ctx(lanternward.kit.upkeep.beep(lanternward.kit.upkeep.safe(s)))

# The last spot the knight stood on solid ground not beside a ledge.
def lanternward.kit.upkeep.safe(+s: J.Val) -> J.Val:
  lanternward.kit.upkeep.safe.go(
    Bool.and(
      Bool.and(lanternward.kis(s, "act", "Free"), U32.is_eq(lanternward.ku(s, "z"), 0)),
      Bool.not(lanternward.props.Session.pit(s, lanternward.ku(s, "x"), lanternward.ku(s, "y")))),
    s)

def lanternward.kit.upkeep.safe.go(ok: Bool, +s: J.Val) -> J.Val:
  match ok:
    case False{}:
      s
    case True{}:
      lanternward.kit.upkeep.safe.set(
        Bool.and(
          Bool.not(lanternward.kit.upkeep.ledge(ga.u(s, "room"), lanternward.world.tile_of(lanternward.ku(s, "x")), lanternward.world.tile_of(lanternward.ku(s, "y")))),
          lanternward.props.Session.clear(s, lanternward.ku(s, "x"), lanternward.ku(s, "y"), 1280)),
        s)

def lanternward.kit.upkeep.safe.set(ok: Bool, +s: J.Val) -> J.Val:
  match ok:
    case False{}:
      s
    case True{}:
      lanternward.ksv(s, "safe", ga.pair_i(lanternward.ku(s, "x"), lanternward.ku(s, "y")))

def lanternward.kit.upkeep.ledge(+room: U32, +tx: U32, +ty: U32) -> Bool:
  Bool.or(
    Bool.or(
      Bool.or(lanternward.kit.upkeep.pit_at(room, (tx - 1 : U32), (ty - 1 : U32)), lanternward.kit.upkeep.pit_at(room, tx, (ty - 1 : U32))),
      Bool.or(lanternward.kit.upkeep.pit_at(room, (tx + 1 : U32), (ty - 1 : U32)), lanternward.kit.upkeep.pit_at(room, (tx - 1 : U32), ty))),
    Bool.or(
      Bool.or(lanternward.kit.upkeep.pit_at(room, tx, ty), lanternward.kit.upkeep.pit_at(room, (tx + 1 : U32), ty)),
      Bool.or(
        Bool.or(lanternward.kit.upkeep.pit_at(room, (tx - 1 : U32), (ty + 1 : U32)), lanternward.kit.upkeep.pit_at(room, tx, (ty + 1 : U32))),
        lanternward.kit.upkeep.pit_at(room, (tx + 1 : U32), (ty + 1 : U32)))))

def lanternward.kit.upkeep.pit_at(+room: U32, +tx: U32, +ty: U32) -> Bool:
  U32.is_eq(lanternward.world.Plan.at(room, tx, ty), 61)

# The heart beep: a low knight chirps now and then.
def lanternward.kit.upkeep.beep(+s: J.Val) -> J.Val:
  lanternward.kit.upkeep.beep.go(
    Bool.and(lanternward.kit.Seat.up(J.get(s, "$k")), U32.is_le(lanternward.ku(s, "hp"), 8)), s)

def lanternward.kit.upkeep.beep.go(low: Bool, +s: J.Val) -> J.Val:
  match low:
    case False{}:
      lanternward.ks(s, "beep", 0)
    case True{}:
      lanternward.kit.upkeep.beep.chirp(
        U32.mod((lanternward.ku(s, "beep") + 1 : U32), 160), s)

def lanternward.kit.upkeep.beep.chirp(+b: U32, +s: J.Val) -> J.Val:
  lanternward.kit.upkeep.beep.sound(U32.is_eq(U32.mod(b, 40), 1), lanternward.ks(s, "beep", b))

def lanternward.kit.upkeep.beep.sound(yes: Bool, +s: J.Val) -> J.Val:
  match yes:
    case True{}:
      lanternward.sound(s, "mana_empty")
    case False{}:
      s

def lanternward.kit.upkeep.ctx(+s: J.Val) -> J.Val:
  lanternward.kit.upkeep.ctx.go(lanternward.kis(s, "act", "Free"), s)

def lanternward.kit.upkeep.ctx.go(free: Bool, +s: J.Val) -> J.Val:
  match free:
    case True{}:
      lanternward.ksv(s, "ctx", lanternward.ctx.Session.context(s, J.get(s, "$k")))
    case False{}:
      lanternward.ksv(s, "ctx", J.JS{"None"})

# ---- lock-on ----

def lanternward.kit.Session.lock_on(+s: J.Val, +id: U32, +mask: U32, +press: U32, +inp: J.Val) -> J.Val:
  lanternward.kit.lock_on.go(
    Bool.and(U32.is_ne(U32.and(mask, 1), 0), lanternward.kit.Seat.up(J.get(s, "$k"))), s, press, inp)

def lanternward.kit.lock_on.go(holding: Bool, +s: J.Val, +press: U32, +inp: J.Val) -> J.Val:
  match holding:
    case False{}:
      lanternward.ksv(
        lanternward.ks(
          lanternward.kit.lock_on.drop(Bool.not(J.is_null(lanternward.kv(s, "lock"))), s), "relock", 0),
        "aim", ga.pair(J.get(inp, "aim_x"), J.get(inp, "aim_y")))
    case True{}:
      lanternward.kit.lock_on.keep(
        Bool.and(Bool.not(J.is_null(lanternward.kv(s, "lock"))),
          Bool.not(lanternward.kit.Session.lock_holds(s, J.get(s, "$k"), lanternward.kv(s, "lock")))),
        s, press, inp)

def lanternward.kit.lock_on.drop(had: Bool, +s: J.Val) -> J.Val:
  lanternward.ksv(
    Bool.pick(J.Val, had, lanternward.ks(s, "unlock_t", 10), s), "lock", J.JNull{})

# A lock that no longer holds lets go and waits a few ticks to re-lock.
def lanternward.kit.lock_on.keep(lost: Bool, +s: J.Val, +press: U32, +inp: J.Val) -> J.Val:
  match lost:
    case True{}:
      lanternward.kit.lock_on.fresh(lanternward.kit.lock_on.lost(s), press, inp)
    case False{}:
      lanternward.kit.lock_on.fresh(s, press, inp)

def lanternward.kit.lock_on.lost(+s: J.Val) -> J.Val:
  lanternward.ks(lanternward.ksv(s, "lock", J.JNull{}), "relock", 6)

# A fresh press of an aim key while locked cycles to the next target that way.
def lanternward.kit.lock_on.fresh(+s: J.Val, +press: U32, +inp: J.Val) -> J.Val:
  lanternward.kit.lock_on.cycle(
    Bool.and(
      Bool.or(U32.is_ne(ga.u(inp, "aim_x"), 0), U32.is_ne(ga.u(inp, "aim_y"), 0)),
      Bool.not(Bool.and(U32.is_eq(ga.u(inp, "aim_x"), ga.nu(lanternward.kv(s, "aim"), 0)), U32.is_eq(ga.u(inp, "aim_y"), ga.nu(lanternward.kv(s, "aim"), 1))))),
    ga.pair(J.get(inp, "aim_x"), J.get(inp, "aim_y")),
    s, press, inp)

def lanternward.kit.lock_on.cycle(+fresh: Bool, +aim: J.Val, +s: J.Val, +press: U32, +inp: J.Val) -> J.Val:
  lanternward.kit.lock_on.branch(
    Bool.and(Bool.not(J.is_null(lanternward.kv(s, "lock"))), fresh),
    J.is_null(lanternward.kv(s, "lock")),
    ga.sv(s, "$k", J.set(J.get(s, "$k"), "aim", aim)), press, inp)

def lanternward.kit.lock_on.branch(+cycling: Bool, +unlocked: Bool, +s: J.Val, +press: U32, +inp: J.Val) -> J.Val:
  Bool.pick(J.Val, cycling,
    lanternward.kit.lock_on.cycled(
      lanternward.kit.Session.cycle_target(s, J.get(s, "$k"), ga.u(inp, "aim_x"), ga.u(inp, "aim_y")), s),
    Bool.pick(J.Val, unlocked,
      lanternward.kit.lock_on.seek(Bool.or(U32.is_ne(U32.and(press, 1), 0), U32.is_eq(lanternward.ku(s, "relock"), 0)), s, press),
      s))

def lanternward.kit.lock_on.cycled(+l: J.Val, +s: J.Val) -> J.Val:
  match l:
    case J.JNull{}:
      s
    case _:
      lanternward.kit.Session.take_lock(s, l)

def lanternward.kit.lock_on.seek(look: Bool, +s: J.Val, +press: U32) -> J.Val:
  match look:
    case False{}:
      lanternward.ks(s, "relock", (lanternward.ku(s, "relock") - 1 : U32))
    case True{}:
      lanternward.kit.lock_on.found(
        lanternward.kit.Session.pick_target(s, J.get(s, "$k"), J.JNull{}), s, U32.is_ne(U32.and(press, 1), 0))

def lanternward.kit.lock_on.found(+l: J.Val, +s: J.Val, +pressed: Bool) -> J.Val:
  match l:
    case J.JNull{}:
      Bool.pick(J.Val, pressed, lanternward.ks(s, "unlock_t", 10), s)
    case _:
      lanternward.kit.Session.take_lock(s, l)

def lanternward.kit.Session.take_lock(+s: J.Val, +l: J.Val) -> J.Val:
  lanternward.kit.take_lock.at(
    lanternward.kit.Session.lock_point(s, l),
    lanternward.ks(lanternward.ksv(s, "lock", l), "lock_t", 0))

def lanternward.kit.take_lock.at(+pt: J.Val, +s: J.Val) -> J.Val:
  match pt:
    case J.JNull{}:
      lanternward.sound(s, "card_pickup")
    case _:
      lanternward.kit.take_lock.face(
        lanternward.world.unit((ga.nu(pt, 0) - lanternward.ku(s, "x") : U32), (ga.nu(pt, 1) - lanternward.ku(s, "y") : U32)),
        lanternward.props.Session.fx(s, "Tick", ga.nu(pt, 0), ga.nu(pt, 1), 0))

def lanternward.kit.take_lock.face(f: U32 & U32, +s: J.Val) -> J.Val:
  (+fx, +fy) = f
  lanternward.sound(
    Bool.pick(J.Val, Bool.and(U32.is_eq(fx, 0), U32.is_eq(fy, 0)), s, lanternward.ksv(s, "face", ga.pair_i(fx, fy))),
    "card_pickup")

# ---- facing and walking ----

# Face a locked target, else the aim keys, else the way walked.
def lanternward.kit.Session.face_and_move(+s: J.Val, +inp: J.Val) -> J.Val:
  lanternward.kit.face_and_move.go(
    lanternward.kit.Session.lock_point(s, lanternward.kv(s, "lock")), s, inp)

def lanternward.kit.face_and_move.go(+pt: J.Val, +s: J.Val, +inp: J.Val) -> J.Val:
  match pt:
    case J.JNull{}:
      lanternward.kit.face_and_move.aim(
        Bool.or(U32.is_ne(ga.u(inp, "aim_x"), 0), U32.is_ne(ga.u(inp, "aim_y"), 0)), s, inp)
    case _:
      lanternward.kit.face_and_move.lock(
        lanternward.world.unit((ga.nu(pt, 0) - lanternward.ku(s, "x") : U32), (ga.nu(pt, 1) - lanternward.ku(s, "y") : U32)), s, inp)

def lanternward.kit.face_and_move.lock(f: U32 & U32, +s: J.Val, +inp: J.Val) -> J.Val:
  (+fx, +fy) = f
  lanternward.kit.face_and_move.move(
    Bool.pick(J.Val, Bool.and(U32.is_eq(fx, 0), U32.is_eq(fy, 0)), s, lanternward.ksv(s, "face", ga.pair_i(fx, fy))), inp)

def lanternward.kit.face_and_move.aim(has: Bool, +s: J.Val, +inp: J.Val) -> J.Val:
  match has:
    case True{}:
      lanternward.kit.face_and_move.face(lanternward.world.unit(ga.u(inp, "aim_x"), ga.u(inp, "aim_y")), s, inp)
    case False{}:
      lanternward.kit.face_and_move.walk(
        Bool.and(
          Bool.and(
            Bool.or(U32.is_ne(ga.u(inp, "move_x"), 0), U32.is_ne(ga.u(inp, "move_y"), 0)),
            U32.is_eq(lanternward.ku(s, "shield"), 0)),
          lanternward.kis(s, "act", "Free")),
        s, inp)

def lanternward.kit.face_and_move.walk(go: Bool, +s: J.Val, +inp: J.Val) -> J.Val:
  match go:
    case True{}:
      lanternward.kit.face_and_move.face(lanternward.world.unit(ga.u(inp, "move_x"), ga.u(inp, "move_y")), s, inp)
    case False{}:
      lanternward.kit.face_and_move.move(s, inp)

def lanternward.kit.face_and_move.face(f: U32 & U32, +s: J.Val, +inp: J.Val) -> J.Val:
  (+fx, +fy) = f
  lanternward.kit.face_and_move.move(lanternward.ksv(s, "face", ga.pair_i(fx, fy)), inp)

def lanternward.kit.face_and_move.move(+s: J.Val, +inp: J.Val) -> J.Val:
  lanternward.kit.face_and_move.push(lanternward.kis(s, "act", "Free"), s, inp)

# Only a free knight is carried by a push and walks.
def lanternward.kit.face_and_move.push(free: Bool, +s: J.Val, +inp: J.Val) -> J.Val:
  match free:
    case False{}:
      s
    case True{}:
      lanternward.kit.face_and_move.walking(
        lanternward.kit.Session.shove_push(U32.is_gt(lanternward.ku(s, "pusht"), 0), s), inp)

def lanternward.kit.Session.shove_push(yes: Bool, +s: J.Val) -> J.Val:
  match yes:
    case False{}:
      s
    case True{}:
      lanternward.kit.Session.shove(
        lanternward.ks(s, "pusht", (lanternward.ku(s, "pusht") - 1 : U32)),
        ga.nu(lanternward.kv(s, "push"), 0), ga.nu(lanternward.kv(s, "push"), 1))

def lanternward.kit.face_and_move.walking(+s: J.Val, +inp: J.Val) -> J.Val:
  lanternward.kit.face_and_move.speed(
    Bool.and(U32.is_eq(ga.u(inp, "move_x"), 0), U32.is_eq(ga.u(inp, "move_y"), 0)), s, inp)

def lanternward.kit.face_and_move.speed(still: Bool, +s: J.Val, +inp: J.Val) -> J.Val:
  match still:
    case True{}:
      s
    case False{}:
      lanternward.kit.face_and_move.step(
        Bool.pick(U32, U32.is_gt(lanternward.ku(s, "shield"), 0), 230,
          Bool.pick(U32, Bool.or(lanternward.kis(s, "carry", "Pot"), U32.is_gt(lanternward.ku(s, "charge"), 12)), 260,
            Bool.pick(U32, Bool.not(J.is_null(lanternward.kv(s, "lock"))), 340, 440))),
        lanternward.world.unit(ga.u(inp, "move_x"), ga.u(inp, "move_y")), s)

def lanternward.kit.face_and_move.step(+speed: U32, v: U32 & U32, +s: J.Val) -> J.Val:
  (+vx, +vy) = v
  lanternward.kit.face_and_move.go2(
    RS.i_div((vx * speed : U32), 256), RS.i_div((vy * speed : U32), 256), vx, vy, s,
    lanternward.kit.Session.lock_point(s, lanternward.kv(s, "lock")))

def lanternward.kit.face_and_move.go2(+wx: U32, +wy: U32, +vx: U32, +vy: U32, +s: J.Val, +pt: J.Val) -> J.Val:
  match pt:
    case J.JNull{}:
      lanternward.kit.Session.shove(s, wx, wy)
    case _:
      lanternward.kit.Session.orbit(s, ga.nu(pt, 0), ga.nu(pt, 1), wx, wy)

# Walk about a locked target: the sideways part of the step goes round it at
# the same distance, the rest closes or opens it.
def lanternward.kit.Session.orbit(+s: J.Val, +tx: U32, +ty: U32, +vx: U32, +vy: U32) -> J.Val:
  lanternward.kit.orbit.go(
    (lanternward.ku(s, "x") - tx : U32), (lanternward.ku(s, "y") - ty : U32), s, tx, ty, vx, vy)

def lanternward.kit.orbit.go(+rx: U32, +ry: U32, +s: J.Val, +tx: U32, +ty: U32, +vx: U32, +vy: U32) -> J.Val:
  lanternward.kit.orbit.len(lanternward.world.dist(rx, ry), rx, ry, s, tx, ty, vx, vy)

def lanternward.kit.orbit.len(+len: U32, +rx: U32, +ry: U32, +s: J.Val, +tx: U32, +ty: U32, +vx: U32, +vy: U32) -> J.Val:
  lanternward.kit.orbit.len.go(RS.i_lt(len, 256), len, rx, ry, s, tx, ty, vx, vy)

def lanternward.kit.orbit.len.go(near: Bool, +len: U32, +rx: U32, +ry: U32, +s: J.Val, +tx: U32, +ty: U32, +vx: U32, +vy: U32) -> J.Val:
  match near:
    case True{}:
      lanternward.kit.Session.shove(s, vx, vy)
    case False{}:
      lanternward.kit.orbit.rhat(lanternward.world.unit(rx, ry), len, s, tx, ty, vx, vy)

def lanternward.kit.orbit.rhat(rhat: U32 & U32, +len: U32, +s: J.Val, +tx: U32, +ty: U32, +vx: U32, +vy: U32) -> J.Val:
  (+hx, +hy) = rhat
  lanternward.kit.orbit.split(
    lanternward.world.dot4(vx, vy, hx, hy), lanternward.world.dot4(vx, vy, RS.i_neg(hy), hx),
    RS.i_neg(hy), hx, hx, hy, lanternward.world.dist(vx, vy), len, s, tx, ty)

def lanternward.kit.orbit.split(+radial: U32, +tangent: U32, +thx: U32, +thy: U32, +hx: U32, +hy: U32, +speed: U32, +len: U32, +s: J.Val, +tx: U32, +ty: U32) -> J.Val:
  +side = RS.i_ge((RS.i_abs(tangent) * 10 : U32), (speed * 8 : U32))
  +fore = RS.i_ge((RS.i_abs(radial) * 10 : U32), (speed * 8 : U32))
  lanternward.kit.orbit.want(
    Bool.pick(U32, side, 0, radial), Bool.pick(U32, side, tangent, Bool.pick(U32, fore, 0, tangent)),
    thx, thy, hx, hy, len, s, tx, ty)

def lanternward.kit.orbit.want(+radial: U32, +tangent: U32, +thx: U32, +thy: U32, +hx: U32, +hy: U32, +len: U32, +s: J.Val, +tx: U32, +ty: U32) -> J.Val:
  lanternward.kit.orbit.off(
    RS.i_max(RS.i_add(len, radial), 3072),
    RS.i_add(lanternward.world.mul_div(hx, len, 256), RS.i_div((tangent * thx : U32), 256)),
    RS.i_add(lanternward.world.mul_div(hy, len, 256), RS.i_div((tangent * thy : U32), 256)),
    s, tx, ty)

def lanternward.kit.orbit.off(+want: U32, +ox: U32, +oy: U32, +s: J.Val, +tx: U32, +ty: U32) -> J.Val:
  lanternward.kit.orbit.to(lanternward.world.unit(ox, oy), want, s, tx, ty)

def lanternward.kit.orbit.to(u: U32 & U32, +want: U32, +s: J.Val, +tx: U32, +ty: U32) -> J.Val:
  (+ux, +uy) = u
  lanternward.kit.Session.shove(
    s,
    (tx + lanternward.world.mul_div(ux, want, 256) - lanternward.ku(s, "x") : U32),
    (ty + lanternward.world.mul_div(uy, want, 256) - lanternward.ku(s, "y") : U32))

# Move a body by (dx, dy), a whole axis at a time so it slides along a wall;
# leans on a block it meets; falls into a pit it walks into.
def lanternward.kit.Session.shove(+s: J.Val, +dx: U32, +dy: U32) -> J.Val:
  lanternward.kit.shove.fall(
    Bool.not(Bool.or(lanternward.kit.Seat.airborne(J.get(s, "$k")), lanternward.NAME_IN(ga.s(J.get(s, "$k"), "act"), ["Roll", "Hop", "Plunge"]))),
    lanternward.kit.shove.y(lanternward.kit.shove.x(s, dx), dy))

def lanternward.kit.shove.x(+s: J.Val, +dx: U32) -> J.Val:
  lanternward.kit.shove.x.go(U32.is_ne(dx, 0), s, dx)

def lanternward.kit.shove.x.go(moving: Bool, +s: J.Val, +dx: U32) -> J.Val:
  match moving:
    case False{}:
      s
    case True{}:
      lanternward.kit.shove.x.try(
        lanternward.props.Session.clear(s, (lanternward.ku(s, "x") + dx : U32), lanternward.ku(s, "y"), 1280), s, dx)

def lanternward.kit.shove.x.try(ok: Bool, +s: J.Val, +dx: U32) -> J.Val:
  match ok:
    case True{}:
      lanternward.ksi(s, "x", (lanternward.ku(s, "x") + dx : U32))
    case False{}:
      lanternward.kit.Session.lean(s, RS.i_signum(dx), 0)

def lanternward.kit.shove.y(+s: J.Val, +dy: U32) -> J.Val:
  lanternward.kit.shove.y.go(U32.is_ne(dy, 0), s, dy)

def lanternward.kit.shove.y.go(moving: Bool, +s: J.Val, +dy: U32) -> J.Val:
  match moving:
    case False{}:
      s
    case True{}:
      lanternward.kit.shove.y.try(
        lanternward.props.Session.clear(s, lanternward.ku(s, "x"), (lanternward.ku(s, "y") + dy : U32), 1280), s, dy)

def lanternward.kit.shove.y.try(ok: Bool, +s: J.Val, +dy: U32) -> J.Val:
  match ok:
    case True{}:
      lanternward.ksi(s, "y", (lanternward.ku(s, "y") + dy : U32))
    case False{}:
      lanternward.kit.Session.lean(s, 0, RS.i_signum(dy))

# `airborne` was decided before the move (Rust reads it first).
def lanternward.kit.shove.fall(+ground: Bool, +s: J.Val) -> J.Val:
  lanternward.kit.shove.fall.go(
    Bool.and(Bool.and(ground, lanternward.props.Session.pit(s, lanternward.ku(s, "x"), lanternward.ku(s, "y"))), lanternward.kis(s, "act", "Free")),
    s)

def lanternward.kit.shove.fall.go(drop: Bool, +s: J.Val) -> J.Val:
  match drop:
    case False{}:
      s
    case True{}:
      lanternward.ks(lanternward.ks(lanternward.ksv(lanternward.ks(lanternward.ksn(s, "act", "Fall"), "t", 0), "lock", J.JNull{}), "lamp", 0), "shield", 0)

# A body pressing on a block: after a moment of leaning, it slides.
def lanternward.kit.Session.lean(+s: J.Val, +sx: U32, +sy: U32) -> J.Val:
  lanternward.kit.lean.find(
    lanternward.kit.lean.block(ga.items(s, "props"), 0,
      (lanternward.ku(s, "x") + sx * 2048 : U32), (lanternward.ku(s, "y") + sy * 2048 : U32)),
    s, sx, sy)

def lanternward.kit.lean.block(+ps: List<&2, J.Val>, +i: U32, +x: U32, +y: U32) -> J.Val:
  match ps:
    case Nil{}:
      J.JNull{}
    case Con{+p, +t}:
      lanternward.kit.lean.block.hit(
        Bool.and(String.eq(ga.s(p, "kind"), "Block"), Bool.and(U32.is_eq(ga.u(p, "on"), 0), lanternward.props.Prop.blocks(p, x, y))),
        t, i, x, y)

def lanternward.kit.lean.block.hit(hit: Bool, +t: List<&2, J.Val>, +i: U32, +x: U32, +y: U32) -> J.Val:
  match hit:
    case True{}:
      J.JU{i}
    case False{}:
      lanternward.kit.lean.block(t, (i + 1 : U32), x, y)

def lanternward.kit.lean.find(+b: J.Val, +s: J.Val, +sx: U32, +sy: U32) -> J.Val:
  match b:
    case J.JNull{}:
      s
    case _:
      lanternward.kit.lean.push(J.u32(b), lanternward.es(s, "props", J.u32(b), "t", RS.u_min((lanternward.eu(s, "props", J.u32(b), "t") + 2 : U32), 65535)), sx, sy)

def lanternward.kit.lean.push(+b: U32, +s: J.Val, +sx: U32, +sy: U32) -> J.Val:
  lanternward.kit.lean.slide(U32.is_ge(lanternward.eu(s, "props", b, "t"), 10), b, s, sx, sy)

def lanternward.kit.lean.slide(go: Bool, +b: U32, +s: J.Val, +sx: U32, +sy: U32) -> J.Val:
  match go:
    case False{}:
      s
    case True{}:
      lanternward.ctx.Session.start_slide(
        lanternward.es(lanternward.es(s, "props", b, "t", 0), "props", b, "on",
          Bool.pick(U32, U32.is_eq(sx, 1), 1, Bool.pick(U32, U32.is_eq(sx, 4294967295), 2, Bool.pick(U32, U32.is_eq(sy, 1), 3, 4)))),
        b)

def lanternward.kit.Session.falling(+s: J.Val) -> J.Val:
  lanternward.kit.falling.go(U32.is_ge((lanternward.ku(s, "t") + 1 : U32), 18), lanternward.ks(s, "t", (lanternward.ku(s, "t") + 1 : U32)))

def lanternward.kit.falling.go(done: Bool, +s: J.Val) -> J.Val:
  match done:
    case False{}:
      s
    case True{}:
      lanternward.ks(
        lanternward.ks(
          lanternward.ks(
            lanternward.ksn(
              lanternward.ksi(lanternward.ksi(lanternward.ks(s, "hp", RS.u_max(RS.u_sat_sub(lanternward.ku(s, "hp"), 2), 1)),
                "x", ga.nu(lanternward.kv(s, "safe"), 0)), "y", ga.nu(lanternward.kv(s, "safe"), 1)),
              "act", "Free"),
            "t", 0),
          "inv", 36),
        "t", 0)

def lanternward.kit.Session.hurting(+s: J.Val) -> J.Val:
  lanternward.kit.hurting.go(
    lanternward.kit.Session.shove_push(U32.is_gt(lanternward.ku(s, "pusht"), 0), lanternward.ks(s, "t", (lanternward.ku(s, "t") + 1 : U32))))

def lanternward.kit.hurting.go(+s: J.Val) -> J.Val:
  lanternward.kit.hurting.end(U32.is_ge(lanternward.ku(s, "t"), 10), s)

def lanternward.kit.hurting.end(done: Bool, +s: J.Val) -> J.Val:
  match done:
    case False{}:
      s
    case True{}:
      lanternward.ks(lanternward.ksn(s, "act", "Free"), "t", 0)

# ---- the verbs ----

def lanternward.kit.Session.verbs(+s: J.Val, +id: U32, +mask: U32, +press: U32, +inp: J.Val) -> J.Val:
  lanternward.kit.verbs.charge(lanternward.kit.verbs.shield(s, mask), id, mask, press, inp)

# The shield: held on the ground with a free arm.
def lanternward.kit.verbs.shield(+s: J.Val, +mask: U32) -> J.Val:
  lanternward.kit.verbs.shield.go(
    Bool.and(
      Bool.and(U32.is_ne(U32.and(mask, 2), 0), lanternward.kis(s, "act", "Free")),
      Bool.and(
        Bool.and(lanternward.kis(s, "carry", "Nothing"), U32.is_eq(lanternward.ku(s, "z"), 0)),
        U32.is_eq(lanternward.ku(s, "guard_cd"), 0))),
    s)

def lanternward.kit.verbs.shield.go(up: Bool, +s: J.Val) -> J.Val:
  match up:
    case True{}:
      lanternward.ks(
        lanternward.kit.verbs.shield.sound(U32.is_eq(lanternward.ku(s, "shield"), 0), s),
        "shield", RS.u_min((lanternward.ku(s, "shield") + 1 : U32), 255))
    case False{}:
      lanternward.ks(
        Bool.pick(J.Val, Bool.and(U32.is_gt(lanternward.ku(s, "shield"), 0), lanternward.kis(s, "act", "Free")),
          lanternward.ks(s, "guard_cd", 10), s),
        "shield", 0)

def lanternward.kit.verbs.shield.sound(first: Bool, +s: J.Val) -> J.Val:
  match first:
    case True{}:
      lanternward.sound(s, "shield_up")
    case False{}:
      s

# The sword button held builds the spin.
def lanternward.kit.verbs.charge(+s: J.Val, +id: U32, +mask: U32, +press: U32, +inp: J.Val) -> J.Val:
  lanternward.kit.verbs.recover(
    lanternward.kit.verbs.charge.go(Bool.and(U32.is_ne(U32.and(mask, 4), 0), U32.is_eq(lanternward.ku(s, "shield"), 0)), s),
    id, mask, press, inp)

def lanternward.kit.verbs.charge.go(on: Bool, +s: J.Val) -> J.Val:
  match on:
    case False{}:
      s
    case True{}:
      lanternward.kit.verbs.charge.mark(
        U32.is_eq(RS.u_min((lanternward.ku(s, "charge") + 1 : U32), 255), 22),
        lanternward.ks(s, "charge", RS.u_min((lanternward.ku(s, "charge") + 1 : U32), 255)))

def lanternward.kit.verbs.charge.mark(full: Bool, +s: J.Val) -> J.Val:
  match full:
    case False{}:
      s
    case True{}:
      lanternward.props.Session.fx(lanternward.sound(s, "spell_cast"), "Spark", lanternward.ku(s, "x"), (lanternward.ku(s, "y") - 2048 : U32), 1)

# A swing's recovery gives way to a dodge or the shield.
def lanternward.kit.Session.recovering(+k: J.Val) -> Bool:
  Bool.pick(Bool, String.eq(ga.s(k, "act"), "Slash"),
    U32.is_gt(ga.u(k, "t"), (lanternward.kit.slash_wind(RS.u_min(ga.u(k, "n"), 2)) + lanternward.kit.slash_active(RS.u_min(ga.u(k, "n"), 2)) : U32)),
    Bool.pick(Bool, String.eq(ga.s(k, "act"), "Lunge"), U32.is_ge(ga.u(k, "t"), 9), False{}))

def lanternward.kit.slash_wind(+n: U32) -> U32:
  Bool.pick(U32, U32.is_eq(n, 2), 4, 2)

def lanternward.kit.slash_active(+n: U32) -> U32:
  Bool.pick(U32, U32.is_eq(n, 2), 4, 3)

def lanternward.kit.slash_rest(+n: U32) -> U32:
  Bool.pick(U32, U32.is_eq(n, 2), 12, 6)

def lanternward.kit.slash_reach(+n: U32) -> U32:
  Bool.pick(U32, U32.is_eq(n, 2), 22, 18)

def lanternward.kit.slash_cos(+n: U32) -> U32:
  Bool.pick(U32, U32.is_eq(n, 2), 4294967252, 66)

def lanternward.kit.slash_damage(+n: U32) -> U32:
  Bool.pick(U32, U32.is_eq(n, 2), 4, 2)

def lanternward.kit.verbs.recover(+s: J.Val, +id: U32, +mask: U32, +press: U32, +inp: J.Val) -> J.Val:
  lanternward.kit.verbs.act(
    lanternward.kit.verbs.recover.go(
      Bool.and(lanternward.kit.Session.recovering(J.get(s, "$k")), Bool.or(U32.is_ne(U32.and(press, 8), 0), U32.is_ne(U32.and(mask, 2), 0))), s),
    id, mask, press, inp)

def lanternward.kit.verbs.recover.go(yes: Bool, +s: J.Val) -> J.Val:
  match yes:
    case False{}:
      s
    case True{}:
      lanternward.ksb(lanternward.ks(lanternward.ks(lanternward.ks(lanternward.ksn(s, "act", "Free"), "t", 0), "chain", 0), "step", 0), "queued", False{})

def lanternward.kit.verbs.act(+s: J.Val, +id: U32, +mask: U32, +press: U32, +inp: J.Val) -> J.Val:
  lanternward.kit.verbs.release(
    lanternward.kit.verbs.act.go(lanternward.kit.act_code(ga.s(J.get(s, "$k"), "act")), s, id, mask, press, inp),
    id, mask)

def lanternward.kit.verbs.act.go(+code: U32, +s: J.Val, +id: U32, +mask: U32, +press: U32, +inp: J.Val) -> J.Val:
  match code:
    case 0:
      lanternward.kit.Session.free(s, id, press, inp)
    case 1:
      lanternward.kit.Session.swinging(s, id, press)
    case 2:
      lanternward.kit.Session.swinging(s, id, press)
    case 3:
      lanternward.kit.Session.swinging(s, id, press)
    case 4:
      lanternward.kit.Session.rolling(s)
    case 5:
      lanternward.kit.Session.hopping(s, id, press)
    case 6:
      lanternward.kit.Session.plunging(s, id)
    case _:
      s

# Letting the sword go ends the winding; a wound spin goes off. Then the lamp.
def lanternward.kit.verbs.release(+s: J.Val, +id: U32, +mask: U32) -> J.Val:
  lanternward.kit.Session.lantern(
    lanternward.kit.verbs.release.go(U32.is_eq(U32.and(mask, 4), 0), s), id, mask)

def lanternward.kit.verbs.release.go(up: Bool, +s: J.Val) -> J.Val:
  match up:
    case False{}:
      s
    case True{}:
      lanternward.ks(
        lanternward.kit.verbs.release.spin(
          Bool.and(
            Bool.and(lanternward.kit.Seat.charged(J.get(s, "$k")), lanternward.kis(s, "act", "Free")),
            U32.is_eq(lanternward.ku(s, "shield"), 0)),
          s),
        "charge", 0)

def lanternward.kit.verbs.release.spin(go: Bool, +s: J.Val) -> J.Val:
  match go:
    case True{}:
      lanternward.kit.Session.begin_spin(s)
    case False{}:
      s

def lanternward.kit.Session.free(+s: J.Val, +id: U32, +press: U32, +inp: J.Val) -> J.Val:
  lanternward.kit.free.shield(U32.is_gt(lanternward.ku(s, "shield"), 0), s, id, press, inp)

# A shield up trades the sword and the hop for a wall; E still meets the world.
def lanternward.kit.free.shield(up: Bool, +s: J.Val, +id: U32, +press: U32, +inp: J.Val) -> J.Val:
  match up:
    case True{}:
      lanternward.kit.free.ctx_only(U32.is_ne(U32.and(press, 8), 0), s, id)
    case False{}:
      lanternward.kit.free.sword(
        Bool.and(U32.is_ne(U32.and(press, 4), 0), lanternward.kis(s, "carry", "Nothing")), s, id, press, inp)

def lanternward.kit.free.ctx_only(pressed: Bool, +s: J.Val, +id: U32) -> J.Val:
  match pressed:
    case True{}:
      J.del(lanternward.ctx.Session.use_context(s, id), "$ret")
    case False{}:
      s

def lanternward.kit.free.sword(go: Bool, +s: J.Val, +id: U32, +press: U32, +inp: J.Val) -> J.Val:
  match go:
    case True{}:
      lanternward.kit.free.sword.lunge(
        lanternward.kit.free.forward(lanternward.kit.Session.lock_point(s, lanternward.kv(s, "lock")), s, inp), s)
    case False{}:
      lanternward.kit.free.ctx(U32.is_ne(U32.and(press, 8), 0), s, id, press, inp)

# Whether the stick pushes toward the locked target, for a lunge.
def lanternward.kit.free.forward(+pt: J.Val, +s: J.Val, +inp: J.Val) -> Bool:
  match pt:
    case J.JNull{}:
      False{}
    case _:
      lanternward.kit.free.forward.go(
        lanternward.world.unit(ga.u(inp, "move_x"), ga.u(inp, "move_y")), pt, s)

def lanternward.kit.free.forward.go(v: U32 & U32, +pt: J.Val, +s: J.Val) -> Bool:
  (+vx, +vy) = v
  Bool.and(
    Bool.not(Bool.and(U32.is_eq(vx, 0), U32.is_eq(vy, 0))),
    RS.i_ge(lanternward.world.dot_unit(vx, vy, (ga.nu(pt, 0) - lanternward.ku(s, "x") : U32), (ga.nu(pt, 1) - lanternward.ku(s, "y") : U32)), 128))

def lanternward.kit.free.sword.lunge(+forward: Bool, +s: J.Val) -> J.Val:
  lanternward.kit.free.sword.lunge.go(Bool.and(forward, U32.is_eq(lanternward.ku(s, "chain"), 0)), s)

def lanternward.kit.free.sword.lunge.go(lunge: Bool, +s: J.Val) -> J.Val:
  match lunge:
    case True{}:
      lanternward.kit.Session.begin_lunge(s)
    case False{}:
      lanternward.kit.Session.begin_slash(s, Bool.pick(U32, U32.is_gt(lanternward.ku(s, "chain"), 0), lanternward.ku(s, "step"), 0))

def lanternward.kit.free.ctx(pressed: Bool, +s: J.Val, +id: U32, +press: U32, +inp: J.Val) -> J.Val:
  match pressed:
    case True{}:
      lanternward.kit.free.ctx.go(lanternward.ctx.Session.use_context(s, id), inp)
    case False{}:
      lanternward.kit.free.bomb(
        Bool.and(Bool.and(U32.is_ne(U32.and(press, 32), 0), U32.is_gt(lanternward.ku(s, "bombs"), 0)), lanternward.kis(s, "carry", "Nothing")),
        s, id)

def lanternward.kit.free.ctx.go(+s: J.Val, +inp: J.Val) -> J.Val:
  lanternward.kit.free.ctx.then(ga.b(s, "$ret"), J.del(s, "$ret"), inp)

def lanternward.kit.free.ctx.then(used: Bool, +s: J.Val, +inp: J.Val) -> J.Val:
  match used:
    case True{}:
      s
    case False{}:
      lanternward.kit.Session.begin_dodge(s, inp)

def lanternward.kit.free.bomb(go: Bool, +s: J.Val, +id: U32) -> J.Val:
  match go:
    case False{}:
      s
    case True{}:
      lanternward.sound(
        lanternward.kit.free.bomb.fuse(
          U32.is_lt(lanternward.len(s, "fuses"), 8),
          lanternward.ks(s, "bombs", (lanternward.ku(s, "bombs") - 1 : U32)), id),
        "bomb_toss")

def lanternward.kit.free.bomb.fuse(room: Bool, +s: J.Val, +id: U32) -> J.Val:
  match room:
    case False{}:
      s
    case True{}:
      lanternward.push(s, "fuses",
        ga.su(ga.su(ga.si(ga.si(J.obj(), "x", (lanternward.ku(s, "x") + lanternward.kface(s, 0) * 12 : U32)),
          "y", (lanternward.ku(s, "y") + lanternward.kface(s, 1) * 12 : U32)), "t", 75), "owner", U32.and(id, 255)))

# ---- the sword ----

def lanternward.kit.Session.begin_slash(+s: J.Val, +step: U32) -> J.Val:
  lanternward.sound(
    lanternward.ksv(
      lanternward.ksb(
        lanternward.ks(lanternward.ks(lanternward.ks(lanternward.ksn(s, "act", "Slash"), "t", 0), "n", RS.u_min(step, 2)), "swing", (lanternward.ku(s, "swing") + 1 : U32)),
        "queued", False{}),
      "mv", ga.pair_i(0, 0)),
    "swing")

def lanternward.kit.Session.begin_lunge(+s: J.Val) -> J.Val:
  lanternward.sound(
    lanternward.ksv(
      lanternward.ks(lanternward.ks(lanternward.ks(lanternward.ks(lanternward.ksn(s, "act", "Lunge"), "t", 0), "swing", (lanternward.ku(s, "swing") + 1 : U32)), "chain", 0), "step", 0),
      "mv", ga.pair_i(RS.i_div((lanternward.kface(s, 0) * 1100 : U32), 256), RS.i_div((lanternward.kface(s, 1) * 1100 : U32), 256))),
    "swing")

def lanternward.kit.Session.begin_spin(+s: J.Val) -> J.Val:
  lanternward.sound(
    lanternward.ks(lanternward.ks(lanternward.ks(lanternward.ks(lanternward.ks(lanternward.ksn(s, "act", "Spin"), "t", 0), "swing", (lanternward.ku(s, "swing") + 1 : U32)), "charge", 0), "chain", 0), "step", 0),
    "axe_spin")

def lanternward.kit.Session.swinging(+s: J.Val, +id: U32, +press: U32) -> J.Val:
  lanternward.kit.swinging.act(ga.s(J.get(s, "$k"), "act"), lanternward.ks(s, "t", (lanternward.ku(s, "t") + 1 : U32)), id, press)

def lanternward.kit.swinging.act(+act: String, +s: J.Val, +id: U32, +press: U32) -> J.Val:
  Bool.pick(J.Val, String.eq(act, "Slash"), lanternward.kit.swinging.slash(s, id, press),
    Bool.pick(J.Val, String.eq(act, "Lunge"), lanternward.kit.swinging.lunge(s, id),
      Bool.pick(J.Val, String.eq(act, "Spin"), lanternward.kit.swinging.spin(s, id), s)))

def lanternward.kit.swinging.slash(+s: J.Val, +id: U32, +press: U32) -> J.Val:
  lanternward.kit.swinging.slash.sweep(
    RS.u_min(lanternward.ku(s, "n"), 2),
    Bool.pick(J.Val, U32.is_ne(U32.and(press, 4), 0), lanternward.ksb(s, "queued", True{}), s), id)

def lanternward.kit.swinging.slash.sweep(+n: U32, +s: J.Val, +id: U32) -> J.Val:
  lanternward.kit.swinging.slash.sweep.go(
    Bool.and(U32.is_gt(lanternward.ku(s, "t"), lanternward.kit.slash_wind(n)), U32.is_le(lanternward.ku(s, "t"), (lanternward.kit.slash_wind(n) + lanternward.kit.slash_active(n) : U32))),
    n, s, id)

def lanternward.kit.swinging.slash.sweep.go(active: Bool, +n: U32, +s: J.Val, +id: U32) -> J.Val:
  match active:
    case False{}:
      lanternward.kit.swinging.slash.chain(n, s)
    case True{}:
      lanternward.kit.swinging.slash.chain(n, lanternward.kit.swinging.slash.freeze(n, lanternward.kit.Session.sweep(s, id, lanternward.kit.slash_reach(n), lanternward.kit.slash_cos(n), lanternward.kit.slash_damage(n), U32.is_eq(n, 2))))

# A landed blade holds its owner a moment.
def lanternward.kit.swinging.slash.freeze(+n: U32, +s: J.Val) -> J.Val:
  lanternward.kit.swinging.freeze(ga.b(s, "$ret"), Bool.pick(U32, U32.is_eq(n, 2), 4, 2), J.del(s, "$ret"))

def lanternward.kit.swinging.freeze(landed: Bool, +amount: U32, +s: J.Val) -> J.Val:
  match landed:
    case False{}:
      s
    case True{}:
      lanternward.ks(s, "freeze", RS.u_max(lanternward.ku(s, "freeze"), amount))

def lanternward.kit.swinging.slash.chain(+n: U32, +s: J.Val) -> J.Val:
  lanternward.kit.swinging.slash.end(
    n,
    Bool.pick(J.Val, U32.is_eq(lanternward.ku(s, "t"), (lanternward.kit.slash_wind(n) + lanternward.kit.slash_active(n) : U32)),
      lanternward.ks(
        lanternward.ks(s, "chain", (lanternward.kit.slash_rest(n) + lanternward.R(s, "combo") : U32)),
        "step", Bool.pick(U32, U32.is_eq(n, 2), 0, (n + 1 : U32))),
      s))

# The recovery ends: free again, and a queued press starts the next slash.
def lanternward.kit.swinging.slash.end(+n: U32, +s: J.Val) -> J.Val:
  lanternward.kit.swinging.slash.end.go(
    U32.is_ge(lanternward.ku(s, "t"), (lanternward.kit.slash_wind(n) + lanternward.kit.slash_active(n) + lanternward.kit.slash_rest(n) : U32)), s)

def lanternward.kit.swinging.slash.end.go(done: Bool, +s: J.Val) -> J.Val:
  match done:
    case False{}:
      s
    case True{}:
      lanternward.kit.swinging.slash.next(
        Bool.and(lanternward.kb(s, "queued"), Bool.and(U32.is_gt(lanternward.ku(s, "step"), 0), U32.is_gt(lanternward.ku(s, "chain"), 0))),
        lanternward.ks(lanternward.ksn(s, "act", "Free"), "t", 0))

def lanternward.kit.swinging.slash.next(go: Bool, +s: J.Val) -> J.Val:
  match go:
    case False{}:
      s
    case True{}:
      lanternward.kit.Session.begin_slash(s, lanternward.ku(s, "step"))

def lanternward.kit.swinging.lunge(+s: J.Val, +id: U32) -> J.Val:
  lanternward.kit.swinging.lunge.end(
    lanternward.kit.swinging.lunge.hit(
      Bool.and(U32.is_ge(lanternward.ku(s, "t"), 3), U32.is_lt(lanternward.ku(s, "t"), 9)), s, id))

def lanternward.kit.swinging.lunge.hit(go: Bool, +s: J.Val, +id: U32) -> J.Val:
  match go:
    case False{}:
      s
    case True{}:
      lanternward.kit.swinging.lunge.hit.go(
        lanternward.kit.Session.lunge_hit(lanternward.kit.Session.shove(s, ga.nu(lanternward.kv(s, "mv"), 0), ga.nu(lanternward.kv(s, "mv"), 1)), id))

def lanternward.kit.swinging.lunge.hit.go(+s: J.Val) -> J.Val:
  lanternward.kit.swinging.freeze(ga.b(s, "$ret"), 3, J.del(s, "$ret"))

def lanternward.kit.swinging.lunge.end(+s: J.Val) -> J.Val:
  lanternward.kit.swinging.lunge.end.go(U32.is_ge(lanternward.ku(s, "t"), 19), s)

def lanternward.kit.swinging.lunge.end.go(done: Bool, +s: J.Val) -> J.Val:
  match done:
    case True{}:
      lanternward.ks(lanternward.ksn(s, "act", "Free"), "t", 0)
    case False{}:
      s

def lanternward.kit.swinging.spin(+s: J.Val, +id: U32) -> J.Val:
  lanternward.kit.swinging.spin.end(
    lanternward.kit.swinging.spin.hit(
      Bool.and(U32.is_ge(lanternward.ku(s, "t"), 2), U32.is_lt(lanternward.ku(s, "t"), 12)), s, id))

def lanternward.kit.swinging.spin.hit(go: Bool, +s: J.Val, +id: U32) -> J.Val:
  match go:
    case False{}:
      s
    case True{}:
      lanternward.kit.swinging.lunge.hit.go(lanternward.kit.Session.sweep(s, id, 22, 4294967040, 3, True{}))

def lanternward.kit.swinging.spin.end(+s: J.Val) -> J.Val:
  lanternward.kit.swinging.spin.end.go(U32.is_ge(lanternward.ku(s, "t"), 14), s)

def lanternward.kit.swinging.spin.end.go(done: Bool, +s: J.Val) -> J.Val:
  match done:
    case True{}:
      lanternward.ks(lanternward.ksn(s, "act", "Free"), "t", 0)
    case False{}:
      s

# A swing's arc: whether (x, y) lies within `reach` px (plus `extra`) of the
# knight and, unless the swing is a full circle, in front of the knight.
def lanternward.kit.inside(+k: J.Val, +reach: U32, +cos: U32, +x: U32, +y: U32, +extra: U32) -> Bool:
  lanternward.kit.inside.d(lanternward.world.dist((x - ga.u(k, "x") : U32), (y - ga.u(k, "y") : U32)), k, reach, cos, x, y, extra)

def lanternward.kit.inside.d(+d: U32, +k: J.Val, +reach: U32, +cos: U32, +x: U32, +y: U32, +extra: U32) -> Bool:
  Bool.and(
    RS.i_le(d, (reach * 256 + extra : U32)),
    Bool.or(
      Bool.or(RS.i_le(cos, 4294967040), RS.i_lt(d, 1536)),
      RS.i_ge(lanternward.world.dot_unit(ga.nu(ga.v(k, "face"), 0), ga.nu(ga.v(k, "face"), 1), (x - ga.u(k, "x") : U32), (y - ga.u(k, "y") : U32)), cos)))

def lanternward.kit.hit_slot(+id: U32) -> U32:
  RS.u_min((id - 1 : U32), 5)

# Every foe, pot, blade-cut thing and iron bolt inside the arc is struck once.
# `$ret` is true when something was hurt.
def lanternward.kit.Session.sweep(+s: J.Val, +id: U32, +reach: U32, +cos: U32, +dmg: U32, +heavy: Bool) -> J.Val:
  lanternward.kit.sweep.fin(
    lanternward.kit.sweep.bolts(
      lanternward.kit.sweep.props(
        lanternward.kit.sweep.boss(
          lanternward.kit.sweep.foes(ga.sb(ga.sb(s, "$landed", False{}), "$cut", False{}), id, reach, cos, dmg, 0),
          id, reach, cos, dmg),
        reach, cos, 0),
      reach, cos, 0),
    heavy)

def lanternward.kit.sweep.foes(+s: J.Val, +id: U32, +reach: U32, +cos: U32, +dmg: U32, +i: U32) -> J.Val:
  lanternward.kit.sweep.foes.go(U32.is_lt(i, lanternward.len(s, "foes")), s, id, reach, cos, dmg, i)

def lanternward.kit.sweep.foes.go(more: Bool, +s: J.Val, +id: U32, +reach: U32, +cos: U32, +dmg: U32, +i: U32) -> J.Val:
  match more:
    case False{}:
      s
    case True{}:
      lanternward.kit.sweep.foes(
        lanternward.kit.sweep.foe(
          Bool.and(
            Bool.and(
              Bool.not(U32.is_eq(J.u32(J.nth(lanternward.ev(s, "foes", i, "hit"), lanternward.kit.hit_slot(id))), lanternward.ku(s, "swing"))),
              lanternward.foes.Foe.alive(lanternward.el(s, "foes", i))),
            lanternward.kit.inside(J.get(s, "$k"), reach, cos, lanternward.eu(s, "foes", i, "x"), lanternward.eu(s, "foes", i, "y"),
              lanternward.foes.Foe.radius(lanternward.el(s, "foes", i)))),
          s, id, dmg, i),
        id, reach, cos, dmg, (i + 1 : U32))

def lanternward.kit.sweep.foe(hit: Bool, +s: J.Val, +id: U32, +dmg: U32, +i: U32) -> J.Val:
  match hit:
    case False{}:
      s
    case True{}:
      lanternward.kit.sweep.landed(
        lanternward.blow.Session.hurt_foe(
          lanternward.eset(s, "foes", i, "hit", J.set_nth(lanternward.ev(s, "foes", i, "hit"), lanternward.kit.hit_slot(id), J.JU{lanternward.ku(s, "swing")})),
          i, dmg, lanternward.ku(s, "x"), lanternward.ku(s, "y")))

# hurt_foe / hurt_boss left whether they hurt in `$ret`: fold it into `$landed`.
def lanternward.kit.sweep.landed(+s: J.Val) -> J.Val:
  lanternward.kit.sweep.landed.go(ga.b(s, "$ret"), J.del(s, "$ret"))

def lanternward.kit.sweep.landed.go(hurt: Bool, +s: J.Val) -> J.Val:
  match hurt:
    case True{}:
      ga.sb(s, "$landed", True{})
    case False{}:
      s

def lanternward.kit.sweep.boss(+s: J.Val, +id: U32, +reach: U32, +cos: U32, +dmg: U32) -> J.Val:
  lanternward.kit.sweep.boss.go(
    Bool.and(Bool.not(J.is_null(J.get(s, "boss"))), U32.is_gt(ga.u(J.get(s, "boss"), "hp"), 0)), s, id, reach, cos, dmg)

def lanternward.kit.sweep.boss.go(alive: Bool, +s: J.Val, +id: U32, +reach: U32, +cos: U32, +dmg: U32) -> J.Val:
  match alive:
    case False{}:
      s
    case True{}:
      lanternward.kit.sweep.boss.hit(
        Bool.and(
          lanternward.kit.inside(J.get(s, "$k"), reach, cos, ga.u(J.get(s, "boss"), "x"), ga.u(J.get(s, "boss"), "y"), 4608),
          Bool.not(U32.is_eq(J.u32(J.nth(ga.v(J.get(s, "boss"), "hit"), lanternward.kit.hit_slot(id))), lanternward.ku(s, "swing")))),
        s, id, dmg)

def lanternward.kit.sweep.boss.hit(hit: Bool, +s: J.Val, +id: U32, +dmg: U32) -> J.Val:
  match hit:
    case False{}:
      s
    case True{}:
      lanternward.kit.sweep.landed(
        lanternward.boss.Session.hurt_boss(
          ga.sv(s, "boss", ga.sv(J.get(s, "boss"), "hit", J.set_nth(ga.v(J.get(s, "boss"), "hit"), lanternward.kit.hit_slot(id), J.JU{lanternward.ku(s, "swing")}))),
          dmg, lanternward.ku(s, "x"), lanternward.ku(s, "y"), id))

def lanternward.kit.sweep.props(+s: J.Val, +reach: U32, +cos: U32, +i: U32) -> J.Val:
  lanternward.kit.sweep.props.go(U32.is_lt(i, lanternward.len(s, "props")), s, reach, cos, i)

def lanternward.kit.sweep.props.go(more: Bool, +s: J.Val, +reach: U32, +cos: U32, +i: U32) -> J.Val:
  match more:
    case False{}:
      s
    case True{}:
      lanternward.kit.sweep.props(
        lanternward.kit.sweep.prop(
          Bool.and(
            Bool.and(lanternward.NAME_IN(ga.s(lanternward.el(s, "props", i), "kind"), ["Pot", "Grass", "Crystal"]),
              U32.is_eq(lanternward.eu(s, "props", i, "on"), 0)),
            lanternward.kit.inside(J.get(s, "$k"), reach, cos, lanternward.eu(s, "props", i, "x"), lanternward.eu(s, "props", i, "y"), 1536)),
          s, i),
        reach, cos, (i + 1 : U32))

def lanternward.kit.sweep.prop(cut: Bool, +s: J.Val, +i: U32) -> J.Val:
  match cut:
    case False{}:
      s
    case True{}:
      Bool.pick(J.Val, lanternward.eis(s, "props", i, "kind", "Crystal"),
        lanternward.ctx.Session.light_crystal(s, i),
        lanternward.props.Session.shatter(s, i))

def lanternward.kit.sweep.bolts(+s: J.Val, +reach: U32, +cos: U32, +i: U32) -> J.Val:
  lanternward.kit.sweep.bolts.go(U32.is_lt(i, lanternward.len(s, "bolts")), s, reach, cos, i)

def lanternward.kit.sweep.bolts.go(more: Bool, +s: J.Val, +reach: U32, +cos: U32, +i: U32) -> J.Val:
  match more:
    case False{}:
      s
    case True{}:
      lanternward.kit.sweep.bolts(
        lanternward.kit.sweep.bolt(
          Bool.and(
            lanternward.foes.Bolt.hostile(lanternward.el(s, "bolts", i)),
            lanternward.kit.inside(J.get(s, "$k"), reach, cos, lanternward.eu(s, "bolts", i, "x"), lanternward.eu(s, "bolts", i, "y"), 1024)),
          s, i),
        reach, cos, (i + 1 : U32))

def lanternward.kit.sweep.bolt(cut: Bool, +s: J.Val, +i: U32) -> J.Val:
  match cut:
    case False{}:
      s
    case True{}:
      ga.sb(lanternward.es(s, "bolts", i, "ttl", 0), "$cut", True{})

def lanternward.kit.sweep.fin(+s: J.Val, +heavy: Bool) -> J.Val:
  lanternward.kit.sweep.out(ga.b(s, "$landed"), heavy, lanternward.kit.sweep.clang(ga.b(s, "$cut"), s))

def lanternward.kit.sweep.clang(cut: Bool, +s: J.Val) -> J.Val:
  match cut:
    case False{}:
      s
    case True{}:
      lanternward.props.Session.fx(
        lanternward.sound(s, "clang"), "Spark",
        (lanternward.ku(s, "x") + lanternward.kface(s, 0) * 18 : U32), (lanternward.ku(s, "y") + lanternward.kface(s, 1) * 18 : U32), 0)

def lanternward.kit.sweep.out(+landed: Bool, +heavy: Bool, +s: J.Val) -> J.Val:
  ga.sb(J.del(J.del(lanternward.kit.sweep.heavy(Bool.and(landed, heavy), lanternward.kit.sweep.stop(landed, s)), "$landed"), "$cut"), "$ret", landed)

def lanternward.kit.sweep.stop(landed: Bool, +s: J.Val) -> J.Val:
  match landed:
    case True{}:
      lanternward.stop(s, "Heavy")
    case False{}:
      s

def lanternward.kit.sweep.heavy(hard: Bool, +s: J.Val) -> J.Val:
  match hard:
    case True{}:
      lanternward.sound(ga.su(s, "shake", RS.u_max(ga.u(s, "shake"), 8)), "hit_axe")
    case False{}:
      s

# A lunge's line: from the knight out ahead, narrow and long. `$ret` is true
# when it landed.
def lanternward.kit.Session.lunge_hit(+s: J.Val, +id: U32) -> J.Val:
  lanternward.kit.lunge_hit.go(
    lanternward.variant("Capsule",
      ga.si(ga.si(ga.si(ga.si(ga.si(J.obj(), "ax", lanternward.ku(s, "x")), "ay", lanternward.ku(s, "y")),
        "bx", (lanternward.ku(s, "x") + lanternward.kface(s, 0) * 30 : U32)), "by", (lanternward.ku(s, "y") + lanternward.kface(s, 1) * 30 : U32)), "r", 1280)),
    s, id)

def lanternward.kit.lunge_hit.go(+line: J.Val, +s: J.Val, +id: U32) -> J.Val:
  lanternward.kit.lunge_hit.fin(
    lanternward.kit.lunge_hit.boss(lanternward.kit.lunge_hit.foes(ga.sb(s, "$landed", False{}), line, id, 0), line, id), 0)

def lanternward.kit.lunge_hit.fin(+s: J.Val, +z: U32) -> J.Val:
  lanternward.kit.lunge_hit.fin.go(ga.b(s, "$landed"), s)

def lanternward.kit.lunge_hit.fin.go(+landed: Bool, +s: J.Val) -> J.Val:
  ga.sb(J.del(Bool.pick(J.Val, landed, lanternward.stop(s, "Heavy"), s), "$landed"), "$ret", landed)

def lanternward.kit.lunge_hit.foes(+s: J.Val, +line: J.Val, +id: U32, +i: U32) -> J.Val:
  lanternward.kit.lunge_hit.foes.go(U32.is_lt(i, lanternward.len(s, "foes")), s, line, id, i)

def lanternward.kit.lunge_hit.foes.go(more: Bool, +s: J.Val, +line: J.Val, +id: U32, +i: U32) -> J.Val:
  match more:
    case False{}:
      s
    case True{}:
      lanternward.kit.lunge_hit.foes(
        lanternward.kit.lunge_hit.foe(
          Bool.and(
            Bool.and(
              Bool.not(U32.is_eq(J.u32(J.nth(lanternward.ev(s, "foes", i, "hit"), lanternward.kit.hit_slot(id))), lanternward.ku(s, "swing"))),
              lanternward.foes.Foe.alive(lanternward.el(s, "foes", i))),
            lanternward.foes.Danger.hits(line, lanternward.eu(s, "foes", i, "x"), lanternward.eu(s, "foes", i, "y"),
              lanternward.foes.Foe.radius(lanternward.el(s, "foes", i)))),
          s, id, i),
        line, id, (i + 1 : U32))

def lanternward.kit.lunge_hit.foe(hit: Bool, +s: J.Val, +id: U32, +i: U32) -> J.Val:
  match hit:
    case False{}:
      s
    case True{}:
      lanternward.kit.sweep.landed(
        lanternward.blow.Session.hurt_foe(
          lanternward.eset(s, "foes", i, "hit", J.set_nth(lanternward.ev(s, "foes", i, "hit"), lanternward.kit.hit_slot(id), J.JU{lanternward.ku(s, "swing")})),
          i, 3, lanternward.ku(s, "x"), lanternward.ku(s, "y")))

def lanternward.kit.lunge_hit.boss(+s: J.Val, +line: J.Val, +id: U32) -> J.Val:
  lanternward.kit.lunge_hit.boss.go(
    Bool.and(Bool.not(J.is_null(J.get(s, "boss"))), U32.is_gt(ga.u(J.get(s, "boss"), "hp"), 0)), s, line, id)

def lanternward.kit.lunge_hit.boss.go(alive: Bool, +s: J.Val, +line: J.Val, +id: U32) -> J.Val:
  match alive:
    case False{}:
      s
    case True{}:
      lanternward.kit.lunge_hit.boss.hit(
        Bool.and(
          Bool.not(U32.is_eq(J.u32(J.nth(ga.v(J.get(s, "boss"), "hit"), lanternward.kit.hit_slot(id))), lanternward.ku(s, "swing"))),
          lanternward.foes.Danger.hits(line, ga.u(J.get(s, "boss"), "x"), ga.u(J.get(s, "boss"), "y"), 4608)),
        s, id)

def lanternward.kit.lunge_hit.boss.hit(hit: Bool, +s: J.Val, +id: U32) -> J.Val:
  match hit:
    case False{}:
      s
    case True{}:
      lanternward.kit.sweep.landed(
        lanternward.boss.Session.hurt_boss(
          ga.sv(s, "boss", ga.sv(J.get(s, "boss"), "hit", J.set_nth(ga.v(J.get(s, "boss"), "hit"), lanternward.kit.hit_slot(id), J.JU{lanternward.ku(s, "swing")}))),
          3, lanternward.ku(s, "x"), lanternward.ku(s, "y"), id))

# ---- dodges ----

# E with nothing to do: a roll along the walk, or, locked on, the hop the
# stick asks for.
def lanternward.kit.Session.begin_dodge(+s: J.Val, +inp: J.Val) -> J.Val:
  lanternward.kit.begin_dodge.go(
    lanternward.world.unit(ga.u(inp, "move_x"), ga.u(inp, "move_y")),
    lanternward.kit.Session.lock_point(s, lanternward.kv(s, "lock")), s)

def lanternward.kit.begin_dodge.go(walk: U32 & U32, +pt: J.Val, +s: J.Val) -> J.Val:
  (+wx, +wy) = walk
  lanternward.kit.begin_dodge.done(
    lanternward.sound(
      Bool.pick(J.Val, J.is_null(pt),
        lanternward.kit.begin_dodge.roll(wx, wy, s),
        lanternward.kit.begin_dodge.hop(wx, wy, pt, s)),
      "roll"))

def lanternward.kit.begin_dodge.done(+s: J.Val) -> J.Val:
  lanternward.ks(lanternward.ks(lanternward.ks(s, "chain", 0), "step", 0), "charge", 0)

def lanternward.kit.begin_dodge.roll(+wx: U32, +wy: U32, +s: J.Val) -> J.Val:
  lanternward.kit.begin_dodge.roll.go(
    Bool.pick(U32, Bool.and(U32.is_eq(wx, 0), U32.is_eq(wy, 0)), lanternward.kface(s, 0), wx),
    Bool.pick(U32, Bool.and(U32.is_eq(wx, 0), U32.is_eq(wy, 0)), lanternward.kface(s, 1), wy), s)

def lanternward.kit.begin_dodge.roll.go(+vx: U32, +vy: U32, +s: J.Val) -> J.Val:
  lanternward.ksv(
    lanternward.ksv(lanternward.ks(lanternward.ks(lanternward.ksn(s, "act", "Roll"), "t", 0), "n", 0), "mv",
      ga.pair_i(RS.i_div((vx * 1100 : U32), 256), RS.i_div((vy * 1100 : U32), 256))),
    "face", ga.pair_i(vx, vy))

# Locked on: forward, back or to a side relative to the lock.
def lanternward.kit.begin_dodge.hop(+wx: U32, +wy: U32, +pt: J.Val, +s: J.Val) -> J.Val:
  lanternward.kit.begin_dodge.hop.fwd(
    lanternward.world.unit((ga.nu(pt, 0) - lanternward.ku(s, "x") : U32), (ga.nu(pt, 1) - lanternward.ku(s, "y") : U32)), wx, wy, s)

def lanternward.kit.begin_dodge.hop.fwd(fwd: U32 & U32, +wx: U32, +wy: U32, +s: J.Val) -> J.Val:
  (+fx, +fy) = fwd
  +still = Bool.and(U32.is_eq(wx, 0), U32.is_eq(wy, 0))
  lanternward.kit.begin_dodge.hop.pick(
    Bool.pick(U32, still, 4294967040, lanternward.world.dot4(wx, wy, fx, fy)),
    Bool.pick(U32, still, 0, lanternward.world.dot4(wx, wy, RS.i_neg(fy), fx)),
    fx, fy, s)

def lanternward.kit.begin_dodge.hop.pick(+f: U32, +sd: U32, +fx: U32, +fy: U32, +s: J.Val) -> J.Val:
  lanternward.kit.begin_dodge.hop.set(
    Bool.and(RS.i_gt(RS.i_abs(sd), RS.i_abs(f)), RS.i_gt(RS.i_abs(sd), 64)),
    RS.i_gt(f, 64), RS.i_signum(sd), f, fx, fy, s)

def lanternward.kit.begin_dodge.hop.set(+side: Bool, +fwd: Bool, +sign: U32, +f: U32, +fx: U32, +fy: U32, +s: J.Val) -> J.Val:
  lanternward.kit.begin_dodge.hop.fin(
    Bool.pick(U32, side, 0, Bool.pick(U32, fwd, 2, 1)),
    Bool.pick(U32, side, (RS.i_neg(fy) * sign : U32), Bool.pick(U32, fwd, fx, RS.i_neg(fx))),
    Bool.pick(U32, side, (fx * sign : U32), Bool.pick(U32, fwd, fy, RS.i_neg(fy))),
    Bool.pick(U32, side, 640, Bool.pick(U32, fwd, 640, 683)),
    s)

def lanternward.kit.begin_dodge.hop.fin(+kind: U32, +vx: U32, +vy: U32, +speed: U32, +s: J.Val) -> J.Val:
  lanternward.ks(
    lanternward.ksv(
      lanternward.ks(lanternward.ks(lanternward.ksn(s, "act", "Hop"), "n", kind), "t", 0),
      "mv", ga.pair_i(RS.i_div((vx * speed : U32), 256), RS.i_div((vy * speed : U32), 256))),
    "lock_t", 0)

def lanternward.kit.Session.rolling(+s: J.Val) -> J.Val:
  lanternward.kit.rolling.end(
    lanternward.kit.rolling.move(U32.is_lt((lanternward.ku(s, "t") + 1 : U32), 12), lanternward.ks(s, "t", (lanternward.ku(s, "t") + 1 : U32))))

def lanternward.kit.rolling.move(go: Bool, +s: J.Val) -> J.Val:
  match go:
    case False{}:
      s
    case True{}:
      lanternward.kit.Session.shove(s, ga.nu(lanternward.kv(s, "mv"), 0), ga.nu(lanternward.kv(s, "mv"), 1))

def lanternward.kit.rolling.end(+s: J.Val) -> J.Val:
  lanternward.kit.rolling.inv(lanternward.kit.rolling.done(U32.is_ge(lanternward.ku(s, "t"), 15), s))

def lanternward.kit.rolling.done(over: Bool, +s: J.Val) -> J.Val:
  match over:
    case False{}:
      s
    case True{}:
      lanternward.kit.rolling.pit(
        lanternward.props.Session.pit(s, lanternward.ku(s, "x"), lanternward.ku(s, "y")),
        lanternward.ks(lanternward.ksn(s, "act", "Free"), "t", 0))

def lanternward.kit.rolling.pit(pit: Bool, +s: J.Val) -> J.Val:
  match pit:
    case True{}:
      lanternward.ksn(s, "act", "Fall")
    case False{}:
      s

def lanternward.kit.rolling.inv(+s: J.Val) -> J.Val:
  lanternward.ks(s, "inv", RS.u_max(lanternward.ku(s, "inv"), Bool.to_u32(Bool.and(U32.is_ge(lanternward.ku(s, "t"), 2), U32.is_le(lanternward.ku(s, "t"), 10)))))

def lanternward.kit.Session.hopping(+s: J.Val, +id: U32, +press: U32) -> J.Val:
  lanternward.kit.hopping.move(
    lanternward.ks(s, "t", (lanternward.ku(s, "t") + 1 : U32)), press)

def lanternward.kit.hopping.move(+s: J.Val, +press: U32) -> J.Val:
  lanternward.kit.hopping.z(
    Bool.pick(J.Val, U32.is_lt(lanternward.ku(s, "t"), 16),
      lanternward.kit.Session.shove(s, ga.nu(lanternward.kv(s, "mv"), 0), ga.nu(lanternward.kv(s, "mv"), 1)), s),
    press)

# A parabola, peaking mid-hop.
def lanternward.kit.hopping.z(+s: J.Val, +press: U32) -> J.Val:
  lanternward.kit.hopping.plunge(
    lanternward.ksi(
      s, "z",
      RS.i_max(RS.i_div((3584 * 4 * lanternward.ku(s, "t") * (18 - lanternward.ku(s, "t") : U32) : U32), 324), 0)),
    press)

def lanternward.kit.hopping.plunge(+s: J.Val, +press: U32) -> J.Val:
  lanternward.kit.hopping.jump(
    Bool.and(U32.is_ne(U32.and(press, 4), 0), RS.i_gt(lanternward.ku(s, "z"), 512)),
    lanternward.ks(s, "inv",
      Bool.pick(U32, Bool.and(U32.is_ge(lanternward.ku(s, "t"), 2), U32.is_le(lanternward.ku(s, "t"), 14)),
        RS.u_max(lanternward.ku(s, "inv"), 1), lanternward.ku(s, "inv"))))

# The jump attack: straight down on whatever is below.
def lanternward.kit.hopping.jump(go: Bool, +s: J.Val) -> J.Val:
  match go:
    case True{}:
      lanternward.sound(
        lanternward.ksv(
          lanternward.ks(lanternward.ks(lanternward.ksn(s, "act", "Plunge"), "t", 0), "swing", (lanternward.ku(s, "swing") + 1 : U32)),
          "mv", ga.pair_i(0, 0)),
        "swing")
    case False{}:
      lanternward.kit.hopping.land(U32.is_ge(lanternward.ku(s, "t"), 18), s)

def lanternward.kit.hopping.land(down: Bool, +s: J.Val) -> J.Val:
  match down:
    case False{}:
      s
    case True{}:
      lanternward.kit.rolling.pit(
        lanternward.props.Session.pit(s, lanternward.ku(s, "x"), lanternward.ku(s, "y")),
        lanternward.props.Session.fx(
          lanternward.ks(lanternward.ksn(lanternward.ksi(s, "z", 0), "act", "Free"), "t", 0),
          "Dust", lanternward.ku(s, "x"), lanternward.ku(s, "y"), 0))

def lanternward.kit.Session.plunging(+s: J.Val, +id: U32) -> J.Val:
  lanternward.kit.plunging.fall(
    lanternward.ksi(
      lanternward.ks(lanternward.ks(s, "t", (lanternward.ku(s, "t") + 1 : U32)), "inv", RS.u_max(lanternward.ku(s, "inv"), 1)),
      "z", RS.i_max(RS.i_sub(lanternward.ku(s, "z"), 768), 0)),
    id)

def lanternward.kit.plunging.fall(+s: J.Val, +id: U32) -> J.Val:
  lanternward.kit.plunging.down(U32.is_eq(lanternward.ku(s, "z"), 0), s, id)

# Down: the blow lands in a ring about the knight.
def lanternward.kit.plunging.down(grounded: Bool, +s: J.Val, +id: U32) -> J.Val:
  match grounded:
    case False{}:
      s
    case True{}:
      lanternward.kit.plunging.land(lanternward.kit.Session.sweep(s, id, 18, 4294967040, 4, True{}))

def lanternward.kit.plunging.land(+s: J.Val) -> J.Val:
  lanternward.kit.plunging.end(
    ga.b(s, "$ret"),
    lanternward.props.Session.fx(J.del(s, "$ret"), "Ring", lanternward.ku(s, "x"), lanternward.ku(s, "y"), 24))

def lanternward.kit.plunging.end(+hit: Bool, +s: J.Val) -> J.Val:
  lanternward.kit.rolling.pit(
    lanternward.props.Session.pit(s, lanternward.ku(s, "x"), lanternward.ku(s, "y")),
    lanternward.ks(
      lanternward.ks(
        lanternward.ksn(
          lanternward.ks(
            Bool.pick(J.Val, hit, lanternward.ks(lanternward.kit.plunging.thud(s), "freeze", 3), lanternward.kit.plunging.thud(s)),
            "t", 0),
          "act", "Free"),
        "t", 0),
      "guard_cd", 8))

def lanternward.kit.plunging.thud(+s: J.Val) -> J.Val:
  ga.su(lanternward.sound(s, "rock_land"), "shake", RS.u_max(ga.u(s, "shake"), 8))

# ---- the lantern ----

def lanternward.kit.Session.lantern(+s: J.Val, +id: U32, +mask: U32) -> J.Val:
  lanternward.kit.lantern.owned(
    Bool.and(lanternward.kit.Session.lantern_owned(s), lanternward.NAME_IN(ga.s(J.get(s, "$k"), "act"), ["Free", "Slash", "Hop"])),
    s, id, mask)

def lanternward.kit.lantern.owned(go: Bool, +s: J.Val, +id: U32, +mask: U32) -> J.Val:
  match go:
    case False{}:
      lanternward.ks(s, "lamp", 0)
    case True{}:
      lanternward.kit.lantern.held(U32.is_ne(U32.and(mask, 16), 0), s, id)

def lanternward.kit.lantern.held(held: Bool, +s: J.Val, +id: U32) -> J.Val:
  match held:
    case True{}:
      lanternward.kit.lantern.burn(
        lanternward.kit.lantern.cast(
          U32.is_eq(RS.u_min((lanternward.ku(s, "lamp") + 1 : U32), 255), 8),
          lanternward.ks(s, "lamp", RS.u_min((lanternward.ku(s, "lamp") + 1 : U32), 255))))
    case False{}:
      lanternward.ks(lanternward.kit.lantern.release(s, id), "lamp", 0)

def lanternward.kit.lantern.cast(first: Bool, +s: J.Val) -> J.Val:
  match first:
    case True{}:
      Bool.pick(J.Val, U32.is_gt(lanternward.ku(s, "flame"), 0), lanternward.sound(s, "spell_cast"), s)
    case False{}:
      s

def lanternward.kit.lantern.burn(+s: J.Val) -> J.Val:
  lanternward.kit.lantern.burn.go(lanternward.kit.Seat.flaring(J.get(s, "$k")), s)

def lanternward.kit.lantern.burn.go(flaring: Bool, +s: J.Val) -> J.Val:
  match flaring:
    case False{}:
      s
    case True{}:
      lanternward.kit.lantern.out(
        U32.is_eq(RS.u_sat_sub(lanternward.ku(s, "flame"), 1), 0),
        lanternward.ks(s, "flame", RS.u_sat_sub(lanternward.ku(s, "flame"), 1)))

# Sputters out: the knight cannot flare again for a bit.
def lanternward.kit.lantern.out(dead: Bool, +s: J.Val) -> J.Val:
  match dead:
    case False{}:
      s
    case True{}:
      lanternward.sound(lanternward.ks(lanternward.ks(s, "lamp", 0), "lamp_cd", 45), "mana_empty")

# A tap throws a bolt of light on the release.
def lanternward.kit.lantern.release(+s: J.Val, +id: U32) -> J.Val:
  lanternward.kit.lantern.tap(
    Bool.and(
      Bool.and(U32.is_gt(lanternward.ku(s, "lamp"), 0), U32.is_lt(lanternward.ku(s, "lamp"), 8)),
      U32.is_eq(lanternward.ku(s, "lamp_cd"), 0)),
    s, id)

def lanternward.kit.lantern.tap(go: Bool, +s: J.Val, +id: U32) -> J.Val:
  match go:
    case False{}:
      s
    case True{}:
      lanternward.kit.lantern.cost(U32.div(lanternward.R(s, "flame"), 5), s, id)

def lanternward.kit.lantern.cost(+cost: U32, +s: J.Val, +id: U32) -> J.Val:
  lanternward.kit.lantern.cost.go(U32.is_ge(lanternward.ku(s, "flame"), cost), cost, s, id)

def lanternward.kit.lantern.cost.go(afford: Bool, +cost: U32, +s: J.Val, +id: U32) -> J.Val:
  match afford:
    case True{}:
      lanternward.kit.lantern.throw(
        lanternward.kit.Session.lock_point(s, lanternward.kv(s, "lock")),
        lanternward.ks(lanternward.ks(s, "flame", (lanternward.ku(s, "flame") - cost : U32)), "lamp_cd", 14), id)
    case False{}:
      lanternward.sound(s, "mana_empty")

def lanternward.kit.lantern.throw(+pt: J.Val, +s: J.Val, +id: U32) -> J.Val:
  match pt:
    case J.JNull{}:
      lanternward.kit.lantern.bolt(lanternward.kface(s, 0), lanternward.kface(s, 1), s, id)
    case _:
      lanternward.kit.lantern.aim(
        lanternward.world.unit((ga.nu(pt, 0) - lanternward.ku(s, "x") : U32), (ga.nu(pt, 1) - lanternward.ku(s, "y") : U32)), s, id)

def lanternward.kit.lantern.aim(a: U32 & U32, +s: J.Val, +id: U32) -> J.Val:
  (+ax, +ay) = a
  lanternward.kit.lantern.bolt(ax, ay, s, id)

def lanternward.kit.lantern.bolt(+ax: U32, +ay: U32, +s: J.Val, +id: U32) -> J.Val:
  lanternward.sound(
    Bool.pick(J.Val, U32.is_lt(lanternward.len(s, "bolts"), 40),
      lanternward.push(s, "bolts",
        ga.su(
          ga.su(
            ga.si(
              ga.si(
                ga.si(
                  ga.si(ga.sv(J.obj(), "kind", J.JS{"Light"}), "x", (lanternward.ku(s, "x") + ax * 8 : U32)),
                  "y", (lanternward.ku(s, "y") + ay * 8 : U32)),
                "vx", RS.i_div((ax * 1150 : U32), 256)),
              "vy", RS.i_div((ay * 1150 : U32), 256)),
            "ttl", 30),
          "owner", U32.and(id, 255))),
      s),
    "shot_spark")

# ---- camera ----

# The view eases to the knight, leaning toward a lock; a fresh lock swings it
# in fast, a let-go eases it back slowly.
def lanternward.kit.Session.camera(+s: J.Val) -> J.Val:
  lanternward.kit.camera.lock(lanternward.kit.Session.lock_point(s, lanternward.kv(s, "lock")), s)

def lanternward.kit.camera.lock(+pt: J.Val, +s: J.Val) -> J.Val:
  match pt:
    case J.JNull{}:
      lanternward.kit.camera.boss(lanternward.ku(s, "x"), lanternward.ku(s, "y"), s)
    case _:
      lanternward.kit.camera.boss(
        (lanternward.ku(s, "x") + RS.i_div((RS.i_mul((ga.nu(pt, 0) - lanternward.ku(s, "x") : U32), 38)), 100) : U32),
        (lanternward.ku(s, "y") + RS.i_div((RS.i_mul((ga.nu(pt, 1) - lanternward.ku(s, "y") : U32), 38)), 100) : U32), s)

# The Warden stands head and shoulders over a knight: while it is up and not
# the lock, the view leans toward it, up to 52 px.
def lanternward.kit.camera.boss(+tx: U32, +ty: U32, +s: J.Val) -> J.Val:
  lanternward.kit.camera.boss.go(
    Bool.and(
      Bool.not(lanternward.tag_is(lanternward.kv(s, "lock"), "Boss")),
      Bool.and(
        Bool.not(J.is_null(J.get(s, "boss"))),
        Bool.and(U32.is_gt(ga.u(J.get(s, "boss"), "hp"), 0), Bool.not(lanternward.is(J.get(s, "boss"), "st", "Asleep"))))),
    tx, ty, s)

def lanternward.kit.camera.boss.go(pull: Bool, +tx: U32, +ty: U32, +s: J.Val) -> J.Val:
  match pull:
    case False{}:
      lanternward.kit.camera.cue(tx, ty, s)
    case True{}:
      lanternward.kit.camera.pull(
        (ga.u(J.get(s, "boss"), "x") - tx : U32), (ga.u(J.get(s, "boss"), "y") - ty : U32), tx, ty, s)

def lanternward.kit.camera.pull(+dx: U32, +dy: U32, +tx: U32, +ty: U32, +s: J.Val) -> J.Val:
  lanternward.kit.camera.pull.go(RS.i_max(lanternward.world.dist(dx, dy), 1), dx, dy, tx, ty, s)

def lanternward.kit.camera.pull.go(+d: U32, +dx: U32, +dy: U32, +tx: U32, +ty: U32, +s: J.Val) -> J.Val:
  lanternward.kit.camera.pull.fin(RS.i_min(RS.i_div(RS.i_mul(d, 70), 100), 13312), d, dx, dy, tx, ty, s)

def lanternward.kit.camera.pull.fin(+pull: U32, +d: U32, +dx: U32, +dy: U32, +tx: U32, +ty: U32, +s: J.Val) -> J.Val:
  lanternward.kit.camera.cue(
    (tx + lanternward.world.mul_div(dx, pull, d) : U32), (ty + lanternward.world.mul_div(dy, pull, d) : U32), s)

# A door or a switch that came right: every view looks at it a moment.
def lanternward.kit.camera.cue(+tx: U32, +ty: U32, +s: J.Val) -> J.Val:
  lanternward.kit.camera.cue.has(J.is_null(J.get(s, "cue")), tx, ty, s)

def lanternward.kit.camera.cue.has(none: Bool, +tx: U32, +ty: U32, +s: J.Val) -> J.Val:
  match none:
    case True{}:
      lanternward.kit.camera.ease(tx, ty, False{}, s)
    case False{}:
      lanternward.kit.camera.cue.on(
        ga.nu(J.get(s, "cue"), 2), ga.nu(J.get(s, "cue"), 0), ga.nu(J.get(s, "cue"), 1), tx, ty, s)

def lanternward.kit.camera.cue.on(+t: U32, +cx: U32, +cy: U32, +tx: U32, +ty: U32, +s: J.Val) -> J.Val:
  lanternward.kit.camera.cue.go(
    Bool.pick(U32, U32.is_gt(t, 36), RS.i_div((RS.i_mul((44 - t : U32), 100)), 8),
      Bool.pick(U32, U32.is_gt(t, 8), 100, RS.i_div((RS.i_mul(t, 100)), 8))),
    cx, cy, tx, ty, s)

def lanternward.kit.camera.cue.go(+on: U32, +cx: U32, +cy: U32, +tx: U32, +ty: U32, +s: J.Val) -> J.Val:
  lanternward.kit.camera.ease(
    (tx + RS.i_div(RS.i_mul((cx - tx : U32), on), 100) : U32), (ty + RS.i_div(RS.i_mul((cy - ty : U32), on), 100) : U32),
    RS.i_gt(on, 0), s)

def lanternward.kit.camera.ease(+tx: U32, +ty: U32, +cued: Bool, +s: J.Val) -> J.Val:
  lanternward.kit.camera.clamp(
    Bool.pick(U32, cued, 3,
      Bool.pick(U32, Bool.and(Bool.not(J.is_null(lanternward.kv(s, "lock"))), U32.is_lt(lanternward.ku(s, "lock_t"), 6)), 6,
        Bool.pick(U32, Bool.not(J.is_null(lanternward.kv(s, "lock"))), 3,
          Bool.pick(U32, U32.is_gt(lanternward.ku(s, "unlock_t"), 0), 2, 4)))),
    tx, ty, s)

def lanternward.kit.camera.clamp(+rate: U32, +tx: U32, +ty: U32, +s: J.Val) -> J.Val:
  lanternward.kit.camera.fin(
    RS.i_add(ga.nu(lanternward.kv(s, "cam"), 0), RS.i_div(RS.i_mul((tx - ga.nu(lanternward.kv(s, "cam"), 0) : U32), rate), 8)),
    RS.i_add(ga.nu(lanternward.kv(s, "cam"), 1), RS.i_div(RS.i_mul((ty - ga.nu(lanternward.kv(s, "cam"), 1) : U32), rate), 8)),
    (lanternward.world.Plan.w(ga.u(s, "room")) * 4096 : U32), (lanternward.world.Plan.h(ga.u(s, "room")) * 4096 : U32), s)

def lanternward.kit.camera.fin(+cx: U32, +cy: U32, +rw: U32, +rh: U32, +s: J.Val) -> J.Val:
  lanternward.ksv(s, "cam",
    ga.pair_i(
      Bool.pick(U32, RS.i_le(rw, 98304), RS.i_div(rw, 2), RS.i_clamp(cx, 49152, (rw - 49152 : U32))),
      Bool.pick(U32, RS.i_le(rh, 57344), RS.i_div(rh, 2), RS.i_clamp(cy, 28672, (rh - 28672 : U32)))))
