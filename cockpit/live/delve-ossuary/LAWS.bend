import Base
import ./ossuary.bend as Ossuary

law an_idle_line_waits:
  for fallen: U32
  for next: U32
  for raised: U32
  {Ossuary.tick(Ossuary.Line{False{}, False{}, fallen, next, 0, raised, Nil{}}, Nil{}) == Ossuary.Out{Ossuary.Line{False{}, False{}, fallen, next, 0, raised, Nil{}}, Nil{}} : Ossuary.Step}

law a_stalled_line_only_counts_down:
  for fallen: U32
  for next: U32
  for raised: U32
  {Ossuary.tick(Ossuary.Line{False{}, False{}, fallen, next, 5, raised, Nil{}}, Nil{}) == Ossuary.Out{Ossuary.Line{False{}, False{}, fallen, next, 4, raised, Nil{}}, Nil{}} : Ossuary.Step}

law the_last_stalled_tick_stands_the_line:
  for next: U32
  for raised: U32
  {Ossuary.tick(Ossuary.Line{False{}, False{}, 20, next, 1, raised, Nil{}}, Nil{}) == Ossuary.Out{Ossuary.Line{False{}, False{}, 0, next, 0, raised, Nil{}}, Nil{}} : Ossuary.Step}

law a_full_nook_raises_nothing:
  for x: F32
  for y: F32
  {Ossuary.raise_one(4, x, y) == Ossuary.Rose{4, Nil{}} : Ossuary.Raised}
