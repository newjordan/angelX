import Base

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
