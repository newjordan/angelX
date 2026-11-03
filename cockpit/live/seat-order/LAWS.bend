import Base
import ./seat-order.bend as SeatOrder

# Five small claims about the failover step and the seating. All of them are
# about control flow only: no arithmetic, no scoring, nothing the checker would
# have to axiomatize.

# With no candidate links there is nothing to seat, whatever the order asks for.
law no_links_seat_nothing:
  for slot: U32
  {SeatOrder.order(Con{slot, Nil{}}, Nil{}) == SeatOrder.Pln{Nil{}} : SeatOrder.Plan}

# A cancelled stream is never routed around: the step stops and the caller
# surfaces that attempt's own error instead of trying the neighbour.
law a_cancelled_stream_never_continues:
  {SeatOrder.advance(0, 4, True{}, False{}, True{}) == SeatOrder.Adv{False{}, 0, True{}} : SeatOrder.Advance}

# A partially streamed reply cannot be replayed onto the next link either.
law a_partial_stream_is_never_replayed:
  {SeatOrder.advance(0, 4, True{}, True{}, False{}) == SeatOrder.Adv{False{}, 0, True{}} : SeatOrder.Advance}

# From the last position of a four-link plan there is no next link left.
law the_last_streamed_link_never_advances:
  {SeatOrder.advance(3, 4, True{}, False{}, False{}) == SeatOrder.Adv{False{}, 0, False{}} : SeatOrder.Advance}

# The plain (non-streaming) walk has no partial reply to protect, so it always
# advances to the next link while one is left.
law a_plain_walk_advances_on_the_first_link:
  {SeatOrder.advance(0, 4, False{}, True{}, True{}) == SeatOrder.Adv{True{}, 1, False{}} : SeatOrder.Advance}
