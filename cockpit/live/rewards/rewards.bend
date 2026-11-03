import Base

# The reward ledger's decisions: which row is an episode's effective reward, and
# whether a new provisional shaping event or official result is appended,
# recognised as an existing row, or refused.
#
# Rust keeps everything a decision cannot see: the ids, receipts, scores,
# canonical decimal arithmetic, `binding_sha`, the validation error strings and
# the ledger storage. It hands over the row flags the native scans read — which
# existing row carries the same shaping id, the same official result id, the
# same receipt, the same episode, the same reward identity, the same full
# binding — plus each binding's revision split into two U32 halves, and this
# module owns which row wins and what happens next.
#
# The native scans in `cockpit/src/drive/competition/rewards.rs` stay as the
# fallback and the parity oracle:
#   * `effective_reward` takes the newest binding for the episode and only falls
#     back to the newest provisional shaping event when there is none (so
#     `canonical_binding` is a reverse scan);
#   * `record_shaping` appends, recognises an identical row, or refuses a
#     same-id row that differs;
#   * `bind_official` looks up the official result id first, then the receipt,
#     and only then decides whether the binding is the episode's first reward or
#     a correction that advances the canonical one.

type Hit is Data:
  Hit{found: Bool, at: U32}

type ShapingRow is Data:
  ShapingRow{id_equal: Bool, entry_equal: Bool}

type BindingRow is Data:
  BindingRow{
    result_id_equal: Bool,
    entry_equal: Bool,
    provenance_equal: Bool,
    same_event: Bool,
    episode_equal: Bool,
    identity_equal: Bool,
    correction_targets: Bool,
    revision_hi: U32,
    revision_lo: U32
  }

# The incoming binding's own facts: whether it claims to be a correction, and
# its revision as two U32 halves so the comparison is exact for any u64.
type Incoming is Data:
  Incoming{has_correction: Bool, incoming_hi: U32, incoming_lo: U32}

type Selection is Data:
  Official{index: U32}
  Provisional{index: U32}
  Absent{}

type Shaping is Data:
  Append{}
  Duplicate{index: U32}
  Conflict{index: U32}

type Binding is Data:
  Appended{}
  AlreadyBound{index: U32}
  ConflictId{index: U32}
  ConflictReceipt{index: U32}
  RejectedFirstCorrection{}
  RejectedCorrection{index: U32}

type Step is Data:
  Stop{binding: Binding}
  Continue{}

type Canon is Data:
  Canon{
    found: Bool,
    at: U32,
    identity_equal: Bool,
    correction_targets: Bool,
    revision_hi: U32,
    revision_lo: U32
  }

# The newest row whose flag is set, which is `iter().rev().find(...)`.
def flag_last(+flag: Bool, +at: U32, +hit: Hit) -> Hit:
  match flag:
    case True{}:
      Hit{True{}, at}
    case False{}:
      hit

def last_match(+flags: List<&2, Bool>, +at: U32, +hit: Hit) -> Hit:
  match flags:
    case Nil{}:
      hit
    case Con{+head, tail}:
      last_match(tail, U32.add(at, 1), flag_last(head, at, hit))

# An official binding outranks a provisional shaping event; a shaping event is
# only the effective reward when the episode has no binding at all.
def provisional(+hit: Hit) -> Selection:
  match hit:
    case Hit{found, at}:
      match found:
        case True{}:
          Provisional{at}
        case False{}:
          Absent{}

def official(+hit: Hit, +shaping: List<&2, Bool>) -> Selection:
  match hit:
    case Hit{found, at}:
      match found:
        case True{}:
          Official{at}
        case False{}:
          provisional(last_match(shaping, 0, Hit{False{}, 0}))

def select(+bindings: List<&2, Bool>, +shaping: List<&2, Bool>) -> Selection:
  official(last_match(bindings, 0, Hit{False{}, 0}), shaping)

# The first existing shaping row with the same shaping id decides the outcome:
# an identical row is the same event, any other row is a conflicting identity.
def shaping_at(+rows: List<&2, ShapingRow>, +at: U32) -> Shaping:
  match rows:
    case Nil{}:
      Append{}
    case Con{+head, tail}:
      match head:
        case ShapingRow{id_equal, entry_equal}:
          match id_equal:
            case True{}:
              match entry_equal:
                case True{}:
                  Duplicate{at}
                case False{}:
                  Conflict{at}
            case False{}:
              shaping_at(tail, U32.add(at, 1))

def record_shaping(+rows: List<&2, ShapingRow>) -> Shaping:
  shaping_at(rows, 0)

# `bind_official`'s three scans, in the native order: result id, then receipt,
# then the canonical binding of the episode with its correction rule.
def result_at(+rows: List<&2, BindingRow>, +at: U32) -> Step:
  match rows:
    case Nil{}:
      Continue{}
    case Con{+head, tail}:
      match head:
        case BindingRow{
          result_id_equal,
          entry_equal,
          provenance_equal,
          same_event,
          episode_equal,
          identity_equal,
          correction_targets,
          revision_hi,
          revision_lo
        }:
          match result_id_equal:
            case True{}:
              match entry_equal:
                case True{}:
                  Stop{AlreadyBound{at}}
                case False{}:
                  Stop{ConflictId{at}}
            case False{}:
              result_at(tail, U32.add(at, 1))

def provenance_at(+rows: List<&2, BindingRow>, +at: U32) -> Step:
  match rows:
    case Nil{}:
      Continue{}
    case Con{+head, tail}:
      match head:
        case BindingRow{
          result_id_equal,
          entry_equal,
          provenance_equal,
          same_event,
          episode_equal,
          identity_equal,
          correction_targets,
          revision_hi,
          revision_lo
        }:
          match provenance_equal:
            case True{}:
              match same_event:
                case True{}:
                  Stop{AlreadyBound{at}}
                case False{}:
                  Stop{ConflictReceipt{at}}
            case False{}:
              provenance_at(tail, U32.add(at, 1))

# `canonical_binding`: the newest binding of the episode, and the facts the
# correction rule reads from it.
def episode_at(+rows: List<&2, BindingRow>, +at: U32, +canon: Canon) -> Canon:
  match rows:
    case Nil{}:
      canon
    case Con{+head, tail}:
      match head:
        case BindingRow{
          result_id_equal,
          entry_equal,
          provenance_equal,
          same_event,
          episode_equal,
          identity_equal,
          correction_targets,
          revision_hi,
          revision_lo
        }:
          match episode_equal:
            case True{}:
              episode_at(
                tail,
                U32.add(at, 1),
                Canon{True{}, at, identity_equal, correction_targets, revision_hi, revision_lo}
              )
            case False{}:
              episode_at(tail, U32.add(at, 1), canon)

# A correction must strictly advance the canonical revision, halves and all.
def half_rule(+greater: Bool, +same: Bool, +low_greater: Bool) -> Bool:
  match greater:
    case True{}:
      True{}
    case False{}:
      match same:
        case True{}:
          low_greater
        case False{}:
          False{}

def advances(
  +canon_hi: U32,
  +canon_lo: U32,
  +incoming_hi: U32,
  +incoming_lo: U32
) -> Bool:
  half_rule(
    U32.is_gt(incoming_hi, canon_hi),
    U32.is_eq(incoming_hi, canon_hi),
    U32.is_gt(incoming_lo, canon_lo)
  )

def target_rule(+targets: Bool, +identity_equal: Bool, +advanced: Bool) -> Bool:
  match targets:
    case False{}:
      False{}
    case True{}:
      match identity_equal:
        case False{}:
          False{}
        case True{}:
          advanced

def first_reward(+has_correction: Bool) -> Binding:
  match has_correction:
    case True{}:
      RejectedFirstCorrection{}
    case False{}:
      Appended{}

def correction_outcome(+accepted: Bool, +at: U32) -> Binding:
  match accepted:
    case True{}:
      Appended{}
    case False{}:
      RejectedCorrection{at}

def after_canonical(
  +canon: Canon,
  +has_correction: Bool,
  +incoming_hi: U32,
  +incoming_lo: U32
) -> Binding:
  match canon:
    case Canon{found, at, identity_equal, correction_targets, revision_hi, revision_lo}:
      match found:
        case False{}:
          first_reward(has_correction)
        case True{}:
          correction_outcome(
            target_rule(
              correction_targets,
              identity_equal,
              advances(revision_hi, revision_lo, incoming_hi, incoming_lo)
            ),
            at
          )

def canonical_step(
  +step: Step,
  +rows: List<&2, BindingRow>,
  +incoming: Incoming
) -> Binding:
  match step:
    case Stop{binding}:
      binding
    case Continue{}:
      match incoming:
        case Incoming{has_correction, incoming_hi, incoming_lo}:
          after_canonical(
            episode_at(rows, 0, Canon{False{}, 0, False{}, False{}, 0, 0}),
            has_correction,
            incoming_hi,
            incoming_lo
          )

def binding_step(
  +step: Step,
  +rows: List<&2, BindingRow>,
  +incoming: Incoming
) -> Binding:
  match step:
    case Stop{binding}:
      binding
    case Continue{}:
      canonical_step(provenance_at(rows, 0), rows, incoming)

def bind_official(+rows: List<&2, BindingRow>, +incoming: Incoming) -> Binding:
  binding_step(result_at(rows, 0), rows, incoming)
