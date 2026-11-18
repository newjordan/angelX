import Base

type Facts is Data:
  Facts{seat: String, hop: U32, friction: Bool, tool_errors: U32, failed_checks: U32, task_len: U32, tests: Bool, errors: Bool, design: Bool, red: Bool, stuck: Bool}

type Ask is Data:
  Ask{native: Maybe<&2, String>, allowed: List<&2, String>, facts: Facts}

def off(thinking_off: Bool, level: String) -> Maybe<&2, String>:
  match thinking_off:
    case True{}:
      Some{"high"}
    case False{}:
      Some{level}

def lift(native: Maybe<&2, String>) -> Maybe<&2, String>:
  match native:
    case None{}:
      None{}
    case Some{+level}:
      off(String.eq(level, "none"), level)

def wake(on: Bool, native: Maybe<&2, String>) -> Maybe<&2, String>:
  match on:
    case False{}:
      native
    case True{}:
      lift(native)

def choose(ask: Ask) -> Maybe<&2, String>:
  match ask:
    case Ask{native, allowed, Facts{seat, hop, friction, tool_errors, failed_checks, task_len, tests, errors, design, red, stuck}}:
      wake(red || stuck, native)

def member(allowed: List<&2, String>, +level: String) -> Bool:
  match allowed:
    case Nil{}:
      False{}
    case Con{head, tail}:
      String.eq(head, level) || member(tail, level)

def keep(admitted: Bool, wanted: Maybe<&2, String>, native: Maybe<&2, String>) -> Maybe<&2, String>:
  match admitted:
    case True{}:
      wanted
    case False{}:
      native

def guard(+allowed: List<&2, String>, wanted: Maybe<&2, String>, native: Maybe<&2, String>) -> Maybe<&2, String>:
  match wanted:
    case None{}:
      None{}
    case Some{+level}:
      keep(member(allowed, level), Some{level}, native)

def decide(ask: Ask) -> Maybe<&2, String>:
  match ask:
    case Ask{+native, +allowed, +facts}:
      guard(allowed, choose(Ask{native, allowed, facts}), native)
