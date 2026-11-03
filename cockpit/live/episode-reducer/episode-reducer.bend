import Base

# The deep-cut episode state machine. Rust keeps the episode ids, the canonical
# bytes and hashes, the stored bodies, the durability authority and every error
# string; this leg owns which event the current state accepts, which phase
# follows that append, and whether a permit is granted.

# The three event bodies the state machine distinguishes.
type EventKind is Data:
  KStart{}
  KEvidence{}
  KTerminal{}

# The reduced episode state. `seq` is the next sequence as two U32 halves so the
# increment and the u64 exhaustion test are exact for the whole range.
type St is Data:
  St{
    has_episode: Bool,
    has_terminal: Bool,
    seq_hi: U32,
    seq_lo: U32,
    journal_hi: U32,
    journal_lo: U32
  }

# One candidate event, with every comparison Rust already made. The `*_ok` flags
# are the five append-only equalities of `validate_next`: schema, canonical
# digest, next sequence, previous head and the episode id.
type Ev is Data:
  Ev{
    kind: EventKind,
    schema_ok: Bool,
    sha_ok: Bool,
    sequence_ok: Bool,
    previous_ok: Bool,
    episode_ok: Bool,
    start_episode_equal: Bool,
    start_valid: Bool,
    link_valid: Bool,
    link_post_terminal: Bool,
    terminal_valid: Bool,
    end_hi: U32,
    end_lo: U32
  }

# A durable start receipt and the start it claims to complete.
type Rc is Data:
  Rc{
    episode_equal: Bool,
    seq_hi: U32,
    seq_lo: U32,
    sha_equal: Bool,
    start_valid: Bool,
    completed_hi: U32,
    completed_lo: U32,
    started_hi: U32,
    started_lo: U32
  }

# Every refusal the native reducer can return. Rust maps each back to the exact
# string, because the message text stays there.
type Fault is Data:
  NotAppendOnly{}
  StartFirst{}
  DuplicateStart{}
  TerminalEffects{}
  RangeBackward{}
  StartInvalid{}
  LinkInvalid{}
  TerminalInvalid{}
  SequenceExhausted{}
  EpisodeClosed{}
  NeedsStart{}
  NotOfficial{}
  ReceiptMismatch{}

# Which phase the accepted event moves the episode to. `PNone` leaves the
# reduced state alone.
type Phase is Data:
  PNone{}
  PStarted{}
  PClosed{}
  PCompleted{}

type Decision is Data:
  Allow{phase: Phase, next_hi: U32, next_lo: U32, post_terminal: Bool}
  Deny{fault: Fault}

# The five append-only equalities, in the native conjunction.
def chain(
  +schema_ok: Bool,
  +sha_ok: Bool,
  +sequence_ok: Bool,
  +previous_ok: Bool,
  +episode_ok: Bool
) -> Bool:
  Bool.and(schema_ok, Bool.and(sha_ok, Bool.and(sequence_ok, Bool.and(previous_ok, episode_ok))))

# The high half decides a u64 comparison unless the halves are equal.
def ge_halves(+gt: Bool, +lt: Bool, +lo_ge: Bool) -> Bool:
  match gt:
    case True{}:
      True{}
    case False{}:
      match lt:
        case True{}:
          False{}
        case False{}:
          lo_ge

# a >= b over two U32 halves, which is how one u64 comparison is decided.
def ge64(+a_hi: U32, +a_lo: U32, +b_hi: U32, +b_lo: U32) -> Bool:
  ge_halves(U32.is_gt(a_hi, b_hi), U32.is_lt(a_hi, b_hi), U32.is_ge(a_lo, b_lo))

# The largest u32, as the leg reads it: all thirty-two bits set.
def max_u32() -> U32:
  4294967295

# u64::MAX is both halves at their maximum, which `checked_add(1)` refuses.
def exhausted(+hi: U32, +lo: U32) -> Bool:
  Bool.and(U32.is_eq(hi, max_u32()), U32.is_eq(lo, max_u32()))

# The high half only moves when the low half carries out of the u32 range.
def carry(+lo: U32) -> U32:
  Bool.to_u32(U32.is_eq(lo, max_u32()))

def accepted(+phase: Phase) -> Decision:
  Allow{phase, 0, 0, False{}}

def start_arm(+valid: Bool) -> Decision:
  match valid:
    case True{}:
      accepted(PStarted{})
    case False{}:
      Deny{StartInvalid{}}

def link_arm(+valid: Bool) -> Decision:
  match valid:
    case True{}:
      accepted(PNone{})
    case False{}:
      Deny{LinkInvalid{}}

# A closed episode keeps only the official evidence kinds; anything else is an
# effect it can no longer accept.
def closed_evidence(+allowed: Bool, +valid: Bool) -> Decision:
  match allowed:
    case True{}:
      link_arm(valid)
    case False{}:
      Deny{TerminalEffects{}}

def range_arm(+in_range: Bool) -> Decision:
  match in_range:
    case True{}:
      accepted(PClosed{})
    case False{}:
      Deny{RangeBackward{}}

# The terminal body must validate and may not walk the action-journal range back
# past the durable start's journal sequence.
def terminal_arm(+valid: Bool, +end_hi: U32, +end_lo: U32, +journal_hi: U32, +journal_lo: U32) -> Decision:
  match valid:
    case False{}:
      Deny{TerminalInvalid{}}
    case True{}:
      range_arm(ge64(end_hi, end_lo, journal_hi, journal_lo))

# A first event must be the start of this very episode.
def start_gate(+same_episode: Bool, +valid: Bool) -> Decision:
  match same_episode:
    case True{}:
      start_arm(valid)
    case False{}:
      Deny{StartFirst{}}

# `validate_next`'s state/body arms once the append-only chain has matched.
def event_arm(
  +has_episode: Bool,
  +kind: EventKind,
  +has_terminal: Bool,
  +start_episode_equal: Bool,
  +start_valid: Bool,
  +link_valid: Bool,
  +link_post_terminal: Bool,
  +terminal_valid: Bool,
  +end_hi: U32,
  +end_lo: U32,
  +journal_hi: U32,
  +journal_lo: U32
) -> Decision:
  match has_episode:
    case False{}:
      match kind:
        case KStart{}:
          start_gate(start_episode_equal, start_valid)
        case KEvidence{}:
          Deny{StartFirst{}}
        case KTerminal{}:
          Deny{StartFirst{}}
    case True{}:
      match kind:
        case KStart{}:
          Deny{DuplicateStart{}}
        case KEvidence{}:
          match has_terminal:
            case True{}:
              closed_evidence(link_post_terminal, link_valid)
            case False{}:
              link_arm(link_valid)
        case KTerminal{}:
          match has_terminal:
            case True{}:
              Deny{TerminalEffects{}}
            case False{}:
              terminal_arm(terminal_valid, end_hi, end_lo, journal_hi, journal_lo)

# The append-only chain first, then the state/body arm in the native order.
def chained(
  +matched: Bool,
  +state: St,
  +kind: EventKind,
  +start_episode_equal: Bool,
  +start_valid: Bool,
  +link_valid: Bool,
  +link_post_terminal: Bool,
  +terminal_valid: Bool,
  +end_hi: U32,
  +end_lo: U32
) -> Decision:
  match matched:
    case False{}:
      Deny{NotAppendOnly{}}
    case True{}:
      match state:
        case St{has_episode, has_terminal, _, _, journal_hi, journal_lo}:
          event_arm(
            has_episode,
            kind,
            has_terminal,
            start_episode_equal,
            start_valid,
            link_valid,
            link_post_terminal,
            terminal_valid,
            end_hi,
            end_lo,
            journal_hi,
            journal_lo
          )

def validate(state: St, event: Ev) -> Decision:
  match event:
    case Ev{
      kind,
      schema_ok,
      sha_ok,
      sequence_ok,
      previous_ok,
      episode_ok,
      start_episode_equal,
      start_valid,
      link_valid,
      link_post_terminal,
      terminal_valid,
      end_hi,
      end_lo
    }:
      chained(
        chain(schema_ok, sha_ok, sequence_ok, previous_ok, episode_ok),
        state,
        kind,
        start_episode_equal,
        start_valid,
        link_valid,
        link_post_terminal,
        terminal_valid,
        end_hi,
        end_lo
      )

# The checked sequence increment a durable append stores; u64::MAX is refused.
def store(+full: Bool, +phase: Phase, +seq_hi: U32, +seq_lo: U32) -> Decision:
  match full:
    case True{}:
      Deny{SequenceExhausted{}}
    case False{}:
      Allow{phase, U32.add(seq_hi, carry(seq_lo)), U32.add(seq_lo, 1), False{}}

def advance(+step: Decision, +state: St) -> Decision:
  match step:
    case Deny{fault}:
      Deny{fault}
    case Allow{phase, _, _, _}:
      match state:
        case St{_, _, seq_hi, seq_lo, _, _}:
          store(exhausted(seq_hi, seq_lo), phase, seq_hi, seq_lo)

# `append_durable`: the same acceptance, then the stored transition.
def durable(+state: St, event: Ev) -> Decision:
  advance(validate(state, event), state)

def body_arm(+valid: Bool) -> Decision:
  match valid:
    case True{}:
      accepted(PNone{})
    case False{}:
      Deny{TerminalInvalid{}}

# `EpisodeEventV1::append` behind an effect permit: the permit already proved a
# durable start and an open episode, so only the body is in question.
def append(event: Ev) -> Decision:
  match event:
    case Ev{kind, _, _, _, _, _, _, _, link_valid, _, terminal_valid, _, _}:
      match kind:
        case KStart{}:
          Deny{DuplicateStart{}}
        case KEvidence{}:
          link_arm(link_valid)
        case KTerminal{}:
          body_arm(terminal_valid)

# `link_evidence` refuses a closed episode's unofficial evidence with its own
# fault, which is why the permit path does not reuse the append arm's name.
def official_only(+closed: Bool, +official: Bool, +valid: Bool) -> Decision:
  match closed:
    case False{}:
      link_arm(valid)
    case True{}:
      match official:
        case True{}:
          link_arm(valid)
        case False{}:
          Deny{NotOfficial{}}

# `link_evidence`: an already closed episode accepts official evidence only.
def link_closed(+closed: Bool, +event: Ev) -> Decision:
  match event:
    case Ev{_, _, _, _, _, _, _, _, link_valid, link_post_terminal, _, _, _}:
      official_only(closed, link_post_terminal, link_valid)

def link(state: St, event: Ev) -> Decision:
  match state:
    case St{_, has_terminal, _, _, _, _}:
      link_closed(has_terminal, event)

def clock_arm(+not_before: Bool) -> Decision:
  match not_before:
    case True{}:
      accepted(PCompleted{})
    case False{}:
      Deny{ReceiptMismatch{}}

def completion(
  +start_valid: Bool,
  +same_receipt: Bool,
  +completed_hi: U32,
  +completed_lo: U32,
  +started_hi: U32,
  +started_lo: U32
) -> Decision:
  match start_valid:
    case False{}:
      Deny{StartInvalid{}}
    case True{}:
      match same_receipt:
        case False{}:
          Deny{ReceiptMismatch{}}
        case True{}:
          clock_arm(ge64(completed_hi, completed_lo, started_hi, started_lo))

# `start_action_completion`: the receipt must be the durable start of this very
# episode, at sequence zero, with the start's canonical digest and a completion
# time that does not precede the start.
def complete(receipt: Rc) -> Decision:
  match receipt:
    case Rc{
      episode_equal,
      seq_hi,
      seq_lo,
      sha_equal,
      start_valid,
      completed_hi,
      completed_lo,
      started_hi,
      started_lo
    }:
      completion(
        start_valid,
        Bool.and(
          episode_equal,
          Bool.and(U32.is_zero(seq_hi), Bool.and(U32.is_zero(seq_lo), sha_equal))
        ),
        completed_hi,
        completed_lo,
        started_hi,
        started_lo
      )

def permit_gate(+closed: Bool, +started: Bool) -> Decision:
  match closed:
    case True{}:
      Deny{EpisodeClosed{}}
    case False{}:
      match started:
        case True{}:
          accepted(PNone{})
        case False{}:
          Deny{NeedsStart{}}

# `effect_permit`: a closed episode grants none, and an episode without a durable
# start has no cursor to offer.
def effect(state: St) -> Decision:
  match state:
    case St{has_episode, has_terminal, _, _, _, _}:
      permit_gate(has_terminal, has_episode)

def evidence_gate(+started: Bool, +closed: Bool) -> Decision:
  match started:
    case False{}:
      Deny{NeedsStart{}}
    case True{}:
      Allow{PNone{}, 0, 0, closed}

# `evidence_permit`: official evidence is the one thing a closed episode still
# takes, so the permit reports whether the episode is already terminal.
def evidence(state: St) -> Decision:
  match state:
    case St{has_episode, has_terminal, _, _, _, _}:
      evidence_gate(has_episode, has_terminal)
