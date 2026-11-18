def griffinwing.rail.LEN(+i: U32) -> U32:
  Bool.pick(U32, U32.is_eq(i, 0), 1800, 2400)

def griffinwing.rail.BOSS_AT() -> U32:
  480

def griffinwing.rail.ORIGIN(+i: U32) -> U32:
  Bool.pick(U32, U32.is_eq(i, 0), 0, Bool.pick(U32, U32.is_eq(i, 1), (1800 * griffinwing.types.PACE() : U32), (4200 * griffinwing.types.PACE() : U32)))

def griffinwing.rail.beat_of(+s: U32) -> U32:
  Bool.pick(U32, gw.ge(s, griffinwing.rail.ORIGIN(2)), 2, Bool.pick(U32, gw.ge(s, griffinwing.rail.ORIGIN(1)), 1, 0))

def griffinwing.rail.FIGHT_LEN() -> U32:
  (3 * 60 * 30 : U32)
