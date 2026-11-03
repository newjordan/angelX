import Base
import ./ossuary.bend as Ossuary

# Four claims about the Bone Dominoes' control flow, fixed to concrete counts:
# a symbolic subtraction would leave `U32.sub` stuck in the checker, so each
# claim pins the numbers it talks about and quantifies the rest.

# An idle line with nobody at the first bone does nothing, whoever else is about.
law an_idle_line_waits:
  for fallen: U32
  for next: U32
  for raised: U32
  {Ossuary.tick(Ossuary.Line{False{}, False{}, fallen, next, 0, raised, Nil{}}, Nil{}) == Ossuary.Out{Ossuary.Line{False{}, False{}, fallen, next, 0, raised, Nil{}}, Nil{}} : Ossuary.Step}

# A stalled line only counts down: five ticks to stand again becomes four, with
# no sound, no banner and no monster, and the fallen bones still lie.
law a_stalled_line_only_counts_down:
  for fallen: U32
  for next: U32
  for raised: U32
  {Ossuary.tick(Ossuary.Line{False{}, False{}, fallen, next, 5, raised, Nil{}}, Nil{}) == Ossuary.Out{Ossuary.Line{False{}, False{}, fallen, next, 4, raised, Nil{}}, Nil{}} : Ossuary.Step}

# The last tick of the stall stands every bone up again.
law the_last_stalled_tick_stands_the_line:
  for next: U32
  for raised: U32
  {Ossuary.tick(Ossuary.Line{False{}, False{}, 20, next, 1, raised, Nil{}}, Nil{}) == Ossuary.Out{Ossuary.Line{False{}, False{}, 0, next, 0, raised, Nil{}}, Nil{}} : Ossuary.Step}

# A nook that has raised its four monsters raises no more, whatever happens.
law a_full_nook_raises_nothing:
  for x: F32
  for y: F32
  {Ossuary.raise_one(4, x, y) == Ossuary.Rose{4, Nil{}} : Ossuary.Raised}
