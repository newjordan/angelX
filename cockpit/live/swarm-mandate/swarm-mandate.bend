import Base

# The mandate decisions of one swarm turn: what the classifier's verdict means,
# whether the advisor reviews the final answer, whether that review annotates
# it, whether the turn takes the single direct pass, and whether the
# live-research route engages. Rust does the text work (upper-casing a verdict,
# substring tests, trimming the answer, parsing the advisor's BLOCK:/NOTE:
# prefix) and hands these records booleans; the combinations below are the whole
# policy.

type ClassifyOut is Data:
  Open{}
  Tight{}

type ClassifyIn is Data:
  Verdict{call_ok: Bool, saw_tight: Bool, saw_open: Bool}

type AdviseIn is Data:
  Review{enabled: Bool, blank: Bool, annotated: Bool}

type AdviseOut is Data:
  Advise{advise: Bool}

type AnnotateIn is Data:
  Note{clear: Bool, has_annotation: Bool}

type AnnotateOut is Data:
  Annotate{annotate: Bool}

type PassIn is Data:
  Pass{smart_route: Bool, research_route: Bool, fastpath: Bool, tight: Bool, classify_open: Bool, deliberation: Bool, always: Bool}

type PassOut is Data:
  Passed{direct: Bool}

type ResearchIn is Data:
  Ask{wants_live: Bool, research_enabled: Bool, researcher_present: Bool}

type ResearchOut is Data:
  Route{research_route: Bool}

# Fanning out is the swarm's reason to exist, so a failed classifier call
# defaults to `Open`; a verdict that mentions both labels is ambiguous and also
# stays `Open`. Only an unambiguous TIGHT answer takes the single pass.
# A match may only scrutinize a parameter, so the three facts are the branch.
def classify_branch(call_ok: Bool, saw_tight: Bool, saw_open: Bool) -> ClassifyOut:
  match call_ok saw_tight saw_open:
    case True{} True{} False{}:
      Tight{}
    case True{} True{} True{}:
      Open{}
    case True{} False{} True{}:
      Open{}
    case True{} False{} False{}:
      Open{}
    case False{} True{} True{}:
      Open{}
    case False{} True{} False{}:
      Open{}
    case False{} False{} True{}:
      Open{}
    case False{} False{} False{}:
      Open{}

def classify(i: ClassifyIn) -> ClassifyOut:
  match i:
    case Verdict{+call_ok, +saw_tight, +saw_open}:
      classify_branch(call_ok, saw_tight, saw_open)

# One reviewer pass over the final answer, only when the advisor is switched on,
# there is an answer to review, and it does not already carry an annotation.
def advise(i: AdviseIn) -> AdviseOut:
  match i:
    case Review{+enabled, +blank, +annotated}:
      Advise{Bool.and(enabled, Bool.and(Bool.not(blank), Bool.not(annotated)))}

# A NOTE/BLOCK verdict earns an inline annotation; a clear review returns the
# answer untouched.
def annotate(i: AnnotateIn) -> AnnotateOut:
  match i:
    case Note{+clear, +has_annotation}:
      Annotate{Bool.and(Bool.not(clear), has_annotation)}

# The smart route consults the classifier: research already earned the fan-out,
# a trivial fast-path turn does not, and otherwise the classifier decides. The
# default speed route never asks.
def deliberate_of(smart_route: Bool, research_route: Bool, fastpath: Bool, tight: Bool, classify_open: Bool, deliberation: Bool) -> Bool:
  match smart_route:
    case True{}:
      Bool.or(research_route, Bool.and(Bool.not(Bool.and(fastpath, tight)), classify_open))
    case False{}:
      Bool.or(research_route, deliberation)

# `ANGEL_SWARM_ALWAYS` skips the gate; the direct pass is the complement.
def pass(i: PassIn) -> PassOut:
  match i:
    case Pass{+smart_route, +research_route, +fastpath, +tight, +classify_open, +deliberation, +always}:
      Passed{Bool.and(Bool.not(always), Bool.not(deliberate_of(smart_route, research_route, fastpath, tight, classify_open, deliberation)))}

# Live research needs both a request that asks for outside-world context and a
# club or knob that can answer it.
def research_route(i: ResearchIn) -> ResearchOut:
  match i:
    case Ask{+wants_live, +research_enabled, +researcher_present}:
      Route{Bool.and(wants_live, Bool.or(research_enabled, researcher_present))}
