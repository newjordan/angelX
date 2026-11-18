import Base
import ./episode-reducer.bend as Reducer

law a_durable_append_advances_the_recorded_sequence:
  {Reducer.durable(Reducer.St{True{}, False{}, 0, 7, 0, 2}, Reducer.Ev{Reducer.KEvidence{}, True{}, True{}, True{}, True{}, True{}, False{}, True{}, True{}, False{}, True{}, 0, 12}) == Reducer.Allow{Reducer.PNone{}, 0, 8, False{}} : Reducer.Decision}

law a_closed_episode_grants_no_effect:
  {Reducer.effect(Reducer.St{True{}, True{}, 0, 9, 0, 2}) == Reducer.Deny{Reducer.EpisodeClosed{}} : Reducer.Decision}

law an_episode_without_a_start_has_no_permit:
  {Reducer.effect(Reducer.St{False{}, False{}, 0, 0, 0, 0}) == Reducer.Deny{Reducer.NeedsStart{}} : Reducer.Decision}

law a_body_before_the_start_is_refused:
  {Reducer.validate(Reducer.St{False{}, False{}, 0, 0, 0, 0}, Reducer.Ev{Reducer.KTerminal{}, True{}, True{}, True{}, True{}, True{}, False{}, True{}, True{}, False{}, True{}, 0, 0}) == Reducer.Deny{Reducer.StartFirst{}} : Reducer.Decision}

law the_last_sequence_cannot_be_advanced:
  {Reducer.durable(Reducer.St{True{}, False{}, 4294967295, 4294967295, 0, 0}, Reducer.Ev{Reducer.KEvidence{}, True{}, True{}, True{}, True{}, True{}, False{}, True{}, True{}, False{}, True{}, 0, 0}) == Reducer.Deny{Reducer.SequenceExhausted{}} : Reducer.Decision}

law a_low_half_carry_moves_the_high_half:
  {Reducer.durable(Reducer.St{True{}, False{}, 0, 4294967295, 0, 0}, Reducer.Ev{Reducer.KEvidence{}, True{}, True{}, True{}, True{}, True{}, False{}, True{}, True{}, False{}, True{}, 0, 0}) == Reducer.Allow{Reducer.PNone{}, 1, 0, False{}} : Reducer.Decision}

law a_backward_terminal_range_is_refused:
  {Reducer.validate(Reducer.St{True{}, False{}, 0, 5, 0, 9}, Reducer.Ev{Reducer.KTerminal{}, True{}, True{}, True{}, True{}, True{}, False{}, True{}, True{}, False{}, True{}, 0, 8}) == Reducer.Deny{Reducer.RangeBackward{}} : Reducer.Decision}

law the_terminal_range_may_end_where_it_began:
  {Reducer.validate(Reducer.St{True{}, False{}, 0, 5, 0, 9}, Reducer.Ev{Reducer.KTerminal{}, True{}, True{}, True{}, True{}, True{}, False{}, True{}, True{}, False{}, True{}, 0, 9}) == Reducer.Allow{Reducer.PClosed{}, 0, 0, False{}} : Reducer.Decision}

law a_completion_never_precedes_the_start:
  {Reducer.complete(Reducer.Rc{True{}, 0, 0, True{}, True{}, 0, 6, 0, 8}) == Reducer.Deny{Reducer.ReceiptMismatch{}} : Reducer.Decision}

law a_completion_at_the_start_instant_completes:
  {Reducer.complete(Reducer.Rc{True{}, 0, 0, True{}, True{}, 0, 8, 0, 8}) == Reducer.Allow{Reducer.PCompleted{}, 0, 0, False{}} : Reducer.Decision}

law a_receipt_after_sequence_zero_is_refused:
  {Reducer.complete(Reducer.Rc{True{}, 0, 1, True{}, True{}, 0, 9, 0, 8}) == Reducer.Deny{Reducer.ReceiptMismatch{}} : Reducer.Decision}

law a_closed_episode_admits_official_evidence_only:
  {Reducer.link(Reducer.St{True{}, True{}, 0, 5, 0, 2}, Reducer.Ev{Reducer.KEvidence{}, True{}, True{}, True{}, True{}, True{}, False{}, True{}, True{}, False{}, True{}, 0, 0}) == Reducer.Deny{Reducer.NotOfficial{}} : Reducer.Decision}

law an_open_episode_admits_unofficial_evidence:
  {Reducer.link(Reducer.St{True{}, False{}, 0, 5, 0, 2}, Reducer.Ev{Reducer.KEvidence{}, True{}, True{}, True{}, True{}, True{}, False{}, True{}, True{}, False{}, True{}, 0, 0}) == Reducer.Allow{Reducer.PNone{}, 0, 0, False{}} : Reducer.Decision}

law a_closed_episode_effect_is_refused:
  {Reducer.validate(Reducer.St{True{}, True{}, 0, 5, 0, 2}, Reducer.Ev{Reducer.KEvidence{}, True{}, True{}, True{}, True{}, True{}, False{}, True{}, True{}, False{}, True{}, 0, 0}) == Reducer.Deny{Reducer.TerminalEffects{}} : Reducer.Decision}
