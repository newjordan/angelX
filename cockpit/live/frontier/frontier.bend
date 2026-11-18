import Base

type Row is Data:
  Row{index: U32, score: U32, rank_hi: U32, rank_lo: U32, id: U32, personal: Bool, source: Bool}

type Seat is Data:
  Seat{index: U32, score: U32, rank_hi: U32, rank_lo: U32, id: U32}
  Empty{}

type Choice is Data:
  Choice{global: Seat, personal: Seat, source: Seat}

def below(
  +hi_less: Bool,
  +hi_same: Bool,
  +lo_less: Bool,
  +lo_same: Bool,
  +id_less: Bool
) -> Bool:
  match hi_less:
    case True{}:
      True{}
    case False{}:
      match hi_same:
        case False{}:
          False{}
        case True{}:
          match lo_less:
            case True{}:
              True{}
            case False{}:
              match lo_same:
                case True{}:
                  id_less
                case False{}:
                  False{}

def rank_less(
  +a_hi: U32,
  +a_lo: U32,
  +a_id: U32,
  +b_hi: U32,
  +b_lo: U32,
  +b_id: U32
) -> Bool:
  below(
    U32.is_lt(a_hi, b_hi),
    U32.is_eq(a_hi, b_hi),
    U32.is_lt(a_lo, b_lo),
    U32.is_eq(a_lo, b_lo),
    U32.is_lt(a_id, b_id)
  )

def better_score(+higher: Bool, +same: Bool, +tie: Bool) -> Bool:
  match higher:
    case True{}:
      True{}
    case False{}:
      match same:
        case True{}:
          tie
        case False{}:
          False{}

def beats(
  +candidate_score: U32,
  +candidate_hi: U32,
  +candidate_lo: U32,
  +candidate_id: U32,
  +winner_score: U32,
  +winner_hi: U32,
  +winner_lo: U32,
  +winner_id: U32
) -> Bool:
  better_score(
    U32.is_gt(candidate_score, winner_score),
    U32.is_eq(candidate_score, winner_score),
    rank_less(candidate_hi, candidate_lo, candidate_id, winner_hi, winner_lo, winner_id)
  )

def scope(
  +use_personal: Bool,
  +use_source: Bool,
  +personal: Bool,
  +source: Bool
) -> Bool:
  match use_personal:
    case True{}:
      personal
    case False{}:
      match use_source:
        case True{}:
          source
        case False{}:
          True{}

def swap(
  +replace: Bool,
  +candidate_index: U32,
  +candidate_score: U32,
  +candidate_hi: U32,
  +candidate_lo: U32,
  +candidate_id: U32,
  +winner_index: U32,
  +winner_score: U32,
  +winner_hi: U32,
  +winner_lo: U32,
  +winner_id: U32
) -> Seat:
  match replace:
    case True{}:
      Seat{candidate_index, candidate_score, candidate_hi, candidate_lo, candidate_id}
    case False{}:
      Seat{winner_index, winner_score, winner_hi, winner_lo, winner_id}

def consider(
  +wanted: Bool,
  +candidate_index: U32,
  +candidate_score: U32,
  +candidate_hi: U32,
  +candidate_lo: U32,
  +candidate_id: U32,
  +seat: Seat
) -> Seat:
  match wanted:
    case False{}:
      seat
    case True{}:
      match seat:
        case Empty{}:
          Seat{candidate_index, candidate_score, candidate_hi, candidate_lo, candidate_id}
        case Seat{index, score, rank_hi, rank_lo, id}:
          swap(
            beats(candidate_score, candidate_hi, candidate_lo, candidate_id, score, rank_hi, rank_lo, id),
            candidate_index,
            candidate_score,
            candidate_hi,
            candidate_lo,
            candidate_id,
            index,
            score,
            rank_hi,
            rank_lo,
            id
          )

def seat_after(
  +row: Row,
  +use_personal: Bool,
  +use_source: Bool,
  +seat: Seat
) -> Seat:
  match row:
    case Row{index, score, rank_hi, rank_lo, id, personal, source}:
      consider(
        scope(use_personal, use_source, personal, source),
        index,
        score,
        rank_hi,
        rank_lo,
        id,
        seat
      )

def best(
  +rows: List<&2, Row>,
  +use_personal: Bool,
  +use_source: Bool,
  +seat: Seat
) -> Seat:
  match rows:
    case Nil{}:
      seat
    case Con{+head, tail}:
      best(tail, use_personal, use_source, seat_after(head, use_personal, use_source, seat))

def choose(+rows: List<&2, Row>) -> Choice:
  Choice{
    best(rows, False{}, False{}, Empty{}),
    best(rows, True{}, False{}, Empty{}),
    best(rows, False{}, True{}, Empty{})
  }
