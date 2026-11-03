# The core group's consts that the generator (consts.py) could not evaluate: references across
# modules, tuples, arrays.

def wishcraft.CHILL_FOE() -> U32:
  lich.CHILL()

def wishcraft.CHILL_PACE() -> F32:
  lich.CHILL_PACE()

# fortune.rs: the lever, plate, coffer and stand rectangles as (col, row, w, h), and Fortune herself.
def fortune.FORTUNE_AT_X() -> F32:
  7.6

def fortune.FORTUNE_AT_Y() -> F32:
  7.2

def trophies.KAY_AT_X() -> F32:
  8.6

def trophies.KAY_AT_Y() -> F32:
  2.6

def swamp.NOTICE_X() -> F32:
  15.8

def swamp.NOTICE_Y() -> F32:
  8.0
