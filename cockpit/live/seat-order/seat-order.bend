import Base

# SOTA-MoA link seating and failover walk, ported from
# cockpit/src/agent/club/failover.rs.
#
# Rust keeps every club object, its Arc/AtomicBool availability gate, metadata,
# env reads and the network walk itself. The two text questions (does a link's
# alias/label/live model name equal one of the cheap-allowlist names, and which
# positions of the cheap fallback order does it match) are precomputed per link
# into `matches`/`allowed`; the seating algorithm, the walk plan and the
# after-failure step below are the decisions this module owns.

# One candidate link: its index in the configured link vector, the identity of
# its alias (links sharing an alias share a group, so the case-insensitive
# de-duplication `ordered_cheap_links` performs is a number comparison here),
# whether the cheap allowlist admits it, and which cheap-order slots it matches.
type Link is Data:
  Lnk{index: U32, group: U32, allowed: Bool, matches: List<&2, U32>}

# One position of a failover chain: the availability gate (may be absent), and
# the metadata fields `compatible_chat_chain` consults.
type Chain is Data:
  Chn{index: U32, gated: Bool, gate_open: Bool, has_metadata: Bool, supports_tools: Bool, context_window: U32}

type Seat is Data:
  St{link: U32, group: U32}

type Plan is Data:
  Pln{ids: List<&2, U32>}

# `proceed` = try the link at `next`; `abort` distinguishes a partially streamed
# or cancelled attempt (surface that error as-is) from an exhausted chain
# (report the last failure).
type Advance is Data:
  Adv{proceed: Bool, next: U32, abort: Bool}

# ---------------------------------------------------------------------------
# ordered_cheap_links
# ---------------------------------------------------------------------------

# A link matches an order slot when any of its precomputed match flags is that
# slot; this is `link_matches` folded over the three string comparisons Rust
# already made.
def hits(+ids: List<&2, U32>, +wanted: U32, +acc: Bool) -> Bool:
  match ids:
    case Nil{}:
      acc
    case Con{head, tail}:
      hits(tail, wanted, Bool.or(acc, U32.is_eq(head, wanted)))

def any_seat(+xs: List<&2, Seat>, +group: U32, +acc: Bool) -> Bool:
  match xs:
    case Nil{}:
      acc
    case Con{head, tail}:
      match head:
        case St{link, seen_group}:
          any_seat(tail, group, Bool.or(acc, U32.is_eq(seen_group, group)))

def any_id(+xs: List<&2, U32>, +group: U32, +acc: Bool) -> Bool:
  match xs:
    case Nil{}:
      acc
    case Con{head, tail}:
      any_id(tail, group, Bool.or(acc, U32.is_eq(head, group)))

def keep_if(+keep: Bool, +id: U32, +acc: List<&2, U32>) -> List<&2, U32>:
  match keep:
    case True{}:
      id <> acc
    case False{}:
      acc

def seat_if(+keep: Bool, +seat: Seat, +acc: List<&2, Seat>) -> List<&2, Seat>:
  match keep:
    case True{}:
      seat <> acc
    case False{}:
      acc

# Every link that may take this one order slot, in configured link order. The
# allowlist refusal and the match test are both folded into `seat_if`'s flag so
# nothing here inspects a computed boolean directly.
def slot(+wanted: U32, +links: List<&2, Link>, +acc: List<&2, Seat>) -> List<&2, Seat>:
  match links:
    case Nil{}:
      acc
    case Con{head, tail}:
      match head:
        case Lnk{index, group, allowed, matches}:
          slot(
            wanted,
            tail,
            seat_if(
              Bool.and(allowed, hits(matches, wanted, False{})),
              St{index, group},
              acc
            )
          )

def slots(+order: List<&2, U32>, +links: List<&2, Link>, +acc: List<&2, Seat>) -> List<&2, Seat>:
  match order:
    case Nil{}:
      acc
    case Con{head, tail}:
      slots(tail, links, slot(head, links, acc))

def groups_add(+seen: Bool, +group: U32, +groups: List<&2, U32>) -> List<&2, U32>:
  match seen:
    case True{}:
      groups
    case False{}:
      group <> groups

def out_add(+seen: Bool, +link: U32, +out: List<&2, U32>) -> List<&2, U32>:
  match seen:
    case True{}:
      out
    case False{}:
      link <> out

# `ordered_cheap_links` refuses a second link whose alias was already seated.
# The alias identity is the precomputed group id, so the check is one membership
# test over the groups seated so far.
def dedupe(+seats: List<&2, Seat>, +groups: List<&2, U32>, +out: List<&2, U32>) -> List<&2, U32>:
  match seats:
    case Nil{}:
      out
    case Con{head, tail}:
      match head:
        case St{link, group}:
          +seen = any_id(groups, group, False{})
          dedupe(tail, groups_add(seen, group, groups), out_add(seen, link, out))

def reel_seats(+xs: List<&2, Seat>, +acc: List<&2, Seat>) -> List<&2, Seat>:
  match xs:
    case Nil{}:
      acc
    case Con{head, tail}:
      reel_seats(tail, head <> acc)

def reel_ids(+xs: List<&2, U32>, +acc: List<&2, U32>) -> List<&2, U32>:
  match xs:
    case Nil{}:
      acc
    case Con{head, tail}:
      reel_ids(tail, head <> acc)

# Seat order for the cheap profile: walk the preference slots in order, seat
# every admitted matching link once (first alias group wins), keep the rest out.
def order(+slots_order: List<&2, U32>, +links: List<&2, Link>) -> Plan:
  Pln{reel_ids(dedupe(reel_seats(slots(slots_order, links, Nil{}), Nil{}), Nil{}, Nil{}), Nil{})}

# ---------------------------------------------------------------------------
# live_chain / compatible_chat_chain
# ---------------------------------------------------------------------------

# The netting of two gates: `live_chain` skips a link whose prober gate is shut,
# but a chain the prober marks entirely down is still walked in full — the bits
# are an optimization, never a brick.
def admit(+open: Bool, +gated: Bool, +gate_open: Bool) -> Bool:
  match open:
    case False{}:
      True{}
    case True{}:
      Bool.or(Bool.not(gated), gate_open)

def fits(
  +has_tools: Bool,
  +needed: U32,
  +has_metadata: Bool,
  +supports_tools: Bool,
  +context_window: U32
) -> Bool:
  Bool.or(
    Bool.not(has_metadata),
    Bool.and(
      Bool.or(Bool.not(has_tools), supports_tools),
      Bool.or(U32.is_eq(context_window, 0), U32.is_gt(context_window, (needed + 256 : U32)))
    )
  )

def fits_when(
  +chat: Bool,
  +has_tools: Bool,
  +needed: U32,
  +has_metadata: Bool,
  +supports_tools: Bool,
  +context_window: U32
) -> Bool:
  match chat:
    case False{}:
      True{}
    case True{}:
      fits(has_tools, needed, has_metadata, supports_tools, context_window)

# Whether any gate is open at all, which selects between the gated chain and the
# whole chain. Recomputed here rather than as a separate pass.
def any_open(+links: List<&2, Chain>, +acc: Bool) -> Bool:
  match links:
    case Nil{}:
      acc
    case Con{head, tail}:
      match head:
        case Chn{index, gated, gate_open, has_metadata, supports_tools, context_window}:
          any_open(tail, Bool.or(acc, Bool.and(gated, gate_open)))

def walk_list(
  +chat: Bool,
  +has_tools: Bool,
  +needed: U32,
  +open: Bool,
  +links: List<&2, Chain>,
  +acc: List<&2, U32>
) -> List<&2, U32>:
  match links:
    case Nil{}:
      acc
    case Con{head, tail}:
      match head:
        case Chn{index, gated, gate_open, has_metadata, supports_tools, context_window}:
          walk_list(
            chat,
            has_tools,
            needed,
            open,
            tail,
            keep_if(
              Bool.and(
                admit(open, gated, gate_open),
                fits_when(chat, has_tools, needed, has_metadata, supports_tools, context_window)
              ),
              index,
              acc
            )
          )

# The order in which `respond`/`chat`/`chat_streaming_with_effort` attempt the
# chain, as chain positions.
def walk(+chat: Bool, +has_tools: Bool, +needed: U32, +links: List<&2, Chain>) -> Plan:
  Pln{reel_ids(walk_list(chat, has_tools, needed, any_open(links, False{}), links, Nil{}), Nil{})}

# ---------------------------------------------------------------------------
# the next link after a failure
# ---------------------------------------------------------------------------

def at_end(+done: Bool, +next: U32) -> Advance:
  match done:
    case True{}:
      Adv{False{}, 0, False{}}
    case False{}:
      Adv{True{}, next, False{}}

def step_on(+step: U32, +plan_len: U32) -> Advance:
  at_end(U32.is_ge((step + 1 : U32), plan_len), (step + 1 : U32))

def emitted_step(+step: U32, +plan_len: U32, +emitted: Bool) -> Advance:
  match emitted:
    case True{}:
      # A partially streamed reply cannot be replayed onto the next link.
      Adv{False{}, 0, True{}}
    case False{}:
      step_on(step, plan_len)

# A cancel and a partially streamed reply are the same kind of stop: the next
# link cannot replay a stream, and a user cancel is never routed around.
def streamed_step(+step: U32, +plan_len: U32, +emitted: Bool, +cancelled: Bool) -> Advance:
  match cancelled:
    case True{}:
      Adv{False{}, 0, True{}}
    case False{}:
      emitted_step(step, plan_len, emitted)

def advance(+step: U32, +plan_len: U32, +streaming: Bool, +emitted: Bool, +cancelled: Bool) -> Advance:
  match streaming:
    case True{}:
      streamed_step(step, plan_len, emitted, cancelled)
    case False{}:
      step_on(step, plan_len)
