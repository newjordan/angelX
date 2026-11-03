# Lanternward's rules, as Bend. This entry is what the host's engine calls: one
# tick of the adventure (the knights' inputs and the session in, the session after
# it out), a session at the first room's start, and the knights' bot. The
# marshalling that turns the session's JSON into the words these rules hold lives
# in index.js; the rules themselves are in the files this imports, in the order the
# Rust session took them.
import Base
import ./r.bend as R
import ./tk.bend as T

# One tick of the adventure: the world in, the world after it out.
def tick(+w: R.W) -> R.W: T.tick(w)

# The first n phases of a tick, to find where two runs part.
def upto(+n: Nat, +w: R.W) -> R.W: T.upto(n, w)

# A session at the first room's start for the knights listed in the world.
def new_session(+w: R.W) -> R.W: T.new_session(w)
