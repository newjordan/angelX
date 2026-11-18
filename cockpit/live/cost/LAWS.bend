import Base
import ./cost.bend as Cost

law subscription_fee_is_the_whole_answer:
  for t: Nat
  for f: Nat
  {Cost.Cost.decide(True{}, True{}, True{}, True{}, t, t, t, True{}, True{}, True{},
    True{}, t, True{}, t, True{}, True{}, True{}, True{}, True{}, True{},
    t, t, t, t, f)
  == Cost.Priced{Cost.Fee{}, 0n, 0n, False{}, f} : Cost.Cost.Decision}

law a_reported_zero_token_field_is_still_a_priced_term:
  {Cost.Cost.decide(False{}, True{}, True{}, True{}, 0n, 0n, 0n, False{}, False{}, False{},
    True{}, 0n, False{}, 0n, True{}, False{}, False{}, False{}, True{}, False{},
    0n, 0n, 200n, 0n, 0n)
  == Cost.Priced{Cost.List{}, 0n, 0n, False{}, 0n} : Cost.Cost.Decision}

law a_priced_term_is_the_plain_product:
  for t: Nat
  for r: Nat
  {Cost.cost_term(True{}, t, r) == (t * r : Nat) : Nat}

law a_missing_rate_never_prices:
  for t: Nat
  {Cost.cost_term(False{}, t, t) == 0n : Nat}
