import Base
import ./r.bend as R
import ./tk.bend as T

def tick(+w: R.W) -> R.W: T.tick(w)

def upto(+n: Nat, +w: R.W) -> R.W: T.upto(n, w)

def new_session(+w: R.W) -> R.W: T.new_session(w)
