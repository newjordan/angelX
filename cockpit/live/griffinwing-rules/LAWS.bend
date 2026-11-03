import Base
import ./griffinwing-rules.bend as R

# Claims about the numbers the rules stand on, fixed to concrete words: a symbolic U32 would
# leave the checker stuck, so each claim names the numbers it talks about.

# A signed division goes toward zero, as the sim's integer division always has.
law a_negative_divides_toward_zero:
  {R.sdv(R.ng(7), 2) == R.ng(3) : U32}

# A negative is the two's-complement word, so its negation is the number it was.
law a_negation_undoes_itself:
  {R.ng(R.ng(5)) == 5 : U32}

# Ten tenths of a unit are 256 in fixed point, and a negative count of them is the negative.
law ten_tenths_are_a_unit:
  {R.dm(10) == 256 : U32}

law a_negative_tenth_count_is_the_negative:
  {R.dn(30) == R.ng(R.dm(30)) : U32}

# The triangle wave bottoms out at the start of its period and tops out half way.
law the_wave_starts_at_the_bottom:
  {R.tri(0, 60, 768) == R.ng(768) : U32}

law the_wave_tops_out_half_way:
  {R.tri(30, 60, 768) == 768 : U32}

# A word written is the word read back, and the next word is untouched.
law a_word_written_is_read_back:
  {R.rd(R.wr(R.Z{}, 5, 77), 5) == 77 : U32}

law a_word_beside_it_is_untouched:
  {R.rd(R.wr(R.Z{}, 5, 77), 6) == 0 : U32}

# The first knight flies at the middle of the formation, the second to its left.
law the_first_knight_leads_the_formation:
  {R.slot_x(1) == 0 : U32}

law the_second_knight_flies_to_the_left:
  {R.slot_x(2) == R.ng(563) : U32}

# A fall puts the beat back in under a second.
law a_wipe_is_over_in_under_a_second:
  {R.lt(R.RETRY(), R.HZ()) == True{} : Bool}
