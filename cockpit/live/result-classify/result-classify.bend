import Base

type Facts is Data:
  Facts{error: Bool, verifier: Bool, has_exit: Bool, exit: U32, empty_marker: Bool, club_stream: Bool}

type Verdict is Data:
  Verdict{error: Bool, red_verifier: Bool, dispatch_failure: Bool, empty_reply: Bool}

def error_of(verdict: Verdict) -> Bool:
  match verdict:
    case Verdict{error, red_verifier, dispatch_failure, empty_reply}:
      error

def red_of(verdict: Verdict) -> Bool:
  match verdict:
    case Verdict{error, red_verifier, dispatch_failure, empty_reply}:
      red_verifier

def dispatch_of(verdict: Verdict) -> Bool:
  match verdict:
    case Verdict{error, red_verifier, dispatch_failure, empty_reply}:
      dispatch_failure

def empty_of(verdict: Verdict) -> Bool:
  match verdict:
    case Verdict{error, red_verifier, dispatch_failure, empty_reply}:
      empty_reply

def verdict_exit(has_exit: Bool, +exit: U32) -> Bool:
  match has_exit:
    case False{}:
      False{}
    case True{}:
      Bool.not(U32.is_eq(exit, 126) || U32.is_eq(exit, 127))

def red(error: Bool, verifier: Bool, has_exit: Bool, exit: U32) -> Bool:
  error && verifier && verdict_exit(has_exit, exit)

def dispatch(error: Bool, red_verifier: Bool) -> Bool:
  error && Bool.not(red_verifier)

def empty(empty_marker: Bool, club_stream: Bool) -> Bool:
  empty_marker || club_stream

def decide(facts: Facts) -> Verdict:
  match facts:
    case Facts{+error, +verifier, +has_exit, +exit, empty_marker, club_stream}:
      Verdict{error, red(error, verifier, has_exit, exit), dispatch(error, red(error, verifier, has_exit, exit)), empty(empty_marker, club_stream)}
