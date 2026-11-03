import Base

type Field is Data:
  Dormant{}
  Active{ticks: Nat, radius: U32}

type Step is Data:
  Quiet{next: Field}
  Pulse{next: Field, radius: U32, stun: U32}

def tick(field: Field) -> Step:
  match field:
    case Dormant{}:
      Quiet{Dormant{}}
    case Active{0n, radius}:
      Quiet{Dormant{}}
    case Active{1n+rest, +radius}:
      Pulse{Active{rest, radius}, radius, 24}
