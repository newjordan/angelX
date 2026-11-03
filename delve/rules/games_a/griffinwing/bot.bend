# crates/delve/src/rules/griffinwing/bot.rs — a bot's hands: what a scripted
# knight presses to fly Griffinwing. An Option<(a, b)> is null or [a, b].

# How long F is held before it is a charge (the flight's own number).
def griffinwing.bot.HOLD_AT_BOT() -> U32:
  7

# A steady scatter in `-span..=span` from a knight, a tick and a salt.
def griffinwing.bot.noise(+id: U32, +t: U32, +salt: U32, +span: U32) -> U32:
  griffinwing.bot.noise.mix(U32.xor(U32.xor((id * 2654435761 : U32), (t * 2246822507 : U32)), (salt * 3266489909 : U32)), span)

def griffinwing.bot.noise.mix(+h0: U32, +span: U32) -> U32:
  griffinwing.bot.noise.fin(U32.xor(h0, U32.shrn(h0, 15n)), span)

def griffinwing.bot.noise.fin(+h1: U32, +span: U32) -> U32:
  griffinwing.bot.noise.fin2((h1 * 740637549 : U32), span)

def griffinwing.bot.noise.fin2(+h2: U32, +span: U32) -> U32:
  griffinwing.bot.noise.fin3(U32.xor(h2, U32.shrn(h2, 13n)), span)

def griffinwing.bot.noise.fin3(+h: U32, +span: U32) -> U32:
  (U32.mod(h, (2 * span + 1 : U32)) - span : U32)

# `steer`: -1, 0 or 1 toward `to`, with a dead zone.
def griffinwing.bot.steer(+from: U32, +to: U32, +dead: U32) -> U32:
  Bool.pick(U32, U32.is_le(gw.abs((to - from : U32)), dead), 0, gw.sig((to - from : U32)))

# What knight `id` presses this tick, with a sure hand.
def griffinwing.bot.Session.bot_input(+s: J.Val, +id: U32) -> J.Val:
  griffinwing.bot.Session.bot_with(s, id, griffinwing.Nums.default(), False{})

# The Input J: the default, with the keys set.
def griffinwing.bot.Input.make(+mx: U32, +my: U32, +ax: U32, +ay: U32, +fire: Bool, +dash: Bool, +bomb: Bool, +swing: Bool) -> J.Val:
  i0 = rules.Input.default()
  i1 = ga.si(i0, "move_x", mx)
  i2 = ga.si(i1, "move_y", my)
  i3 = ga.si(i2, "aim_x", ax)
  i4 = ga.si(i3, "aim_y", ay)
  i5 = ga.sb(i4, "fire", fire)
  i6 = ga.sb(i5, "dash", dash)
  i7 = ga.sb(i6, "bomb", bomb)
  ga.sb(i7, "swing", swing)

# What knight `id` presses this tick, for the numbers `n` stand at.
def griffinwing.bot.Session.bot_with(+s: J.Val, +id: U32, +n: J.Val, +novice: Bool) -> J.Val:
  gw.ifj(Bool.not(ga.has_key(J.get(s, "seats"), id)),
    _ => rules.Input.default(),
    _ => griffinwing.bot.Session.bot_with.seated(s, id, n, novice, gw.craft_of(s, id)))

def griffinwing.bot.Session.bot_with.seated(+s: J.Val, +id: U32, +n: J.Val, +novice: Bool, +c: J.Val) -> J.Val:
  gw.ifj(griffinwing.Session.Mode.is(s, "Tally"),
    _ => gw.ifj(U32.is_zero(U32.mod(ga.u(s, "mode_t"), 2)), _ => ga.sb(rules.Input.default(), "interact", True{}), _ => rules.Input.default()),
    _ => gw.ifj(Bool.or(griffinwing.Session.Mode.is(s, "Retry"), U32.is_gt(ga.u(c, "down"), 0)),
      _ => rules.Input.default(),
      _ => griffinwing.bot.Session.bot_with.plan(s, id, n, novice, c, griffinwing.flight.Session.craft_s(s, c))))

# Where to be, whether to roll, whether to brake: the plan, then the hands.
def griffinwing.bot.Session.bot_with.plan(+s: J.Val, +id: U32, +n: J.Val, +novice: Bool, +c: J.Val, +s_c: U32) -> J.Val:
  griffinwing.bot.Session.bot_with.hands(s, id, n, novice, c, s_c,
    griffinwing.bot.Session.plan(s, id, n, novice, c, s_c))

# The plan: {"want": null | [x, z], "rolls", "brake", "threat": null | [t, px, pz, gloam]}.
def griffinwing.bot.Session.plan(+s: J.Val, +id: U32, +n: J.Val, +novice: Bool, +c: J.Val, +s_c: U32) -> J.Val:
  griffinwing.bot.Session.plan.react(s, id, n, novice, c, s_c,
    Bool.and(griffinwing.combat.Session.latched(s, id), Bool.and(U32.is_zero(ga.u(c, "rest")), U32.is_zero(ga.u(c, "roll")))),
    griffinwing.bot.Session.plan.chaser(ga.items(s, "foes"), id, s_c),
    griffinwing.bot.Session.threat(s, c, s_c),
    gw.ifj(griffinwing.bot.Session.reflector(s, id), _ => griffinwing.bot.Session.intercept(s, s_c), _ => J.JNull{}))

# A friend with a kite on their tail, level with this knight: brake until the
# chaser is in front.
def griffinwing.bot.Session.plan.chaser(+fs: List<&2, J.Val>, +id: U32, +s_c: U32) -> Bool:
  match fs:
    case Nil{}:
      False{}
    case Con{+f, +t}:
      gw.ifb(Bool.and(gw.isn(f, "st", "Latched"), Bool.and(U32.is_le(ga.u(f, "on"), 6), Bool.and(U32.is_ne(ga.u(f, "on"), id), U32.is_ne(ga.u(f, "on"), 0)))),
        _ => gw.lt((ga.u(f, "s") - s_c : U32), griffinwing.types.dm(30)),
        _ => griffinwing.bot.Session.plan.chaser(t, id, s_c))

def griffinwing.bot.Session.plan.react(+s: J.Val, +id: U32, +n: J.Val, +novice: Bool, +c: J.Val, +s_c: U32, +rolls0: Bool, +brake0: Bool, +threat: J.Val, +seek: J.Val) -> J.Val:
  gw.ifj(Bool.not(J.is_null(seek)),
    _ => griffinwing.bot.Session.plan.seek(s, id, n, novice, c, s_c, brake0, threat, seek),
    _ => gw.ifj(Bool.not(J.is_null(threat)),
      _ => griffinwing.bot.Session.plan.dodge(s, id, n, novice, c, s_c, rolls0, brake0, threat),
      _ => griffinwing.bot.Session.plan.spots(s, id, n, c, s_c, J.JNull{}, rolls0, brake0, threat)))

# The Wyrm's lamp is shut: be where its spit will be, and roll. A novice meets
# the Wyrm's spit half the time.
def griffinwing.bot.Session.plan.seek(+s: J.Val, +id: U32, +n: J.Val, +novice: Bool, +c: J.Val, +s_c: U32, +brake0: Bool, +threat: J.Val, +seek: J.Val) -> J.Val:
  griffinwing.bot.Session.plan.spots(s, id, n, c, s_c, J.JA{[J.of_i32(ga.nu(seek, 1)), J.of_i32(ga.nu(seek, 2))]},
    Bool.and(Bool.or(Bool.not(novice), gw.ge(griffinwing.bot.noise(id, U32.div(ga.u(s, "t"), 60), 5, 1), 0)),
      Bool.and(U32.is_zero(ga.u(c, "rest")), Bool.and(U32.is_zero(ga.u(c, "roll")), U32.is_le(ga.nu(seek, 0), gw.ifu(novice, _ => 6, _ => 8))))),
    brake0, threat)

# A hostile bolt will cross: roll into a Gloam bolt, or make room.
def griffinwing.bot.Session.plan.dodge(+s: J.Val, +id: U32, +n: J.Val, +novice: Bool, +c: J.Val, +s_c: U32, +rolls0: Bool, +brake0: Bool, +threat: J.Val) -> J.Val:
  griffinwing.bot.Session.plan.dodge2(s, id, n, novice, c, s_c, rolls0, brake0, threat,
    Bool.and(Bool.and(U32.is_zero(ga.u(c, "rest")), U32.is_zero(ga.u(c, "roll"))),
      Bool.and(J.bool(J.nth(threat, 3)),
        Bool.and(Bool.or(griffinwing.bot.Session.reflector(s, id), U32.is_le(ga.nu(threat, 0), 6)),
          Bool.and(U32.is_le(ga.nu(threat, 0), gw.ifu(novice, _ => 6, _ => 9)),
            Bool.not(Bool.and(novice, U32.is_eq(griffinwing.bot.noise(id, U32.div(ga.u(s, "t"), 45), 9, 2), 0))))))))

def griffinwing.bot.Session.plan.dodge2(+s: J.Val, +id: U32, +n: J.Val, +novice: Bool, +c: J.Val, +s_c: U32, +rolls0: Bool, +brake0: Bool, +threat: J.Val, +roll: Bool) -> J.Val:
  gw.ifj(roll,
    _ => griffinwing.bot.Session.plan.spots(s, id, n, c, s_c, J.JNull{}, True{}, brake0, threat),
    _ => gw.ifj(rolls0,
      _ => griffinwing.bot.Session.plan.spots(s, id, n, c, s_c, J.JNull{}, True{}, brake0, threat),
      _ => gw.ifj(U32.is_le(ga.nu(threat, 0), gw.ifu(novice, _ => 22, _ => 40)),
        _ => griffinwing.bot.Session.plan.spots(s, id, n, c, s_c,
          griffinwing.bot.Session.room_from(c, ga.nu(threat, 1), ga.nu(threat, 2), n), rolls0, brake0, threat),
        _ => griffinwing.bot.Session.plan.spots(s, id, n, c, s_c, J.JNull{}, rolls0, brake0, threat))))

# Still no place to be: away from a footprint, a corridor, the next prop.
def griffinwing.bot.Session.plan.spots(+s: J.Val, +id: U32, +n: J.Val, +c: J.Val, +s_c: U32, +want: J.Val, +rolls: Bool, +brake: Bool, +threat: J.Val) -> J.Val:
  griffinwing.bot.Session.plan.fin(
    griffinwing.bot.Session.plan.props(s, id, n, c, s_c,
      gw.ifj(J.is_null(want),
        _ => gw.ifj(J.is_null(griffinwing.bot.Session.footprint_away(s, c, n)),
          _ => griffinwing.bot.Session.boss_corridor(s, c, n),
          _ => griffinwing.bot.Session.footprint_away(s, c, n)),
        _ => want),
      brake),
    rolls, threat)

def griffinwing.bot.Session.plan.fin(+w: J.Val, +rolls: Bool, +threat: J.Val) -> J.Val:
  ga.sv(ga.sb(w, "rolls", rolls), "threat", threat)

# The next thing on the rail to thread or take, when nothing else says where to
# be. {"want", "brake"}.
def griffinwing.bot.Session.plan.props(+s: J.Val, +id: U32, +n: J.Val, +c: J.Val, +s_c: U32, +want: J.Val, +brake: Bool) -> J.Val:
  gw.ifj(J.is_null(want),
    _ => griffinwing.bot.Session.plan.prop(s, id, n, c, s_c, ga.items(s, "props"), brake),
    _ => ga.sb(ga.sv(J.obj(), "want", want), "brake", brake))

def griffinwing.bot.Session.plan.prop(+s: J.Val, +id: U32, +n: J.Val, +c: J.Val, +s_c: U32, +ps: List<&2, J.Val>, +brake: Bool) -> J.Val:
  match ps:
    case Nil{}:
      ga.sb(ga.sv(J.obj(), "want", J.JNull{}), "brake", brake)
    case Con{+p, +t}:
      gw.ifj(gw.within((ga.u(p, "s") - s_c : U32), 0, griffinwing.types.dm(300)),
        _ => griffinwing.bot.Session.plan.take(s, id, n, c, s_c, p, t, brake),
        _ => griffinwing.bot.Session.plan.prop(s, id, n, c, s_c, t, brake))

def griffinwing.bot.Session.plan.take(+s: J.Val, +id: U32, +n: J.Val, +c: J.Val, +s_c: U32, +p: J.Val, +t: List<&2, J.Val>, +brake: Bool) -> J.Val:
  gw.ifj(Bool.and(gw.isn(p, "thing", "Arch"), gw.lt((ga.u(p, "s") - s_c : U32), griffinwing.types.dm(260))),
    _ => ga.sb(ga.sv(J.obj(), "want", ga.pair_i(ga.u(p, "x"), ga.u(p, "z"))), "brake", brake),
    _ => gw.ifj(Bool.and(Bool.or(gw.isn(p, "thing", "Gate"), gw.isn(p, "thing", "Ring")),
          Bool.and(U32.is_zero(U32.and(ga.u(p, "taken"), griffinwing.bit(id))), Bool.or(gw.isn(p, "thing", "Gate"), gw.lt(ga.u(c, "shield"), 80)))),
      _ => ga.sb(ga.sv(J.obj(), "want", ga.pair_i(ga.u(p, "x"), ga.u(p, "z"))), "brake", brake),
      _ => gw.ifj(Bool.and(gw.isn(p, "thing", "Shutter"), gw.lt((ga.u(p, "s") - s_c : U32), griffinwing.types.dm(220))),
        _ => ga.sb(ga.sv(J.obj(), "want", ga.pair_i(ga.u(p, "x"), ga.u(c, "z"))), "brake",
          Bool.or(brake, gw.lt(griffinwing.things.shutter_gap(p, (ga.u(s, "t") + gw.max(gw.div((ga.u(p, "s") - s_c : U32), griffinwing.types.PACE()), 0) : U32), griffinwing.Nums.wx(n)), griffinwing.types.dm(16)))),
        _ => gw.ifj(Bool.and(gw.isn(p, "thing", "Ridge"),
              Bool.and(gw.lt(gw.abs((ga.u(p, "x") - ga.u(c, "x") : U32)), (ga.u(p, "w") + griffinwing.types.dm(16) : U32)), gw.lt((ga.u(p, "s") - s_c : U32), griffinwing.types.dm(200)))),
          _ => ga.sb(ga.sv(J.obj(), "want", ga.pair_i(gw.neg((gw.sig(ga.u(p, "x")) * griffinwing.types.dm(20) : U32)), ga.u(c, "z"))), "brake", brake),
          _ => griffinwing.bot.Session.plan.prop(s, id, n, c, s_c, t, brake)))))

# The hands: steer, roll, aim, fire, bomb.
def griffinwing.bot.Session.bot_with.hands(+s: J.Val, +id: U32, +n: J.Val, +novice: Bool, +c: J.Val, +s_c: U32, +plan: J.Val) -> J.Val:
  griffinwing.bot.Session.bot_with.roll(s, id, n, novice, c, s_c, plan,
    gw.ifj(J.is_null(J.get(plan, "want")), _ => ga.pair_i(0, 0), _ => J.get(plan, "want")))

def griffinwing.bot.Session.bot_with.roll(+s: J.Val, +id: U32, +n: J.Val, +novice: Bool, +c: J.Val, +s_c: U32, +plan: J.Val, +to: J.Val) -> J.Val:
  griffinwing.bot.Session.bot_with.roll2(s, id, n, novice, c, s_c, plan,
    griffinwing.bot.steer(ga.u(c, "x"), ga.nu(to, 0), griffinwing.types.dm(8)),
    gw.neg(griffinwing.bot.steer(ga.u(c, "z"), ga.nu(to, 1), griffinwing.types.dm(8))))

def griffinwing.bot.Session.bot_with.roll2(+s: J.Val, +id: U32, +n: J.Val, +novice: Bool, +c: J.Val, +s_c: U32, +plan: J.Val, +mx: U32, +my: U32) -> J.Val:
  gw.ifj(Bool.and(ga.b(plan, "rolls"), Bool.and(griffinwing.combat.Session.latched(s, id), J.is_null(J.get(plan, "threat")))),
    _ => griffinwing.bot.Input.make(mx, my, 0, 0, False{}, U32.is_zero(U32.and(ga.u(c, "keys"), griffinwing.flight.K_DASH())), False{}, ga.b(plan, "brake")),
    _ => gw.ifj(ga.b(plan, "rolls"),
      _ => griffinwing.bot.Input.make(
        gw.ifu(Bool.and(Bool.not(J.is_null(J.get(plan, "threat"))), gw.ge(ga.nu(J.get(plan, "threat"), 1), ga.u(c, "x"))), _ => gw.neg(1), _ => 1),
        my, 0, 0, False{}, U32.is_zero(U32.and(ga.u(c, "keys"), griffinwing.flight.K_DASH())), False{}, ga.b(plan, "brake")),
      _ => griffinwing.bot.Session.bot_with.shoot(s, id, n, novice, c, s_c, mx, my, ga.b(plan, "brake"))))

# What to shoot: a kite on a friend's tail, a lamp that is open, a flight to
# sweep, the nearest foe.
def griffinwing.bot.Session.bot_with.shoot(+s: J.Val, +id: U32, +n: J.Val, +novice: Bool, +c: J.Val, +s_c: U32, +mx: U32, +my: U32, +brake: Bool) -> J.Val:
  griffinwing.bot.Session.bot_with.shoot2(s, id, n, novice, c, s_c, mx, my, brake,
    griffinwing.bot.Session.lockable(ga.items(s, "foes"), s_c))

# foes.filter(|f| near = if Latched { dm(15) } else { dm(40) }; f.lockable() && (near..=dm(480)).contains(f.s - s_c))
def griffinwing.bot.Session.lockable(+fs: List<&2, J.Val>, +s_c: U32) -> List<&2, J.Val>:
  match fs:
    case Nil{}:
      Nil{}
    case Con{+f, +t}:
      gw.ifjl(Bool.and(griffinwing.types.Foe.lockable(f),
          gw.within((ga.u(f, "s") - s_c : U32), gw.ifu(gw.isn(f, "st", "Latched"), _ => griffinwing.types.dm(15), _ => griffinwing.types.dm(40)), griffinwing.types.dm(480))),
        _ => Con{f, griffinwing.bot.Session.lockable(t, s_c)},
        _ => griffinwing.bot.Session.lockable(t, s_c))

# The foes of the list that are not lamps.
def griffinwing.bot.Session.fliers(+fs: List<&2, J.Val>) -> List<&2, J.Val>:
  match fs:
    case Nil{}:
      Nil{}
    case Con{+f, +t}:
      gw.ifjl(gw.isn(f, "kind", "Lamp"), _ => griffinwing.bot.Session.fliers(t), _ => Con{f, griffinwing.bot.Session.fliers(t)})

# The first foe of the list in this state / kind, or null.
def griffinwing.bot.Session.first_kind(+fs: List<&2, J.Val>, +kind: String) -> J.Val:
  match fs:
    case Nil{}:
      J.JNull{}
    case Con{+f, +t}:
      gw.ifj(gw.isn(f, "kind", kind), _ => f, _ => griffinwing.bot.Session.first_kind(t, kind))

# `lockable.filter(Latched).min_by_key(|f| f.s)`: the first of the least `s`.
def griffinwing.bot.Session.latched_first(+fs: List<&2, J.Val>, +best: J.Val) -> J.Val:
  match fs:
    case Nil{}:
      best
    case Con{+f, +t}:
      griffinwing.bot.Session.latched_first(t,
        gw.ifj(Bool.and(gw.isn(f, "st", "Latched"), Bool.or(J.is_null(best), gw.lt(ga.u(f, "s"), ga.u(best, "s")))), _ => f, _ => best))

# `fliers.filter(!in_locks).min_by_key(|f| |f.x - c.x| + |f.z - c.z|)`
def griffinwing.bot.Session.sweep_next(+fs: List<&2, J.Val>, +c: J.Val, +best: J.Val, +bd: U32) -> J.Val:
  match fs:
    case Nil{}:
      best
    case Con{+f, +t}:
      griffinwing.bot.Session.sweep_next2(f, t, c, best, bd,
        (gw.abs((ga.u(f, "x") - ga.u(c, "x") : U32)) + gw.abs((ga.u(f, "z") - ga.u(c, "z") : U32)) : U32),
        griffinwing.bot.Session.in_locks(ga.items(c, "locks"), ga.u(f, "id")))

def griffinwing.bot.Session.sweep_next2(+f: J.Val, +t: List<&2, J.Val>, +c: J.Val, +best: J.Val, +bd: U32, +d: U32, +locked: Bool) -> J.Val:
  gw.ifj(Bool.and(Bool.not(locked), Bool.or(J.is_null(best), gw.lt(d, bd))),
    _ => griffinwing.bot.Session.sweep_next(t, c, f, d),
    _ => griffinwing.bot.Session.sweep_next(t, c, best, bd))

def griffinwing.bot.Session.in_locks(+ls: List<&2, J.Val>, +id: U32) -> Bool:
  match ls:
    case Nil{}:
      False{}
    case Con{+l, +t}:
      Bool.or(U32.is_eq(ga.u(l, "foe"), id), griffinwing.bot.Session.in_locks(t, id))

# `fliers.min_by_key(|f| (f.s - s_c, f.id))`
def griffinwing.bot.Session.nearest(+fs: List<&2, J.Val>, +s_c: U32, +best: J.Val) -> J.Val:
  match fs:
    case Nil{}:
      best
    case Con{+f, +t}:
      griffinwing.bot.Session.nearest(t, s_c,
        gw.ifj(Bool.or(J.is_null(best),
            Bool.or(gw.lt(ga.u(f, "s"), ga.u(best, "s")), Bool.and(U32.is_eq(ga.u(f, "s"), ga.u(best, "s")), U32.is_lt(ga.u(f, "id"), ga.u(best, "id"))))),
          _ => f, _ => best))

def griffinwing.bot.Session.bot_with.shoot2(+s: J.Val, +id: U32, +n: J.Val, +novice: Bool, +c: J.Val, +s_c: U32, +mx: U32, +my: U32, +brake: Bool, +lockable: List<&2, J.Val>) -> J.Val:
  griffinwing.bot.Session.bot_with.shoot3(s, id, n, novice, c, s_c, mx, my, brake, lockable,
    griffinwing.bot.Session.first_kind(lockable, "Lamp"),
    griffinwing.bot.Session.latched_first(lockable, J.JNull{}),
    griffinwing.bot.Session.fliers(lockable))

def griffinwing.bot.Session.bot_with.shoot3(+s: J.Val, +id: U32, +n: J.Val, +novice: Bool, +c: J.Val, +s_c: U32, +mx: U32, +my: U32, +brake: Bool, +lockable: List<&2, J.Val>, +lamp_open: J.Val, +latched: J.Val, +fliers: List<&2, J.Val>) -> J.Val:
  griffinwing.bot.Session.bot_with.shoot4(s, id, n, novice, c, s_c, mx, my, brake, lamp_open, latched, fliers,
    U32.is_gt(ga.u(c, "held"), griffinwing.bot.HOLD_AT_BOT()),
    gw.ifj(Bool.not(J.is_null(latched)), _ => latched,
      _ => gw.ifj(Bool.not(J.is_null(lamp_open)), _ => lamp_open,
        _ => gw.ifj(U32.is_gt(ga.u(c, "held"), griffinwing.bot.HOLD_AT_BOT()),
          _ => griffinwing.bot.Session.sweep_next(fliers, c, J.JNull{}, 0),
          _ => griffinwing.bot.Session.nearest(fliers, s_c, J.JNull{})))))

def griffinwing.bot.Session.bot_with.shoot4(+s: J.Val, +id: U32, +n: J.Val, +novice: Bool, +c: J.Val, +s_c: U32, +mx: U32, +my: U32, +brake: Bool, +lamp_open: J.Val, +latched: J.Val, +fliers: List<&2, J.Val>, +held: Bool, +target: J.Val) -> J.Val:
  griffinwing.bot.Session.bot_with.fire(s, id, n, novice, c, s_c, mx, my, brake, lamp_open, latched, fliers, held, target,
    griffinwing.bot.Session.aim(s, id, novice, c, s_c, target))

# The reticle's key presses toward the target: [aim_x, aim_y].
def griffinwing.bot.Session.aim(+s: J.Val, +id: U32, +novice: Bool, +c: J.Val, +s_c: U32, +target: J.Val) -> J.Val:
  gw.ifj(J.is_null(target),
    _ => ga.pair_i(griffinwing.bot.steer(ga.u(c, "rx"), 0, griffinwing.flight.RET_V()), gw.neg(griffinwing.bot.steer(ga.u(c, "rz"), 0, griffinwing.flight.RET_V()))),
    _ => griffinwing.bot.Session.aim2(c, target,
      gw.max((ga.u(target, "s") - s_c : U32), griffinwing.types.dm(30)),
      gw.ifu(novice, _ => griffinwing.bot.noise(id, U32.div(ga.u(s, "t"), 14), 1, griffinwing.types.dm(15)), _ => 0),
      gw.ifu(novice, _ => griffinwing.bot.noise(id, U32.div(ga.u(s, "t"), 14), 2, griffinwing.types.dm(10)), _ => 0)))

def griffinwing.bot.Session.aim2(+c: J.Val, +f: J.Val, +ds: U32, +jx: U32, +jz: U32) -> J.Val:
  griffinwing.bot.Session.aim3(c,
    gw.clamp(gw.muldiv((ga.u(f, "x") - ga.u(c, "x") + jx : U32), griffinwing.types.FAR(), ds), gw.neg(griffinwing.flight.RX_MAX()), griffinwing.flight.RX_MAX()),
    gw.clamp(gw.muldiv((ga.u(f, "z") - ga.u(c, "z") + jz : U32), griffinwing.types.FAR(), ds), gw.neg(griffinwing.flight.RZ_MAX()), griffinwing.flight.RZ_MAX()))

def griffinwing.bot.Session.aim3(+c: J.Val, +rx: U32, +rz: U32) -> J.Val:
  ga.pair_i(
    griffinwing.bot.steer(ga.u(c, "rx"), rx, U32.div((griffinwing.flight.RET_V() * 3 : U32), 4)),
    gw.neg(griffinwing.bot.steer(ga.u(c, "rz"), rz, U32.div((griffinwing.flight.RET_V() * 3 : U32), 4))))

def griffinwing.bot.Session.bot_with.fire(+s: J.Val, +id: U32, +n: J.Val, +novice: Bool, +c: J.Val, +s_c: U32, +mx: U32, +my: U32, +brake: Bool, +lamp_open: J.Val, +latched: J.Val, +fliers: List<&2, J.Val>, +held: Bool, +target: J.Val, +aim: J.Val) -> J.Val:
  griffinwing.bot.Session.bot_with.bomb(s, c, s_c, mx, my, brake, fliers, aim,
    griffinwing.bot.Session.fire_key(s, id, n, novice, c, lamp_open, latched, fliers, held, target))

# Whether F is held down this tick.
def griffinwing.bot.Session.fire_key(+s: J.Val, +id: U32, +n: J.Val, +novice: Bool, +c: J.Val, +lamp_open: J.Val, +latched: J.Val, +fliers: List<&2, J.Val>, +held: Bool, +target: J.Val) -> Bool:
  gw.ifb(held,
    _ => Bool.not(griffinwing.bot.Session.fire_done(s, id, n, novice, c, lamp_open, latched, fliers)),
    _ => gw.ifb(U32.is_gt(ga.u(c, "held"), 0),
      _ => True{},
      _ => gw.ifb(Bool.or(Bool.not(J.is_null(lamp_open)), U32.is_ge(gw.len_j(fliers), 3)),
        _ => U32.is_zero(U32.and(ga.u(c, "keys"), griffinwing.flight.K_FIRE())),
        _ => gw.ifb(Bool.not(J.is_null(target)),
          _ => U32.is_zero(U32.and(ga.u(c, "keys"), griffinwing.flight.K_FIRE())),
          _ => False{}))))

def gw.len_j(+xs: List<&2, J.Val>) -> U32:
  match xs:
    case Nil{}:
      0
    case Con{+h, +t}:
      (1 + gw.len_j(t) : U32)

# Hold until the locks are in or the charge is full, then let go. A lamp wants
# the whole charge; a flight, whatever is in hand. A novice lets go early now and
# then, a lamp's charge unfinished.
def griffinwing.bot.Session.fire_done(+s: J.Val, +id: U32, +n: J.Val, +novice: Bool, +c: J.Val, +lamp_open: J.Val, +latched: J.Val, +fliers: List<&2, J.Val>) -> Bool:
  griffinwing.bot.Session.fire_done2(n, c, griffinwing.types.Craft.lock_total(ga.items(c, "locks")),
    Bool.and(novice, U32.is_eq(griffinwing.bot.noise(id, U32.div(ga.u(s, "t"), 90), 3, 2), 0)),
    Bool.and(Bool.not(J.is_null(lamp_open)), J.is_null(latched)),
    U32.min(gw.len_j(fliers), griffinwing.types.MAX_LOCKS()))

def griffinwing.bot.Session.fire_done2(+n: J.Val, +c: J.Val, +total: U32, +early: Bool, +lamp: Bool, +want_locks: U32) -> Bool:
  gw.ifb(lamp,
    _ => Bool.or(Bool.and(U32.is_ge(ga.u(c, "charge"), ga.u(n, "charge")), U32.is_ge(total, 2)), Bool.and(early, U32.is_ge(ga.u(c, "charge"), U32.div(ga.u(n, "charge"), 2)))),
    _ => Bool.or(Bool.and(U32.is_ge(total, want_locks), U32.is_ge(ga.u(c, "charge"), U32.div(ga.u(n, "charge"), 2))), U32.is_ge(ga.u(c, "charge"), ga.u(n, "charge"))))

# Starfall when worn down in a crowded sky.
def griffinwing.bot.Session.bot_with.bomb(+s: J.Val, +c: J.Val, +s_c: U32, +mx: U32, +my: U32, +brake: Bool, +fliers: List<&2, J.Val>, +aim: J.Val, +fire: Bool) -> J.Val:
  griffinwing.bot.Input.make(mx, my, ga.nu(aim, 0), ga.nu(aim, 1), fire, False{},
    Bool.and(gw.le(ga.u(c, "shield"), 35),
      Bool.and(U32.is_ge(griffinwing.bot.Session.crowd(fliers, s_c), 3),
        Bool.and(U32.is_gt(ga.u(c, "bombs"), 0), U32.is_zero(U32.and(ga.u(c, "keys"), griffinwing.flight.K_BOMB()))))),
    brake)

def griffinwing.bot.Session.crowd(+fs: List<&2, J.Val>, +s_c: U32) -> U32:
  match fs:
    case Nil{}:
      0
    case Con{+f, +t}:
      (Bool.to_u32(gw.lt((ga.u(f, "s") - s_c : U32), griffinwing.types.dm(260))) + griffinwing.bot.Session.crowd(t, s_c) : U32)

# Whether this knight is one of the rollers: the even seats, and any knight at
# all when no even seat is flying.
def griffinwing.bot.Session.reflector(+s: J.Val, +id: U32) -> Bool:
  Bool.or(U32.is_zero(U32.mod(id, 2)), Bool.not(griffinwing.bot.Session.even_flying(s, ga.ids(J.get(s, "seats")))))

def griffinwing.bot.Session.even_flying(+s: J.Val, +ids: List<&2, U32>) -> Bool:
  match ids:
    case Nil{}:
      False{}
    case Con{+k, +t}:
      Bool.or(Bool.and(U32.is_zero(U32.mod(k, 2)), griffinwing.types.Craft.flying(gw.craft_of(s, k))), griffinwing.bot.Session.even_flying(s, t))

# While the lit lamp is shut, the soonest of the Wyrm's spits that is owed to
# it: [ticks, x, z] or null.
def griffinwing.bot.Session.intercept(+s: J.Val, +s_c: U32) -> J.Val:
  gw.ifj(J.is_null(J.get(s, "boss")),
    _ => J.JNull{},
    _ => griffinwing.bot.Session.intercept2(s, s_c, J.u32(J.nth(J.get(J.get(s, "boss"), "lamps"), U32.min(ga.u(J.get(s, "boss"), "phase"), 2)))))

def griffinwing.bot.Session.intercept2(+s: J.Val, +s_c: U32, +lit: U32) -> J.Val:
  gw.ifj(griffinwing.bot.Session.shut(ga.items(s, "foes"), lit),
    _ => griffinwing.bot.Session.intercept3(ga.items(s, "bolts"), s_c, lit, J.JNull{}),
    _ => J.JNull{})

def griffinwing.bot.Session.shut(+fs: List<&2, J.Val>, +lit: U32) -> Bool:
  match fs:
    case Nil{}:
      False{}
    case Con{+f, +t}:
      Bool.or(Bool.and(U32.is_eq(ga.u(f, "id"), lit), gw.isn(f, "st", "Shut")), griffinwing.bot.Session.shut(t, lit))

# The first of the least ticks.
def griffinwing.bot.Session.intercept3(+bs: List<&2, J.Val>, +s_c: U32, +lit: U32, +best: J.Val) -> J.Val:
  match bs:
    case Nil{}:
      best
    case Con{+b, +t}:
      gw.ifj(Bool.and(gw.isn(b, "shot", "Gloam"), Bool.and(U32.is_eq(ga.u(b, "src"), lit), gw.gt(ga.u(b, "s"), s_c))),
        _ => griffinwing.bot.Session.intercept4(b, t, s_c, lit, best,
          gw.max(gw.div((ga.u(b, "s") - s_c : U32), gw.max((griffinwing.types.PACE() - ga.u(b, "vs") : U32), 1)), 0)),
        _ => griffinwing.bot.Session.intercept3(t, s_c, lit, best))

def griffinwing.bot.Session.intercept4(+b: J.Val, +t: List<&2, J.Val>, +s_c: U32, +lit: U32, +best: J.Val, +ticks: U32) -> J.Val:
  gw.ifj(Bool.or(J.is_null(best), U32.is_lt(ticks, ga.nu(best, 0))),
    _ => griffinwing.bot.Session.intercept3(t, s_c, lit,
      J.JA{[J.JU{ticks}, J.of_i32((ga.u(b, "x") + (ga.u(b, "vx") * ticks : U32) : U32)), J.of_i32((ga.u(b, "z") + (ga.u(b, "vz") * ticks : U32) : U32))]}),
    _ => griffinwing.bot.Session.intercept3(t, s_c, lit, best))

# The soonest hostile bolt that will pass near this craft's plane: [ticks to it,
# x, z where it will cross, whether it is a Gloam bolt] or null.
def griffinwing.bot.Session.threat(+s: J.Val, +c: J.Val, +s_c: U32) -> J.Val:
  griffinwing.bot.Session.threat2(ga.items(s, "bolts"), c, s_c, J.JNull{})

def griffinwing.bot.Session.threat2(+bs: List<&2, J.Val>, +c: J.Val, +s_c: U32, +best: J.Val) -> J.Val:
  match bs:
    case Nil{}:
      best
    case Con{+b, +t}:
      gw.ifj(Bool.and(Bool.or(gw.isn(b, "shot", "Spark"), gw.isn(b, "shot", "Gloam")),
          gw.within((ga.u(b, "s") - s_c : U32), 1, griffinwing.types.dm(220))),
        _ => griffinwing.bot.Session.threat3(b, t, c, s_c, best,
          gw.max(gw.div((ga.u(b, "s") - s_c : U32), gw.max((griffinwing.types.PACE() - ga.u(b, "vs") : U32), 1)), 0)),
        _ => griffinwing.bot.Session.threat2(t, c, s_c, best))

def griffinwing.bot.Session.threat3(+b: J.Val, +t: List<&2, J.Val>, +c: J.Val, +s_c: U32, +best: J.Val, +ticks: U32) -> J.Val:
  griffinwing.bot.Session.threat4(b, t, c, s_c, best, ticks,
    (ga.u(b, "x") + (ga.u(b, "vx") * ticks : U32) : U32), (ga.u(b, "z") + (ga.u(b, "vz") * ticks : U32) : U32))

def griffinwing.bot.Session.threat4(+b: J.Val, +t: List<&2, J.Val>, +c: J.Val, +s_c: U32, +best: J.Val, +ticks: U32, +px: U32, +pz: U32) -> J.Val:
  gw.ifj(Bool.and(gw.nlt(griffinwing.types.flat(px, pz, ga.u(c, "x"), ga.u(c, "z")), griffinwing.types.dm(26)),
      Bool.or(J.is_null(best), U32.is_lt(ticks, ga.nu(best, 0)))),
    _ => griffinwing.bot.Session.threat2(t, c, s_c, J.JA{[J.JU{ticks}, J.of_i32(px), J.of_i32(pz), J.JBool{gw.isn(b, "shot", "Gloam")}]}),
    _ => griffinwing.bot.Session.threat2(t, c, s_c, best))

# Where to go to be clear of the point `(ox, oz)`: four ways out of it (left,
# right, up, down), those the steer box allows, the one that ends farthest from
# it; the last of equals wins (`max_by_key`).
def griffinwing.bot.Session.room_from(+c: J.Val, +ox: U32, +oz: U32, +n: J.Val) -> J.Val:
  griffinwing.bot.Session.room_from.pick(c, ox, oz, n,
    ga.pair_i(0, 0), 0, 0)

def griffinwing.bot.Session.room_from.pick(+c: J.Val, +ox: U32, +oz: U32, +n: J.Val, +best: J.Val, +bd: U32, +k: U32) -> J.Val:
  gw.ifj(U32.is_lt(k, 4),
    _ => griffinwing.bot.Session.room_from.cand(c, ox, oz, n, best, bd, k,
      gw.clamp((ga.u(c, "x") + griffinwing.bot.Session.room_from.dx(k) : U32), gw.neg(griffinwing.Nums.wx(n)), griffinwing.Nums.wx(n)),
      gw.clamp((ga.u(c, "z") + griffinwing.bot.Session.room_from.dz(k) : U32), gw.neg(griffinwing.Nums.wz(n)), griffinwing.Nums.wz(n))),
    _ => best)

def griffinwing.bot.Session.room_from.dx(+k: U32) -> U32:
  Bool.pick(U32, U32.is_eq(k, 0), griffinwing.types.dm(40), Bool.pick(U32, U32.is_eq(k, 1), gw.neg(griffinwing.types.dm(40)), 0))

def griffinwing.bot.Session.room_from.dz(+k: U32) -> U32:
  Bool.pick(U32, U32.is_eq(k, 2), griffinwing.types.dm(30), Bool.pick(U32, U32.is_eq(k, 3), gw.neg(griffinwing.types.dm(30)), 0))

def griffinwing.bot.Session.room_from.cand(+c: J.Val, +ox: U32, +oz: U32, +n: J.Val, +best: J.Val, +bd: U32, +k: U32, +x: U32, +z: U32) -> J.Val:
  griffinwing.bot.Session.room_from.cmp(c, ox, oz, n, best, bd, k, x, z, griffinwing.types.flat(x, z, ox, oz))

def griffinwing.bot.Session.room_from.cmp(+c: J.Val, +ox: U32, +oz: U32, +n: J.Val, +best: J.Val, +bd: U32, +k: U32, +x: U32, +z: U32, +d: U32) -> J.Val:
  gw.ifj(Bool.or(U32.is_zero(k), U32.is_ge(d, bd)),
    _ => griffinwing.bot.Session.room_from.pick(c, ox, oz, n, ga.pair_i(x, z), d, (k + 1 : U32)),
    _ => griffinwing.bot.Session.room_from.pick(c, ox, oz, n, best, bd, (k + 1 : U32)))

# A footprint locked on this craft (a turret's, a bomber's lane): the place to
# be instead. [x, z] or null.
def griffinwing.bot.Session.footprint_away(+s: J.Val, +c: J.Val, +n: J.Val) -> J.Val:
  griffinwing.bot.Session.footprint_away.foes(s, c, n, ga.items(s, "foes"))

def griffinwing.bot.Session.footprint_away.foes(+s: J.Val, +c: J.Val, +n: J.Val, +fs: List<&2, J.Val>) -> J.Val:
  match fs:
    case Nil{}:
      J.JNull{}
    case Con{+f, +t}:
      gw.ifj(Bool.and(gw.isn(f, "st", "Tell"), Bool.or(gw.isn(f, "kind", "Turret"), Bool.or(gw.isn(f, "kind", "Bomber"), gw.isn(f, "kind", "Wisp")))),
        _ => griffinwing.bot.Session.footprint_away.marks(s, c, n, t, J.items(griffinwing.combat.Session.marks(f, griffinwing.Session.pressure(s)))),
        _ => griffinwing.bot.Session.footprint_away.foes(s, c, n, t))

def griffinwing.bot.Session.footprint_away.marks(+s: J.Val, +c: J.Val, +n: J.Val, +t: List<&2, J.Val>, +ms: List<&2, J.Val>) -> J.Val:
  match ms:
    case Nil{}:
      griffinwing.bot.Session.footprint_away.foes(s, c, n, t)
    case Con{+m, +mt}:
      gw.ifj(gw.nlt(griffinwing.types.flat(ga.u(c, "x"), ga.u(c, "z"), ga.nu(m, 0), ga.nu(m, 1)), griffinwing.types.dm(24)),
        _ => griffinwing.bot.Session.room_from(c, ga.nu(m, 0), ga.nu(m, 1), n),
        _ => griffinwing.bot.Session.footprint_away.marks(s, c, n, t, mt))

# The Wyrm's coil wants the gap, its roar wants the column left. [x, z] or null.
def griffinwing.bot.Session.boss_corridor(+s: J.Val, +c: J.Val, +n: J.Val) -> J.Val:
  gw.ifj(J.is_null(J.get(s, "boss")),
    _ => J.JNull{},
    _ => griffinwing.bot.Session.boss_corridor2(c, n, J.get(s, "boss")))

def griffinwing.bot.Session.boss_corridor2(+c: J.Val, +n: J.Val, +b: J.Val) -> J.Val:
  gw.ifj(Bool.or(gw.isn(b, "st", "Tell"), Bool.or(gw.isn(b, "st", "Coil"), gw.isn(b, "st", "Roar"))),
    _ => gw.ifj(U32.is_eq(ga.u(b, "atk"), 1),
      _ => ga.pair_i(ga.u(b, "gap"), ga.u(c, "z")),
      _ => gw.ifj(U32.is_eq(ga.u(b, "atk"), 2),
        _ => griffinwing.bot.Session.boss_corridor3(c, n, b),
        _ => J.JNull{})),
    _ => J.JNull{})

def griffinwing.bot.Session.boss_corridor3(+c: J.Val, +n: J.Val, +b: J.Val) -> J.Val:
  gw.ifj(gw.lt(gw.abs((ga.u(c, "x") - ga.u(b, "col") : U32)), (griffinwing.boss.COL_HALF() + griffinwing.types.dm(16) : U32)),
    _ => ga.pair_i(
      gw.clamp((ga.u(b, "col") + (gw.ifu(gw.ge(ga.u(c, "x"), ga.u(b, "col")), _ => 1, _ => gw.neg(1)) * (griffinwing.boss.COL_HALF() + griffinwing.types.dm(30)) : U32) : U32),
        gw.neg(griffinwing.Nums.wx(n)), griffinwing.Nums.wx(n)),
      ga.u(c, "z")),
    _ => J.JNull{})
