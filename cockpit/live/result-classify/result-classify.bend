import Base

# One tool result (or one provider reply) reduced by the host to the flags
# below. The host owns every prefix, marker and exit-code scan; this module only
# combines them, exactly as the native classifier does: a red verifier run is an
# error result that a verifier wrote and that reached a verdict, a dispatch
# failure is an error that reached no verdict, and an empty reply is one of the
# two no-content markers. The host checks each verdict and keeps the native one
# whenever the shape is out of contract.

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

# A run reached a verdict when the process exited with a status. Exit 126/127
# (not executable, not found) never reached one; neither did a death by signal,
# which carries no code at all.
def verdict_exit(has_exit: Bool, +exit: U32) -> Bool:
  match has_exit:
    case False{}:
      False{}
    case True{}:
      Bool.not(U32.is_eq(exit, 126) || U32.is_eq(exit, 127))

# Evidence about the code, not a call that failed to run.
def red(error: Bool, verifier: Bool, has_exit: Bool, exit: U32) -> Bool:
  error && verifier && verdict_exit(has_exit, exit)

# A call that gave the model nothing to act on.
def dispatch(error: Bool, red_verifier: Bool) -> Bool:
  error && Bool.not(red_verifier)

def empty(empty_marker: Bool, club_stream: Bool) -> Bool:
  empty_marker || club_stream

def decide(facts: Facts) -> Verdict:
  match facts:
    case Facts{+error, +verifier, +has_exit, +exit, empty_marker, club_stream}:
      Verdict{error, red(error, verifier, has_exit, exit), dispatch(error, red(error, verifier, has_exit, exit)), empty(empty_marker, club_stream)}
