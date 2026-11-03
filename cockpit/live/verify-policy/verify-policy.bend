import Base

# Verify-before-done policy. The host keeps every lexical job: shell quoting,
# control operators, status-bearing segments, test-runner and build-command
# recognition, and mutation target paths. Those become the numbers, enums,
# booleans and per-segment classes below; this module owns only the decision.
type Tool is Data:
  Shell{}
  Cargo{}
  Fmt{}
  Check{}
  Other{}

type Verb is Data:
  Test{}
  CheckVerb{}
  Clippy{}
  FmtVerb{}
  Build{}
  NoVerb{}

# One shell segment, as the host's lexer classified it.
type Segment is Data:
  RunsVerifier{}
  RunsBuild{}
  RunsBoth{}
  RunsNothing{}

# What a whole segment list contains, folded once because a List is a Type and
# cannot be read twice.
type Class is Data:
  HasVerifier{}
  HasBuild{}
  HasBoth{}
  HasNone{}

type Features is Type:
  Features{
    tool: Tool,
    fmt_check: Bool,
    verb: Verb,
    check_flag: Bool,
    mutation: Bool,
    targets: U32,
    code_targets: U32,
    or_branch: Bool,
    async_control: Bool,
    status: List<Segment>,
    anywhere: List<Segment>
  }

type Decision is Data:
  Decision{
    requires_verification: Bool,
    is_verification: Bool,
    is_progress_verifier: Bool,
    runs_tests_anywhere: Bool
  }

def class_of(verifier: Bool, build: Bool) -> Class:
  match verifier:
    case True{}:
      match build:
        case True{}:
          HasBoth{}
        case False{}:
          HasVerifier{}
    case False{}:
      match build:
        case True{}:
          HasBuild{}
        case False{}:
          HasNone{}

def classify(xs: List<Segment>, +verifier: Bool, +build: Bool) -> Class:
  match xs:
    case Nil{}:
      class_of(verifier, build)
    case h <> t:
      match h:
        case RunsVerifier{}:
          classify(t, Bool.or(verifier, True{}), build)
        case RunsBoth{}:
          classify(t, Bool.or(verifier, True{}), Bool.or(build, True{}))
        case RunsBuild{}:
          classify(t, verifier, Bool.or(build, True{}))
        case RunsNothing{}:
          classify(t, verifier, build)

# An `||` branch or an asynchronous `&` job can hide a verifier's failure, so
# neither form is evidence even when a segment looks like a verifier.
def clean(or_branch: Bool, async_control: Bool) -> Bool:
  match or_branch:
    case True{}:
      False{}
    case False{}:
      match async_control:
        case True{}:
          False{}
        case False{}:
          True{}

def mutation_owes(mutation: Bool, targets: U32, code_targets: U32) -> Bool:
  match mutation:
    case False{}:
      False{}
    case True{}:
      Bool.or(U32.is_eq(targets, 0), U32.is_gt(code_targets, 0))

def anywhere_credit(c: Class) -> Bool:
  match c:
    case HasVerifier{}:
      True{}
    case HasBoth{}:
      True{}
    case HasBuild{}:
      False{}
    case HasNone{}:
      False{}

def shell_class_decision(status: Class, anywhere: Class, +clean_ok: Bool, requires: Bool) -> Decision:
  match status:
    case HasVerifier{}:
      Decision{requires, clean_ok, clean_ok, anywhere_credit(anywhere)}
    case HasBoth{}:
      Decision{requires, clean_ok, clean_ok, anywhere_credit(anywhere)}
    case HasBuild{}:
      Decision{requires, False{}, clean_ok, anywhere_credit(anywhere)}
    case HasNone{}:
      Decision{requires, False{}, False{}, anywhere_credit(anywhere)}

def shell_decision(+or_branch: Bool, +async_control: Bool, status: List<Segment>, anywhere: List<Segment>, requires: Bool) -> Decision:
  shell_class_decision(
    classify(status, False{}, False{}),
    classify(anywhere, False{}, False{}),
    clean(or_branch, async_control),
    requires
  )

def cargo_decision(verb: Verb, +check_flag: Bool, requires: Bool) -> Decision:
  match verb:
    case Test{}:
      Decision{requires, True{}, True{}, False{}}
    case CheckVerb{}:
      Decision{requires, True{}, True{}, False{}}
    case Clippy{}:
      Decision{requires, True{}, True{}, False{}}
    case FmtVerb{}:
      Decision{requires, check_flag, check_flag, False{}}
    case Build{}:
      Decision{requires, False{}, True{}, False{}}
    case NoVerb{}:
      Decision{requires, False{}, False{}, False{}}

def decide(f: Features) -> Decision:
  match f:
    case Features{+tool, +fmt_check, +verb, +check_flag, +mutation, +targets, +code_targets, +or_branch, +async_control, status, anywhere}:
      match tool:
        case Shell{}:
          shell_decision(or_branch, async_control, status, anywhere, mutation_owes(mutation, targets, code_targets))
        case Cargo{}:
          cargo_decision(verb, check_flag, mutation_owes(mutation, targets, code_targets))
        case Fmt{}:
          Decision{mutation_owes(mutation, targets, code_targets), fmt_check, fmt_check, False{}}
        case Check{}:
          Decision{mutation_owes(mutation, targets, code_targets), True{}, True{}, False{}}
        case Other{}:
          Decision{mutation_owes(mutation, targets, code_targets), False{}, False{}, False{}}

# The host adapter and the checked laws talk to this flat form.
def decide_parts(tool: Tool, fmt_check: Bool, verb: Verb, check_flag: Bool, mutation: Bool, targets: U32, code_targets: U32, or_branch: Bool, async_control: Bool, status: List<Segment>, anywhere: List<Segment>) -> Decision:
  decide(Features{tool, fmt_check, verb, check_flag, mutation, targets, code_targets, or_branch, async_control, status, anywhere})
