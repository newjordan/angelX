import Base
import ./types.bend as T
import ./rooms-rules.bend as Rooms

# The claims below are about the authored tables and the size query only; the
# drawing (mirroring, doorway reach, the floor share) is checked natively where
# a room can be laid, and by the oracle test beside the cockpit's rules tests.

def quads(+art: T.Art) -> List<&2, T.Quad>:
  match art:
    case T.Art{quads}:
      quads

def drawn(+qs: List<&2, T.Quad>) -> Bool:
  match qs:
    case Nil{}:
      False{}
    case Con{_, _}:
      True{}

def at_size(+qs: List<&2, T.Quad>, +w: U32, +h: U32) -> Bool:
  match qs:
    case Nil{}:
      True{}
    case Con{+head, tail}:
      match head:
        case T.Quad{qw, qh, _}:
          Bool.and(Bool.and(U32.is_eq(qw, w), U32.is_eq(qh, h)), at_size(tail, w, h))

# The Swamp's mire is laid over the packs' own grid, so it draws no rooms.
law the_swamp_has_drawn_no_one_screen_rooms:
  {Rooms.small(6) == T.Art{Nil{}} : T.Art}

# A size no dungeon master drew falls through to the pillared grid.
law a_size_nobody_drew_answers_nothing:
  {Rooms.sized(0, 1, 1) == T.Art{Nil{}} : T.Art}

# The Crypt really has halls at the first size, and a query at one size never
# answers a quadrant drawn at another.
law the_crypt_has_halls_at_the_first_size:
  {drawn(quads(Rooms.sized(0, 15, 9))) == True{} : Bool}

law a_sized_answer_only_holds_its_own_size:
  {at_size(quads(Rooms.sized(0, 15, 9)), 15, 9) == True{} : Bool}
