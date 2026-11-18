import Base

def hz() -> U32:
  30

def goal() -> U32:
  10

def title() -> U32:
  90

def respawn_ticks() -> U32:
  60

def protection_ticks() -> U32:
  60

def podium() -> U32:
  180

def pickup_rest() -> U32:
  450

def power_time() -> U32:
  240

def heal() -> U32:
  35

def plate() -> U32:
  2

def burn_every() -> U32:
  15

def burn_blow() -> U32:
  4

def burn_ticks() -> U32:
  60

def feed_len() -> U32:
  5

def feed_life() -> U32:
  180

def turret_beat() -> U32:
  20

def field_beat() -> U32:
  10

def max_u32() -> U32:
  4294967295

def max_u16() -> U32:
  65535

type Rules is Data:
  Rules{goal: U32, title: U32, respawn: U32, protection: U32, podium: U32, pickup_rest: U32, power_time: U32}

def consts() -> Rules:
  Rules{goal(), title(), respawn_ticks(), protection_ticks(), podium(), pickup_rest(), power_time()}

def melee_tick(+winner: Bool, +connected: U32) -> Bool:
  Bool.and(Bool.not(winner), U32.is_gt(connected, 1))

type ContactTier is Data:
  Light{}
  Medium{}
  Heavy{}

def contact_tier(+damage: U32) -> ContactTier:
  Bool.pick(ContactTier, U32.is_gt(damage, 44), Heavy{}, Bool.pick(ContactTier, U32.is_gt(damage, 19), Medium{}, Light{}))

def sat_sub(+a: U32, +b: U32) -> U32:
  Bool.pick(U32, U32.is_gt(a, b), (a - b : U32), 0)

def sat_inc(+a: U32) -> U32:
  Bool.pick(U32, U32.is_eq(a, max_u32()), a, (a + 1 : U32))

def sat_inc16(+a: U32) -> U32:
  Bool.pick(U32, U32.is_ge(a, max_u16()), max_u16(), (a + 1 : U32))

def dec_all(xs: List<&2, U32>) -> List<&2, U32>:
  match xs:
    case Nil{}:
      Nil{}
    case Con{h, t}:
      Con{sat_sub(h, 1), dec_all(t)}

type Open is Data:
  Open{go: Bool, ended: U32, ticks: U32, pickups: List<&2, U32>}

def titled(+playing: Bool, +ended: U32, +ticks: U32, pickups: List<&2, U32>) -> Open:
  match playing:
    case True{}:
      Open{True{}, ended, ticks, dec_all(pickups)}
    case False{}:
      Open{False{}, ended, ticks, pickups}

def begun(+few: Bool, +ended: U32, +ticks: U32, pickups: List<&2, U32>) -> Open:
  match few:
    case True{}:
      Open{False{}, ended, ticks, pickups}
    case False{}:
      +next = sat_inc(ticks)
      titled(U32.is_gt(next, title()), ended, next, pickups)

def begin(+winner: Bool, +ended: U32, +connected: U32, +ticks: U32, pickups: List<&2, U32>) -> Open:
  match winner:
    case True{}:
      Open{False{}, (ended + 1 : U32), ticks, pickups}
    case False{}:
      begun(U32.is_lt(connected, 2), ended, ticks, pickups)

type Seat is Data:
  Seat{id: U32, player: Bool, absent: Bool, away: Bool, outside: Bool, fighter: Bool, gold: U32}

type Posted is Data:
  Posted{id: U32, join: Bool, connected: Bool, stake: U32}

def post(+crossroads: Bool, +winner: Bool, s: Seat) -> Posted:
  match s:
    case Seat{id, +player, +absent, +away, +outside, +fighter, gold}:
      +here = Bool.and(player, Bool.not(absent))
      +eligible = Bool.and(Bool.not(crossroads), Bool.and(here, Bool.not(outside)))
      Posted{id, Bool.and(eligible, Bool.and(Bool.not(winner), Bool.not(fighter))), Bool.and(Bool.and(here, Bool.not(away)), Bool.or(crossroads, Bool.not(outside))), (gold / 10 : U32)}

def sync(+crossroads: Bool, +winner: Bool, rows: List<&2, Seat>) -> List<&2, Posted>:
  match rows:
    case Nil{}:
      Nil{}
    case Con{h, t}:
      Con{post(crossroads, winner, h), sync(crossroads, winner, t)}

type Body is Data:
  Body{connected: Bool, respawn: U32, protection: U32, power: U32, flash: U32, stun: U32, fart_wait: U32, burn: U32}

type Kit is Data:
  Kit{leeched: U32, passive: U32, chilled: U32, invulnerable: U32, fire_cooldown: U32, sword_cooldown: U32, play_cooldown: U32, bomb_cooldown: U32, spell_cooldowns: List<&2, U32>}

type Stance is Data:
  Stance{fury: U32, windup: U32, rot: U32}

type Turn is Data:
  Turn{ticks: U32, orbit_every: U32, rot_every: U32, winner: Bool, living: Bool, fighter: Body, hero: Kit, champ: Stance}

type Act is Data:
  Skip{}
  Spawn{}
  Play{}

type Tick is Data:
  Tick{act: Act, fighter: Body, hero: Kit, champ: Stance, burn_blow: U32, slam: Bool, pulse: Bool, orbit: Bool, power_num: U32, armed: Bool}

type Flags is Data:
  Flags{burn_blow: U32, orbit: Bool, power_num: U32, armed: Bool}

def burn_pulse(+burn: U32) -> Bool:
  Bool.and(U32.is_gt(burn, 0), U32.is_eq((sat_sub(burn, 1) % burn_every() : U32), 0))

def rot_pulse(+rot: U32, +every: U32) -> Bool:
  Bool.and(U32.is_gt(rot, 0), U32.is_eq((rot % every : U32), 0))

def per_second(+ticks: U32, +kept: U32) -> U32:
  Bool.pick(U32, U32.is_eq((ticks % hz() : U32), 0), 0, kept)

def age_kit(k: Kit, +ticks: U32) -> Kit:
  match k:
    case Kit{leeched, passive, chilled, invulnerable, fire_cooldown, sword_cooldown, play_cooldown, bomb_cooldown, spell_cooldowns}:
      Kit{per_second(ticks, leeched), per_second(ticks, passive), sat_sub(chilled, 1), sat_sub(invulnerable, 1), sat_sub(fire_cooldown, 1), sat_sub(sword_cooldown, 1), sat_sub(play_cooldown, 1), sat_sub(bomb_cooldown, 1), dec_all(spell_cooldowns)}

def age_body(b: Body) -> Body:
  match b:
    case Body{connected, respawn, protection, power, flash, stun, fart_wait, burn}:
      Body{connected, respawn, sat_sub(protection, 1), sat_sub(power, 1), sat_sub(flash, 1), sat_sub(stun, 1), sat_sub(fart_wait, 1), sat_sub(burn, 1)}

def with_respawn(b: Body, +left: U32) -> Body:
  match b:
    case Body{connected, respawn, protection, power, flash, stun, fart_wait, burn}:
      Body{connected, left, protection, power, flash, stun, fart_wait, burn}

def flags(b: Body, +ticks: U32, +orbit_every: U32, +winner: Bool, +living: Bool) -> Flags:
  match b:
    case Body{connected, respawn, protection, power, flash, stun, fart_wait, burn}:
      Flags{Bool.pick(U32, burn_pulse(burn), burn_blow(), 0), Bool.and(U32.is_eq(sat_sub(protection, 1), 0), U32.is_eq((ticks % orbit_every : U32), 0)), Bool.pick(U32, U32.is_gt(sat_sub(power, 1), 0), 3, 2), Bool.and(Bool.not(winner), Bool.and(Bool.and(connected, living), Bool.and(U32.is_eq(respawn, 0), U32.is_eq(sat_sub(stun, 1), 0))))}

def played_with(f: Flags, b: Body, k: Kit, s: Stance, +every: U32) -> Tick:
  match f:
    case Flags{burn_blow, orbit, power_num, armed}:
      match s:
        case Stance{fury, +windup, +rot}:
          Tick{Play{}, b, k, Stance{sat_sub(fury, 1), sat_sub(windup, 1), sat_sub(rot, 1)}, burn_blow, U32.is_eq(windup, 1), rot_pulse(sat_sub(rot, 1), every), orbit, power_num, armed}

def played(+b: Body, k: Kit, s: Stance, +ticks: U32, +orbit_every: U32, +rot_every: U32, +winner: Bool, +living: Bool) -> Tick:
  played_with(flags(b, ticks, orbit_every, winner, living), age_body(b), age_kit(k, ticks), s, rot_every)

def sat_out(b: Body, k: Kit, s: Stance) -> Tick:
  Tick{Skip{}, b, k, s, 0, False{}, False{}, False{}, 2, False{}}

def rested(+waking: Bool, b: Body, k: Kit, s: Stance) -> Tick:
  match waking:
    case True{}:
      Tick{Spawn{}, b, k, s, 0, False{}, False{}, False{}, 2, False{}}
    case False{}:
      sat_out(b, k, s)

def counted(+resting: Bool, +b: Body, k: Kit, s: Stance, +ticks: U32, +orbit_every: U32, +rot_every: U32, +respawn: U32, +winner: Bool, +living: Bool) -> Tick:
  match resting:
    case True{}:
      rested(U32.is_eq(respawn, 1), with_respawn(b, sat_sub(respawn, 1)), k, s)
    case False{}:
      played(b, k, s, ticks, orbit_every, rot_every, winner, living)

def stood(+b: Body, k: Kit, s: Stance, +ticks: U32, +orbit_every: U32, +rot_every: U32, +winner: Bool, +living: Bool) -> Tick:
  match b:
    case Body{+connected, +respawn, protection, power, flash, stun, fart_wait, burn}:
      match connected:
        case False{}:
          sat_out(b, k, s)
        case True{}:
          counted(U32.is_gt(respawn, 0), b, k, s, ticks, orbit_every, rot_every, respawn, winner, living)

def turn(t: Turn) -> Tick:
  match t:
    case Turn{ticks, orbit_every, rot_every, winner, living, fighter, hero, champ}:
      stood(fighter, hero, champ, ticks, orbit_every, rot_every, winner, living)

def turns(ts: List<&2, Turn>) -> List<&2, Tick>:
  match ts:
    case Nil{}:
      Nil{}
    case Con{h, t}:
      Con{turn(h), turns(t)}

type Fresh is Data:
  Fresh{index: U32, protection: U32, power: U32, stun: U32, burn: U32, invulnerable: U32, fire_cooldown: U32, spell_cooldowns: List<&2, U32>}

def farthest(clear: List<&2, F32>, +i: U32, +best: U32, +far: F32) -> U32:
  match clear:
    case Nil{}:
      best
    case Con{+c, rest}:
      +better = F32.is_gt(c, far)
      farthest(rest, (i + 1 : U32), Bool.pick(U32, better, i, best), Bool.pick(F32, better, c, far))

def spawn(clear: List<&2, F32>) -> Fresh:
  Fresh{farthest(clear, 0, 0, (0.0 - 1.0 : F32)), protection_ticks(), 0, 0, 0, 0, 0, Con{0, Con{0, Con{0, Con{0, Nil{}}}}}}

type Blow is Data:
  Blow{owner: U32, target: U32, winner: Bool, owner_in: Bool, target_in: Bool, connected: Bool, hp: U32, respawn: U32, protection: U32, blocked: Bool}

type Verdict is Data:
  Reject{}
  Turned{}
  Strike{}

def lands(+blocked: Bool) -> Verdict:
  match blocked:
    case True{}:
      Turned{}
    case False{}:
      Strike{}

def open_to(+fair: Bool, +blocked: Bool) -> Verdict:
  match fair:
    case True{}:
      lands(blocked)
    case False{}:
      Reject{}

def hit(b: Blow) -> Verdict:
  match b:
    case Blow{+owner, +target, winner, owner_in, target_in, connected, hp, respawn, protection, blocked}:
      +living = Bool.and(connected, Bool.and(U32.is_gt(hp, 0), U32.is_eq(respawn, 0)))
      open_to(Bool.and(Bool.and(Bool.not(U32.is_eq(owner, target)), Bool.not(winner)), Bool.and(Bool.and(owner_in, target_in), Bool.and(living, U32.is_eq(protection, 0)))), blocked)

type Frag is Data:
  Frag{killer: U32, victim: U32, tick: U32}

type Struck is Data:
  Struck{owner: U32, target: U32, before: U32, after: U32, ticks: U32, kills: U32, deaths: U32, feed: List<&2, Frag>}

type Scored is Data:
  Scored{landed: Bool, died: Bool, respawn: U32, deaths: U32, kills: U32, feed: List<&2, Frag>, winner: Bool, ended: U32}

def length(xs: List<&2, Frag>, +n: U32) -> U32:
  match xs:
    case Nil{}:
      n
    case Con{h, t}:
      length(t, (n + 1 : U32))

def drop_head(xs: List<&2, Frag>) -> List<&2, Frag>:
  match xs:
    case Nil{}:
      Nil{}
    case Con{h, t}:
      t

def trimmed(+full: Bool, feed: List<&2, Frag>) -> List<&2, Frag>:
  match full:
    case True{}:
      drop_head(feed)
    case False{}:
      feed

def pushed(+feed: List<&2, Frag>, f: Frag) -> List<&2, Frag>:
  +all = List.append(&2, Frag, feed, Con{f, Nil{}})
  trimmed(U32.is_gt(length(all, 0), feed_len()), all)

def slain(+kills: U32, +deaths: U32, +owner: U32, +target: U32, +ticks: U32, feed: List<&2, Frag>) -> Scored:
  +made = (kills + 1 : U32)
  Scored{True{}, True{}, respawn_ticks(), sat_inc16(deaths), made, pushed(feed, Frag{owner, target, ticks}), U32.is_eq(made, goal()), 0}

def wounded(+hurt: Bool, +died: Bool, +kills: U32, +deaths: U32, +owner: U32, +target: U32, +ticks: U32, feed: List<&2, Frag>) -> Scored:
  match hurt:
    case False{}:
      Scored{False{}, False{}, 0, deaths, kills, feed, False{}, 0}
    case True{}:
      match died:
        case True{}:
          slain(kills, deaths, owner, target, ticks, feed)
        case False{}:
          Scored{True{}, False{}, 0, deaths, kills, feed, False{}, 0}

def struck(s: Struck) -> Scored:
  match s:
    case Struck{owner, target, +before, +after, ticks, kills, deaths, feed}:
      wounded(Bool.not(U32.is_eq(before, after)), U32.is_eq(after, 0), kills, deaths, owner, target, ticks, feed)

type Spot is Data:
  Spot{near: Bool, kind: U32, wait: U32}

type Grab is Data:
  Grab{spots: List<&2, Spot>, hp: U32, max_hp: U32, armor: U32, cap: U32, power: U32}

type Got is Data:
  Got{taken: Bool, hp: U32, armor: U32, power: U32}

type Taken is Data:
  Taken{pickups: List<&2, U32>, hp: U32, armor: U32, power: U32}

def gives_power(+kind: U32, +hp: U32, +armor: U32, +power: U32) -> Got:
  Bool.pick(Got, U32.is_eq(kind, 2), Got{True{}, hp, armor, power_time()}, Got{False{}, hp, armor, power})

def gives_armor(+kind: U32, +hp: U32, +armor: U32, +cap: U32, +power: U32) -> Got:
  Bool.pick(Got, U32.is_eq(kind, 1), Bool.pick(Got, U32.is_lt(armor, cap), Got{True{}, hp, U32.min((armor + plate() : U32), cap), power}, Got{False{}, hp, armor, power}), gives_power(kind, hp, armor, power))

def taking(+kind: U32, +hp: U32, +max_hp: U32, +armor: U32, +cap: U32, +power: U32) -> Got:
  Bool.pick(Got, U32.is_eq(kind, 0), Bool.pick(Got, U32.is_lt(hp, max_hp), Got{True{}, U32.min((hp + heal() : U32), max_hp), armor, power}, Got{False{}, hp, armor, power}), gives_armor(kind, hp, armor, cap, power))

def got_taken(g: Got) -> Bool:
  match g:
    case Got{taken, hp, armor, power}:
      taken

def got_hp(g: Got) -> U32:
  match g:
    case Got{taken, hp, armor, power}:
      hp

def got_armor(g: Got) -> U32:
  match g:
    case Got{taken, hp, armor, power}:
      armor

def got_power(g: Got) -> U32:
  match g:
    case Got{taken, hp, armor, power}:
      power

def with_wait(+w: U32, t: Taken) -> Taken:
  match t:
    case Taken{pickups, hp, armor, power}:
      Taken{Con{w, pickups}, hp, armor, power}

def rested_after(+taken: Bool, +wait: U32) -> U32:
  Bool.pick(U32, taken, pickup_rest(), wait)

def gather(spots: List<&2, Spot>, +hp: U32, +max_hp: U32, +armor: U32, +cap: U32, +power: U32) -> Taken:
  match spots:
    case Nil{}:
      Taken{Nil{}, hp, armor, power}
    case Con{spot, rest}:
      match spot:
        case Spot{+near, +kind, +wait}:
          +g = taking(kind, hp, max_hp, armor, cap, power)
          +ready = Bool.and(near, U32.is_eq(wait, 0))
          +got = Bool.pick(Got, ready, g, Got{False{}, hp, armor, power})
          with_wait(rested_after(got_taken(got), wait), gather(rest, got_hp(got), max_hp, got_armor(got), cap, got_power(got)))

def grab(g: Grab) -> Taken:
  match g:
    case Grab{spots, hp, max_hp, armor, cap, power}:
      gather(spots, hp, max_hp, armor, cap, power)

type Arm is Data:
  Arm{winner: Bool, connected: Bool, hp: U32, respawn: U32, stun: U32}

def arm(a: Arm) -> Bool:
  match a:
    case Arm{winner, connected, hp, respawn, stun}:
      Bool.and(Bool.not(winner), Bool.and(Bool.and(connected, U32.is_gt(hp, 0)), Bool.and(U32.is_eq(respawn, 0), U32.is_eq(stun, 0))))

type Acted is Data:
  Acted{protection: U32, flash: U32, card: Bool, bomb: Bool, swing: Bool, fire: Bool}

type Ward is Data:
  Ward{protection: U32, flash: U32}

def flashed(+fire: Bool, +lit: Bool, +flash: U32) -> U32:
  Bool.pick(U32, fire, 4, Bool.pick(U32, lit, 5, flash))

def armed_end(a: Acted) -> Ward:
  match a:
    case Acted{protection, flash, +card, bomb, +swing, +fire}:
      Ward{Bool.pick(U32, Bool.or(Bool.or(card, bomb), Bool.or(swing, fire)), 0, protection), flashed(fire, Bool.or(card, swing), flash)}

type Land is Data:
  Land{stun_t: U32, chill_t: U32, chill_unit: U32, fire: Bool, poison: U32, burst: U32, chain: U32, mend: U32, fart: U32, damage: U32, vamp: U32, owner_alive: Bool, fart_wait: U32, stun: U32, chilled: U32}

type Landed is Data:
  Landed{stun: U32, chilled: U32, burning: Bool, burn: U32, bursts: Bool, burst_damage: U32, chain: U32, chain_damage: U32, leech: Bool, leech_amount: U32, fart: Bool, fart_wait: U32, fart_left: U32, fart_reach: F32}

def farts(+fart: U32, +wait: U32) -> Bool:
  Bool.and(U32.is_gt(fart, 0), U32.is_eq(wait, 0))

def landed(l: Land) -> Landed:
  match l:
    case Land{stun_t, chill_t, chill_unit, fire, poison, +burst, chain, mend, +fart, +damage, vamp, owner_alive, +fart_wait, stun, chilled}:
      +cloud = farts(fart, fart_wait)
      Landed{U32.max(stun, U32.min(stun_t, hz())), U32.max(chilled, (chill_t * chill_unit : U32)), Bool.or(fire, U32.is_gt(poison, 0)), (2 * hz() : U32), U32.is_gt(burst, 0), (damage * burst / 100 : U32), chain, (damage / 2 : U32), owner_alive, (damage * U32.min(vamp, 20) / 100 + mend : U32), cloud, Bool.pick(U32, cloud, (2 * hz() / fart : U32), fart_wait), (3 * hz() : U32), 2.0}

type FieldKind is Data:
  Hazard{}
  Turret{}
  Decoy{}
  Hedge{}

type Field is Data:
  Field{owner: U32, x: F32, y: F32, radius: F32, damage: U32, left: U32, wait: U32, kind: FieldKind}

type Sweep is Data:
  Sweep{ticks: U32, fields: List<&2, Field>, feed: List<&2, Frag>}

type Closed is Data:
  Closed{fields: List<&2, Field>, pulses: List<&2, Field>, feed: List<&2, Frag>}

def beat(k: FieldKind) -> U32:
  match k:
    case Turret{}:
      turret_beat()
    case Hazard{}:
      field_beat()
    case Decoy{}:
      field_beat()
    case Hedge{}:
      field_beat()

def pulse_of(f: Field) -> Bool:
  match f:
    case Field{owner, x, y, radius, damage, left, wait, kind}:
      Bool.and(U32.is_eq(sat_sub(wait, 1), 0), U32.is_gt(damage, 0))

def lasts(f: Field) -> Bool:
  match f:
    case Field{owner, x, y, radius, damage, left, wait, kind}:
      U32.is_gt(left, 1)

def age_field(+f: Field) -> Field:
  match f:
    case Field{owner, x, y, radius, damage, left, wait, kind}:
      Field{owner, x, y, radius, damage, sat_sub(left, 1), Bool.pick(U32, pulse_of(f), beat(kind), sat_sub(wait, 1)), kind}

def keep_field(+keep: Bool, f: Field, rest: List<&2, Field>) -> List<&2, Field>:
  match keep:
    case True{}:
      Con{f, rest}
    case False{}:
      rest

def lasting(fields: List<&2, Field>) -> List<&2, Field>:
  match fields:
    case Nil{}:
      Nil{}
    case Con{+f, t}:
      keep_field(lasts(f), age_field(f), lasting(t))

def pulsing(fields: List<&2, Field>) -> List<&2, Field>:
  match fields:
    case Nil{}:
      Nil{}
    case Con{+f, t}:
      keep_field(pulse_of(f), f, pulsing(t))

def kept_frag(+keep: Bool, f: Frag, rest: List<&2, Frag>) -> List<&2, Frag>:
  match keep:
    case True{}:
      Con{f, rest}
    case False{}:
      rest

def fresh_feed(+ticks: U32, feed: List<&2, Frag>) -> List<&2, Frag>:
  match feed:
    case Nil{}:
      Nil{}
    case Con{f, rest}:
      match f:
        case Frag{killer, victim, +tick}:
          kept_frag(U32.is_lt(sat_sub(ticks, tick), feed_life()), Frag{killer, victim, tick}, fresh_feed(ticks, rest))

def close(c: Sweep) -> Closed:
  match c:
    case Sweep{ticks, +fields, feed}:
      Closed{lasting(fields), pulsing(fields), fresh_feed(ticks, feed)}

type Muzzle is Data:
  Muzzle{id: U32, alive: Bool, flash: U32}

type Stake is Data:
  Stake{seat: U32, stake: U32, present: Bool, gold: U32}

type Wrap is Data:
  Wrap{crossroads: Bool, fresh: Bool, winner: Bool, winner_id: U32, moved: Bool, kills_before: U32, kills_after: U32, ended: U32, purse: U32, muzzles: List<&2, Muzzle>, stakes: List<&2, Stake>}

type Cue is Data:
  Shot{id: U32}
  Kill{}
  Card{}
  Won{}

type Take is Data:
  Take{seat: U32, amount: U32, laurel: Bool, present: Bool}

type Settled is Data:
  Settled{takes: List<&2, Take>, purse: U32}

type Wrapped is Data:
  Wrapped{cues: List<&2, Cue>, takes: List<&2, Take>, purse: U32, settle: Bool, home: Bool}

def muzzle_cue(+shoots: Bool, +id: U32, rest: List<&2, Cue>) -> List<&2, Cue>:
  match shoots:
    case True{}:
      Con{Shot{id}, rest}
    case False{}:
      rest

def muzzles(m: List<&2, Muzzle>) -> List<&2, Cue>:
  match m:
    case Nil{}:
      Nil{}
    case Con{h, t}:
      match h:
        case Muzzle{id, alive, flash}:
          muzzle_cue(Bool.and(alive, U32.is_eq(flash, 4)), id, muzzles(t))

def more_takes(t: Take, s: Settled) -> Settled:
  match s:
    case Settled{takes, purse}:
      Settled{Con{t, takes}, purse}

def settle_stakes(stakes: List<&2, Stake>, +winner_id: U32, +purse: U32) -> Settled:
  match stakes:
    case Nil{}:
      Settled{Nil{}, purse}
    case Con{h, t}:
      match h:
        case Stake{+seat, +stake, +present, +gold}:
          +amount = U32.min(stake, gold)
          more_takes(Take{seat, amount, U32.is_eq(seat, winner_id), present}, settle_stakes(t, winner_id, (purse + Bool.pick(U32, present, amount, 0) : U32)))

def rung(+hit: Bool, cues: List<&2, Cue>) -> List<&2, Cue>:
  match hit:
    case True{}:
      List.append(&2, Cue, cues, Con{Kill{}, Nil{}})
    case False{}:
      cues

def sentenced(+won: Bool, +cues: List<&2, Cue>, s: Settled, +crossroads: Bool, +winner: Bool, +ended: U32) -> Wrapped:
  match s:
    case Settled{takes, purse}:
      Wrapped{Bool.pick(List<&2, Cue>, won, List.append(&2, Cue, cues, Con{Card{}, Con{Won{}, Nil{}}}), cues), takes, purse, won, Bool.and(Bool.not(crossroads), Bool.and(winner, U32.is_ge(ended, podium())))}

def idle_settle(+purse: U32) -> Settled:
  Settled{Nil{}, purse}

def wrap_up(+won: Bool, cues: List<&2, Cue>, stakes: List<&2, Stake>, +winner_id: U32, +purse: U32, +crossroads: Bool, +winner: Bool, +ended: U32) -> Wrapped:
  match won:
    case True{}:
      sentenced(True{}, cues, settle_stakes(stakes, winner_id, purse), crossroads, winner, ended)
    case False{}:
      sentenced(False{}, cues, idle_settle(purse), crossroads, winner, ended)

def wrap(w: Wrap) -> Wrapped:
  match w:
    case Wrap{+crossroads, +fresh, +winner, winner_id, +moved, kills_before, kills_after, ended, purse, muzzles_in, stakes}:
      wrap_up(Bool.and(Bool.not(crossroads), Bool.and(fresh, winner)), rung(U32.is_gt(kills_after, kills_before), Bool.pick(List<&2, Cue>, Bool.and(fresh, moved), muzzles(muzzles_in), Nil{})), stakes, winner_id, purse, crossroads, winner, ended)
