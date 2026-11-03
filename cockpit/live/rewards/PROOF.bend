import Base
import ./LAWS.bend as Laws

# Each law is a closed equality between literal decisions, so its proof only
# re-runs the same scans: no arithmetic is assumed and nothing is claimed beyond
# the five ledgers above.

def Laws.an_empty_ledger_selects_nothing():
  {==}

def Laws.a_binding_outranks_a_shaping_event():
  {==}

def Laws.the_newest_shaping_event_wins():
  {==}

def Laws.an_empty_shaping_ledger_appends():
  {==}

def Laws.a_same_id_row_that_differs_is_a_conflict():
  {==}
