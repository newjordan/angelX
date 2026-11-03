import Base

# Authored ability policy. The adapter supplies cast identity and room context;
# this module owns placement, timing, expiry and area-status requests.
type Parameters is Data:
  Params{radius: F32, duration: U32, hold: U32, pulse: U32}

type Definition is Data:
  Skill{name: String, mana: F32, cooldown: U32, icon: String, parameters: Parameters}

def definition() -> Definition:
  Skill{"Fairy Stun Field", 18.0, 240, "..P.P../.PPPPP./..P.P../...P.../..P.P..", Params{3.5, 150, 12, 12}}

type Field is Data:
  Fresh{}
  Planted{x: F32, y: F32, location: String, next: U32}

type Context is Data:
  Tick{age: U32, x: F32, y: F32, location: String}

type Area is Data:
  Stun{x: F32, y: F32, radius: F32, ticks: U32}

type Step is Data:
  End{}
  Keep{field: Field, opened: Bool, area: Maybe<&2, Area>}

def decide(live: Bool, due: Bool, opened: Bool, +age: U32, next: U32, +x: F32, +y: F32, +location: String, radius: F32, hold: U32, pulse: U32) -> Step:
  match live due:
    case False{} _:
      End{}
    case True{} True{}:
      Keep{Planted{x, y, location, (age + pulse : U32)}, opened, Some{Stun{x, y, radius, hold}}}
    case True{} False{}:
      Keep{Planted{x, y, location, next}, opened, None{}}

def tick(field: Field, event: Context, parameters: Parameters) -> Step:
  match field event parameters:
    case Fresh{} Tick{+age, +x, +y, +location} Params{radius, duration, hold, pulse}:
      decide((age < duration : U32), True{}, True{}, age, 0, x, y, location, radius, hold, pulse)
    case Planted{+x, +y, +location, +next} Tick{+age, px, py, current} Params{radius, duration, hold, pulse}:
      decide(String.eq(location, current) && (age < duration : U32), (age >= next : U32), False{}, age, next, x, y, location, radius, hold, pulse)
