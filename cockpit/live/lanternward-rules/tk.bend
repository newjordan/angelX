# One tick of the adventure, in the order the Rust session took it: whoever is here
# and awake sits at the table (a knight who leaves stands up, one who joins late
# sits down at the room's start and scales what is standing), then every body acts
# in its order. crates/delve/src/rules/lanternward.rs `Session::step` and `new`.
import Base
import ./r.bend as R
import ./c.bend as C
import ./m.bend as M
import ./l.bend as L
import ./gd.bend as Gd
import ./gr.bend as Gr
import ./w.bend as W
import ./ix.bend as Ix
import ./ke.bend as Ke
import ./se.bend as Seat
import ./q.bend as Q
import ./ht.bend as H
import ./fo.bend as Fo
import ./bs.bend as Bs
import ./kt.bend as Kt
import ./sg.bend as Sg

def S() -> Data: R.W

# ----- the table ----------------------------------------------------------------------------------------

# A knight sits down at its start: full hearts for the party's containers, the
# lantern's flame if the lantern is found, and the gems and bombs it carried when it
# stood up.
def seat_new(+id: U32, +joined: U32, +x: U32, +y: U32) -> R.T8:
  +k = Seat.set_safey(Seat.set_safex(Seat.set_camy(Seat.set_camx(Seat.set_y(Seat.set_x(Seat.set_joined(Seat.set_id(L.zero64(), id), joined), x), y), x), y), x), y)
  Seat.set_inv(Seat.set_max_hp(Seat.set_hp(Seat.set_fy(k, 256), C.start_hp()), C.start_hp()), C.spawn_inv())

# The record of what a seat carried when it stood up, by seat: this one's, or
# zeros.
def kept_of(+ks: List<&2, R.W8>, +id: U32) -> R.W8:
  match ks:
    case Nil{}:
      R.W8{id, 0, 0, 0, 0, 0, 0, 0}
    case Con{k, t}:
      Bool.pick(R.W8, U32.is_eq(Ke.id(k), id), k, kept_of(t, id))

def kept_drop(+ks: List<&2, R.W8>, +id: U32) -> List<&2, R.W8>:
  match ks:
    case Nil{}:
      Nil{}
    case Con{k, t}:
      Bool.pick(List<&2, R.W8>, U32.is_eq(Ke.id(k), id), t, Con{k, kept_drop(t, id)})

# A record put in its place by seat, replacing one with the same seat.
def kept_put(+ks: List<&2, R.W8>, +r: R.W8) -> List<&2, R.W8>:
  match ks:
    case Nil{}:
      Con{r, Nil{}}
    case Con{k, t}:
      Bool.pick(List<&2, R.W8>, U32.is_lt(Ke.id(r), Ke.id(k)), Con{r, Con{k, t}}, Bool.pick(List<&2, R.W8>, U32.is_eq(Ke.id(r), Ke.id(k)), Con{r, t}, Con{k, kept_put(t, r)}))

# Seats that left the table are kept, by seat, for when they come back.
def stand_go(+ks: List<&2, R.T8>, +here: List<&2, U32>, +kept: List<&2, R.W8>) -> List<&2, R.W8>:
  match ks:
    case Nil{}:
      kept
    case Con{k, t}:
      stand_go(t, here, Bool.pick(List<&2, R.W8>, L.has(here, Seat.id(k)), kept, kept_put(kept, R.W8{Seat.id(k), Seat.gems(k), Seat.bombs(k), 0, 0, 0, 0, 0})))

def stay_go(+ks: List<&2, R.T8>, +here: List<&2, U32>) -> List<&2, R.T8>:
  match ks:
    case Nil{}:
      Nil{}
    case Con{k, t}:
      Bool.pick(List<&2, R.T8>, L.has(here, Seat.id(k)), Con{k, stay_go(t, here)}, stay_go(t, here))

def sit_one(+seated: Bool, +w: S, +id: U32) -> S:
  match seated:
    case True{}:
      w
    case False{}:
      +r = kept_of(W.kept(w), id)
      +k = seat_new(id, W.elapsed(w), Gr.start_x(W.room(w), id), Gr.start_y(W.room(w), id))
      +k1 = Seat.set_bombs(Seat.set_gems(k, Ke.gems(r)), Ke.bombs(r))
      +mx = U32.add(C.start_hp(), U32.mul(C.heart(), W.p_cont(w)))
      +k2 = Seat.set_hp(Seat.set_max_hp(k1, mx), mx)
      +k3 = Bool.pick(R.T8, U32.is_ne(W.p_lantern(w), 0), Seat.set_flame(k2, W.r_flame(w)), k2)
      W.set_seats(W.set_kept(w, kept_drop(W.kept(w), id)), H.seat_insert(W.seats(w), k3))

# Each knight who is here and has no seat sits down in its place.
def sit_go(+ids: List<&2, U32>, +w: S) -> S:
  match ids:
    case Nil{}:
      w
    case Con{id, t}:
      sit_go(t, sit_one(H.seat_has(W.seats(w), id), w, id))

# Whoever is not here stands up and is kept; whoever is, and has no seat, sits down.
def seat_table(+w: S) -> S:
  +here = W.here(w)
  +w1 = W.set_kept(w, stand_go(W.seats(w), here, W.kept(w)))
  sit_go(here, W.set_seats(w1, stay_go(W.seats(w1), here)))

# ----- the phases -----------------------------------------------------------------------------------------

# A seat's input: its row, or nothing pressed.
def ix_of(+rows: List<&2, R.W8>, +id: U32) -> R.W8:
  match rows:
    case Nil{}:
      R.W8{id, 0, 0, 0, 0, 0, 0, 0}
    case Con{r, t}:
      Bool.pick(R.W8, U32.is_eq(Ix.id(r), id), r, ix_of(t, id))

def seats_go(+ids: List<&2, U32>, +w: S) -> S:
  match ids:
    case Nil{}:
      w
    case Con{id, t}:
      seats_go(t, Kt.step_seat(w, id, ix_of(W.ins(w), id)))

def seat_ids(+ks: List<&2, R.T8>) -> List<&2, U32>:
  match ks:
    case Nil{}:
      Nil{}
    case Con{k, t}:
      Con{Seat.id(k), seat_ids(t)}

# The first phase: the clock, the table, the grid of the room.
def phase_join(+w: S) -> S:
  +w1 = W.set_elapsed(w, U32.min(U32.add(W.elapsed(w), 1), C.limit()))
  Fo.provision(seat_table(W.set_grid(w1, Gd.of(W.room(w1)))))

# Every body acts in its order.
def phase(+n: Nat, +w: S) -> S:
  match n:
    case 0n:
      phase_join(w)
    case 1n+a:
      match a:
        case 0n:
          seats_go(seat_ids(W.seats(w)), w)
        case 1n+b:
          match b:
            case 0n:
              Sg.tick_props(w)
            case 1n+c:
              match c:
                case 0n:
                  Sg.unstick(w)
                case 1n+d:
                  match d:
                    case 0n:
                      Fo.step_foes(w)
                    case 1n+e:
                      match e:
                        case 0n:
                          Bs.step_boss(w)
                        case 1n+f:
                          match f:
                            case 0n:
                              H.step_bolts(w)
                            case 1n+g:
                              match g:
                                case 0n:
                                  H.step_fuses(w)
                                case 1n+h:
                                  match h:
                                    case 0n:
                                      Sg.pickups(w)
                                    case 1n+i:
                                      match i:
                                        case 0n:
                                          Sg.revive_pass(w)
                                        case 1n+j:
                                          match j:
                                            case 0n:
                                              Sg.tick_stage(w)
                                            case 1n+k:
                                              match k:
                                                case 0n:
                                                  Sg.tick_wipe(w)
                                                case 1n+l:
                                                  Sg.age(w)

# The first `n` phases of a tick, in order.
def upto(+n: Nat, +w: S) -> S:
  match n:
    case 0n:
      w
    case 1n+p:
      phase(p, upto(p, w))

# One tick.
def tick(+w: S) -> S: upto(13n, w)

# ----- a session begins ------------------------------------------------------------------------------------------

# Everyone here sits at the table at the first room's start, then the room opens.
def sit_all(+ids: List<&2, U32>, +ks: List<&2, R.T8>) -> List<&2, R.T8>:
  match ids:
    case Nil{}:
      ks
    case Con{id, t}:
      sit_all(t, H.seat_insert(ks, seat_new(id, 0, Gr.start_x(0, id), Gr.start_y(0, id))))

def new_session(+w: S) -> S:
  +w1 = W.set_seed_lo(W.set_seed_hi(W.set_seats(w, sit_all(W.here(w), Nil{})), 1279348308), 1163021911)
  Sg.enter_room(w1, 0)
