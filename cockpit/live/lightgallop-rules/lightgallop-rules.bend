import Base
import ./sl.bend as S
import ./level.bend as L
import ./session.bend as N
import ./bot.bend as B

def road() -> L.Road:
  L.road()

def tick(+rd: L.Road, +st: S.Sl) -> S.Sl:
  N.tick(rd, st)

def start(+rd: L.Road, +st: S.Sl) -> S.Sl:
  N.start(rd, st)

def bot(+rd: L.Road, +st: S.Sl, +id: U32) -> B.Pr:
  B.drive(rd, st, id)
