import Base

# The verifier's planning decision. Rust keeps every filesystem and lexical job
# that cannot be a decision: manifest reads, path confinement, glob expansion,
# source discovery, argv text and the byte limits. Those become the workspace
# facts below. This module owns which verification step runs, in what order, and
# which request is refused.
#
# The codes below are the shared table (plan.rs `Step`, index.js passthrough):
#   route    0 rust        1 go            2 verify       3 reject
#   runtime  0 none        1 node          2 python       3 go
#   mode     0 none        1 check         2 tests
#   driver   0 none        1 node-tests    2 pytest       3 unittest
#   reject   0 none        1 ambiguous     2 lint         3 go-needs-tests

type Requested is Data:
  Auto{}
  Rust{}
  Go{}
  Node{}
  Python{}

type Entrypoint is Data:
  NoEntrypoint{}
  NodeEntrypoint{}
  Unittest{}
  Pytest{}
  Other{}

type KindCode is Data:
  Check{}
  Tests{}
  Lint{}

# The one language the plan may run in, or the fact that there is no single one.
type Language is Data:
  RustLang{}
  GoLang{}
  NodeLang{}
  PythonLang{}
  Ambiguous{}

type Features is Type:
  Features{
    requested: Requested,
    entrypoint: Entrypoint,
    kind: KindCode,
    has_cargo_toml: Bool,
    has_package_json: Bool,
    has_pyproject_toml: Bool,
    has_python_setup: Bool,
    has_pytest_config: Bool
  }

type Step is Data:
  Plan{route: U32, runtime: U32, mode: U32, driver: U32, reject: U32}

# A workspace with exactly one recognizable language plans for it; none at all or
# several is refused rather than guessed.
def detected(has_cargo_toml: Bool, has_package_json: Bool, has_python: Bool) -> Language:
  match has_cargo_toml:
    case True{}:
      match has_package_json:
        case True{}:
          Ambiguous{}
        case False{}:
          match has_python:
            case True{}:
              Ambiguous{}
            case False{}:
              RustLang{}
    case False{}:
      match has_package_json:
        case True{}:
          match has_python:
            case True{}:
              Ambiguous{}
            case False{}:
              NodeLang{}
        case False{}:
          match has_python:
            case True{}:
              PythonLang{}
            case False{}:
              Ambiguous{}

# An explicit entrypoint settles the language first; only an unset or
# unrecognized one falls through to the manifests.
def auto_language(entrypoint: Entrypoint, has_cargo_toml: Bool, has_package_json: Bool, has_python: Bool) -> Language:
  match entrypoint:
    case NodeEntrypoint{}:
      NodeLang{}
    case Unittest{}:
      PythonLang{}
    case Pytest{}:
      PythonLang{}
    case NoEntrypoint{}:
      detected(has_cargo_toml, has_package_json, has_python)
    case Other{}:
      detected(has_cargo_toml, has_package_json, has_python)

def language(requested: Requested, entrypoint: Entrypoint, has_cargo_toml: Bool, has_package_json: Bool, has_python: Bool) -> Language:
  match requested:
    case Auto{}:
      auto_language(entrypoint, has_cargo_toml, has_package_json, has_python)
    case Rust{}:
      RustLang{}
    case Go{}:
      GoLang{}
    case Node{}:
      NodeLang{}
    case Python{}:
      PythonLang{}

# pytest runs for an explicit request, or for a configured pytest when the
# entrypoint is not an explicit unittest one.
def configured_driver(has_pytest_config: Bool) -> U32:
  match has_pytest_config:
    case True{}:
      2
    case False{}:
      3

def python_driver(entrypoint: Entrypoint, has_pytest_config: Bool) -> U32:
  match entrypoint:
    case Pytest{}:
      2
    case Unittest{}:
      3
    case NoEntrypoint{}:
      configured_driver(has_pytest_config)
    case NodeEntrypoint{}:
      configured_driver(has_pytest_config)
    case Other{}:
      configured_driver(has_pytest_config)

# A lint request has no trusted adapter at all, so it is refused after the
# language is settled and before any step is planned.
def node_plan(kind: KindCode) -> Step:
  match kind:
    case Lint{}:
      Plan{3, 1, 0, 0, 2}
    case Check{}:
      Plan{2, 1, 1, 0, 0}
    case Tests{}:
      Plan{2, 1, 2, 1, 0}

def python_plan(kind: KindCode, entrypoint: Entrypoint, has_pytest_config: Bool) -> Step:
  match kind:
    case Lint{}:
      Plan{3, 2, 0, 0, 2}
    case Check{}:
      Plan{2, 2, 1, 0, 0}
    case Tests{}:
      Plan{2, 2, 2, python_driver(entrypoint, has_pytest_config), 0}

def go_plan(kind: KindCode) -> Step:
  match kind:
    case Tests{}:
      Plan{1, 3, 2, 0, 0}
    case Check{}:
      Plan{3, 0, 0, 0, 3}
    case Lint{}:
      Plan{3, 0, 0, 0, 3}

def plan_language(+lang: Language, +kind: KindCode, +entrypoint: Entrypoint, +has_pytest_config: Bool) -> Step:
  match lang:
    case RustLang{}:
      Plan{0, 0, 0, 0, 0}
    case Ambiguous{}:
      Plan{3, 0, 0, 0, 1}
    case GoLang{}:
      go_plan(kind)
    case NodeLang{}:
      node_plan(kind)
    case PythonLang{}:
      python_plan(kind, entrypoint, has_pytest_config)

def step(f: Features) -> Step:
  match f:
    case Features{+requested, +entrypoint, +kind, +has_cargo_toml, +has_package_json, +has_pyproject_toml, +has_python_setup, +has_pytest_config}:
      plan_language(
        language(requested, entrypoint, has_cargo_toml, has_package_json, Bool.or(has_pyproject_toml, has_python_setup)),
        kind,
        entrypoint,
        has_pytest_config)
