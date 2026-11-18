import Base

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

def advise(i: AdviseIn) -> AdviseOut:
  match i:
    case Review{+enabled, +blank, +annotated}:
      Advise{Bool.and(enabled, Bool.and(Bool.not(blank), Bool.not(annotated)))}

def annotate(i: AnnotateIn) -> AnnotateOut:
  match i:
    case Note{+clear, +has_annotation}:
      Annotate{Bool.and(Bool.not(clear), has_annotation)}

def deliberate_of(smart_route: Bool, research_route: Bool, fastpath: Bool, tight: Bool, classify_open: Bool, deliberation: Bool) -> Bool:
  match smart_route:
    case True{}:
      Bool.or(research_route, Bool.and(Bool.not(Bool.and(fastpath, tight)), classify_open))
    case False{}:
      Bool.or(research_route, deliberation)

def pass(i: PassIn) -> PassOut:
  match i:
    case Pass{+smart_route, +research_route, +fastpath, +tight, +classify_open, +deliberation, +always}:
      Passed{Bool.and(Bool.not(always), Bool.not(deliberate_of(smart_route, research_route, fastpath, tight, classify_open, deliberation)))}

def research_route(i: ResearchIn) -> ResearchOut:
  match i:
    case Ask{+wants_live, +research_enabled, +researcher_present}:
      Route{Bool.and(wants_live, Bool.or(research_enabled, researcher_present))}
