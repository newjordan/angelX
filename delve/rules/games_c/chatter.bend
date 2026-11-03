# crates/delve/src/rules/chatter.rs — the knights' own noises.

def chatter.QUIET() -> W.W:
  W.of_u32(750)

def chatter.BREATH() -> W.W:
  W.of_u32(90)

def chatter.GRUNT_GAP() -> W.W:
  W.of_u32(120)

def chatter.OOF_GAP() -> W.W:
  W.of_u32(180)

def chatter.HUP_GAP() -> W.W:
  W.of_u32(360)

def chatter.CLIPS() -> List<&2, String>:
  ["cheer", "ooh", "uhoh", "aww", "thanks", "go", "hello", "hehe", "hup", "oof", "hyah", "waah"]

# The sound `knights/<voice>_<clip>_<take>` of knight `id`: the voice follows the
# seat, an unknown clip is the first one; `names()` lists them voice-major.
def chatter.clip(+id: U32, +clip: String, +take: U32) -> String:
  "knights/" ++ gc.nth_s(["a", "b", "c", "d"], U32.mod(gc.ssub(id, 1), 4)) ++ "_" ++ chatter.clip.name(clip) ++ "_" ++ U32.show((U32.mod(take, 2) + 1 : U32))

def chatter.clip.name(+clip: String) -> String:
  gc.ifs(chatter.clip.known(clip, chatter.CLIPS()), _ => clip, _ => "cheer")

def chatter.clip.known(+clip: String, +xs: List<&2, String>) -> Bool:
  match xs:
    case Nil{}:
      False{}
    case Con{+h, +t}:
      Bool.or(String.eq(h, clip), chatter.clip.known(clip, t))

# `cue.split(':').next()`
def chatter.base(+cue: String) -> String:
  chatter.base.go(String.split(cue, ':'))

def chatter.base.go(+xs: List<&2, String>) -> String:
  match xs:
    case Nil{}:
      ""
    case Con{+h, +t}:
      h

def chatter.in(+s: String, +xs: List<&2, String>) -> Bool:
  chatter.clip.known(s, xs)

# fn mood(cue) -> Option<(clip, big)>: the clip, "" for none, and whether it is big.
def chatter.mood.clip(+base: String) -> String:
  gc.ifs(chatter.in(base, ["victory", "boss_fall", "outlaw_fall", "grail", "bounty_paid", "joust_won"]), _ => "cheer",
    _ => gc.ifs(chatter.in(base, ["wipe", "knight_down"]), _ => "aww",
      _ => gc.ifs(chatter.in(base, ["boss_rise", "lair", "outlaw_rise", "mimic"]), _ => "uhoh",
        _ => gc.ifs(chatter.in(base, ["revive", "second_wind"]), _ => "thanks",
          _ => gc.ifs(chatter.in(base, ["run_start", "descend", "the_deep"]), _ => "go",
            _ => gc.ifs(chatter.in(base, ["pastime_won", "goblin_caught", "dare_kept", "flawless", "slay4"]), _ => "hehe",
              _ => gc.ifs(chatter.in(base, ["card_relic", "card_rare", "secret_found", "key_drop", "wish_granted", "achievement", "trophy_new", "box_opened", "level_up"]), _ => "ooh",
                _ => gc.ifs(chatter.in(base, ["low_hp", "wave_last", "collapse_warn", "trap_spikes", "trap_rocks", "trap_fire", "trap_axes", "trap_hammers", "trap_boulder", "trap_saw", "gorge_fall"]), _ => "uhoh",
                  _ => gc.ifs(chatter.in(base, ["home", "built", "rescued", "hired", "brannoc_arrives", "npc"]), _ => "hello",
                    _ => gc.ifs(chatter.in(base, ["pastime_lost", "key_lost", "dare_broken", "joust_lost", "goblin_escaped"]), _ => "aww", _ => ""))))))))))

def chatter.mood.big(+base: String) -> Bool:
  Bool.or(chatter.in(base, ["victory", "boss_fall", "outlaw_fall", "grail", "bounty_paid", "joust_won", "wipe", "knight_down", "boss_rise", "lair", "outlaw_rise", "mimic", "revive", "second_wind", "run_start", "descend", "the_deep"]), False{})

# The first big mood among the cues from `from` on, else the first mood: (clip, big), clip "" for none.
def chatter.moods(+cues: List<&2, J.Val>, +first: String, +firstbig: Bool) -> String & Bool:
  match cues:
    case Nil{}:
      (first, firstbig)
    case Con{+h, +t}:
      chatter.moods.cue(chatter.mood.clip(chatter.base(J.str(h))), chatter.mood.big(chatter.base(J.str(h))), t, first, firstbig)

def chatter.moods.cue(+clip: String, +big: Bool, +t: List<&2, J.Val>, +first: String, +firstbig: Bool) -> String & Bool:
  gc.if(String & Bool, String.is_empty(clip),
    _ => chatter.moods(t, first, firstbig),
    _ => gc.if(String & Bool, big,
      _ => (clip, True{}),
      _ => chatter.moods(t, gc.ifs(String.is_empty(first), _ => clip, _ => first), gc.ifb(String.is_empty(first), _ => big, _ => firstbig))))

# free(last, gap): last.is_none_or(|at| tick >= at + gap)
def chatter.free(+last: J.Val, +tick: W.W, +gap: W.W) -> Bool:
  gc.ifb(J.is_null(last), _ => True{}, _ => W.ge(tick, W.add(W.of_j(last), gap)))

def chatter.body(+hp: U32, +dash: U32, +charge: U32) -> J.Val:
  gc.su(gc.su(gc.su(J.obj(), "hp", hp), "dash", dash), "charge", charge)

# fn chatter(&mut self, from)
def chatter.Run.chatter(+run: J.Val, +from: U32) -> J.Val:
  chatter.Run.chatter.a(run, from, gc.tick(run))

def chatter.Run.chatter.a(+run: J.Val, +from: U32, +tick: W.W) -> J.Val:
  chatter.Run.chatter.b(run, from, tick, chatter.players(J.get(run, "players"), gc.ids(J.get(run, "players")), tick, W.rem_u32(tick, 2), J.get(run, "chatter"), Nil{}, chatter.free(J.get(J.get(run, "chatter"), "grunted"), tick, chatter.GRUNT_GAP())))

# Each knight in id order: (chatter, noises).
def chatter.players(+hs: J.Val, +ids: List<&2, U32>, +tick: W.W, +take: U32, +ch: J.Val, +noises: List<&2, String>, +quiet: Bool) -> J.Val & List<&2, String>:
  match ids:
    case Nil{}:
      (ch, noises)
    case Con{+id, +t}:
      chatter.player(hs, id, t, tick, take, ch, noises, quiet, gc.seat(hs, id), gc.seat(J.get(ch, "bodies"), id))

def chatter.player(+hs: J.Val, +id: U32, +t: List<&2, U32>, +tick: W.W, +take: U32, +ch: J.Val, +noises: List<&2, String>, +quiet: Bool, +hero: J.Val, +was: J.Val) -> J.Val & List<&2, String>:
  chatter.player.go(hs, id, t, tick, take, J.set(ch, "bodies", J.set(J.get(ch, "bodies"), gc.key(id), chatter.body(gc.u(hero, "hp"), gc.u(hero, "dash_ticks"), gc.u(hero, "ult_charge")))), noises, quiet, hero, was)

def chatter.player.go(+hs: J.Val, +id: U32, +t: List<&2, U32>, +tick: W.W, +take: U32, +ch: J.Val, +noises: List<&2, String>, +quiet: Bool, +hero: J.Val, +was: J.Val) -> J.Val & List<&2, String>:
  gc.if(J.Val & List<&2, String>, J.is_null(was),
    _ => chatter.players(hs, t, tick, take, ch, noises, quiet),
    _ => chatter.player.grunt(hs, id, t, tick, take, ch, noises, quiet, gc.u(hero, "hp"), gc.u(hero, "dash_ticks"), gc.u(hero, "ult_charge"), gc.u(was, "hp"), gc.u(was, "dash"), gc.u(was, "charge")))

def chatter.player.grunt(+hs: J.Val, +id: U32, +t: List<&2, U32>, +tick: W.W, +take: U32, +ch: J.Val, +noises: List<&2, String>, +quiet: Bool, +hp: U32, +dash: U32, +charge: U32, +was_hp: U32, +was_dash: U32, +was_charge: U32) -> J.Val & List<&2, String>:
  gc.if(J.Val & List<&2, String>, Bool.and(U32.is_gt(was_hp, 0), U32.is_zero(hp)),
    _ => chatter.players(hs, t, tick, take, ch, List.append(&2, String, noises, [chatter.clip(id, "waah", take)]), quiet),
    _ => gc.if(J.Val & List<&2, String>, Bool.not(quiet),
      _ => chatter.players(hs, t, tick, take, ch, noises, quiet),
      _ => gc.if(J.Val & List<&2, String>, Bool.and(U32.is_lt(hp, was_hp), chatter.free(gc.seat(J.get(ch, "oof"), id), tick, chatter.OOF_GAP())),
        _ => chatter.spoke(hs, id, t, tick, take, J.set(ch, "oof", J.set(J.get(ch, "oof"), gc.key(id), W.to_j(tick))), noises, "oof"),
        _ => gc.if(J.Val & List<&2, String>, Bool.and(U32.is_ge(was_charge, ults.ULT_FULL()), U32.is_zero(charge)),
          _ => chatter.spoke(hs, id, t, tick, take, ch, noises, "hyah"),
          _ => gc.if(J.Val & List<&2, String>, Bool.and(U32.is_gt(dash, was_dash), chatter.free(gc.seat(J.get(ch, "hup"), id), tick, chatter.HUP_GAP())),
            _ => chatter.spoke(hs, id, t, tick, take, J.set(ch, "hup", J.set(J.get(ch, "hup"), gc.key(id), W.to_j(tick))), noises, "hup"),
            _ => chatter.players(hs, t, tick, take, ch, noises, quiet))))))

# noises.push(clip(id, grunt, take)); grunted = Some(tick); quiet = false
def chatter.spoke(+hs: J.Val, +id: U32, +t: List<&2, U32>, +tick: W.W, +take: U32, +ch: J.Val, +noises: List<&2, String>, +grunt: String) -> J.Val & List<&2, String>:
  chatter.players(hs, t, tick, take, J.set(ch, "grunted", W.to_j(tick)), List.append(&2, String, noises, [chatter.clip(id, grunt, take)]), False{})

def chatter.Run.chatter.b(+run: J.Val, +from: U32, +tick: W.W, +r: J.Val & List<&2, String>) -> J.Val:
  (+ch, +noises) = r
  chatter.Run.chatter.d(gc.sounds_extend(J.set(run, "chatter", J.set(ch, "bodies", gc.retain_seats(J.get(ch, "bodies"), J.get(run, "players")))), noises), from, tick)

# bodies.retain(|id, _| players.contains_key(id))
def gc.retain_seats.go(+xs: List<&2, Sigma<&2, &2, String, _ => J.Val>>, +keep: J.Val, +acc: J.Val) -> J.Val:
  match xs:
    case Nil{}:
      acc
    case Con{(+k, +v), +t}:
      gc.retain_seats.go(t, keep, gc.ifj(J.has(keep, k), _ => J.set(acc, k, v), _ => acc))

def gc.retain_seats(+m: J.Val, +keep: J.Val) -> J.Val:
  gc.retain_seats.go(J.int_fields(m), keep, J.obj())

# self.sounds.extend(strings)
def gc.sounds_extend(+run: J.Val, +xs: List<&2, String>) -> J.Val:
  match xs:
    case Nil{}:
      run
    case Con{+h, +t}:
      gc.sounds_extend(gc.sound(run, h), t)

def chatter.Run.chatter.d(+run: J.Val, +from: U32, +tick: W.W) -> J.Val:
  chatter.Run.chatter.e(run, tick, chatter.moods(J.items(gc.drop_front(J.get(run, "cues"), gc.umin(from, gc.len(J.items(J.get(run, "cues")))))), "", False{}))

def chatter.Run.chatter.e(+run: J.Val, +tick: W.W, +m: String & Bool) -> J.Val:
  (+name, +big) = m
  gc.ifj(String.is_empty(name),
    _ => run,
    _ => chatter.Run.chatter.f(run, tick, name, gc.ifw(big, chatter.BREATH(), chatter.QUIET())))

def gc.ifw(+c: Bool, +a: W.W, +b: W.W) -> W.W:
  Bool.pick(W.W, c, a, b)

def chatter.Run.chatter.f(+run: J.Val, +tick: W.W, +name: String, +rest: W.W) -> J.Val:
  gc.ifj(Bool.and(Bool.not(J.is_null(J.get(J.get(run, "chatter"), "said"))), W.lt(tick, W.add(W.of_j(J.get(J.get(run, "chatter"), "said")), rest))),
    _ => run,
    _ => chatter.Run.chatter.g(run, tick, name, chatter.standing(run)))

# the ids standing (hp > 0), else every id
def chatter.standing(+run: J.Val) -> List<&2, U32>:
  chatter.standing.pick(chatter.standing.go(J.get(run, "players"), gc.ids(J.get(run, "players"))), gc.ids(J.get(run, "players")))

def chatter.standing.pick(+up: List<&2, U32>, +all: List<&2, U32>) -> List<&2, U32>:
  match up:
    case Nil{}:
      all
    case Con{+h, +t}:
      h <> t

def chatter.standing.go(+hs: J.Val, +ids: List<&2, U32>) -> List<&2, U32>:
  match ids:
    case Nil{}:
      Nil{}
    case Con{+id, +t}:
      gc.ifx_us(U32.is_gt(gc.u(gc.seat(hs, id), "hp"), 0), id, chatter.standing.go(hs, t))

def chatter.Run.chatter.g(+run: J.Val, +tick: W.W, +name: String, +speakers: List<&2, U32>) -> J.Val:
  chatter.Run.chatter.h(run, tick, name, gc.len_u(speakers), gc.u(J.get(run, "chatter"), "turn"), speakers)

def chatter.Run.chatter.h(+run: J.Val, +tick: W.W, +name: String, +n: U32, +turn: U32, +speakers: List<&2, U32>) -> J.Val:
  gc.ifj(U32.is_zero(n),
    _ => run,
    _ => chatter.Run.chatter.say(run, tick, name, gc.nth_u(speakers, U32.mod(turn, n)), U32.mod(U32.div(turn, n), 2), turn))

def chatter.Run.chatter.say(+run: J.Val, +tick: W.W, +name: String, +id: U32, +take: U32, +turn: U32) -> J.Val:
  gc.sound(J.set(run, "chatter", gc.su(J.set(J.get(run, "chatter"), "said", W.to_j(tick)), "turn", (turn + 1 : U32))), chatter.clip(id, name, take))
