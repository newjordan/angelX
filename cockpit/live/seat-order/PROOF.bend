import Base
import ./LAWS.bend as Laws

# The laws are closed statements; each proof only has to discharge the one
# quantified slot, and the verdict itself is the statement.
def Laws.no_links_seat_nothing(slot):
  {==}

def Laws.a_cancelled_stream_never_continues():
  {==}

def Laws.a_partial_stream_is_never_replayed():
  {==}

def Laws.the_last_streamed_link_never_advances():
  {==}

def Laws.a_plain_walk_advances_on_the_first_link():
  {==}
