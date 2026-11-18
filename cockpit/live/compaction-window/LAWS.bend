import Base
import ./compaction-window.bend as Window

law the_counted_tail_stays_out_of_the_window:
  for w0: U32
  for w1: U32
  for w2: U32
  for w3: U32
  for w4: U32
  for w5: U32
  {Window.window(Con{Window.Say{True{}, False{}, w0}, Con{Window.Say{False{}, False{}, w1}, Con{Window.Say{False{}, False{}, w2}, Con{Window.Say{False{}, False{}, w3}, Con{Window.Say{False{}, False{}, w4}, Con{Window.Say{False{}, False{}, w5}, Nil{}}}}}}}, 2, 0) == Window.Chosen{1, 4} : Window.Decision}

law a_boundary_tool_result_joins_the_window:
  for w1: U32
  for w2: U32
  for w3: U32
  {Window.window(Con{Window.Say{True{}, False{}, 0}, Con{Window.Say{False{}, False{}, w1}, Con{Window.Say{False{}, False{}, w2}, Con{Window.Say{False{}, False{}, w3}, Con{Window.Say{False{}, False{}, 5}, Con{Window.Say{False{}, True{}, 7}, Con{Window.Say{False{}, False{}, 3}, Nil{}}}}}}}}, 12, 1) == Window.Chosen{1, 6} : Window.Decision}
