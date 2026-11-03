import Base
import ./LAWS.bend as Laws

# Each claim computes to the same term on both sides once the control flow is
# followed, so reflexivity discharges them; the verdict itself is the statement.
def Laws.an_idle_line_waits(fallen, next, raised):
  {==}

def Laws.a_stalled_line_only_counts_down(fallen, next, raised):
  {==}

def Laws.the_last_stalled_tick_stands_the_line(next, raised):
  {==}

def Laws.a_full_nook_raises_nothing(x, y):
  {==}
