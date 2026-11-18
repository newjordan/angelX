import Base
import ./seat-order.bend as SeatOrder

law no_links_seat_nothing:
  for slot: U32
  {SeatOrder.order(Con{slot, Nil{}}, Nil{}) == SeatOrder.Pln{Nil{}} : SeatOrder.Plan}

law a_cancelled_stream_never_continues:
  {SeatOrder.advance(0, 4, True{}, False{}, True{}) == SeatOrder.Adv{False{}, 0, True{}} : SeatOrder.Advance}

law a_partial_stream_is_never_replayed:
  {SeatOrder.advance(0, 4, True{}, True{}, False{}) == SeatOrder.Adv{False{}, 0, True{}} : SeatOrder.Advance}

law the_last_streamed_link_never_advances:
  {SeatOrder.advance(3, 4, True{}, False{}, False{}) == SeatOrder.Adv{False{}, 0, False{}} : SeatOrder.Advance}

law a_plain_walk_advances_on_the_first_link:
  {SeatOrder.advance(0, 4, False{}, True{}, True{}) == SeatOrder.Adv{True{}, 1, False{}} : SeatOrder.Advance}
