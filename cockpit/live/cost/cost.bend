import Base

# The `cost` decision leg. The adapter and its Rust feature builder do all the
# text work (price-table parsing, model/row matching, the reasoning-convention
# probe). It hands over plain counts, presence flags and integer rates; this
# module owns every gate, every fallback and all of the arithmetic the native
# decision makes.
#
# Units: a rate crosses as whole cents per million tokens (`rate * 100`), so one
# term is exactly `tokens * rate_cents` in 1e-8 dollars. A card with two decimal
# places therefore needs no division and no rounding at all: the native sequence
# `round(tokens * rate / 1e6 * 1e8)` is `tokens * rate_cents`, an integer both
# sides agree on. The Rust builder keeps the two cases this cannot carry (a rate
# finer than a cent, or products past a Bend Nat's 2^48-1 immediate) on the
# native decision rather than guessing.
#
# Presence, not value: a reported field with a rate is a priced term even when
# its token count is zero (`{output: 0}` under an output rate prices 0 and the
# record's basis is list-price). Every gate below tests a presence flag, never
# `tokens > 0`.

type Cost.Basis is Data:
  Fee{}
  List{}
  NoRow{}
  Unreported{}

type Cost.Decision is Data:
  Priced{basis: Cost.Basis, paid_e8: Nat, cached_e8: Nat, has_cached: Bool, fee_e8: Nat}
  Blank{basis: Cost.Basis}

def cost_pick_nat(+c: Bool, +t: Nat, +f: Nat) -> Nat:
  Bool.pick(Nat, c, t, f)

# One priced term in e8 dollars: `tokens * rate_cents` when both sides are
# reported, else 0.
def cost_term(+present: Bool, +tokens: Nat, +rate_cents: Nat) -> Nat:
  match present:
    case True{}:
      (tokens * rate_cents : Nat)
    case False{}:
      0n

# The cached share is unambiguous when a separate uncached total exists, or
# under the OpenAI convention where the input total includes cached tokens.
def cost_use_cache(+has_cache_read: Bool, +has_uncached: Bool, +cached_le_input: Bool) -> Bool:
  (has_cache_read && (has_uncached || cached_le_input) : Bool)

def cost_cached_present(
  +has_cache_read: Bool, +has_uncached: Bool, +cached_le_input: Bool, +rate_cached_p: Bool
) -> Bool:
  (cost_use_cache(has_cache_read, has_uncached, cached_le_input) && rate_cached_p : Bool)

def cost_cached_tokens(
  +has_cache_read: Bool, +has_uncached: Bool, +cached_le_input: Bool, +in_cached: Nat
) -> Nat:
  cost_pick_nat(cost_use_cache(has_cache_read, has_uncached, cached_le_input), in_cached, 0n)

# An uncached total is reported when the provider sent one, or when an input
# total exists to bill at the input rate.
def cost_uncached_present(+has_uncached: Bool, +has_in_total: Bool, +rate_uncached_p: Bool) -> Bool:
  ((has_uncached || has_in_total) && rate_uncached_p : Bool)

def cost_output_present(+has_output: Bool, +rate_output_p: Bool) -> Bool:
  (has_output && rate_output_p : Bool)

# Reasoning is priced only as a separate reported total; when the provider
# includes it in output, that price is already counted.
def cost_reasoning_priced(
  +has_reasoning: Bool, +reasoning_separate: Bool, +rate_reasoning_p: Bool
) -> Bool:
  (has_reasoning && reasoning_separate && rate_reasoning_p : Bool)

# Explicit uncached total, else input minus cached tokens when a cache read
# applied, else the input total; nothing without an input total.
def cost_uncached_tokens(
  +has_uncached: Bool, +in_uncached: Nat, +use_cache: Bool, +has_in_total: Bool,
  +in_total: Nat, +in_cached: Nat,
) -> Nat:
  match has_uncached:
    case True{}:
      in_uncached
    case False{}:
      match use_cache:
        case True{}:
          match has_in_total:
            case True{}:
              (in_total - in_cached : Nat)
            case False{}:
              0n
        case False{}:
          match has_in_total:
            case True{}:
              in_total
            case False{}:
              0n

# The summed e8 numerator over the four priced terms.
def cost_term_sum(
  +in_total: Nat, +in_cached: Nat, +in_uncached: Nat,
  +has_in_total: Bool, +has_cache_read: Bool, +has_uncached: Bool,
  +has_output: Bool, +out_tokens: Nat,
  +has_reasoning: Bool, +reason_tokens: Nat,
  +cached_le_input: Bool, +reasoning_separate: Bool,
  +rate_cached_p: Bool, +rate_uncached_p: Bool, +rate_output_p: Bool, +rate_reasoning_p: Bool,
  +rate_cached_cents: Nat, +rate_uncached_cents: Nat, +rate_output_cents: Nat,
  +rate_reasoning_cents: Nat,
) -> Nat:
  (cost_term(
     cost_cached_present(has_cache_read, has_uncached, cached_le_input, rate_cached_p),
     cost_cached_tokens(has_cache_read, has_uncached, cached_le_input, in_cached),
     rate_cached_cents)
   + cost_term(
       cost_uncached_present(has_uncached, has_in_total, rate_uncached_p),
       cost_uncached_tokens(has_uncached, in_uncached,
         cost_use_cache(has_cache_read, has_uncached, cached_le_input),
         has_in_total, in_total, in_cached),
       rate_uncached_cents)
   + cost_term(cost_output_present(has_output, rate_output_p), out_tokens, rate_output_cents)
   + cost_term(
       cost_reasoning_priced(has_reasoning, reasoning_separate, rate_reasoning_p),
       reason_tokens, rate_reasoning_cents) : Nat)

# With a row in hand: no priced term at all is unreported usage; otherwise the
# paid total and the cached share are exact e8 sums.
def cost_price_gate(
  +any_present: Bool,
  +in_total: Nat, +in_cached: Nat, +in_uncached: Nat,
  +has_in_total: Bool, +has_cache_read: Bool, +has_uncached: Bool,
  +has_output: Bool, +out_tokens: Nat,
  +has_reasoning: Bool, +reason_tokens: Nat,
  +cached_le_input: Bool, +reasoning_separate: Bool,
  +rate_cached_p: Bool, +rate_uncached_p: Bool, +rate_output_p: Bool, +rate_reasoning_p: Bool,
  +rate_cached_cents: Nat, +rate_uncached_cents: Nat, +rate_output_cents: Nat,
  +rate_reasoning_cents: Nat,
) -> Cost.Decision:
  match any_present:
    case False{}:
      Blank{Unreported{}}
    case True{}:
      Priced{List{},
        cost_term_sum(in_total, in_cached, in_uncached, has_in_total, has_cache_read,
          has_uncached, has_output, out_tokens, has_reasoning, reason_tokens,
          cached_le_input, reasoning_separate, rate_cached_p, rate_uncached_p,
          rate_output_p, rate_reasoning_p, rate_cached_cents, rate_uncached_cents,
          rate_output_cents, rate_reasoning_cents),
        cost_term(
          cost_cached_present(has_cache_read, has_uncached, cached_le_input, rate_cached_p),
          cost_cached_tokens(has_cache_read, has_uncached, cached_le_input, in_cached),
          rate_cached_cents),
        cost_cached_present(has_cache_read, has_uncached, cached_le_input, rate_cached_p),
        0n}

def cost_price(
  +in_total: Nat, +in_cached: Nat, +in_uncached: Nat,
  +has_in_total: Bool, +has_cache_read: Bool, +has_uncached: Bool,
  +has_output: Bool, +out_tokens: Nat,
  +has_reasoning: Bool, +reason_tokens: Nat,
  +cached_le_input: Bool, +reasoning_separate: Bool,
  +rate_cached_p: Bool, +rate_uncached_p: Bool, +rate_output_p: Bool, +rate_reasoning_p: Bool,
  +rate_cached_cents: Nat, +rate_uncached_cents: Nat, +rate_output_cents: Nat,
  +rate_reasoning_cents: Nat,
) -> Cost.Decision:
  cost_price_gate(
    (cost_cached_present(has_cache_read, has_uncached, cached_le_input, rate_cached_p)
     || cost_uncached_present(has_uncached, has_in_total, rate_uncached_p)
     || cost_output_present(has_output, rate_output_p)
     || cost_reasoning_priced(has_reasoning, reasoning_separate, rate_reasoning_p) : Bool),
    in_total, in_cached, in_uncached, has_in_total, has_cache_read, has_uncached,
    has_output, out_tokens, has_reasoning, reason_tokens, cached_le_input,
    reasoning_separate, rate_cached_p, rate_uncached_p, rate_output_p,
    rate_reasoning_p, rate_cached_cents, rate_uncached_cents, rate_output_cents,
    rate_reasoning_cents)

def cost_fields_gate(
  +row_exists: Bool,
  +in_total: Nat, +in_cached: Nat, +in_uncached: Nat,
  +has_in_total: Bool, +has_cache_read: Bool, +has_uncached: Bool,
  +has_output: Bool, +out_tokens: Nat,
  +has_reasoning: Bool, +reason_tokens: Nat,
  +cached_le_input: Bool, +reasoning_separate: Bool,
  +rate_cached_p: Bool, +rate_uncached_p: Bool, +rate_output_p: Bool, +rate_reasoning_p: Bool,
  +rate_cached_cents: Nat, +rate_uncached_cents: Nat, +rate_output_cents: Nat,
  +rate_reasoning_cents: Nat,
) -> Cost.Decision:
  match row_exists:
    case False{}:
      Blank{NoRow{}}
    case True{}:
      cost_price(in_total, in_cached, in_uncached, has_in_total, has_cache_read,
        has_uncached, has_output, out_tokens, has_reasoning, reason_tokens,
        cached_le_input, reasoning_separate, rate_cached_p, rate_uncached_p,
        rate_output_p, rate_reasoning_p, rate_cached_cents, rate_uncached_cents,
        rate_output_cents, rate_reasoning_cents)

def cost_usage_gate(
  +has_usage: Bool, +any_usage_field: Bool, +row_exists: Bool,
  +in_total: Nat, +in_cached: Nat, +in_uncached: Nat,
  +has_in_total: Bool, +has_cache_read: Bool, +has_uncached: Bool,
  +has_output: Bool, +out_tokens: Nat,
  +has_reasoning: Bool, +reason_tokens: Nat,
  +cached_le_input: Bool, +reasoning_separate: Bool,
  +rate_cached_p: Bool, +rate_uncached_p: Bool, +rate_output_p: Bool, +rate_reasoning_p: Bool,
  +rate_cached_cents: Nat, +rate_uncached_cents: Nat, +rate_output_cents: Nat,
  +rate_reasoning_cents: Nat,
) -> Cost.Decision:
  match has_usage:
    case False{}:
      Blank{Unreported{}}
    case True{}:
      match any_usage_field:
        case False{}:
          Blank{Unreported{}}
        case True{}:
          cost_fields_gate(row_exists, in_total, in_cached, in_uncached, has_in_total,
            has_cache_read, has_uncached, has_output, out_tokens, has_reasoning,
            reason_tokens, cached_le_input, reasoning_separate, rate_cached_p,
            rate_uncached_p, rate_output_p, rate_reasoning_p, rate_cached_cents,
            rate_uncached_cents, rate_output_cents, rate_reasoning_cents)

# The whole decision for one record that has a model id: a subscription row
# answers first and ignores tokens entirely, then the usage gates, the row
# gate, and the priced terms.
def Cost.decide(
  +subscription: Bool,
  +has_usage: Bool, +any_usage_field: Bool, +row_exists: Bool,
  +in_total: Nat, +in_cached: Nat, +in_uncached: Nat,
  +has_in_total: Bool, +has_cache_read: Bool, +has_uncached: Bool,
  +has_output: Bool, +out_tokens: Nat,
  +has_reasoning: Bool, +reason_tokens: Nat,
  +cached_le_input: Bool, +reasoning_separate: Bool,
  +rate_cached_p: Bool, +rate_uncached_p: Bool, +rate_output_p: Bool, +rate_reasoning_p: Bool,
  +rate_cached_cents: Nat, +rate_uncached_cents: Nat, +rate_output_cents: Nat,
  +rate_reasoning_cents: Nat,
  +fee_e8: Nat,
) -> Cost.Decision:
  match subscription:
    case True{}:
      Priced{Fee{}, 0n, 0n, False{}, fee_e8}
    case False{}:
      cost_usage_gate(has_usage, any_usage_field, row_exists, in_total, in_cached,
        in_uncached, has_in_total, has_cache_read, has_uncached, has_output,
        out_tokens, has_reasoning, reason_tokens, cached_le_input,
        reasoning_separate, rate_cached_p, rate_uncached_p, rate_output_p,
        rate_reasoning_p, rate_cached_cents, rate_uncached_cents, rate_output_cents,
        rate_reasoning_cents)
