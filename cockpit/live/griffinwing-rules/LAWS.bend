import Base
import ./griffinwing-rules.bend as R

law a_negative_divides_toward_zero:
  {R.sdv(R.ng(7), 2) == R.ng(3) : U32}

law a_negation_undoes_itself:
  {R.ng(R.ng(5)) == 5 : U32}

law ten_tenths_are_a_unit:
  {R.dm(10) == 256 : U32}

law a_negative_tenth_count_is_the_negative:
  {R.dn(30) == R.ng(R.dm(30)) : U32}

law the_wave_starts_at_the_bottom:
  {R.tri(0, 60, 768) == R.ng(768) : U32}

law the_wave_tops_out_half_way:
  {R.tri(30, 60, 768) == 768 : U32}

law a_word_written_is_read_back:
  {R.rd(R.wr(R.Z{}, 5, 77), 5) == 77 : U32}

law a_word_beside_it_is_untouched:
  {R.rd(R.wr(R.Z{}, 5, 77), 6) == 0 : U32}

law the_first_knight_leads_the_formation:
  {R.slot_x(1) == 0 : U32}

law the_second_knight_flies_to_the_left:
  {R.slot_x(2) == R.ng(563) : U32}

law a_wipe_is_over_in_under_a_second:
  {R.lt(R.RETRY(), R.HZ()) == True{} : Bool}
