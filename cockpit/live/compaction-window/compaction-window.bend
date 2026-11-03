import Base

# Which span of history gets compacted, given a recent-message tail and a
# recent-token tail. Rust owns every textual question — which leading messages
# are the pinned preamble, which messages carry a Tool role, and what a message
# costs in estimated tokens — and hands this module one record per message. The
# choice of `[start, end)` is the whole of this module, mirroring
# `select_window_with_token_tail` in cockpit/src/agent/compaction.rs.
#
# Bend only matches parameters and record fields, so every boundary test below
# is computed by a small def and passed into the def that branches on it.

type Msg is Data:
  Say{pinned: Bool, tool: Bool, weight: U32}

type Decision is Data:
  Chosen{start: U32, end: U32}
  Absent{}

def messages(+xs: List<&2, Msg>) -> U32:
  match xs:
    case Nil{}:
      0
    case Con{head, rest}:
      (messages(rest) + 1 : U32)

# The pinned preamble is never compacted, so the window starts after it.
def preamble(+xs: List<&2, Msg>) -> U32:
  match xs:
    case Nil{}:
      0
    case Con{head, rest}:
      match head:
        case Say{pinned, tool, weight}:
          match pinned:
            case True{}:
              (preamble(rest) + 1 : U32)
            case False{}:
              0

# Reverse the history so the tail can be walked newest-first.
def backwards(+xs: List<&2, Msg>, +acc: List<&2, Msg>) -> List<&2, Msg>:
  match xs:
    case Nil{}:
      acc
    case Con{head, rest}:
      backwards(rest, Con{head, acc})

# `inside` says the walk has not yet reached the preamble; `short` says the
# retained tail still owes tokens or its two-message floor.
def walk(
  +newest: List<&2, Msg>,
  +inside: Bool,
  +short: Bool,
  +len: U32,
  +sys_end: U32,
  +keep: U32,
  +k: U32,
  +spent: U32
) -> U32:
  match newest:
    case Nil{}:
      (len - k : U32)
    case Con{head, rest}:
      match head:
        case Say{pinned, tool, weight}:
          match inside:
            case False{}:
              (len - k : U32)
            case True{}:
              match short:
                case False{}:
                  (len - k : U32)
                case True{}:
                  walk(
                    rest,
                    U32.is_gt(U32.sub(len, U32.inc(k)), sys_end),
                    Bool.or(U32.is_lt(U32.add(spent, weight), keep), U32.is_lt(U32.inc(k), 2)),
                    len,
                    sys_end,
                    keep,
                    U32.inc(k),
                    U32.add(spent, weight))

# The first index of the retained tail. `k` retained messages always satisfy the
# two-message floor's first step, so the walk starts with `short` true.
def retained(+newest: List<&2, Msg>, +len: U32, +sys_end: U32, +keep: U32) -> U32:
  walk(newest, U32.is_gt(len, sys_end), True{}, len, sys_end, keep, 0, 0)

def dropped(+xs: List<&2, Msg>, +at_zero: Bool, +n: U32) -> List<&2, Msg>:
  match xs:
    case Nil{}:
      Nil{}
    case Con{head, rest}:
      match at_zero:
        case True{}:
          Con{head, rest}
        case False{}:
          dropped(rest, U32.is_eq(U32.sub(n, 1), 0), U32.sub(n, 1))

def drop_at(+xs: List<&2, Msg>, +n: U32) -> List<&2, Msg>:
  dropped(xs, U32.is_eq(n, 0), n)

# The kept suffix must not begin on a tool result orphaned from its call, so the
# window end advances past every leading Tool message of that suffix.
def advance(+xs: List<&2, Msg>, +at: U32) -> U32:
  match xs:
    case Nil{}:
      at
    case Con{head, rest}:
      match head:
        case Say{pinned, tool, weight}:
          match tool:
            case True{}:
              advance(rest, U32.inc(at))
            case False{}:
              at

# Nothing worth a summarizer round-trip: no live tail, or a sliver.
def settle(+late: Bool, +sliver: Bool, +sys_end: U32, +last: U32) -> Decision:
  match late:
    case True{}:
      Absent{}
    case False{}:
      match sliver:
        case True{}:
          Absent{}
        case False{}:
          Chosen{sys_end, last}

def finish(+xs: List<&2, Msg>, +len: U32, +sys_end: U32, +end: U32) -> Decision:
  +last = advance(drop_at(xs, end), end)
  settle(U32.is_ge(last, len), U32.is_lt(U32.sub(last, sys_end), 3), sys_end, last)

def counted_tail(
  +fits: Bool,
  +xs: List<&2, Msg>,
  +len: U32,
  +sys_end: U32,
  +keep_recent: U32
) -> Decision:
  match fits:
    case True{}:
      Absent{}
    case False{}:
      finish(xs, len, sys_end, U32.sub(len, keep_recent))

def token_tail(
  +no_tail: Bool,
  +start: U32,
  +xs: List<&2, Msg>,
  +len: U32,
  +sys_end: U32
) -> Decision:
  match no_tail:
    case True{}:
      Absent{}
    case False{}:
      finish(xs, len, sys_end, start)

def mode(
  +counted: Bool,
  +xs: List<&2, Msg>,
  +len: U32,
  +sys_end: U32,
  +keep_recent: U32,
  +keep_tokens: U32
) -> Decision:
  match counted:
    case True{}:
      counted_tail(U32.is_le(U32.sub(len, sys_end), keep_recent), xs, len, sys_end, keep_recent)
    case False{}:
      +start = retained(backwards(xs, Nil{}), len, sys_end, keep_tokens)
      token_tail(U32.is_le(start, sys_end), start, xs, len, sys_end)

def window(+xs: List<&2, Msg>, +keep_recent: U32, +keep_tokens: U32) -> Decision:
  +len = messages(xs)
  +sys_end = preamble(xs)
  mode(U32.is_eq(keep_tokens, 0), xs, len, sys_end, keep_recent, keep_tokens)
