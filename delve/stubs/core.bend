# Names the core group calls that other groups own, until they land them.

def cards.HAND() -> U32:
  4

def cards.SPELL_SLOTS() -> U32:
  4

def votes.Command.valid(+c: J.Val) -> Bool:
  True{}

def monsters.Enemy.radius(+e: J.Val) -> F32:
  0.5

# --- Run::step's phases that other groups own ------------------------------

def crossroads.Run.tick_crossroads(+run: J.Val, +inputs: J.Val) -> J.Val:
  run

def exits.Run.tick_leave_hold(+run: J.Val, +inputs: J.Val) -> J.Val & List<&2, Maybe<&2, U32>>:
  (run, [None{}, None{}, None{}, None{}, None{}, None{}])

def overclass.Run.keep_wish_floor(+run: J.Val) -> J.Val:
  run

def exits.Run.leave_activity(+run: J.Val, +id: U32) -> J.Val & Bool:
  (run, False{})

def exits.Run.close_behind_the_away(+run: J.Val) -> J.Val:
  run

def exits.Run.detached_seats(+run: J.Val, +inputs: J.Val) -> J.Val & J.Val:
  (run, J.obj())

def floor_plates.Run.tick_floor_plates(+run: J.Val, +inputs: J.Val) -> J.Val:
  run

def votes.Run.ballot_inputs(+run: J.Val, +inputs: J.Val) -> J.Val & Maybe<&2, J.Val>:
  (run, None{})

def genie.Run.offer_inputs(+run: J.Val, +inputs: J.Val) -> J.Val & Maybe<&2, J.Val>:
  (run, None{})

def spectacle.Run.seen(+run: J.Val) -> J.Val:
  J.JNull{}

def fellowship.Run.call_fellowship(+run: J.Val) -> J.Val:
  run

def questline.Run.flush_questlines(+run: J.Val) -> J.Val:
  run

def questline.Run.tick_teachers(+run: J.Val, +inputs: J.Val) -> J.Val:
  run

def kingdom.Run.follow_kingdom_cameras(+run: J.Val) -> J.Val:
  run

def spectacle.Run.watch(+run: J.Val, +seen: J.Val) -> J.Val:
  run

def dares.Run.watch_dare(+run: J.Val, +heard: U32, +sounded: U32) -> J.Val:
  run

def bounties.Run.mark_moments(+run: J.Val, +heard: U32) -> J.Val:
  run

def depths.Run.open_map(+run: J.Val) -> Bool:
  False{}

def rules.Run.barred(+run: J.Val) -> Bool:
  False{}

def deathmatch.Run.deathmatch_showing(+run: J.Val) -> Bool:
  False{}

def deathmatch.Run.tick_deathmatch(+run: J.Val, +inputs: J.Val) -> J.Val:
  run

def cauldron.Run.tend_cauldron(+run: J.Val, +inputs: J.Val) -> J.Val & Maybe<&2, J.Val>:
  (run, None{})

def presence.Run.ward_while_held(+run: J.Val) -> J.Val:
  run

def curios.Run.tick_pockets(+run: J.Val) -> J.Val:
  run

def thief.Run.tick_thieves(+run: J.Val) -> J.Val:
  run

def minions.Run.tick_gravecallers(+run: J.Val, +inputs: J.Val) -> J.Val:
  run

def minions.Run.skeletons_block(+run: J.Val) -> J.Val:
  run

# hunt.rs: `foes_turn(&mut self, sortie: &mut Sortie) -> Deeds`: (run, (sortie, deeds)).
def hunt.Run.foes_turn(+run: J.Val, +sortie: J.Val) -> J.Val & (J.Val & J.Val):
  (run, (sortie, J.JNull{}))

# flight.rs: `shots_fly(&mut self, sortie: &mut Sortie) -> Struck`: (run, (sortie, struck)).
def flight.Run.shots_fly(+run: J.Val, +sortie: J.Val) -> J.Val & (J.Val & J.Val):
  (run, (sortie, J.JNull{}))

def spoils.Run.settle_kills(+run: J.Val) -> J.Val:
  run

def spoils.Run.gather(+run: J.Val) -> J.Val:
  run

def hazards.Run.tick_waves(+run: J.Val) -> J.Val:
  run

def room_end.Run.room_end(+run: J.Val) -> J.Val:
  run

def monsters.Enemy.base_hp(+e: J.Val) -> U32:
  core.u(e, "max_hp")

def home.Run.at_home_now(+run: J.Val) -> Bool:
  False{}

# --- the core group's own, until their modules land --------------------------

def fortune.Run.turbo(+run: J.Val) -> Bool:
  False{}

def trophies.Run.mark_trophies(+run: J.Val, +heard: U32) -> J.Val:
  run

def floor_stats.Run.close_run(+run: J.Val) -> J.Val:
  run

def floor_stats.Run.stand_verdict(+run: J.Val) -> J.Val:
  run

def fx.Run.show_fx(+run: J.Val) -> J.Val:
  run

def fx.Run.present(+run: J.Val) -> J.Val:
  run

def rules.Run.play(+run: J.Val, +inputs: J.Val) -> J.Val:
  run

def stakes.party_fury(+n: U32) -> U32:
  0

# --- the Hero's neighbours ----------------------------------------------------

# curios.rs: `curio_turns(&mut self, damage) -> Option<u32>`: a bucket turns the blow aside whole.
def curios.Hero.curio_turns(+h: J.Val, +damage: U32) -> J.Val & Maybe<&2, U32>:
  (h, Some{damage})

def curios.Hero.curio_biscuit(+h: J.Val) -> J.Val:
  h

# runes.rs: `has_rune(kind)`: kind spelled as the serde snake_case name.
def runes.Hero.has_rune(+h: J.Val, +kind: String) -> Bool:
  False{}

def classes.Hero.peasant(+h: J.Val) -> Bool:
  False{}

# rules.rs: `ShotTraits::default()` as serde writes it.
def projectile.ShotTraits.default() -> J.Val:
  J.set(J.set(J.set(J.set(J.set(J.set(J.set(J.set(J.set(J.set(J.obj(), "bounce", J.JU{0}), "homing", J.JU{0}), "chain", J.JU{0}), "mend", J.JU{0}), "back", J.JU{0}), "owner", J.JU{0}), "out", J.JU{0}), "burst", J.JU{0}), "struck", J.JU{0}), "stun", J.JU{0})

def pitstop.Pit.bare(+pit: J.Val) -> Maybe<&2, U32>:
  None{}

def pitstop.Pit.tick(+pit: J.Val) -> J.Val:
  pit

def alehouse.Run.tick_alehouse(+run: J.Val, +inputs: J.Val) -> J.Val:
  run

def altar.Run.tick_wish_altar(+run: J.Val, +inputs: J.Val) -> J.Val:
  run

def barony.Run.tick_barony(+run: J.Val, +inputs: J.Val) -> J.Val:
  run

def brasswake.Run.tick_brasswake(+run: J.Val, +inputs: J.Val) -> J.Val:
  run

def champions.Run.champion_feat(+run: J.Val, +id: U32, +feat: String) -> J.Val:
  run

def champions.Run.tick_champions(+run: J.Val) -> J.Val:
  run

def chariot.Run.tick_hippodrome_race(+run: J.Val, +inputs: J.Val) -> J.Val:
  run

def cinder.Run.tick_cinder(+run: J.Val, +inputs: J.Val) -> J.Val:
  run

def classes.Run.tick_field_banner(+run: J.Val, +inputs: J.Val) -> J.Val:
  run

def college.Run.tick_college(+run: J.Val, +inputs: J.Val) -> J.Val:
  run

def countryside.Run.tick_countryside(+run: J.Val, +inputs: J.Val) -> J.Val:
  run

def critters.Run.tick_critters(+run: J.Val) -> J.Val:
  run

def depths.Run.fighting(+run: J.Val) -> Bool:
  False{}

def feel.Run.land_synergies(+run: J.Val, +burns: J.Val, +turned: J.Val, +kegs: J.Val) -> J.Val:
  run

def foes.Run.apply_deeds(+run: J.Val, +deeds: J.Val) -> J.Val:
  run

def foes.Run.tick_flings(+run: J.Val) -> J.Val:
  run

def foes.Run.tick_kegs_and_lobs(+run: J.Val) -> J.Val:
  run

def foes.Run.shoot_kegs(+run: J.Val) -> J.Val:
  run

def foes.Run.tick_oathbound(+run: J.Val) -> J.Val:
  run

def foes.Run.tick_wells(+run: J.Val) -> J.Val:
  run

def folly.Run.tick_folly(+run: J.Val) -> J.Val:
  run

def folly.Run.credit_bats(+run: J.Val, +credits: J.Val) -> J.Val:
  run

def folly.Run.tick_bats(+run: J.Val) -> J.Val:
  run

def forest_road.Run.tick_forest_road(+run: J.Val, +inputs: J.Val) -> J.Val:
  run

def frogs.Run.tick_frogs(+run: J.Val) -> J.Val:
  run

def gantry.Run.tick_gantry(+run: J.Val) -> J.Val:
  run

def hazards.Run.tick_traps(+run: J.Val) -> J.Val:
  run

def hell.Run.rain_embers(+run: J.Val) -> J.Val:
  run

def hireling.Run.tick_hireling(+run: J.Val) -> J.Val:
  run

def holdings.Run.tick_holdings(+run: J.Val, +inputs: J.Val) -> J.Val:
  run

def home.Run.second_winds(+run: J.Val) -> J.Val:
  run

def home.Run.tick_home(+run: J.Val, +inputs: J.Val) -> J.Val:
  run

def hop.Run.tick_book_hop(+run: J.Val) -> J.Val:
  run

def horrors.Run.tick_grip(+run: J.Val) -> J.Val:
  run

def imps.Run.call_imps(+run: J.Val, +owner: U32, +card_id: String) -> J.Val:
  run

def imps.Run.tick_imps(+run: J.Val) -> J.Val:
  run

def joust.Run.tick_lists(+run: J.Val, +inputs: J.Val) -> J.Val:
  run

def joust.Run.tick_stables(+run: J.Val, +inputs: J.Val) -> J.Val:
  run

def kingdom.Run.tick_kingdom(+run: J.Val, +inputs: J.Val) -> J.Val:
  run

def latch.Run.tick_latch(+run: J.Val, +inputs: J.Val) -> J.Val:
  run

def merchant.Run.tick_merchant(+run: J.Val) -> J.Val:
  run

def minions.Run.call_minion(+run: J.Val, +owner: U32, +card_id: String) -> J.Val:
  run

def minions.Run.tick_minions(+run: J.Val) -> J.Val:
  run

def mossbell.Run.tick_mossbell(+run: J.Val, +inputs: J.Val) -> J.Val:
  run

def nooks.Run.tick_nook(+run: J.Val, +inputs: J.Val) -> J.Val:
  run

def pennon.Run.tick_pennon(+run: J.Val, +inputs: J.Val) -> J.Val:
  run

def pitstop.Run.pit_admissions(+run: J.Val) -> J.Val:
  J.JNull{}

def pitstop.Run.visit_privy(+run: J.Val) -> J.Val:
  run

def pitstop.Run.pit_events(+run: J.Val, +events: J.Val) -> J.Val:
  run

def pots.Run.tick_pots(+run: J.Val) -> J.Val:
  run

def powder.Run.tick_powder(+run: J.Val, +inputs: J.Val) -> J.Val:
  run

def race3d.Run.tick_eyrie(+run: J.Val) -> J.Val:
  run

def race3d.Run.race3d_showing(+run: J.Val) -> Bool:
  False{}

def rafterwake.Run.tick_rafterwake(+run: J.Val, +inputs: J.Val) -> J.Val:
  run

def ranch.Run.tick_ranch(+run: J.Val, +inputs: J.Val) -> J.Val:
  run

def rescues.Run.tick_cage(+run: J.Val, +inputs: J.Val) -> J.Val:
  run

def runes.Run.tick_runes(+run: J.Val) -> J.Val:
  run

def secrets.Run.tick_snibbet(+run: J.Val) -> J.Val:
  run

def secrets.Run.small_secrets(+run: J.Val, +booms: J.Val, +draughts: J.Val) -> J.Val:
  run

def swamp.Run.tick_swamp_notice(+run: J.Val, +inputs: J.Val) -> J.Val:
  run

def tavern.Run.tick_wing_plate(+run: J.Val, +inputs: J.Val) -> J.Val:
  run

def tavern.Run.tick_tavern(+run: J.Val, +inputs: J.Val) -> J.Val:
  run

def tavern.Run.tick_song(+run: J.Val) -> J.Val:
  run

def wishcraft.Run.poison_hits(+run: J.Val, +hits: J.Val) -> J.Val:
  run

def works.Run.tick_works(+run: J.Val, +inputs: J.Val) -> J.Val:
  run

def world.Run.tick_world(+run: J.Val, +inputs: J.Val) -> J.Val:
  run

def yard.Run.tick_yard(+run: J.Val, +inputs: J.Val) -> J.Val:
  run

def activity.Run.ride(+run: J.Val, +inputs: J.Val) -> J.Val & Maybe<&2, J.Val>:
  (run, None{})

def aftermath.Run.aftermath(+run: J.Val, +inputs: J.Val, +sortie: J.Val, +struck: J.Val, +deeds: J.Val) -> J.Val:
  run

def arms.Run.knights_turn(+run: J.Val, +inputs: J.Val) -> J.Val & J.Val:
  (run, J.JNull{})

def arms.Run.bombs_burst(+run: J.Val, +sortie: J.Val) -> J.Val:
  run

def feel.Run.tick_feel(+run: J.Val, +credits: J.Val, +grazes: J.Val) -> J.Val:
  run

def fx.Run.fade(+run: J.Val) -> J.Val:
  run

def shoulders.Run.shoulders_begin(+run: J.Val, +inputs: J.Val) -> J.Val & (Maybe<&2, J.Val> & J.Val):
  (run, (None{}, J.JNull{}))

def shoulders.Run.shoulders_end(+run: J.Val, +ledger: J.Val) -> J.Val:
  run

def stakes.Run.set_stakes(+run: J.Val) -> J.Val:
  run

def wounds.Run.tally_wounds(+run: J.Val, +before: J.Val, +boss_beats: J.Val) -> J.Val:
  run

def hazards.ROCK_FALL() -> U32:
  36

def houses.knight_name(+key: String, +slot: U32) -> Maybe<&2, String>:
  None{}

def critters.Enemy.touch(+self: J.Val) -> U32:
  0

def critters.Enemy.hurts_by_touch(+self: J.Val) -> Bool:
  False{}

def monsters.Enemy.asleep(+self: J.Val, +tick: W.W) -> Bool:
  False{}

def cards.cap(+name: String) -> U32:
  8


def activity.Run.tick_follies(+run: J.Val, +inputs: J.Val) -> J.Val:
  run

def activity.Run.tick_places(+run: J.Val, +inputs: J.Val) -> J.Val:
  run

def bard.Run.tick_bard_songs(+run: J.Val) -> J.Val:
  run

def hexer.Run.tick_hexes(+run: J.Val) -> J.Val:
  run

def items.Run.tick_items(+run: J.Val) -> J.Val:
  run

def lich.Run.tick_chill(+run: J.Val) -> J.Val:
  run

def skills.Run.fart_hits(+run: J.Val, +farts: J.Val) -> J.Val:
  run

def skulls.Run.tick_skulls(+run: J.Val) -> J.Val:
  run

def ults.Run.cast_ults(+run: J.Val, +casts: J.Val) -> J.Val:
  run

def ults.Run.tick_ults(+run: J.Val) -> J.Val:
  run

# secrets.rs: `blast_wall_open(&mut self, at: (f32, f32))` and `blast_wall(&mut self, at: Option<(f32, f32)>)`.
def secrets.Run.blast_wall_open(+run: J.Val, +x: F32, +y: F32) -> J.Val:
  run

def secrets.Run.blast_wall(+run: J.Val, +at: J.Val) -> J.Val:
  run

# feel.rs: `Feel::blow(&mut self, enemy, owner, blow)`; gore.rs `Blow::Blast { at }` as its serde object.
def feel.Feel.blow(+feel: J.Val, +enemy: U32, +owner: U32, +blow: J.Val) -> J.Val:
  feel

def gore.Blow.blast(+x: F32, +y: F32) -> J.Val:
  J.set(J.obj(), "Blast", J.set(J.obj(), "at", J.JA{[J.JF{x}, J.JF{y}]}))

def bannerpeak.knight.Knight.hit(+self: J.Val, +from: U32) -> J.Val & Bool:
  (self, False{})

def curios.Hero.curio_damage(+self: J.Val) -> U32:
  0

def curios.Hero.curio_wait(+self: J.Val) -> U32:
  0

def dead_wishes.Gifts.hit(+self: J.Val, +weight: U32) -> U32:
  0

def pastimes.siege.Keep.hit(+self: J.Val, +col: U32, +row: U32, +dmg: U32) -> J.Val & U32:
  (self, 0)

def pitstop.Pit.hit(+self: J.Val) -> U32:
  0

def cat.Run.pet_tallow(+run: J.Val, +inputs: J.Val) -> J.Val:
  run

def cat.Run.tick_tallow(+run: J.Val) -> J.Val:
  run

def hunters.Run.tick_hunters(+run: J.Val) -> J.Val:
  run

# feats.rs: `notice(&mut self, id)`: an achievement noticed, once (the real one is the combat group's).
def feats.Run.notice(+run: J.Val, +id: String) -> J.Val:
  run

def curios.Hero.curio_gold(+self: J.Val, +n: U32) -> U32:
  0

def joust.Hit.flash(+self: J.Val, +aim: J.Val) -> String:
  ""

def skills.Run.flash(+run: J.Val, +cloud: J.Val) -> J.Val:
  run

# skills.rs: `afflict(banes: &mut Vec<Bane>, foe, kind, power, owner)`: the banes after.
def skills.afflict(+banes: J.Val, +foe: U32, +kind: String, +power: U32, +owner: U32) -> J.Val:
  banes

def griffinwing.types.Kind.radius(+self: J.Val) -> U32:
  0

def griffinwing.types.Foe.radius(+self: J.Val) -> U32:
  0

def lanternward.boss.Warden.radius(+self: J.Val) -> U32:
  0

def lanternward.foes.Foe.radius(+self: J.Val) -> U32:
  0

def monsters.EnemyKind.radius(+self: J.Val) -> F32:
  0.0

def skills.Cloud.radius(+self: J.Val) -> F32:
  0.0

# realm/mod.rs: `Spoils::add(&mut self, spoil, n)`: the purse after (a zero adds nothing).
def realm.Spoils.add(+sp: J.Val, +spoil: String, +n: U32) -> J.Val:
  Bool.pick(J.Val, U32.is_zero(n), sp, J.set(sp, spoil, J.JU{core.uadd(J.u32(J.get(sp, spoil)), n)}))

# layout.rs: `DIRS`: north, east, south, west as (dx, dy), each axis as an i32 word.
def layout.DIRS_X(+i: U32) -> U32:
  Bool.pick(U32, U32.is_eq(i, 1), 1, Bool.pick(U32, U32.is_eq(i, 3), 4294967295, 0))

def layout.DIRS_Y(+i: U32) -> U32:
  Bool.pick(U32, U32.is_eq(i, 0), 4294967295, Bool.pick(U32, U32.is_eq(i, 2), 1, 0))

# shield.rs: `Wear::rest(&mut self, max_hp)`.
def shield.Wear.rest(+wear: J.Val, +max_hp: U32) -> J.Val:
  wear

def champions.HAUL_SPEED() -> F32:
  24.0

def render.overworld.arena.moonfall.labels.Layout.keeps(+self: J.Val) -> J.Val:
  J.JNull{}

def hexer.Hero.frog(+self: J.Val) -> Bool:
  False{}

def looks.Hero.shot_look(+self: J.Val, +book: J.Val) -> Maybe<&2, String & String>:
  None{}

def spells.Hero.equip_spell(+self: J.Val, +card: J.Val) -> J.Val & Bool:
  (self, False{})

# signatures.rs: `rear_bonus(enemy, bosses, at, dealt)`: extra damage for a blow from behind.
def signatures.rear_bonus(+enemy: J.Val, +bosses: J.Val, +x: F32, +y: F32, +dealt: U32) -> U32:
  0

# gore.rs: `Blow::Swing { dir, forged, fists }` as its serde object.
def gore.Blow.swing(+dx: F32, +dy: F32, +forged: Bool, +fists: Bool) -> J.Val:
  J.set(J.obj(), "Swing", J.set(J.set(J.set(J.obj(), "dir", J.JA{[J.JF{dx}, J.JF{dy}]}), "forged", J.JBool{forged}), "fists", J.JBool{fists}))

# bodies.rs: `loose(hero, id, pack, grid, enemies, salt, shots) -> Option<Loosed>`:
# (hero, (enemies, (shots, loosed))) with loosed {"Bite": [[i, dealt]..]} | {"Volley": n} | null.
def bodies.loose(+hero: J.Val, +id: U32, +pack: String, +room: J.Val, +barred: Bool, +enemies: J.Val, +salt: W.W, +shots: J.Val) -> J.Val & (J.Val & (J.Val & J.Val)):
  (hero, (enemies, (shots, J.JNull{})))

def champions.Hero.is_hammerer(+h: J.Val) -> Bool:
  False{}

def champions.Hero.is_alchemist(+h: J.Val) -> Bool:
  False{}

def champions.FLASK_REARM() -> U32:
  60

def spells.Run.spell_power_of(+run: J.Val, +id: String, +base: U32) -> U32:
  base

def monsters.EnemyKind.flies(+kind: J.Val) -> Bool:
  False{}

def rules.in_flight(+shots: J.Val, +id: U32) -> U32:
  0

def curios.Hero.curio_veer(+self: J.Val, +loosed: U32) -> F32:
  0.0

def looks.Looks.dresses_shots(+self: J.Val) -> Bool:
  False{}

def loot.Weapon.arms(+weapon: String) -> J.Val:
  J.set(J.set(J.set(J.set(J.set(J.set(J.set(J.set(J.obj(), "speed", J.JF{20.0}), "damage", J.JU{20}), "cooldown", J.JU{9}), "pierce", J.JU{0}), "ttl", J.JU{40}), "jitter", J.JF{0.0}), "arc", J.JF{0.0}), "shots", J.JU{1})

def realm.forge.Weapon.shots(+w: J.Val) -> U32:
  1

def cards.FORGED_SPEED() -> F32:
  14.0

def looks.CARD_LOOK() -> String:
  "card"

def classes.shot_look_of(+h: J.Val) -> J.Val:
  J.JNull{}

def dead_wishes.Hero.card_is_dead(+h: J.Val, +id: String) -> Bool:
  False{}

def cards.Looks.dresses_shots(+looks: J.Val) -> Bool:
  False{}

def curios.Hero.curio_veer(+h: J.Val, +loosed: U32) -> F32:
  0.0

# cards.rs: `Effect::spoil`: the effect names that carry a spoil.
def cards.Effect.is_spoil(+k: String) -> Bool:
  Bool.or(Bool.or(Bool.or(String.eq(k, "gold"), String.eq(k, "bone")), Bool.or(String.eq(k, "wax"), String.eq(k, "ore"))), Bool.or(String.eq(k, "gem"), Bool.or(String.eq(k, "ember"), String.eq(k, "scale"))))

# spell_upgrades.rs: a knight's own spell damage and power at their tiers.
def spell_upgrades.Hero.spell_damage(+h: J.Val, +spell: String, +damage: U32) -> U32:
  damage

def spell_upgrades.Hero.spell_power_of(+h: J.Val, +spell: String, +base: U32) -> U32:
  base

def bodies.cast(+hero: J.Val, +id: U32, +room: J.Val, +barred: Bool, +enemies: J.Val, +salt: W.W, +shots: J.Val) -> J.Val & (J.Val & (J.Val & J.Val)):
  (hero, (J.JNull{}, (shots, enemies)))

def spell_upgrades.Run.spell_wait(+run: J.Val, +card: J.Val) -> U32:
  30

def spells.Run.cabbage_storm(+run: J.Val, +h: J.Val, +card: String, +count: U32, +w: F32, +hh: F32, +owner: U32) -> J.Val:
  run

def spells.Run.call_imposture(+run: J.Val, +h: J.Val, +card: String, +owner: U32) -> J.Val:
  run

def spells.Run.hedge_maze(+run: J.Val, +h: J.Val, +card: String, +owner: U32, +w: F32, +hh: F32) -> J.Val:
  run

def cards.Card.has_imps(+card: J.Val) -> Bool:
  False{}

def cards.Card.has_minion(+card: J.Val) -> Bool:
  False{}

def cards.Card.has_rip(+card: J.Val) -> Bool:
  False{}

def cards.Card.has_summon(+card: J.Val) -> Bool:
  False{}

def champions.is_spell(+id: String) -> Bool:
  False{}

def hireling.Run.is_beaumains(+run: J.Val) -> Bool:
  False{}

def minions.Hero.summoner(+h: J.Val) -> Bool:
  False{}

def hireling.Run.take_minion(+run: J.Val, +card: J.Val, +h: J.Val, +id: U32, +depth: U32, +room: J.Val, +barred: Bool) -> J.Val:
  run

def cards.vec_remove(+a: J.Val, +i: U32) -> J.Val:
  feel.remove_nth(a, i)

def deathmatch.Fighter.walk(+self: J.Val, +input: J.Val) -> J.Val:
  self

def folly.WaxMaze.glide(+self: J.Val, +id: U32, +room: J.Val, +dist: F32) -> J.Val & Maybe<&2, F32 & F32>:
  (self, None{})

def questline.Ledger.walk(+self: J.Val, +who: String, +power: String) -> J.Val:
  J.JNull{}

def skills.Run.shield_out(+run: J.Val, +id: U32) -> Bool:
  False{}

def minions.Run.order_key(+run: J.Val, +id: U32) -> J.Val:
  run

def presence.wake_on_first_move(+h: J.Val, +input: J.Val) -> J.Val:
  h

# pitstop.rs: `step(hero, input, admitted, &mut events)`: (hero, events).
def pitstop.step(+h: J.Val, +input: J.Val, +admitted: Bool, +run: J.Val) -> J.Val & J.Val:
  (h, J.JA{[]})

def peasant.start(+h: J.Val, +owner: U32, +tick: W.W, +run: J.Val) -> J.Val & J.Val:
  (h, run)

def works.Run.shield_out(+run: J.Val, +id: U32) -> Bool:
  False{}

def works.Run.set_duck(+run: J.Val, +id: U32, +duck: Bool) -> J.Val:
  run

def folly.Run.glide_for(+run: J.Val, +id: U32, +h: J.Val, +mx: F32, +my: F32, +dx: F32, +dy: F32, +speed: F32) -> J.Val:
  J.JNull{}

def ledge.walk(+h: J.Val, +input: J.Val, +room: J.Val, +barred: Bool, +dx: F32, +rolling: Bool) -> J.Val & Bool:
  (h, False{})

def ledge.FOOT() -> F32:
  0.9

def champions.Hero.busy(+h: J.Val) -> Bool:
  False{}

def shoulders.Shoulders.pace(+s: J.Val, +id: U32) -> F32:
  1.0

def pitstop.Pit.pace(+pit: J.Val) -> F32:
  1.0

def peasant.PACE() -> F32:
  1.15

def hunters.WEB_PACE() -> F32:
  0.5

def runes.HASTE() -> F32:
  1.3

def horrors.GRIP_PACE() -> F32:
  0.5

def tavern.SONG_HASTE() -> F32:
  1.2

def curios.Hero.curio_pace(+h: J.Val) -> F32:
  1.0

def hunters.Run.webbed(+webs: J.Val, +x: F32, +y: F32) -> Bool:
  False{}

def kingdom.Run.on_road_at(+run: J.Val, +x: F32, +y: F32) -> Bool:
  False{}

# pastimes.rs: a festival round is playing in this room (`round.game.room() == room.kind`).
def pastimes.Run.round_in_room(+run: J.Val) -> Bool:
  False{}

def depths.OPEN_BLAST() -> F32:
  14.0

def pitstop.Run.pit_admissions(+run: J.Val) -> J.Val:
  J.JA{[]}

# shoulders.rs, the active path: towers, flights and ride keys are not ported; the ledger is empty.
def shoulders.Run.shoulders_begin_active(+run: J.Val, +inputs: J.Val) -> J.Val & (Maybe<&2, J.Val> & J.Val):
  (run, (None{}, J.JA{[]}))

def shoulders.Run.share_loss(+run: J.Val, +carrier: U32, +was: U32) -> J.Val:
  run

def fortune.Run.tick_fortune(+run: J.Val, +inputs: J.Val) -> J.Val:
  run

def trophies.Run.tick_trophies(+run: J.Val) -> J.Val:
  run

# The Round Table adventures' registry rows (their `ACTIVITY` statics): whether on, the ride, how it ends.

def lanternward.ACTIVITY.on(+run: J.Val) -> Bool:
  False{}

def lanternward.ACTIVITY.ride(+run: J.Val, +inputs: J.Val) -> J.Val & Maybe<&2, J.Val>:
  (run, None{})

def lanternward.ACTIVITY.end(+run: J.Val) -> J.Val:
  run

def moonfall.ACTIVITY.on(+run: J.Val) -> Bool:
  False{}

def moonfall.ACTIVITY.ride(+run: J.Val, +inputs: J.Val) -> J.Val & Maybe<&2, J.Val>:
  (run, None{})

def moonfall.ACTIVITY.end(+run: J.Val) -> J.Val:
  run

def bannerpeak.ACTIVITY.on(+run: J.Val) -> Bool:
  False{}

def bannerpeak.ACTIVITY.ride(+run: J.Val, +inputs: J.Val) -> J.Val & Maybe<&2, J.Val>:
  (run, None{})

def bannerpeak.ACTIVITY.end(+run: J.Val) -> J.Val:
  run

def griffinwing.ACTIVITY.on(+run: J.Val) -> Bool:
  False{}

def griffinwing.ACTIVITY.ride(+run: J.Val, +inputs: J.Val) -> J.Val & Maybe<&2, J.Val>:
  (run, None{})

def griffinwing.ACTIVITY.end(+run: J.Val) -> J.Val:
  run

def lightgallop.ACTIVITY.on(+run: J.Val) -> Bool:
  False{}

def lightgallop.ACTIVITY.ride(+run: J.Val, +inputs: J.Val) -> J.Val & Maybe<&2, J.Val>:
  (run, None{})

def lightgallop.ACTIVITY.end(+run: J.Val) -> J.Val:
  run

def knight_racers.ACTIVITY.on(+run: J.Val) -> Bool:
  False{}

def knight_racers.ACTIVITY.ride(+run: J.Val, +inputs: J.Val) -> J.Val & Maybe<&2, J.Val>:
  (run, None{})

def knight_racers.ACTIVITY.end(+run: J.Val) -> J.Val:
  run

def deathmatch.ACTIVITY.on(+run: J.Val) -> Bool:
  False{}

def deathmatch.ACTIVITY.ride(+run: J.Val, +inputs: J.Val) -> J.Val & Maybe<&2, J.Val>:
  (run, None{})

def deathmatch.ACTIVITY.end(+run: J.Val) -> J.Val:
  run

def joust.Joust.seated(+j: J.Val) -> J.Val:
  J.JA{[]}

def chariot.Chariot.seat_ids(+c: J.Val) -> J.Val:
  J.JA{[]}

def layout.RoomKind.in_world(+kind: String) -> Bool:
  False{}

def alehouse.is_room(+kind: String) -> Bool:
  False{}

def forest.Run.tick_forest(+run: J.Val) -> J.Val:
  run

# altar.rs: `wish_altar() -> Option<(f32, f32)>`: where the altar stands, as [x, y], or null.
def altar.Run.wish_altar(+run: J.Val) -> J.Val:
  J.JNull{}
