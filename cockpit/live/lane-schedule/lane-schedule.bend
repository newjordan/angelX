import Base

# The scheduler's lane decision: which competition lanes get a seat this tick.
# Rust keeps the scheduler state (tick, ages, service counts, checkpoints and
# worker generations), validates it and applies the choice; the leg only ranks
# the lanes it is handed. The native oracle is `select_lanes`/`choose_oldest` in
# cockpit/src/drive/competition/scheduler.rs, and this module reproduces it:
#   - seats >= 2, or fewer than two runnable lanes: every runnable lane, in
#     canonical order;
#   - one seat: a lane whose next age would reach the configured service gap is
#     served first (a due lane preempts the frontier boost), then the boosted
#     frontier lane, then the oldest runnable lane;
#   - "oldest" is the maximum of (age, not-served-last), and a tie goes to the
#     later lane in canonical order, exactly like Rust's `max_by_key`.

# One lane's scheduling state. `index` is the canonical position Rust keeps for
# the lane id, so the leg answers in indices and Rust maps them back to lane ids.
type Lane is Data:
  Lane{index: U32, runnable: Bool, age: U32}

def count(+lanes: List<&2, Lane>, +acc: U32) -> U32:
  match lanes:
    case Nil{}:
      acc
    case Con{+head, +tail}:
      count(tail, (acc + 1 : U32))

# `Some(lane) != last` from the oracle: a lane that did not serve last tick
# outranks one that did when their ages are equal.
def fresh(+index: U32, +last: Maybe<&2, U32>) -> Bool:
  match last:
    case None{}:
      True{}
    case Some{+value}:
      Bool.not(U32.is_eq(index, value))

def same_age(+younger: Bool, +a_fresh: Bool, +b_fresh: Bool) -> Bool:
  match younger:
    case True{}:
      False{}
    case False{}:
      Bool.or(b_fresh, Bool.not(a_fresh))

def later(+older: Bool, +younger: Bool, +a_fresh: Bool, +b_fresh: Bool) -> Bool:
  match older:
    case True{}:
      True{}
    case False{}:
      same_age(younger, a_fresh, b_fresh)

# Whether `b`'s key (age, fresh) is at least `a`'s, so `b` replaces `a`.
def at_least(+a_age: U32, +a_fresh: Bool, +b_age: U32, +b_fresh: Bool) -> Bool:
  later(U32.is_gt(b_age, a_age), U32.is_lt(b_age, a_age), a_fresh, b_fresh)

# The oracle's comparison for one candidate pair, as a flag to pick on.
def better(
  +index: U32,
  +age: U32,
  +best_index: U32,
  +best_age: U32,
  +last: Maybe<&2, U32>
) -> Bool:
  at_least(best_age, fresh(best_index, last), age, fresh(index, last))

# `choose_oldest`: walk the candidates from left to right, keeping the better
# one; a tie keeps the later candidate, which is why `better` is "at least".
def choose(+lanes: List<&2, Lane>, +best: Lane, +last: Maybe<&2, U32>) -> Lane:
  match lanes:
    case Nil{}:
      best
    case Con{+head, +tail}:
      match head:
        case Lane{+index, +runnable, +age}:
          match best:
            case Lane{+best_index, +best_runnable, +best_age}:
              choose(
                tail,
                Bool.pick(
                  Lane,
                  better(index, age, best_index, best_age, last),
                  head,
                  best),
                last)

def keep_by(+runnable: Bool, +due: Bool, +due_now: Bool) -> Bool:
  match runnable:
    case False{}:
      False{}
    case True{}:
      match due:
        case False{}:
          True{}
        case True{}:
          due_now

def wanted(+lane: Lane, +due: Bool, +gap: U32) -> Bool:
  match lane:
    case Lane{+index, +runnable, +age}:
      keep_by(runnable, due, U32.is_ge((age + 1 : U32), gap))

def cons_if(+keep: Bool, +head: Lane, +tail: List<&2, Lane>) -> List<&2, Lane>:
  match keep:
    case True{}:
      Con{head, tail}
    case False{}:
      tail

# The runnable lanes (`due` false), or only the ones whose next age reaches the
# service gap, always in canonical order.
def collect(+lanes: List<&2, Lane>, +due: Bool, +gap: U32) -> List<&2, Lane>:
  match lanes:
    case Nil{}:
      Nil{}
    case Con{+head, +tail}:
      cons_if(wanted(head, due, gap), head, collect(tail, due, gap))

def oldest(+pool: List<&2, Lane>, +last: Maybe<&2, U32>) -> List<&2, Lane>:
  match pool:
    case Nil{}:
      Nil{}
    case Con{+head, +tail}:
      Con{choose(Con{head, tail}, head, last), Nil{}}

def boosted_head(
  +flag: Bool,
  +head: Lane,
  +pool: List<&2, Lane>,
  +last: Maybe<&2, U32>
) -> List<&2, Lane>:
  match flag:
    case True{}:
      Con{head, Nil{}}
    case False{}:
      oldest(pool, last)

def frontier(
  +lanes: List<&2, Lane>,
  +pool: List<&2, Lane>,
  +last: Maybe<&2, U32>
) -> List<&2, Lane>:
  match lanes:
    case Nil{}:
      oldest(pool, last)
    case Con{+head, +tail}:
      match head:
        case Lane{+index, +runnable, +age}:
          boosted_head(runnable, head, pool, last)

def boosted(
  +boost: Bool,
  +lanes: List<&2, Lane>,
  +pool: List<&2, Lane>,
  +last: Maybe<&2, U32>
) -> List<&2, Lane>:
  match boost:
    case False{}:
      oldest(pool, last)
    case True{}:
      frontier(lanes, pool, last)

# A due lane takes the single seat even when the frontier boost is pending.
def due_first(
  +due: List<&2, Lane>,
  +pool: List<&2, Lane>,
  +boost: Bool,
  +lanes: List<&2, Lane>,
  +last: Maybe<&2, U32>
) -> List<&2, Lane>:
  match due:
    case Nil{}:
      boosted(boost, lanes, pool, last)
    case Con{+head, +tail}:
      Con{choose(Con{head, tail}, head, last), Nil{}}

def one_seat(
  +narrow: Bool,
  +pool: List<&2, Lane>,
  +due: List<&2, Lane>,
  +boost: Bool,
  +lanes: List<&2, Lane>,
  +last: Maybe<&2, U32>
) -> List<&2, Lane>:
  match narrow:
    case True{}:
      pool
    case False{}:
      due_first(due, pool, boost, lanes, last)

def wide(
  +wide: Bool,
  +narrow: Bool,
  +pool: List<&2, Lane>,
  +due: List<&2, Lane>,
  +boost: Bool,
  +lanes: List<&2, Lane>,
  +last: Maybe<&2, U32>
) -> List<&2, Lane>:
  match wide:
    case True{}:
      pool
    case False{}:
      one_seat(narrow, pool, due, boost, lanes, last)

# The scheduler question: given every lane's state and the seat count, the
# lanes to serve this tick, in canonical order.
def schedule(
  +lanes: List<&2, Lane>,
  +seats: U32,
  +gap: U32,
  +boost: Bool,
  +last: Maybe<&2, U32>
) -> List<&2, Lane>:
  +pool = collect(lanes, False{}, gap)
  +due = collect(pool, True{}, gap)
  wide(U32.is_ge(seats, 2), U32.is_lt(count(pool, 0), 2), pool, due, boost, lanes, last)
