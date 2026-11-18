import Base

type Obs is Data:
  Obs{identity_conflict: Bool, duplicate: Bool, full_snapshot: Bool, has_prior: Bool, prior_hi: U32, prior_lo: U32, has_incoming: Bool, incoming_hi: U32, incoming_lo: U32, refresh_pending: Bool, has_board: Bool}

type Verdict is Data:
  Conflict{}
  Duplicate{}
  Stale{}
  Gap{}
  Replace{}

def and(+a: Bool, +b: Bool) -> Bool:
  match a:
    case True{}:
      b
    case False{}:
      False{}

def or(+a: Bool, +b: Bool) -> Bool:
  match a:
    case True{}:
      True{}
    case False{}:
      b

def negate(+value: Bool) -> Bool:
  match value:
    case True{}:
      False{}
    case False{}:
      True{}

def first(+hi_wins: Bool, +hi_same: Bool, +lo_wins: Bool) -> Bool:
  match hi_wins:
    case True{}:
      True{}
    case False{}:
      match hi_same:
        case True{}:
          lo_wins
        case False{}:
          False{}

def lex_lt(+a_hi: U32, +a_lo: U32, +b_hi: U32, +b_lo: U32) -> Bool:
  first(U32.is_lt(a_hi, b_hi), U32.is_eq(a_hi, b_hi), U32.is_lt(a_lo, b_lo))

def lex_le(+a_hi: U32, +a_lo: U32, +b_hi: U32, +b_lo: U32) -> Bool:
  first(U32.is_lt(a_hi, b_hi), U32.is_eq(a_hi, b_hi), U32.is_le(a_lo, b_lo))

def lex_gt(+a_hi: U32, +a_lo: U32, +b_hi: U32, +b_lo: U32) -> Bool:
  first(U32.is_gt(a_hi, b_hi), U32.is_eq(a_hi, b_hi), U32.is_gt(a_lo, b_lo))

def stale(
  +full_snapshot: Bool,
  +has_prior: Bool,
  +prior_hi: U32,
  +prior_lo: U32,
  +has_incoming: Bool,
  +incoming_hi: U32,
  +incoming_lo: U32
) -> Bool:
  and(
    and(has_prior, has_incoming),
    or(
      and(
        full_snapshot,
        lex_lt(incoming_hi, incoming_lo, prior_hi, prior_lo)
      ),
      and(
        negate(full_snapshot),
        lex_le(incoming_hi, incoming_lo, prior_hi, prior_lo)
      )
    )
  )

def at_max(+hi: U32, +lo: U32) -> Bool:
  and(U32.is_eq(hi, 4294967295), U32.is_eq(lo, 4294967295))

def stepped(+lo: U32, +wraps: Bool) -> U32:
  match wraps:
    case True{}:
      0
    case False{}:
      (lo + 1 : U32)

def next_lo(+lo: U32) -> U32:
  stepped(lo, U32.is_eq(lo, 4294967295))

def carried(+hi: U32, +carry: Bool) -> U32:
  match carry:
    case True{}:
      (hi + 1 : U32)
    case False{}:
      hi

def next_hi(+hi: U32, +lo: U32) -> U32:
  carried(hi, U32.is_eq(lo, 4294967295))

def skipped(
  +has_prior: Bool,
  +prior_hi: U32,
  +prior_lo: U32,
  +has_incoming: Bool,
  +incoming_hi: U32,
  +incoming_lo: U32
) -> Bool:
  and(
    and(has_prior, has_incoming),
    and(
      negate(at_max(prior_hi, prior_lo)),
      lex_gt(
        incoming_hi,
        incoming_lo,
        next_hi(prior_hi, prior_lo),
        next_lo(prior_lo)
      )
    )
  )

def gap(
  +full_snapshot: Bool,
  +has_prior: Bool,
  +prior_hi: U32,
  +prior_lo: U32,
  +has_incoming: Bool,
  +incoming_hi: U32,
  +incoming_lo: U32,
  +refresh_pending: Bool,
  +has_board: Bool
) -> Bool:
  and(
    negate(full_snapshot),
    or(
      refresh_pending,
      or(
        skipped(has_prior, prior_hi, prior_lo, has_incoming, incoming_hi, incoming_lo),
        negate(has_board)
      )
    )
  )

def order(+duplicate: Bool, +stale_hit: Bool, +gap_hit: Bool) -> Verdict:
  match duplicate:
    case True{}:
      Duplicate{}
    case False{}:
      match stale_hit:
        case True{}:
          Stale{}
        case False{}:
          match gap_hit:
            case True{}:
              Gap{}
            case False{}:
              Replace{}

def decide(o: Obs) -> Verdict:
  match o:
    case Obs{+identity_conflict, +duplicate, +full_snapshot, +has_prior, +prior_hi, +prior_lo, +has_incoming, +incoming_hi, +incoming_lo, +refresh_pending, +has_board}:
      match identity_conflict:
        case True{}:
          Conflict{}
        case False{}:
          order(
            duplicate,
            stale(
              full_snapshot,
              has_prior,
              prior_hi,
              prior_lo,
              has_incoming,
              incoming_hi,
              incoming_lo
            ),
            gap(
              full_snapshot,
              has_prior,
              prior_hi,
              prior_lo,
              has_incoming,
              incoming_hi,
              incoming_lo,
              refresh_pending,
              has_board
            )
          )
