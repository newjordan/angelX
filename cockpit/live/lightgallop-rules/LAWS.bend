import Base
import ./math.bend as M

# Laws about the arithmetic the ride is built on, checked by the ordinary Bend
# checker. They state only what the integer code actually does: the anchors of
# the tables, Rust's truncating division and the clamps.

law sin_anchors_its_quadrants:
  {M.sin(64) == 4096 : U32}

law sin_of_zero_is_flat:
  {M.sin(0) == 0 : U32}

law cosine_leads_sine_by_a_quarter:
  {M.cos(0) == 4096 : U32}

law the_compass_starts_east:
  {M.atan2(0, 1) == 0 : U32}

law a_quarter_turn_is_256_brads:
  {M.atan2(1, 0) == 256 : U32}

law division_truncates_toward_zero:
  {M.div(M.neg(7), 2) == M.neg(3) : U32}

law rem_euclid_never_goes_negative:
  {M.rem_euc(M.neg(1), 256) == 255 : U32}

law a_clamp_holds_a_number_to_its_range:
  {M.clamp(5, 0, 3) == 3 : U32}

law a_saturating_subtract_stops_at_zero:
  {M.sat(2, 5) == 0 : U32}
