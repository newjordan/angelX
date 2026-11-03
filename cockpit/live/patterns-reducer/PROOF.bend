import Base
import ./LAWS.bend as Laws

# Each law is a closed equality between literal counters, so its proof only
# re-runs the same fold: no ids, no ordering and no host behaviour is claimed.

def Laws.an_operational_tie_counts_only_its_two_counters():
  {==}

def Laws.a_full_low_half_carries_into_the_high_half():
  {==}

def Laws.an_exhausted_u64_refuses_instead_of_wrapping():
  {==}
