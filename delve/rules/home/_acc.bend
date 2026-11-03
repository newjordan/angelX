# What `Bonus::within` carries while it walks a deck: the summed found words,
# the wishes' numbers by word, the tightest volley, and the card ids seen.
type BonusAcc is Data:
  BonusAcc{sums: List<&2, F32>, wished: List<&2, List<&2, F32>>, volley: U32, seen: List<&2, String>}
