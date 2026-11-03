import Base
import ./sl.bend as S
import ./level.bend as L
import ./session.bend as N
import ./bot.bend as B

# The Lightgallop rules module: the Moonwell Road and everything that rides it.
#
# The host (index.js) calls three things. `road` lays out the level once. `tick`
# takes the road and the ride's state as an array of slots (sl.bend, with the
# layout in rs.bend and hs.bend) and answers the next state. `start` does the
# same for a fresh ride, and `bot` answers the keys a bot presses.

def road() -> L.Road:
  L.road()

def tick(+rd: L.Road, +st: S.Sl) -> S.Sl:
  N.tick(rd, st)

def start(+rd: L.Road, +st: S.Sl) -> S.Sl:
  N.start(rd, st)

def bot(+rd: L.Road, +st: S.Sl, +id: U32) -> B.Pr:
  B.drive(rd, st, id)
