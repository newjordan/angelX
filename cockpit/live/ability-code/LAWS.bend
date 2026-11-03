import Base
import ./fairies.bend as Fairies

# A stopped field cannot issue a status effect. These claims stay separate
# from authored policy and are checked before a candidate is admitted.
law inactive_field_ends:
  for due: Bool
  for opened: Bool
  for age: U32
  for next: U32
  for x: F32
  for y: F32
  for location: String
  for radius: F32
  for hold: U32
  for pulse: U32
  {Fairies.decide(False{}, due, opened, age, next, x, y, location, radius, hold, pulse) == Fairies.End{} : Fairies.Step}

law zero_lifetime_ends:
  for x: F32
  for y: F32
  for location: String
  for radius: F32
  for hold: U32
  for pulse: U32
  {Fairies.tick(Fairies.Fresh{}, Fairies.Tick{0, x, y, location}, Fairies.Params{radius, 0, hold, pulse}) == Fairies.End{} : Fairies.Step}
