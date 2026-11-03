import Base
import ./cost.bend as Cost

# Honest small laws about the cost leg. These stay well inside what the checker
# proves; full f64 parity against the native function is the Rust test suite's
# job, and the module builder gives one compiler call four seconds.

# A subscription row answers with exactly the plan fee: paid is zero, nothing is
# cached, the basis is the fee, and the fee is the one handed in. Token counts
# and rates are unreachable from that branch, so this holds for every value.
law subscription_fee_is_the_whole_answer:
  for t: Nat
  for f: Nat
  {Cost.Cost.decide(True{}, True{}, True{}, True{}, t, t, t, True{}, True{}, True{},
    True{}, t, True{}, t, True{}, True{}, True{}, True{}, True{}, True{},
    t, t, t, t, f)
  == Cost.Priced{Cost.Fee{}, 0n, 0n, False{}, f} : Cost.Cost.Decision}

# A reported field with a rate is a priced term even when the token count is
# zero: one output field of zero tokens under an output rate is list-price at
# zero, not unreported usage.
law a_reported_zero_token_field_is_still_a_priced_term:
  {Cost.Cost.decide(False{}, True{}, True{}, True{}, 0n, 0n, 0n, False{}, False{}, False{},
    True{}, 0n, False{}, 0n, True{}, False{}, False{}, False{}, True{}, False{},
    0n, 0n, 200n, 0n, 0n)
  == Cost.Priced{Cost.List{}, 0n, 0n, False{}, 0n} : Cost.Cost.Decision}

# One priced term is the plain product of tokens and cents per million tokens,
# and a field with no rate contributes nothing whatever its count.
law a_priced_term_is_the_plain_product:
  for t: Nat
  for r: Nat
  {Cost.cost_term(True{}, t, r) == (t * r : Nat) : Nat}

law a_missing_rate_never_prices:
  for t: Nat
  {Cost.cost_term(False{}, t, t) == 0n : Nat}
