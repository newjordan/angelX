import Base
import ./deathmatch-rules.bend as Dm

law the_title_card_holds_the_round:
  for ended: U32
  for pickups: List<&2, U32>
  {Dm.begin(False{}, ended, 4, 0, pickups) == Dm.Open{False{}, ended, 1, pickups} : Dm.Open}

law one_knight_alone_waits:
  for ended: U32
  for ticks: U32
  for pickups: List<&2, U32>
  {Dm.begin(False{}, ended, 1, ticks, pickups) == Dm.Open{False{}, ended, ticks, pickups} : Dm.Open}

law a_won_round_counts_the_podium:
  for connected: U32
  for ticks: U32
  for pickups: List<&2, U32>
  {Dm.begin(True{}, 5, connected, ticks, pickups) == Dm.Open{False{}, 6, ticks, pickups} : Dm.Open}

law a_disconnected_knight_sits_out:
  for ticks: U32
  for orbit_every: U32
  for rot_every: U32
  for winner: Bool
  for living: Bool
  for respawn: U32
  for protection: U32
  for power: U32
  for flash: U32
  for stun: U32
  for fart_wait: U32
  for burn: U32
  for hero: Dm.Kit
  for champ: Dm.Stance
  {Dm.turn(Dm.Turn{ticks, orbit_every, rot_every, winner, living, Dm.Body{False{}, respawn, protection, power, flash, stun, fart_wait, burn}, hero, champ}) == Dm.Tick{Dm.Skip{}, Dm.Body{False{}, respawn, protection, power, flash, stun, fart_wait, burn}, hero, champ, 0, False{}, False{}, False{}, 2, False{}} : Dm.Tick}

law the_last_wait_tick_respawns:
  for ticks: U32
  for orbit_every: U32
  for rot_every: U32
  for winner: Bool
  for living: Bool
  for protection: U32
  for power: U32
  for flash: U32
  for stun: U32
  for fart_wait: U32
  for burn: U32
  for hero: Dm.Kit
  for champ: Dm.Stance
  {Dm.turn(Dm.Turn{ticks, orbit_every, rot_every, winner, living, Dm.Body{True{}, 1, protection, power, flash, stun, fart_wait, burn}, hero, champ}) == Dm.Tick{Dm.Spawn{}, Dm.Body{True{}, 0, protection, power, flash, stun, fart_wait, burn}, hero, champ, 0, False{}, False{}, False{}, 2, False{}} : Dm.Tick}

law a_warded_knight_is_not_struck:
  {Dm.hit(Dm.Blow{1, 2, False{}, True{}, True{}, True{}, 5, 0, 3, False{}}) == Dm.Reject{} : Dm.Verdict}

law a_won_round_strikes_nobody:
  {Dm.hit(Dm.Blow{1, 2, True{}, True{}, True{}, True{}, 5, 0, 0, False{}}) == Dm.Reject{} : Dm.Verdict}

law a_knight_never_strikes_themselves:
  {Dm.hit(Dm.Blow{2, 2, False{}, True{}, True{}, True{}, 5, 0, 0, False{}}) == Dm.Reject{} : Dm.Verdict}

law a_blocked_blow_is_turned:
  {Dm.hit(Dm.Blow{1, 2, False{}, True{}, True{}, True{}, 5, 0, 0, True{}}) == Dm.Turned{} : Dm.Verdict}

law the_tenth_kill_wins:
  for victim: U32
  for ticks: U32
  for deaths: U32
  {Dm.struck(Dm.Struck{1, victim, 9, 0, ticks, 9, 0, Nil{}}) == Dm.Scored{True{}, True{}, 60, 1, 10, Con{Dm.Frag{1, victim, ticks}, Nil{}}, True{}, 0} : Dm.Scored}

law a_blow_that_did_nothing_scores_nothing:
  for kills: U32
  for deaths: U32
  for feed: List<&2, Dm.Frag>
  {Dm.struck(Dm.Struck{1, 2, 7, 7, 40, kills, deaths, feed}) == Dm.Scored{False{}, False{}, 0, deaths, kills, feed, False{}, 0} : Dm.Scored}
