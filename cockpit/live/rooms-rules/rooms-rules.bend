# Rooms drawn by hand, a pack at a time: the dungeon masters' layouts, laid
# beside the packs' own templates and in place of the pillared grid for the
# bigger rooms a party is given. Moved here from `crates/delve/src/rules/rooms/`,
# art and selection together, so a pack's rooms can be re-authored while the
# host runs; Rust keeps the types and the rendering hooks and asks this module
# once per pack (and once per great-room size) instead of holding the tables.
#
# Ops (the adapter marshals ids to indices; only the art rows are text):
#   small{pack}            the pack's one-screen quadrants, 11 by 6
#   great{pack}            every bigger quadrant the pack has drawn
#   sized{pack,w,h}        only those drawn for that size, in authored order
#   consts                 the shared sizes, the authored share and the least floor
#
# `sizes` lists the quadrant sizes of the bigger rooms (interior halves less
# the doorway lanes): a party's rooms (34x20, 42x24, 48x28) and the great
# halls the packs lay (32x20, 36x20, 40x24). `share` is how often, in percent,
# a bigger room of a size someone has drawn is one of theirs rather than the
# pillared grid. `least_floor` is the least share of a room's interior, in
# percent, left as floor; the native adapter checks every answer against it
# where a room can be laid, and the oracle test checks the rest.
import Base
import ./types.bend as T
import ./crypt.bend as Crypt
import ./cavern.bend as Cavern
import ./hellforge.bend as Hellforge
import ./archive.bend as Archive
import ./fungal.bend as Fungal
import ./unknown.bend as Unknown
import ./swamp.bend as Swamp

def small(pack: U32) -> T.Art:
  match pack:
    case 0:
      T.Art{Crypt.small()}
    case 1:
      T.Art{Cavern.small()}
    case 2:
      T.Art{Hellforge.small()}
    case 3:
      T.Art{Archive.small()}
    case 4:
      T.Art{Fungal.small()}
    case 5:
      T.Art{Unknown.small()}
    case 6:
      T.Art{Swamp.small()}
    case _:
      T.Art{Nil{}}

# Every bigger quadrant of `pack`, of any size, in authored order.
def great_quads(pack: U32) -> List<&2, T.Quad>:
  match pack:
    case 0:
      Crypt.great()
    case 1:
      Cavern.great()
    case 2:
      Hellforge.great()
    case 3:
      Archive.great()
    case 4:
      Fungal.great()
    case 5:
      Unknown.great()
    case 6:
      Swamp.great()
    case _:
      Nil{}

def great(pack: U32) -> T.Art:
  T.Art{great_quads(pack)}

def keep(+ok: Bool, +q: T.Quad, +acc: List<&2, T.Quad>) -> List<&2, T.Quad>:
  match ok:
    case True{}:
      List.append(&2, T.Quad, acc, [q])
    case False{}:
      acc

def only_size(+qs: List<&2, T.Quad>, +w: U32, +h: U32, +acc: List<&2, T.Quad>) -> List<&2, T.Quad>:
  match qs:
    case Nil{}:
      acc
    case Con{+head, tail}:
      match head:
        case T.Quad{qw, qh, _}:
          only_size(tail, w, h, keep(Bool.and(U32.is_eq(qw, w), U32.is_eq(qh, h)), head, acc))

# The quadrants of `pack` drawn for a room laid at this size.
def sized(pack: U32, w: U32, h: U32) -> T.Art:
  T.Art{only_size(great_quads(pack), w, h, [])}

def consts() -> T.Consts:
  T.Consts{[T.Size{15, 9}, T.Size{16, 9}, T.Size{17, 9}, T.Size{19, 11}, T.Size{20, 11}, T.Size{23, 13}], 75, 55}
