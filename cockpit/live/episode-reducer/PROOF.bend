import Base
import ./LAWS.bend as Laws

# Each law is a closed equality between literal decisions, so its proof only
# re-runs the same state machine: no arithmetic is assumed and nothing is
# claimed beyond the twelve literal cases above.

def Laws.a_durable_append_advances_the_recorded_sequence():
  {==}

def Laws.a_closed_episode_grants_no_effect():
  {==}

def Laws.an_episode_without_a_start_has_no_permit():
  {==}

def Laws.a_body_before_the_start_is_refused():
  {==}

def Laws.the_last_sequence_cannot_be_advanced():
  {==}

def Laws.a_low_half_carry_moves_the_high_half():
  {==}

def Laws.a_backward_terminal_range_is_refused():
  {==}

def Laws.the_terminal_range_may_end_where_it_began():
  {==}

def Laws.a_completion_never_precedes_the_start():
  {==}

def Laws.a_completion_at_the_start_instant_completes():
  {==}

def Laws.a_receipt_after_sequence_zero_is_refused():
  {==}

def Laws.a_closed_episode_admits_official_evidence_only():
  {==}

def Laws.a_closed_episode_effect_is_refused():
  {==}

def Laws.an_open_episode_admits_unofficial_evidence():
  {==}
