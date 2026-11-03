import Base
import ./compaction-window.bend as Window

# The compacted window always leaves the recent tail live. These claims fix the
# tails they talk about — a symbolic `keep_recent` would leave `U32.sub` stuck
# in the checker — and quantify the message token estimates instead, which the
# count-based tail does not read at all.

# One pinned preamble and five ordinary messages, keep two: the window is
# [1, 4), so the last two messages (indices 4 and 5) are never compacted
# whatever those messages cost.
law the_counted_tail_stays_out_of_the_window:
  for w0: U32
  for w1: U32
  for w2: U32
  for w3: U32
  for w4: U32
  for w5: U32
  {Window.window(Con{Window.Say{True{}, False{}, w0}, Con{Window.Say{False{}, False{}, w1}, Con{Window.Say{False{}, False{}, w2}, Con{Window.Say{False{}, False{}, w3}, Con{Window.Say{False{}, False{}, w4}, Con{Window.Say{False{}, False{}, w5}, Nil{}}}}}}}, 2, 0) == Window.Chosen{1, 4} : Window.Decision}

# A tool result sitting exactly on the token-tail boundary is pulled into the
# window with its call, however the three older messages are priced.
law a_boundary_tool_result_joins_the_window:
  for w1: U32
  for w2: U32
  for w3: U32
  {Window.window(Con{Window.Say{True{}, False{}, 0}, Con{Window.Say{False{}, False{}, w1}, Con{Window.Say{False{}, False{}, w2}, Con{Window.Say{False{}, False{}, w3}, Con{Window.Say{False{}, False{}, 5}, Con{Window.Say{False{}, True{}, 7}, Con{Window.Say{False{}, False{}, 3}, Nil{}}}}}}}}, 12, 1) == Window.Chosen{1, 6} : Window.Decision}
