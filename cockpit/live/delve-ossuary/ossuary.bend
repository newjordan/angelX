import Base

# The Bone Dominoes of the Delve ossuary nook (crates/delve/src/rules/nooks/
# ossuary.rs), one tick at a time. Rust keeps the Run, the knights' bodies, the
# dice, the sound mixer and the save: it hands this leg the nook's own record
# and one row per playable knight, and applies the effects that come back in
# order. The leg owns the whole state machine: standing, running, stalled and
# done, the fall timer, the three gaps, who scored, the monsters the line
# raises (with their cap) and the coffer's coin and crown.

# The nook's own working state, the part of Rust's `Nook` this rule reads and
# writes. `score` is ascending by knight id, like Rust's BTreeMap.
type Tally is Data:
  Mark{id: U32, n: U32}

type State is Data:
  Line{running: Bool, done: Bool, fallen: U32, next: U32, stalled: U32, raised: U32, score: List<&2, Tally>}

# One playable knight (standing, not stone, not in the privy), in id order.
# Rust did the geometry: `at_start` is within reach of the first bone, `at_bone`
# within reach of the bone the line is about to topple (bone `state.fallen`).
type Knight is Data:
  Knight{id: U32, pressed: Bool, at_start: Bool, at_bone: Bool}

# Sounds, banners and monsters are enum indices; Rust owns the names and words.
type Cue is Data:
  Clatter{}
  Fall{}
  Toll{}
  Chest{}

type Said is Data:
  Clack{}
  Timber{}
  Stops{}
  Opens{gold: U32}

type Beast is Data:
  Skeleton{}

# Effects come back in the order the native tick would have performed them.
type Effect is Data:
  Sound{cue: Cue}
  Say{who: U32, said: Said}
  Raise{beast: Beast, x: F32, y: F32}
  Grant{id: U32, gold: U32}
  Crown{id: U32}

type Step is Data:
  Out{state: State, effects: List<&2, Effect>}

type Raised is Data:
  Rose{raised: U32, effects: List<&2, Effect>}

# --- constants: the numbers of ossuary.rs, fixed ------------------------------
# Ticks between one bone and the next; how long a stalled line takes to stand
# again (two seconds at the Delve's 30 Hz); the coffer's coin; the monsters one
# nook may raise in all; the bones in the line.
def fall_ticks() -> U32:
  4

def stand_again() -> U32:
  60

def coffer_gold() -> U32:
  8

def raise_cap() -> U32:
  4

def bones() -> U32:
  42

# --- the line -------------------------------------------------------------------
# The three places in the line where a knight must stand in for a bone.
def is_gap(+i: U32) -> Bool:
  Bool.or(U32.is_eq(i, 12), Bool.or(U32.is_eq(i, 21), U32.is_eq(i, 33)))

# Bone `i`'s tile: 18 along the top (row 3), 6 down the east side (column 20),
# 18 back along the bottom (row 10). Bend has no `if`, so each fork is a helper
# that matches a Bool, and a def must come after what it calls.
def bone_col_east(+east: Bool, +i: U32) -> U32:
  match east:
    case True{}:
      20
    case False{}:
      (44 - i : U32)

def bone_col_top(+top: Bool, +i: U32) -> U32:
  match top:
    case True{}:
      (3 + i : U32)
    case False{}:
      bone_col_east(U32.is_lt(i, 24), i)

def bone_col(+i: U32) -> U32:
  bone_col_top(U32.is_lt(i, 18), i)

def bone_row_east(+east: Bool, +i: U32) -> U32:
  match east:
    case True{}:
      (i - 14 : U32)
    case False{}:
      10

def bone_row_top(+top: Bool, +i: U32) -> U32:
  match top:
    case True{}:
      3
    case False{}:
      bone_row_east(U32.is_lt(i, 24), i)

def bone_row(+i: U32) -> U32:
  bone_row_top(U32.is_lt(i, 18), i)

# The middle of a tile, in tile units (what Rust multiplies by TILE_UNITS).
def mid(+tile: U32) -> F32:
  (U32.to_f32(tile) + 0.5 : F32)

# --- the score ------------------------------------------------------------------
# One more fall for knight `id`, keeping the list ascending by id. `later` is
# the rest of the list already bumped: strict evaluation computes it even when
# it is not needed, which at four knights costs nothing.
def bumped(+same: Bool, +before: Bool, +other: U32, +n: U32, +id: U32, +tail: List<&2, Tally>, +later: List<&2, Tally>) -> List<&2, Tally>:
  match same:
    case True{}:
      Con{Mark{other, (n + 1 : U32)}, tail}
    case False{}:
      match before:
        case True{}:
          Con{Mark{other, n}, later}
        case False{}:
          Con{Mark{id, 1}, Con{Mark{other, n}, tail}}

def bump(+score: List<&2, Tally>, +id: U32) -> List<&2, Tally>:
  match score:
    case Nil{}:
      Con{Mark{id, 1}, Nil{}}
    case Con{+head, +tail}:
      match head:
        case Mark{+other, +n}:
          bumped(U32.is_eq(other, id), U32.is_lt(other, id), other, n, id, tail, bump(tail, id))

# Walk the tally keeping the first knight with the highest count: a strict `>`
# leaves the lower id in front on a tie, like Rust's max_by_key over
# (count, Reverse(id)).
def lead(+score: List<&2, Tally>, +id: U32, +top: U32) -> U32:
  match score:
    case Nil{}:
      id
    case Con{head, tail}:
      match head:
        case Mark{+other, +n}:
          +bigger = U32.is_gt(n, top)
          lead(tail, Bool.pick(U32, bigger, other, id), Bool.pick(U32, bigger, n, top))

def best(+score: List<&2, Tally>) -> Maybe<&2, U32>:
  match score:
    case Nil{}:
      None{}
    case Con{head, tail}:
      match head:
        case Mark{+id, +n}:
          Some{lead(tail, id, n)}

def crowned(+who: Maybe<&2, U32>) -> List<&2, Effect>:
  match who:
    case None{}:
      Nil{}
    case Some{id}:
      Con{Crown{id}, Nil{}}

# The knight who did most is crowned; nobody when nobody scored.
def crown(+score: List<&2, Tally>) -> List<&2, Effect>:
  crowned(best(score))

# --- the knights ------------------------------------------------------------------
def pick_who(+ready: Bool, +id: U32, +later: Maybe<&2, U32>) -> Maybe<&2, U32>:
  match ready:
    case True{}:
      Some{id}
    case False{}:
      later

# The first knight (lowest id) who pressed the key at the first bone.
def starter(+knights: List<&2, Knight>) -> Maybe<&2, U32>:
  match knights:
    case Nil{}:
      None{}
    case Con{head, tail}:
      match head:
        case Knight{+id, +pressed, +at_start, +at_bone}:
          pick_who(Bool.and(pressed, at_start), id, starter(tail))

# The first knight standing in the gap the line has reached.
def standing(+knights: List<&2, Knight>) -> Maybe<&2, U32>:
  match knights:
    case Nil{}:
      None{}
    case Con{head, tail}:
      match head:
        case Knight{+id, +pressed, +at_start, +at_bone}:
          match at_bone:
            case True{}:
              Some{id}
            case False{}:
              standing(tail)

# Coin for every playable knight when the coffer rings.
def grants(+knights: List<&2, Knight>) -> List<&2, Effect>:
  match knights:
    case Nil{}:
      Nil{}
    case Con{head, tail}:
      match head:
        case Knight{+id, +pressed, +at_start, +at_bone}:
          Con{Grant{id, coffer_gold()}, grants(tail)}

# --- the monsters -----------------------------------------------------------------
def raise_if(+room: Bool, +raised: U32, +x: F32, +y: F32) -> Raised:
  match room:
    case True{}:
      Rose{(raised + 1 : U32), Con{Raise{Skeleton{}, x, y}, Nil{}}}
    case False{}:
      Rose{raised, Nil{}}

# A monster rises at tile point (x, y) while the nook has any left to raise.
def raise_one(+raised: U32, +x: F32, +y: F32) -> Raised:
  raise_if(U32.is_lt(raised, raise_cap()), raised, x, y)

# --- the tick, from the leaves up ---------------------------------------------------
# The line is idle and waiting for a knight to press the key at the first bone.
def began(+who: Maybe<&2, U32>, +done: Bool, +fallen: U32, +next: U32, +stalled: U32, +raised: U32, +score: List<&2, Tally>) -> Step:
  match who:
    case None{}:
      Out{Line{False{}, done, fallen, next, stalled, raised, score}, Nil{}}
    case Some{id}:
      Out{Line{True{}, done, 0, fall_ticks(), stalled, raised, score}, Con{Sound{Clatter{}}, Con{Say{id, Clack{}}, Nil{}}}}

# The last bone rings the coffer: coin for everyone, the crown, the toll.
def finished(+complete: Bool, +said: List<&2, Effect>, +done: Bool, +fallen: U32, +stalled: U32, +raised: U32, +score: List<&2, Tally>, +knights: List<&2, Knight>) -> Step:
  match complete:
    case False{}:
      Out{Line{True{}, done, fallen, fall_ticks(), stalled, raised, score}, said}
    case True{}:
      Out{Line{False{}, True{}, fallen, fall_ticks(), stalled, raised, score}, List.append(&2, Effect, said, List.append(&2, Effect, grants(knights), List.append(&2, Effect, crown(score), Con{Sound{Toll{}}, Con{Sound{Chest{}}, Con{Say{0, Opens{coffer_gold()}}, Nil{}}}})))}

# One bone down.
def advanced(+said: List<&2, Effect>, +done: Bool, +fallen: U32, +stalled: U32, +raised: U32, +score: List<&2, Tally>, +knights: List<&2, Knight>) -> Step:
  finished(U32.is_eq((fallen + 1 : U32), bones()), said, done, (fallen + 1 : U32), stalled, raised, score, knights)

# The line stops: the bones that would not fall get up as skeletons, one where
# the gap is and one in the middle of the room.
def stopped_again(+second: Raised, +made: List<&2, Effect>, +done: Bool, +fallen: U32, +score: List<&2, Tally>) -> Step:
  match second:
    case Rose{+raised, +more}:
      Out{Line{False{}, done, fallen, fall_ticks(), stand_again(), raised, score}, Con{Sound{Clatter{}}, Con{Say{0, Stops{}}, List.append(&2, Effect, made, more)}}}

def stopped(+first: Raised, +done: Bool, +fallen: U32, +stalled: U32, +score: List<&2, Tally>) -> Step:
  match first:
    case Rose{+raised, +made}:
      stopped_again(raise_one(raised, (12.0 : F32), (9.0 : F32)), made, done, fallen, score)

# The gap is due: a knight standing in it falls in the bone's place, or the line stops.
def filled(+who: Maybe<&2, U32>, +done: Bool, +fallen: U32, +stalled: U32, +raised: U32, +score: List<&2, Tally>) -> Step:
  match who:
    case Some{id}:
      advanced(Con{Sound{Fall{}}, Con{Say{id, Timber{}}, Nil{}}}, done, fallen, stalled, raised, bump(score, id), Nil{})
    case None{}:
      stopped(raise_one(raised, mid(bone_col(fallen)), mid(bone_row(fallen))), done, fallen, stalled, score)

# The bone is due. A gap needs a knight standing in it; any other bone falls,
# clattering on every third.
def toppled(+gap: Bool, +done: Bool, +fallen: U32, +stalled: U32, +raised: U32, +score: List<&2, Tally>, +knights: List<&2, Knight>) -> Step:
  match gap:
    case True{}:
      filled(standing(knights), done, fallen, stalled, raised, score)
    case False{}:
      advanced(Bool.pick(List<&2, Effect>, U32.is_eq((fallen % 3 : U32), 0), Con{Sound{Clatter{}}, Nil{}}, Nil{}), done, fallen, stalled, raised, score, knights)

def waiting_on(+more: Bool, +next: U32, +done: Bool, +fallen: U32, +stalled: U32, +raised: U32, +score: List<&2, Tally>, +knights: List<&2, Knight>) -> Step:
  match more:
    case True{}:
      Out{Line{True{}, done, fallen, next, stalled, raised, score}, Nil{}}
    case False{}:
      toppled(is_gap(fallen), done, fallen, stalled, raised, score, knights)

# The line is running: count down to the next bone (never below zero).
def counting(+next: U32, +done: Bool, +fallen: U32, +stalled: U32, +raised: U32, +score: List<&2, Tally>, +knights: List<&2, Knight>) -> Step:
  waiting_on(U32.is_gt(next, 0), next, done, fallen, stalled, raised, score, knights)

def lined(+running: Bool, +done: Bool, +fallen: U32, +next: U32, +stalled: U32, +raised: U32, +score: List<&2, Tally>, +knights: List<&2, Knight>) -> Step:
  match running:
    case True{}:
      counting(Bool.pick(U32, U32.is_gt(next, 0), (next - 1 : U32), 0), done, fallen, stalled, raised, score, knights)
    case False{}:
      began(starter(knights), done, fallen, next, stalled, raised, score)

# A stalled line only counts down, ignoring the knights; the tick it reaches
# zero every bone stands again.
def stall(+waiting: Bool, +running: Bool, +done: Bool, +fallen: U32, +next: U32, +stalled: U32, +raised: U32, +score: List<&2, Tally>, +knights: List<&2, Knight>) -> Step:
  match waiting:
    case True{}:
      +left = (stalled - 1 : U32)
      Out{Line{running, done, Bool.pick(U32, U32.is_eq(left, 0), 0, fallen), next, left, raised, score}, Nil{}}
    case False{}:
      lined(running, done, fallen, next, stalled, raised, score, knights)

# The tick: the nook's state and the playable knights in, the next state and
# the effects, in the order the native tick performs them, out.
def tick(+state: State, +knights: List<&2, Knight>) -> Step:
  match state:
    case Line{+running, +done, +fallen, +next, +stalled, +raised, +score}:
      stall(U32.is_gt(stalled, 0), running, done, fallen, next, stalled, raised, score, knights)
