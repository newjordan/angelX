import Base
import ./math.bend as M
import ./sl.bend as S
import ./hs.bend as H
import ./rs.bend as R
import ./terrain.bend as T
import ./road.bend as O
import ./level.bend as L
import ./rider.bend as D

def foe_hp(+st: S.Sl, +i: U32) -> U32:
  S.g(st, (288 + i * 4 : U32))

def foe_pop(+st: S.Sl, +i: U32) -> U32:
  S.g(st, (289 + i * 4 : U32))

def foe_flinch(+st: S.Sl, +i: U32) -> U32:
  S.g(st, (290 + i * 4 : U32))

def set_foe(+st: S.Sl, +i: U32, +hp: U32, +pop: U32, +flinch: U32) -> S.Sl:
  S.p(S.p(S.p(st, (288 + i * 4 : U32), hp), (289 + i * 4 : U32), pop), (290 + i * 4 : U32), flinch)

def fs_kind(+f: L.Fs) -> U32:
  match f:
    case L.Fs{kind, x, y, path, range, min_seats, phase}:
      kind

def fs_x(+f: L.Fs) -> U32:
  match f:
    case L.Fs{kind, x, y, path, range, min_seats, phase}:
      x

def fs_min_seats(+f: L.Fs) -> U32:
  match f:
    case L.Fs{kind, x, y, path, range, min_seats, phase}:
      min_seats

def body_w(+kind: U32) -> U32:
  M.pk(9, M.pk(9, M.pk(12, 22, M.eq(kind, 2)), M.eq(kind, 1)), M.eq(kind, 0))

def body_h(+kind: U32) -> U32:
  M.pk(20, M.pk(18, M.pk(14, 52, M.eq(kind, 2)), M.eq(kind, 1)), M.eq(kind, 0))

def foe_x(+rd: L.Road, +f: L.Fs, +clock: U32) -> U32:
  match f:
    case L.Fs{kind, x, y, path, range, min_seats, phase}:
      +t = (clock + phase : U32)
      +beat = M.max(M.div((range * 10 : U32), 3), 2)
      +ph = U32.mod(t, (beat * 2 : U32))
      +along = M.pk(ph, (beat * 2 - ph : U32), (ph < beat : U32))
      +snuffer = (x - range + M.div((along * 3 : U32), 5) : U32)
      +drift = M.md(M.sin(U32.mod(U32.div((t * 3 : U32), 2), 256)), range, 4096)
      M.pk(snuffer, M.pk((x + drift : U32), x, M.eq(kind, 1)), M.eq(kind, 0))

def foe_y(+rd: L.Road, +f: L.Fs, +clock: U32, +fx: U32) -> U32:
  match f:
    case L.Fs{kind, x, y, path, range, min_seats, phase}:
      +t = (clock + phase : U32)
      +bob = M.md(M.sin(U32.mod((t * 3 : U32), 256)), 5, 4096)
      +walk = M.div(T.y_at_x(T.path_at(O.ro_paths(rd), path), fx), 256)
      M.pk(walk, M.pk((y + bob : U32), y, M.eq(kind, 1)), M.eq(kind, 0))
