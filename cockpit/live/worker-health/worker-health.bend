import Base

# Authored worker-health policy. Rust keeps fingerprinting, lease-store
# mutations and error/health construction; this module decides, from the
# numeric features the adapter supplies, whether a worker is healthy,
# nudged, inspected, replaced, and whether a semantic loop continues,
# pivots, retries or requests a fresh turn. Phase/step codes mirror the
# native enums:
#   phase  0=Granted 1=Active 2=Suspect 3=Nudged 4=Inspected 5=Fenced 6=Released
#   health 0=Fresh 1=Degraded 2=Retrying 3=NeedsAttention

type Recovery is Data:
  Step{state: Nat, phase: Nat}
  Blocked{}

type Semantic is Data:
  Choose{state: Nat, stale_next: Nat}
  Refused{}

def recovery(phase: Nat) -> Recovery:
  match phase:
    case 0n:
      Step{0n, 0n}
    case 1n:
      Step{0n, 1n}
    case 2n:
      Step{1n, 2n}
    case 3n:
      Step{2n, 3n}
    case 4n:
      Step{3n, 4n}
    case 5n:
      Step{0n, 5n}
    case 6n:
      Step{0n, 6n}
    case 7n+p:
      Blocked{}

# progress=True resets staleness and continues fresh; otherwise the native
# caller already incremented the candidate count, so stale_next == 2 pivots
# (degraded), == 4 archives into a fresh turn (needs attention), anything
# else keeps retrying while staying on the direction.
def semantic(progress: Bool, stale_next: Nat) -> Semantic:
  match progress:
    case True{}:
      Choose{0n, 0n}
    case False{}:
      match stale_next:
        case 0n:
          Choose{2n, stale_next}
        case 1n:
          Choose{2n, stale_next}
        case 2n:
          Choose{1n, stale_next}
        case 3n:
          Choose{2n, stale_next}
        case 4n:
          Choose{3n, 0n}
        case 5n:
          Choose{2n, stale_next}
        case 6n:
          Choose{2n, stale_next}
        case 7n+p:
          Refused{}
