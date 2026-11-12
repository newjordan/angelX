//! A knight in the delve: health and grace, the weapon and the guard they
//! carry, their hand and deck of cards, and what they bring home.

use super::*;


/// How hard a hit lands on a knight, in percent: 200 is a knight with half
/// the health to lose.
pub const HIT_SHARE: u32 = 200;
/// The most of a knight's health one guardian's charge-up takes, in
/// percent of their most, after every multiplier: three undodged in a row
/// still leave a knight standing.
pub const BLOW_MOST: u32 = 30;
/// The most of a knight's health any one hit takes, in percent: a hit may
/// leave them on their last sliver, never fell them whole (the operator,
/// 2026-10-18: "no one shots, but a 90% shot is ok").
pub const ONE_HIT_MOST: u32 = 90;
fn no_tool(code: &u8) -> bool {
    *code == 0
}
fn is_unblessed(ticks: &u32) -> bool {
    *ticks == 0
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Hero {
    /// One name-owned squire, independent of hirelings and summoned minions.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub squire: Option<super::squires_lodge::Companion>,
    pub name: String,
    #[serde(default, skip_serializing_if = "creator::Appearance::is_default")]
    pub look: creator::Appearance,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub mirror_key: String,
    #[serde(default, skip_serializing_if = "creator::Session::is_default")]
    pub mirror: creator::Session,
    /// An away host is an untargetable statue, carried with the party.
    #[serde(default)]
    pub stone: bool,
    /// Keeping vigil: stone by this knight's own key, mending those near.
    #[serde(default)]
    pub vigil: bool,
    /// Its player is not here (`Run::set_away`): a page still loading, a
    /// tab hidden, a link gone or an empty seat. Stone while it lasts.
    #[serde(default)]
    pub away: bool,
    /// A round has begun or its player is just back: stone, unseen and
    /// unhurt, until its player first moves (`Run::rest_until_moved`).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub idle: bool,
    /// No player's at all: the host's own place, kept for a host who plays
    /// no knight (`Run::set_absent`). Away, never drawn, never counted.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub absent: bool,
    /// How long a hit wards this knight from the next, in ticks: the run's
    /// tune, told it every tick (`Run::set_stakes`).
    #[serde(
        default = "hit_ward",
        skip_serializing_if = "crate::rules::runtime_state::omit",
        deserialize_with = "crate::rules::runtime_state::hit_ward"
    )]
    pub hit_ward: u32,
    /// The vigil key was down last tick, and the ticks until it answers again.
    #[serde(default)]
    pub(super) vigil_key: bool,
    #[serde(default)]
    pub(super) vigil_rearm: u32,
    pub x: f32,
    pub y: f32,
    /// This seat's eased surface camera, in simulation units.
    #[serde(default)]
    pub camera: Option<(f32, f32)>,
    pub hp: u32,
    pub max_hp: u32,
    pub aim_x: f32,
    pub aim_y: f32,
    pub invulnerable: u32,
    /// Desertion movement ticks, also stepped by client prediction.
    #[serde(default, skip_serializing_if = "peasant::idle")]
    pub flee_ticks: u32,
    /// The side of the doorway this knight just came out of, while the
    /// walk key toward it is still down from walking out: that key walks
    /// nowhere until it is let go (`Hero::held_off`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub door_behind: Option<u8>,
    /// The last guardian's charge-up that struck this knight
    /// (`signatures::tag`): each strikes a knight once.
    #[serde(default, skip_serializing_if = "zero")]
    pub(super) blown: u32,
    pub dash_cooldown: u32,
    /// Re-arm ticks left after a bomb, so a held key throws only one.
    pub bomb_cooldown: u32,
    pub bombs: u32,
    /// This floor's belt; reconciled by the run before charges are granted.
    #[serde(default = "bombs::old_belt")]
    pub bomb_capacity: u32,
    pub armor: u32,
    pub weapon: Weapon,
    /// Pieces owned, distinct from the health-dependent dress stage.
    #[serde(default = "gear::Owned::equipped")]
    pub owned: gear::Owned,
    #[serde(default)]
    pub forged: Option<crate::realm::forge::Weapon>,
    #[serde(default,serialize_with="runtime_state::serialize_avatar")]
    pub avatar: Option<crate::realm::Avatar>,
    #[serde(default)]
    pub swing: u32,
    /// The current swing is the knight's own sword (not a forged weapon's).
    #[serde(default)]
    pub sword: bool,
    #[serde(default)]
    pub sword_cooldown: u32,
    /// The guard of the knight's last stroke while it is young: what it
    /// still turns (`melee::parry`). Left out when none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parrying: Option<melee::Guard>,
    /// Play cards waiting in hand; quaff plays the first.
    #[serde(default)]
    pub hand: Vec<String>,
    /// Spells learned, cast with keys 1–4. A save from the three-slot days
    /// loads with the fourth slot empty.
    #[serde(default, deserialize_with = "cards::slots")]
    pub spells: [Option<String>; cards::SPELL_SLOTS],
    #[serde(default, skip_serializing_if = "abilities::empty_slots")]
    pub module_spells: [Option<abilities::Slot>; cards::SPELL_SLOTS],
    #[serde(default, deserialize_with = "cards::slots")]
    pub spell_cooldowns: [u32; cards::SPELL_SLOTS],
    /// Own spell tiers, by card id; absent is I. Slot moves keep the tier.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub spell_tiers: BTreeMap<String, u8>,
    /// The spells in the order they were learned, mirrored from `spells`
    /// for older readers. Draughts never go here: they stay in the hand,
    /// drunk with the quaff key.
    #[serde(default)]
    pub bar: [Option<cards::BarSlot>; 4],
    /// Hold cards; their bonuses last the run.
    #[serde(default)]
    pub deck: Vec<String>,
    /// Times the arm in hand has been tempered by picking up its twin
    /// (`feel::TEMPER_MAX` at most); a new arm starts again at none.
    #[serde(default)]
    pub temper: u8,
    /// The card of the weapon in hand; none for one forged by command.
    #[serde(default = "bow_card")]
    pub arm: Option<String>,
    #[serde(default)]
    pub bonus: cards::Bonus,
    /// Spoils picked up since the last stairs, not yet in the treasury.
    #[serde(default)]
    pub carried: Spoils,
    #[serde(default)]
    pub play_cooldown: u32,
    /// Ticks left of the Monk's Patience: no firing, half of every blow,
    /// a mend every half second, and no complaints.
    #[serde(default)]
    pub patience: u32,
    /// What the guard key does, and the card that put it there.
    #[serde(default)]
    pub guard: cards::Guard,
    #[serde(default = "roll_card")]
    pub guard_card: Option<String>,
    /// The shield is up this tick.
    #[serde(default)]
    pub shielding: bool,
    /// What the shield has turned (`shield::Wear`).
    #[serde(default, skip_serializing_if = "shield::Wear::fresh")]
    pub shield_wear: shield::Wear,
    /// A wall is held up this tick.
    #[serde(default)]
    pub walling: bool,
    /// The wall ran dry: let go of the key before raising it again.
    #[serde(default)]
    pub wall_spent: bool,
    #[serde(default = "full_mana")]
    pub mana: f32,
    #[serde(default)]
    pub(super) mana_rest: u32,
    /// The floor whose Sanctuary this knight last reforged in (one each).
    #[serde(default)]
    pub reforged_on: Option<u32>,
    /// Identified asynchronous kit work. A new occupant never inherits it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reforge_job: Option<String>,
    /// Session-owned spent wishes; a new seat inherits none.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub wish_jobs: Vec<String>,
    /// Which knight of the company this is, chosen on the Delve's intro.
    #[serde(default)]
    pub knight: Option<String>,
    /// The model house this knight rides for (`crate::world::houses` key),
    /// set when the knight is dressed: the house names the knight.
    #[serde(default)]
    pub house: Option<String>,
    /// Side-on rooms: fall speed, standing on something, the jump left in
    /// the air, the jump key as last seen (a jump is a press, not a hold),
    /// ticks of dropping through a plank, and the last firm footing.
    #[serde(default)]
    pub(super) vy: f32,
    #[serde(default)]
    pub grounded: bool,
    #[serde(default)]
    pub(super) air_jumps: u8,
    #[serde(default)]
    pub(super) jump_held: bool,
    #[serde(default)]
    pub(super) dropping: u32,
    #[serde(default)]
    pub(super) footing: Option<(f32, f32)>,
    /// Ticks left inside a Sanctuary's privy (hidden, the door shut), and
    /// the floor and room of the last one used: once a Sanctuary.
    #[serde(default)]
    pub privy: u32,
    /// Won the last game they played against friends: a laurel on the helm.
    #[serde(default)]
    pub laurel: bool,
    /// Sat on a kingdom bench: mending slowly until they step off.
    #[serde(default)]
    pub sitting: bool,
    /// Ticks left soaking in a bathhouse tub, and whether they came out
    /// scrubbed: the next floor left without the privy costs them nothing.
    #[serde(default)]
    pub soak: u32,
    #[serde(default)]
    pub scrubbed: bool,
    /// The class taken up at a hall (`classes::CLASSES` by name).
    #[serde(default)]
    pub class: Option<String>,
    /// Locked in the village stocks: can't walk, friends' shots are
    /// tomatoes. How many have landed this time.
    #[serde(default)]
    pub stocked: bool,
    #[serde(default)]
    pub splats: u8,
    /// Lost one: the game's booby prize sits on the helm until they win.
    #[serde(default)]
    pub booby: Option<super::pastimes::Game>,
    /// Shots loosed since the run began, for a volley's cadence.
    #[serde(default)]
    pub loosed: u32,
    /// The Bard's groove: counts of its song (one every four ticks) since
    /// the trigger was pressed (`bard::strum`); letting go ends it. Never
    /// written at rest, so a knight who never plays saves and sends as
    /// before.
    #[serde(default, skip_serializing_if = "is_zero_floor")]
    pub groove: u32,
    /// Songs the Bard has moved on from: each Lullaby turns it to the next
    /// song of the set (`bard::song_of`). Never written at rest, like
    /// `groove`.
    #[serde(default, skip_serializing_if = "is_zero_floor")]
    pub songs: u32,
    /// Beats of its current song the Bard has played: the next beat a press
    /// plays. Letting go keeps it, so the next press picks the song up where
    /// it stopped; only a new song (a Lullaby) puts it back to the top.
    #[serde(default, skip_serializing_if = "is_zero_floor")]
    pub tune: u32,
    /// Ticks the Bard's trigger has been held, 0 when it is not: the song's
    /// own clock (`bard::strum`).
    #[serde(default, skip_serializing_if = "is_zero_floor")]
    pub strum: u32,
    /// The privy: a stop under way, stops done, the buffs they left.
    #[serde(default)]
    pub pit: super::pitstop::Pit,
    /// A frog on the shoulder, riding to the next Sanctuary; and what the
    /// last one gave for the floor.
    #[serde(default)]
    pub rider: bool,
    #[serde(default)]
    pub boon: super::frogs::Boon,
    #[serde(default)]
    pub(super) relieved: Option<(u32, usize)>,
    /// The hearth's health already added to `max_hp` by home cards.
    #[serde(default)]
    pub home_hp: u32,
    /// Second winds left: at zero health, the knight rises again.
    #[serde(default)]
    pub winds: u8,
    /// The Talisman from the Pit: falling, this knight rises at once, whole.
    #[serde(default)]
    pub talisman: bool,
    /// Ticks E has been held on an Undercroft plate; a purchase must be let
    /// go of before the next.
    #[serde(default)]
    pub buying: u32,
    #[serde(default)]
    pub(super) buy_spent: bool,
    /// Glass Jaw: this knight hits, and is hit, twice as hard.
    #[serde(default)]
    pub glass: bool,
    /// The ultimate's charge (full at `ults::ULT_FULL`), and its key as
    /// last seen.
    #[serde(default)]
    pub ult_charge: u32,
    #[serde(default)]
    pub(super) ult_key: bool,
    /// Lady's Veil's wings, ticks left.
    #[serde(default)]
    pub angel: u32,
    /// A pilgrim's blessing, ticks left (`wayfarers`): more damage and pace.
    #[serde(default, skip_serializing_if = "is_unblessed")]
    pub blessed: u32,
    /// Bladewind: cuts left, and ticks to the next.
    #[serde(default)]
    pub slashes: u8,
    #[serde(default)]
    pub(super) slash_wait: u32,
    /// Assassinate: the monster aimed at, and ticks until the shot.
    #[serde(default)]
    pub aiming: Option<(u32, u32)>,
    /// A power rune this knight carries, and its ticks left.
    #[serde(default)]
    pub rune: Option<super::runes::Held>,
    /// All Random: the stranger's ultimate this knight carries this delve.
    #[serde(default)]
    pub ult: Option<super::ults::Ult>,
    /// The recruit whose ultimate this knight is wishing, the first to take
    /// them up (`recruit_ults`), and every recruit they were handed a wish
    /// for this delve.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recruit_ult: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub recruits_asked: Vec<String>,
    /// This knight owes the realm a rite of passage (`rite`): something made
    /// in the Creator for the world, asked as a pop-up on loading in.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub rite_owed: bool,
    /// The tick their rite was made, for its flourish (`rite`).
    #[serde(default, skip_serializing_if = "is_zero_u64")]
    pub rite_at: u64,
    /// Hexed: ticks left as a frog, then warded from the next hex.
    #[serde(default)]
    pub hexed: u32,
    /// Chilled by the Lich's frost: ticks left at a slow walk.
    #[serde(default)]
    pub chilled: u32,
    /// Held by a bog hand's grip: ticks left at a wade (`horrors::GRIP`).
    #[serde(default, skip_serializing_if = "super::zero")]
    pub mired: u32,
    /// A Fae Dagger's steps, waiting for the next move; the Pendragon
    /// Sceptre's ticks left; a Censer's mending, waiting to go out.
    #[serde(default)]
    pub blink: u32,
    #[serde(default)]
    pub immune: u32,
    #[serde(default)]
    pub censer: u32,
    /// Hears Sir Dinadan's song this tick.
    #[serde(default)]
    pub singing: bool,
    /// Thrown by a Ravage: ticks left in the air, helpless.
    #[serde(default)]
    pub tossed: u32,
    /// On a friend's shoulders or thrown off them (`shoulders`): no
    /// monster's mark, and no hit lands on them (a rider takes a share of
    /// the carrier's instead). Set by the rules each tick.
    #[serde(
        default,
        skip_serializing_if = "crate::rules::runtime_state::omit",
        deserialize_with = "crate::rules::runtime_state::transient"
    )]
    pub(super) aloft: bool,
    /// What the moment is worth to a mend, what a hit weighs (percent),
    /// and what passive mends have added this second (`stakes`).
    #[serde(default)]
    pub stakes: stakes::Stakes,
    #[serde(default = "hundred")]
    pub incoming: u32,
    #[serde(default)]
    pub passive: u32,
    /// What lifesteal has given this second (`stakes::LEECH_PER_SECOND`).
    #[serde(default)]
    pub leeched: u32,
    /// Ticks left in which a second draught is worth less, and in which a
    /// second ward holds less.
    #[serde(default)]
    pub(super) sipped: u32,
    #[serde(default)]
    pub(super) ward_rest: u32,
    /// An axe spin under way: ticks left, from `spin::TICKS` down.
    #[serde(default)]
    pub spin: u32,
    /// The Axe Storm: ticks left of spinning without a rest.
    #[serde(default)]
    pub whirl: u32,
    /// What the knight still wears: 0 whole, 1 bare-headed, 2 in their
    /// smallclothes (`gear::stage`); the stage before the last change and
    /// the ticks since the helmet went flying, for the pop.
    #[serde(default)]
    pub gear: u8,
    #[serde(default)]
    pub gear_was: u8,
    #[serde(default)]
    pub pop: u32,
    /// The take key was down last tick, and its press waits for `gather`.
    #[serde(default)]
    pub(super) take_key: bool,
    #[serde(default)]
    pub(super) taking: bool,
    /// The next-card key was down last tick (`cycle_hand`). Left out of a
    /// save while it is up, so a run without it writes as it always did.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub(super) cycle_key: bool,
    /// A hit just landed: ticks left in the flinch (`spectacle::STRUCK`
    /// down), whether it was a heavy one, and the way the knight is thrown
    /// back (away from whatever struck).
    #[serde(default)]
    pub struck: u32,
    #[serde(default)]
    pub struck_big: bool,
    #[serde(default)]
    pub struck_dir: (i8, i8),
    /// The tick this knight fell, for the bones to scatter from.
    #[serde(default)]
    pub fell: u64,
    /// Raised by a friend's carried skull into a weak monster's body: the
    /// monster this knight walks as until the next Sanctuary (`skulls`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub possessed: Option<EnemyKind>,
    /// Ticks till a borrowed body's spell is ready again (`bodies`).
    #[serde(default, skip_serializing_if = "no_rearm")]
    pub body_rearm: u32,
    /// Wishes this knight holds (`overclass`): three when a delve starts,
    /// two at each new floor.
    #[serde(default)]
    pub wishes: u8,
    /// Super wishes held: one each time a floor's boss falls. A super wish
    /// takes two steps of a wish's ladder at once.
    #[serde(default)]
    pub great_wishes: u8,
    /// Fairy wishes held (`nooks::gnome`): the next wish lands three steps
    /// up at once, a new one at its third tier, Legendary.
    #[serde(default, skip_serializing_if = "is_zero_u8")]
    pub fairy_wishes: u8,
    /// Freedom wishes held (`freedom`): given when the genie is set free,
    /// spent on the safe room or the weapons locker, never on the knight.
    /// The realm keeps as many by the knight's name.
    #[serde(default, skip_serializing_if = "is_zero_u8")]
    pub freedom_wishes: u8,
    /// A freedom wish begun at a wishing spot: the next words said are for
    /// this place (`freedom`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub place_wish: Option<super::freedom::Place>,
    /// A shooting star's wish, caught standing under it (`starfall`): one
    /// wish, free, spent before the knight's own, at one step of a ladder.
    /// It fades when the party leaves the star's room, and with the floor.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub star_wish: bool,
    /// The shape this knight began as, under the identity start
    /// (`identity`): none for a knight who began with a kit.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shape: Option<super::identity::Shape>,
    /// Wished whole once (`identity::Making::made`), and so for good.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub made: bool,
    /// What this knight bought in the bazaar (`curios`): their pockets.
    /// Left off the wire while empty, so an older seat reads the rest.
    #[serde(default, skip_serializing_if = "super::curios::Pockets::is_empty")]
    pub pockets: super::curios::Pockets,
    /// A wish asked for and not yet sealed: the scroll's choices (`genie`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub offer: Option<super::genie::Offer>,
    /// The next wish is ink alone, handed off by the tavern mirror.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub mirror_wish: bool,
    /// The whole-body wish looks (by card id, a mirror look by its key) this
    /// knight set aside by picking one of the four code-drawn knights at the
    /// pub (`Run::pick_patron`): each wish keeps its power, only its whole
    /// picture is not drawn over the design. A look worn after the pick is
    /// not in it, and draws as it always has.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub set_aside: Vec<String>,
    /// The floor whose budget holds this knight's wishes (`cards::WISH_BUDGET`):
    /// the floor they stand on, kept by `Run::step`.
    #[serde(default, skip_serializing_if = "is_zero_floor")]
    pub wish_floor: u32,
    /// A summoner's wishes for his dead, or for him and them both, by the
    /// wish's id (`dead_wishes`): a wish not here is his own.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub dead_wishes: BTreeMap<String, super::dead_wishes::Target>,
    /// What those wishes give each of his dead, counted with `bonus`.
    #[serde(default, skip_serializing_if = "super::dead_wishes::Gifts::none")]
    pub dead_gifts: super::dead_wishes::Gifts,
    /// The rough block of stone this knight carries up from a claimed face,
    /// by its stone's catalogue id (`quarrying`): heavy, so it slows them.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub block: Option<String>,
    /// The fallen friend whose skull this knight carries (`skulls`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub carrying_skull: Option<u32>,
    /// Fallen: where this knight's skull lies when a carrier dropped it
    /// away from the bones (`skulls`); none while it lies with them.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub skull_at: Option<(f32, f32)>,
    /// Ground covered since the last footstep.
    #[serde(default)]
    pub stride: f32,
    /// The tool of the trade in this knight's hand, at the Keep's site
    /// (`trades::Tool::code`); 0 for bare hands. The host sets it, and every
    /// view draws it in the knight's hand (`hands`).
    #[serde(default, skip_serializing_if = "no_tool")]
    pub tool: u8,
    pub fire_cooldown: u32,
    pub dash_ticks: u32,
    pub(super) dash_x: f32,
    pub(super) dash_y: f32,
    /// The way this knight walks with no aim held, and for how many ticks
    /// they have kept to it: after [`super::TURN_TO_WALK`] they face it.
    #[serde(default)]
    pub(super) walk_dir: (i8, i8),
    #[serde(default)]
    pub(super) walk_ticks: u32,
    /// A champion's own going: a wind-up, a fury, a stink, a haul
    /// (`champions`). Left out when still.
    #[serde(default, skip_serializing_if = "champions::Stance::idle")]
    pub champ: champions::Stance,
    #[serde(default, skip_serializing_if = "heroes::Stance::is_empty")]
    pub hero_kit: heroes::Stance,
    /// A Mage's essences and spells' cooldowns (`weave`).
    #[serde(default, skip_serializing_if = "weave::Weave::is_empty")]
    pub weave: weave::Weave,
    /// The genie's wish scroll is open on its typed line (no offer yet).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub wish_open: bool,
    /// Letters typed on that line. Empty while the line is open and blank.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub wish_line: String,
    /// Which choice row is highlighted (0, 1 or 2).
    #[serde(default, skip_serializing_if = "no_tool")]
    pub wish_pick: u8,
}

impl Hero {
    pub(super) fn new(name: &str, x: f32, y: f32) -> Self {
        Hero {
            look: Default::default(),
            mirror_key: String::new(),
            mirror: Default::default(),
            squire: None,
            name: name.chars().filter(|c| !c.is_control()).take(32).collect(),
            x,
            y,
            camera: None,
            stone: false,
            idle: false,
            vigil: false,
            vigil_key: false,
            vigil_rearm: 0,
            away: false,
            absent: false,
            hit_ward: HIT_WARD,
            hp: 100,
            max_hp: 100,
            aim_x: 0.0,
            aim_y: -1.0,
            // A new knight is warded as long as a hit wards one.
            invulnerable: HIT_WARD,
            flee_ticks: 0,
            door_behind: None,
            blown: 0,
            dash_cooldown: 0,
            bomb_cooldown: 0,
            bombs: 2,
            bomb_capacity: bombs::old_belt(),
            armor: 0,
            weapon: Weapon::Bow,
            owned: gear::Owned::equipped(),
            forged: None,
            avatar: None,
            swing: 0,
            sword: false,
            sword_cooldown: 0,
            parrying: None,
            hand: Vec::new(),
            spells: Default::default(),
            module_spells: Default::default(),
            spell_cooldowns: [0; cards::SPELL_SLOTS],
            spell_tiers: BTreeMap::new(),
            bar: [None, None, None, None],
            deck: Vec::new(),
            arm: bow_card(),
            temper: 0,
            bonus: cards::Bonus::default(),
            carried: Spoils::default(),
            play_cooldown: 0,
            patience: 0,
            guard: cards::Guard::Roll,
            guard_card: roll_card(),
            shielding: false,
            shield_wear: shield::Wear::default(),
            walling: false,
            wall_spent: false,
            mana: MAX_MANA,
            mana_rest: 0,
            reforged_on: None,
            reforge_job: None,
            wish_jobs: Vec::new(),
            knight: None,
            house: None,
            vy: 0.0,
            grounded: false,
            air_jumps: 0,
            jump_held: false,
            dropping: 0,
            footing: None,
            privy: 0,
            laurel: false,
            sitting: false,
            soak: 0,
            scrubbed: false,
            class: None,
            stocked: false,
            splats: 0,
            booby: None,
            pit: Default::default(),
            rider: false,
            boon: Default::default(),
            loosed: 0,
            groove: 0,
            songs: 0,
            tune: 0,
            strum: 0,
            relieved: None,
            home_hp: 0,
            winds: 0,
            talisman: false,
            buying: 0,
            buy_spent: false,
            glass: false,
            ult_charge: 0,
            ult_key: false,
            angel: 0,
            blessed: 0,
            slashes: 0,
            slash_wait: 0,
            aiming: None,
            rune: None,
            ult: None,
            recruit_ult: None,
            recruits_asked: Vec::new(),
            rite_owed: false,
            rite_at: 0,
            hexed: 0,
            chilled: 0,
            mired: 0,
            blink: 0,
            immune: 0,
            censer: 0,
            singing: false,
            tossed: 0,
            aloft: false,
            stakes: stakes::Stakes::Calm,
            incoming: 100,
            passive: 0,
            leeched: 0,
            sipped: 0,
            ward_rest: 0,
            spin: 0,
            whirl: 0,
            gear: 0,
            gear_was: 0,
            pop: 0,
            shape: None,
            made: false,
            pockets: Default::default(),
            take_key: false,
            taking: false,
            cycle_key: false,
            struck: 0,
            struck_big: false,
            struck_dir: (0, 0),
            fell: 0,
            possessed: None,
            body_rearm: 0,
            wishes: 0,
            great_wishes: 0,
            fairy_wishes: 0,
            freedom_wishes: 0,
            place_wish: None,
            star_wish: false,
            offer: None,
            mirror_wish: false,
            set_aside: Vec::new(),
            wish_floor: 0,
            dead_wishes: BTreeMap::new(),
            dead_gifts: Default::default(),
            block: None,
            carrying_skull: None,
            skull_at: None,
            stride: 0.0,
            tool: 0,
            fire_cooldown: 0,
            dash_ticks: 0,
            dash_x: 0.0,
            dash_y: 0.0,
            walk_dir: (0, 0),
            walk_ticks: 0,
            champ: champions::Stance::default(),
            hero_kit: heroes::Stance::default(),
            weave: weave::Weave::default(),
            wish_open: false,
            wish_line: String::new(),
            wish_pick: 0,
        }
    }

    /// Mail turns aside a share of every hit by the pieces worn, up to
    /// `MAX_ARMOR` including Sir Dinadan's iron song; a
    /// hit always stings a little.
    /// A knight who fights up close (a blade and no bow) takes a third
    /// less, as melee does in the games that do it well: they must stand
    /// in it.
    /// Windrun evades attacks, not fire floors, poison or other hazards.
    pub(super) fn attack_hurt(&mut self, damage: u32) {
        // Behind the Lancer's set lance a blow lands at half.
        let damage = if self.champ.brace > 0 { damage * champions::BRACE_TAKES / 100 } else { damage };
        if self.hero_kit.windrun == 0 { self.hurt(damage); }
    }

    pub fn hurt(&mut self, damage: u32) {
        self.hurt_most(damage, u32::MAX);
    }

    /// A guardian's charge-up strikes (`signatures`): a hit like any other,
    /// but taking at most `BLOW_MOST` of the knight's health, and only once
    /// for charge-up `tag` — the knights it has struck are those that carry
    /// its tag. True if it struck. A Peasant deserting takes none of it.
    pub(super) fn blow(&mut self, damage: u32, tag: u32) -> bool {
        if self.stone || self.flee_ticks > 0 || self.aloft || self.invulnerable > 0 || self.hp == 0 || self.blown == tag {
            return false;
        }
        self.blown = tag;
        self.hurt_most(damage, self.max_hp * BLOW_MOST / 100);
        true
    }

    /// A hit, taking at most `most` once every multiplier is in.
    fn hurt_most(&mut self, damage: u32, most: u32) {
        if self.flee_ticks > 0 || self.aloft { return; }
        // A booby prize from the countryside takes the next blow whole, then
        // flies off: the loser's consolation.
        if !self.stone && self.invulnerable == 0 && self.hp > 0 && self.booby.is_some() {
            self.booby = None;
            self.invulnerable = 24;
            return;
        }
        if !self.stone && self.invulnerable == 0 && self.hp > 0 {
            let close = self
                .forged
                .as_ref()
                .is_some_and(|w| w.bolt.is_none() && w.melee.is_some());
            let damage = if close { damage * 2 / 3 } else { damage };
            let damage = if self.glass { damage * 2 } else { damage };
            // The Monk's Patience: half of every blow, and no complaints.
            let damage = if self.patience > 0 { damage / 2 } else { damage };
            // A bucket turns the first blow in a room aside whole; other
            // curios make every blow more or less (`curios`).
            let Some(damage) = self.curio_turns(damage) else {
                self.invulnerable = self.hit_ward;
                return;
            };
            // What the floor and the party's size make of the hit; then the
            // share the mail turns aside.
            let damage = damage * self.incoming / 100;
            // Health counts for half: every hit lands twice as hard.
            let damage = damage * HIT_SHARE / 100;
            // A dressed knight takes the hit as it always was; each missing
            // piece lets more through, so found gear is real protection.
            let exposed = if self.peasant() { 0 } else {
                u32::from(!self.owned.body) * gear::BARE_BODY
                    + u32::from(!self.owned.helmet) * gear::BARE_HEAD
            };
            let damage = damage * (100 + exposed) / 100;
            let turned = if self.peasant() { 0 } else { ARMOR_TURNS[self.armor.min(MAX_ARMOR) as usize] };
            let most = most.min(self.max_hp * ONE_HIT_MOST / 100);
            let damage = (damage * (100 - turned) / 100).max(4).min(most.max(1));
            // A living name-owned shieldbearer can take half the final hit.
            // Do not let a renamed/reused seat borrow the previous owner's guard.
            let owner = self.name.trim().to_lowercase();
            let damage = self.squire.as_mut().filter(|s| s.owner == owner)
                .map_or(damage, |s| s.guard(self.x, self.y, damage));
            self.hp = self.hp.saturating_sub(damage);
            self.invulnerable = self.hit_ward;
            self.curio_biscuit();
            // A hit breaks Regeneration's mending.
            if self.has_rune(super::runes::RuneKind::Regeneration) {
                self.rune = None;
            }
        }
    }
}

/// After a hit, a knight can't be hurt again for this many ticks: a second
/// and a half at 30 Hz (the operator, 2026-10-18: "player invuln for one
/// sec after they are hit"; 2026-10-20, after five friends were felled in
/// their first room: "more invuln frames, we are getting burst down too
/// much"). It blinks while it lasts, and nothing that hurts a knight
/// strips it. The panel's `hit_ward_s` moves it (`Tune::hit_ward`).
pub const HIT_WARD: u32 = super::HZ * 3 / 2;

fn no_rearm(n: &u32) -> bool {
    *n == 0
}

pub(super) fn hit_ward() -> u32 {
    HIT_WARD
}

fn hundred() -> u32 {
    100
}

/// Percent of a hit that mail turns aside, by pieces worn. Songs and cards
/// share the same ceiling.
const ARMOR_TURNS: [u32; 3] = [0, 6, 11];

/// The share of a hit `armor` pieces of mail turn aside, in percent: a
/// knight's, or one of a summoner's dead (`dead_wishes`).
pub(super) fn armor_turns(armor: u32) -> u32 {
    ARMOR_TURNS[armor.min(MAX_ARMOR) as usize]
}

fn bow_card() -> Option<String> {
    Some("bow".into())
}

fn full_mana() -> f32 {
    MAX_MANA
}

impl Hero {
    /// The held wall's ends, across the aim and a little ahead.
    pub fn wall_span(&self) -> Option<((f32, f32), (f32, f32))> {
        let cards::Guard::Wall(wall) = self.guard else {
            return None;
        };
        let (cx, cy) = (
            self.x + self.aim_x * WALL_REACH,
            self.y + self.aim_y * WALL_REACH,
        );
        let (px, py) = (-self.aim_y, self.aim_x);
        let half = wall.width as f32 / 2.0;
        Some((
            (cx - px * half, cy - py * half),
            (cx + px * half, cy + py * half),
        ))
    }

    pub(super) fn wall_empower(&self) -> u32 {
        match self.guard {
            cards::Guard::Wall(wall) => wall.empower,
            _ => 0,
        }
    }

    /// The walk keys as they move this knight: the key toward the doorway
    /// it just came out of (`door_behind`) walks nowhere while it is still
    /// down. Let go (or another way taken), the doorway is an ordinary one
    /// again, and walking into it goes back in on purpose.
    pub(super) fn held_off(&mut self, mut input: Input) -> Input {
        let Some(side) = self.door_behind else {
            return input;
        };
        let (dx, dy) = DIRS[usize::from(side)];
        let toward = (dx != 0 && i32::from(input.move_x.signum()) == dx)
            || (dy != 0 && i32::from(input.move_y.signum()) == dy);
        if !toward {
            self.door_behind = None;
        } else if dx != 0 {
            input.move_x = 0;
        } else {
            input.move_y = 0;
        }
        input
    }
}

fn roll_card() -> Option<String> {
    Some("dodge-roll".into())
}

impl Hero {
    /// Card `from` in hand moved to slot `to`, the rest closing up round
    /// it: a knight lining up what the quaff key drinks next (slot 0).
    /// Refused (false) for a slot past the hand, or a move to where it is.
    pub fn move_card(&mut self, from: usize, to: usize) -> bool {
        if from == to || from >= self.hand.len() || to >= self.hand.len() {
            return false;
        }
        let card = self.hand.remove(from);
        self.hand.insert(to, card);
        true
    }

    /// The next-card key: the first card in hand goes to the back, and the
    /// one behind it is next. Nothing for a hand of one.
    pub fn cycle_hand(&mut self) -> bool {
        self.move_card(0, self.hand.len().saturating_sub(1))
    }

    /// Whether walking over `card` takes it.
    pub(super) fn takes(&self, card: &Card) -> bool {
        use cards::{Effect, Kind};
        match card.id.as_str() {
            "iron-helm" => return !self.owned.helmet,
            "body-armour" => return !self.owned.body,
            _ => {}
        }
        match card.kind {
            Kind::Hold | Kind::Arm => true,
            // The guard already held is no swap: it lies on for the others.
            Kind::Guard => self.guard_card.as_deref() != Some(card.id.as_str()),
            Kind::Play => self.hand.len() < cards::HAND,
            Kind::Spell => {
                !self.spells.iter().flatten().any(|id| id == &card.id)
                    && self.spells.iter().any(Option::is_none)
            }
            // A take card waits on the floor while it would do nothing.
            Kind::Take => card.effects.iter().any(|e| match *e {
                Effect::Heal(_) => self.hp < self.max_hp,
                Effect::Bombs(_) => self.bombs < self.bomb_limit(),
                _ => true,
            }),
        }
    }

    /// A take or play card's effects, now. Nova damage is summed into `nova`.
    pub fn spend(&mut self, card: &Card, score: &mut u32, nova: &mut u32) {
        use cards::Effect;
        for effect in &card.effects {
            if let Some((spoil, n)) = effect.spoil() {
                let n = if spoil == Spoil::Gold { self.curio_gold(n) } else { n };
                self.carried.add(spoil, n);
            }
            match *effect {
                Effect::Heal(v) => {
                    let v = if card.kind == cards::Kind::Spell { self.spell_power(&card.id, v) } else { v };
                    self.mend(v, stakes::Mend::Potion);
                }
                Effect::Gold(v) => *score += v,
                _ if effect.spoil().is_some() => {}
                Effect::Bombs(v) => self.give_bombs(v),
                Effect::MaxHp(v) => {
                    self.max_hp += v;
                    self.hp = (self.hp + v).min(self.max_hp);
                }
                Effect::Nova(v) => {
                    *nova += if card.kind == cards::Kind::Spell { self.spell_power(&card.id, v) } else { v };
                }
                Effect::Ward(v) => {
                    let v = if card.kind == cards::Kind::Spell { self.spell_power(&card.id, v) } else { v };
                    // A second ward soon after the first holds only a third as long.
                    let early = self.stakes == stakes::Stakes::Early;
                    let secs = if self.ward_rest > 0 && !early {
                        v.div_ceil(3)
                    } else {
                        v
                    };
                    self.invulnerable = self.invulnerable.max(secs * HZ);
                    self.ward_rest = stakes::WARD_REST;
                }
                Effect::Blink(v) => self.blink = v,
                Effect::Immune(v) => self.immune = self.immune.max(v * HZ),
                Effect::Censer(v) => self.censer = self.censer.saturating_add(v),
                _ => {}
            }
        }
    }

    /// Count a shot; true when this one is a volley (every Nth flies three
    /// wide, by the tightest cadence held).
    pub(super) fn volley(&mut self) -> bool {
        self.loosed = self.loosed.wrapping_add(1);
        self.bonus.volley > 0 && self.loosed.is_multiple_of(self.bonus.volley)
    }

    /// Recount the held cards' bonus after the deck changed.
    pub fn rebonus(&mut self, book: &Book) {
        // The curios' held cards count with the deck's, under the same caps.
        let floor = Some(self.wish_floor);
        if self.dead_wishes.is_empty() || !super::minions::summoner(self) {
            self.bonus = cards::Bonus::within(
                self.deck
                    .iter()
                    .filter_map(|id| book.get(id))
                    .chain(super::curios::held(&self.pockets)),
                floor,
            );
            self.dead_gifts = Default::default();
        } else {
            // A summoner's wishes for his dead go on them, not on him, and
            // a wish for both half on each (`dead_wishes`).
            let (his, theirs) = self.held_split(book);
            self.bonus = cards::Bonus::within(his.iter().map(|c| &**c).chain(super::curios::held(&self.pockets)), floor);
            self.dead_gifts = super::dead_wishes::Gifts::of(theirs.iter().map(|c| &**c), floor);
        }
        self.armor = if self.peasant() { 0 } else { self.bonus.armor.min(MAX_ARMOR) };
        self.fit_hp();
    }

    pub(crate) fn equip(&mut self, card: &Card) {
        if self.arm.as_deref() != Some(card.id.as_str()) {
            self.temper = 0;
        }
        match &card.arm {
            Some(cards::Arm::Plain(weapon)) => {
                self.weapon = *weapon;
                self.forged = None;
            }
            Some(cards::Arm::Forged(weapon)) => self.forged = Some(weapon.clone()),
            None => return,
        }
        self.arm = Some(card.id.clone());
    }

    /// Whether `card` is the plain arm already in hand, so taking it
    /// tempers it.
    pub(super) fn tempers(&self, card: &Card) -> bool {
        matches!(card.arm, Some(cards::Arm::Plain(w)) if w == self.weapon) && self.forged.is_none()
    }

    /// Percent a pilgrim's blessing adds to damage and pace while it lasts.
    pub fn blessing(&self) -> u32 {
        if self.blessed > 0 { super::wayfarers::BLESS_PERCENT } else { 0 }
    }

    /// Ticks left in a dodge roll, while one lasts.
    pub fn rolling(&self) -> Option<u32> {
        (self.dash_ticks > 0).then_some(self.dash_ticks)
    }

    /// The way the current (or last) roll went.
    pub fn roll_dir(&self) -> (f32, f32) {
        (self.dash_x, self.dash_y)
    }

    pub fn scaled(&self, damage: u32) -> u32 {
        let damage = damage * (100 + self.bonus.damage + self.pit.hit() + self.boon.damage + self.blessing()) / 100;
        // What curios give and take (`curios`).
        let damage = damage * self.curio_damage() / 100;
        let damage = if self.has_rune(super::runes::RuneKind::DoubleDamage) {
            damage * 2
        } else {
            damage
        };
        if self.glass { damage * 2 } else { damage }
    }

    /// The ultimate this knight casts: their own, or All Random's.
    pub fn ult(&self) -> super::ults::Ult {
        self.ult
            .unwrap_or_else(|| super::ults::Ult::of(self.knight.as_deref()))
    }

    pub fn cooldown(&self, ticks: u32) -> u32 {
        (ticks * 100 / (100 + self.bonus.rate + self.boon.rate + self.hero_kit.attack_rate()) * self.curio_wait() / 100).max(1)
    }

    /// The beat of the pull just loosed: the ticks it set before the next
    /// (`fire_cooldown`), what its shots' size follows
    /// (`ShotTraits::beat`).
    pub(super) fn beat(&self) -> u8 {
        self.fire_cooldown.clamp(1, u32::from(u8::MAX)) as u8
    }
}

impl Run {
    /// Knight `id`'s own hand rearranged from their seat, between ticks: a
    /// card dragged from slot `from` onto slot `to` (`Hero::move_card`).
    /// False when there is no such knight or no such move.
    pub fn move_card(&mut self, id: u32, from: usize, to: usize) -> bool {
        self.players.get_mut(&id).is_some_and(|hero| hero.move_card(from, to))
    }
}

fn is_zero_floor(n: &u32) -> bool {
    *n == 0
}

/// What a knight's guard key did on a tick, for the sim to sound out.
#[derive(Clone, Copy, Debug, Default)]
pub(super) struct Guarded {
    pub rolled: bool,
    pub spun: bool,
    pub wall_raised: bool,
    pub mana_out: bool,
    /// A spin's age on this tick, before it ran down: when its axes cut.
    pub spin_age: Option<u32>,
}

impl Hero {
    /// A knight's own keys, the first half of its going on a tick: the
    /// roll's rearm runs down, the walk and the aim are read, and the guard
    /// key acts (a roll starts, a shield or a wall goes up, a spin turns),
    /// a whirl keeps the spin going and mana rests. The sim steps it
    /// (`Run::play`), and a client drawing its own knight ahead steps it
    /// the same (`Run::predict`). Returns the walk.
    pub(super) fn own_keys(&mut self, input: &Input, side: bool, shield_free: bool) -> ((f32, f32), Guarded) {
        self.hero_kit.tick();
        // Learned spells survive kit changes, so Power need not be on key 1.
        // Resolve the held spell here for both the host and prediction.
        let power_held = usize::from(input.cast)
            .checked_sub(1)
            .and_then(|slot| self.spells.get(slot))
            .and_then(Option::as_deref) == Some("gale-power");
        self.hero_kit.keys(power_held);
        let held = self.held_off(*input);
        let input = &held;
        self.flee_ticks = self.flee_ticks.saturating_sub(1);
        let input = if self.flee_ticks > 0 {
            &Input { move_x: input.move_x, move_y: input.move_y, aim_x: input.aim_x, aim_y: input.aim_y, ..Default::default() }
        } else { input };
        let mut guarded = Guarded::default();
        self.dash_cooldown = self.dash_cooldown.saturating_sub(1);
        // In the stocks a knight can aim and throw back, but not walk.
        let (mx, my) = if self.stocked {
            (0.0, 0.0)
        } else {
            unit(input.move_x as f32, input.move_y as f32)
        };
        let (ax, ay) = unit(input.aim_x as f32, input.aim_y as f32);
        // A Swashbuckle dash is steered by the aim held this tick.
        if self.champ.swash.dashing() {
            self.champ.swash.steer = (ax, ay);
        }
        let walk = if self.stocked { (0, 0) } else { (input.move_x.signum(), input.move_y.signum()) };
        if ax != 0.0 || ay != 0.0 || walk == (0, 0) || walk != self.walk_dir {
            self.walk_ticks = 0;
        } else {
            self.walk_ticks = self.walk_ticks.saturating_add(1);
        }
        self.walk_dir = if ax != 0.0 || ay != 0.0 { (0, 0) } else { walk };
        if ax != 0.0 || ay != 0.0 {
            self.aim_x = ax;
            self.aim_y = ay;
        } else if side && mx != 0.0 {
            // Side-on, a knight faces the way they run.
            (self.aim_x, self.aim_y) = (mx.signum(), 0.0);
        } else if self.walk_ticks >= TURN_TO_WALK {
            // Top-down, a knight who keeps walking one way with no aim held
            // turns to face it.
            (self.aim_x, self.aim_y) = (mx, my);
        }
        self.shielding = false;
        self.shield_wear.rest(self.max_hp);
        match self.guard {
            cards::Guard::Roll => {
                if input.dash && self.dash_cooldown == 0 && self.dash_ticks == 0 {
                    // Roll where you're going, or where you aim when standing.
                    let (rx, ry) = if side {
                        // Side-on, a roll runs along the ground.
                        let along = if mx != 0.0 { mx } else { self.aim_x };
                        (if along < 0.0 { -1.0 } else { 1.0 }, 0.0)
                    } else if mx != 0.0 || my != 0.0 {
                        (mx, my)
                    } else {
                        (self.aim_x, self.aim_y)
                    };
                    self.dash_ticks = ROLL_TICKS;
                    self.dash_x = rx;
                    self.dash_y = ry;
                    self.invulnerable = self.invulnerable.max(ROLL_TICKS + 1);
                    self.dash_cooldown = ROLL_REARM;
                    guarded.rolled = true;
                }
            }
            // A shield in the air is not in the hand, nor one knocked aside.
            cards::Guard::Shield => {
                self.shielding = input.dash && shield_free && self.shield_wear.down == 0;
            }
            cards::Guard::Spin => {
                if input.dash && self.dash_cooldown == 0 && self.spin == 0 {
                    self.spin = spin::TICKS;
                    self.dash_cooldown = spin::REARM;
                    guarded.spun = true;
                }
            }
            cards::Guard::Wall(wall) => {
                // Held while there is mana; once spent, it must be let go
                // and raised again.
                if !input.dash {
                    self.wall_spent = false;
                }
                let up = input.dash && !self.wall_spent && self.mana > 0.0;
                if up {
                    guarded.wall_raised = !self.walling;
                    self.mana = (self.mana - wall.drain as f32 * DT).max(0.0);
                    self.mana_rest = 0;
                    if self.mana <= 0.0 {
                        self.wall_spent = true;
                        guarded.mana_out = true;
                    }
                }
                self.walling = up && self.mana > 0.0;
            }
        }
        if self.whirl > 0 {
            self.whirl -= 1;
            if self.spin == 0 {
                self.spin = spin::TICKS;
            }
        }
        if self.spin > 0 {
            guarded.spin_age = Some(spin::TICKS.saturating_sub(self.spin));
            self.spin -= 1;
        }
        if !self.walling {
            self.mana_rest = self.mana_rest.saturating_add(1);
            if self.mana_rest > MANA_REST {
                self.mana = (self.mana + MANA_FLOW * DT).min(MAX_MANA);
            }
        }
        ((mx, my), guarded)
    }

    /// The second half: a Fae Dagger's blink lands (played last tick:
    /// there, now), then the step this tick takes along `walk`, as
    /// (dx, dy, speed): a roll running down, fast out of the tuck and
    /// slowing as the knight comes up; a spin's, a shield's or a wall's
    /// pace; or the walk at `pace`, read where the knight then stands.
    /// Also whether it blinked.
    pub(super) fn own_step(
        &mut self,
        (mx, my): (f32, f32),
        grid: &Grid,
        roll_speed: f32,
        pace: impl FnOnce(&Hero) -> f32,
    ) -> ((f32, f32, f32), bool) {
        let blinked = self.blink > 0;
        if blinked {
            items::blink(self, grid);
        }
        // A Swashbuckle dash moves the knight itself, wall to wall.
        if self.champ.swash.dashing() {
            self.swash_run(grid);
            return ((0.0, 0.0, 0.0), blinked);
        }
        let pace = pace(self) * if self.hero_kit.windrun > 0 { 1.65 } else if self.hero_kit.charging { 0.45 } else { 1.0 };
        let pace = if self.champ.lust > 0 { pace * champions::LUST_PACE } else { pace };
        let step = if self.champ.haul > 0 {
            // Hauled along the Butcher's chain, fast and straight.
            self.champ.haul -= 1;
            (self.champ.haul_x, self.champ.haul_y, champions::HAUL_SPEED)
        } else if self.champ.dash > 0 {
            // A champion's bound: the Reaver's leap, the Lancer's vault or drive.
            self.champ.dash -= 1;
            (self.champ.dash_x, self.champ.dash_y, self.champ.dash_kind.speed())
        } else if self.champ.windup > 0 || self.champ.brace > 0 {
            // Rooted under the maul's wind-up, or behind the set lance.
            (mx, my, 0.0)
        } else if self.flee_ticks > 0 {
            let (dx, dy) = if mx == 0.0 && my == 0.0 { (self.aim_x, self.aim_y) } else { (mx, my) };
            (dx, dy, pace * peasant::SPRINT_PACE)
        } else if self.dash_ticks > 0 {
            self.dash_ticks -= 1;
            let ease = 0.55 + 0.45 * self.dash_ticks as f32 / ROLL_TICKS as f32;
            (self.dash_x, self.dash_y, roll_speed * ease)
        } else if self.spin > 0 {
            (mx, my, pace * spin::PACE)
        } else if self.shielding {
            (mx, my, pace * SHIELD_PACE)
        } else if self.walling {
            (mx, my, pace * WALL_PACE)
        } else {
            (mx, my, pace)
        };
        (step, blinked)
    }
}
/// What a knight's ranged arm let fly on a tick it fired (`Hero::loose`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Loosed {
    /// A forged blade, thrown to come back.
    Throw,
    /// A forged weapon's bolts, this many.
    Bolts(u32),
    /// A plain arm's shots, this many, along its aim.
    Arm(u32, (f32, f32)),
}

impl Hero {
    /// The shots this knight's ranged arm looses on a tick its fire key
    /// fires, into `shots`, with its cooldown set: a forged weapon's thrown
    /// blade or its bolts, else the plain arm's (`Weapon::arms`), with every
    /// card, temper and volley it carries. Fists and a forged weapon's swing
    /// strike rather than shoot; the sim does those itself. The sim fires by
    /// it (`Run::play`), and so does a client drawing its own knight's shots
    /// ahead of the server (`Run::predict_fire`), so the two loose alike.
    /// `burning`: the bow's Fire Arrows are lit; `rng`: the tick's dice,
    /// for how a stream strays. `None` for a forged weapon that neither
    /// throws nor shoots. `flying`: the knight's shots in flight already
    /// (`volley_in_flight`): what is loosed past `KNIGHT_VOLLEY` stays in the
    /// quiver, the volley's middle flying (`fit_volley`).
    pub fn loose(
        &mut self,
        id: u32,
        book: &Book,
        burning: bool,
        rng: u64,
        flying: usize,
        shots: &mut Vec<Projectile>,
    ) -> Option<Loosed> {
        let from = shots.len();
        let loosed = heroes::loose(self, id, shots).or_else(|| self.loose_all(id, book, burning, rng, shots));
        fit_volley(shots, from, KNIGHT_VOLLEY.saturating_sub(flying));
        loosed
    }

    /// `loose`, every shot the arm and its cards make.
    fn loose_all(
        &mut self,
        id: u32,
        book: &Book,
        burning: bool,
        rng: u64,
        shots: &mut Vec<Projectile>,
    ) -> Option<Loosed> {
        if let Some(weapon) = &self.forged {
            if let Some(throw) = weapon.throw {
                // The blade flies out along the aim and comes back.
                self.fire_cooldown = self.cooldown(throw.every);
                self.swing = 6;
                self.sword = false;
                let (ux, uy) = (self.aim_x, self.aim_y);
                let out = (throw.range / BLADE_SPEED * HZ as f32).ceil() as u16;
                shots.push(Projectile {
                    x: self.x + ux * 0.6,
                    y: self.y + uy * 0.6,
                    vx: ux * BLADE_SPEED,
                    vy: uy * BLADE_SPEED,
                    hostile: false,
                    kind: Shot::Blade,
                    look: self.shot_look(book),
                    damage: self.scaled(throw.damage),
                    pierce: u8::MAX,
                    last_hit: None,
                    empowered: false,
                    ttl: u32::from(out) * 2 + 3 * HZ,
                    traits: ShotTraits {
                        back: 1,
                        owner: id as u8,
                        beat: self.beat(),
                        out,
                        ..ShotTraits::of(&self.bonus)
                    },
                });
                return Some(Loosed::Throw);
            }
            let bolt = weapon.bolt?;
            self.fire_cooldown = self.cooldown(bolt.every);
            // (Hero::volley, by field: the forged weapon is borrowed.)
            self.loosed = self.loosed.wrapping_add(1);
            let volley = self.bonus.volley > 0 && self.loosed % self.bonus.volley == 0;
            let dressed = self.shot_look(book);
            let count = weapon.shots() + self.bonus.shots + if volley { 2 } else { 0 };
            let traits = ShotTraits {
                owner: id as u8,
                beat: self.beat(),
                ..ShotTraits::of(&self.bonus)
            };
            let arc = weapon
                .spread
                .map_or(0.0, |spread| spread.arc.to_radians())
                .max(if volley { VOLLEY_FAN } else { FAN } * (count - 1) as f32);
            let angle = self.aim_y.atan2(self.aim_x) + self.curio_veer(self.loosed);
            // However it was forged, its shots fly fast.
            let speed = bolt.speed.max(cards::FORGED_SPEED);
            for i in 0..count {
                let theta = angle
                    + if count > 1 {
                        arc * (i as f32 / (count - 1) as f32 - 0.5)
                    } else {
                        0.0
                    };
                shots.push(Projectile {
                    x: self.x + theta.cos() * 0.6,
                    y: self.y + theta.sin() * 0.6,
                    vx: theta.cos() * speed,
                    vy: theta.sin() * speed,
                    hostile: false,
                    kind: Shot::Bolt,
                    damage: self.scaled(bolt.damage),
                    pierce: self.bonus.pierce as u8,
                    last_hit: None,
                    empowered: false,
                    ttl: (bolt.range / speed * HZ as f32).ceil() as u32,
                    look: dressed.clone().or_else(|| {
                        Some((weapon.colour.clone(), weapon.shape.clone()))
                    }),
                    traits: traits.clone(),
                });
            }
            return Some(Loosed::Bolts(count));
        }
        let arms = self.weapon.arms();
        let temper = self.temper.min(feel::TEMPER_MAX);
        self.fire_cooldown = self.cooldown(arms.cooldown);
        let volley = self.volley();
        let drill = super::drills::pull(self);
        let extra = self.bonus.shots + if volley { 2 } else { 0 };
        // A card's dressing first; else the class's own shot (a Bard's
        // notes, a Peasant's stones).
        let dressed = self.shot_look(book).or_else(|| match self.weapon {
            Weapon::Notes => Some((classes::SONG.into(), "note".into())),
            Weapon::Stones => Some(("stone".into(), "stone".into())),
            _ => classes::shot_look(self),
        });
        let fan = if volley { VOLLEY_FAN } else { FAN };
        let base = ShotTraits {
            owner: id as u8,
            beat: self.beat(),
            temper,
            fire: burning,
            arm: true,
            stone: self.weapon == Weapon::Stones,
            ..ShotTraits::of(&self.bonus)
        };
        // A stream strays a little, so it looks alive.
        let wobble = (rng % 2001) as f32 / 1000.0 - 1.0;
        let angle = self.aim_y.atan2(self.aim_x) + wobble * arms.jitter + self.curio_veer(self.loosed);
        let (ax, ay) = (angle.cos(), angle.sin());
        // Each pull: the arm's own shape, then tempering's.
        let mut pull: Vec<(f32, f32, ShotTraits)> = Vec::new();
        match self.weapon {
            Weapon::Fists => unreachable!("fists use the punch branch"),
            Weapon::Notes | Weapon::Stones => {
                let count = arms.shots + extra;
                for i in 0..count {
                    let k = i as f32 / (count - 1).max(1) as f32 - 0.5;
                    pull.push((angle + arms.arc.to_radians() * k, 0.0, base.clone()));
                }
            }
            Weapon::Handgonne => {
                // Each extra shot a card gives is another fan's worth.
                let balls =
                    (arms.shots + if temper >= 1 { 2 } else { 0 }) * (1 + extra);
                let arc = arms.arc.to_radians() + fan * extra as f32;
                let traits = ShotTraits {
                    bounce: base.bounce.max(u8::from(temper >= 2)),
                    ..base
                };
                for i in 0..balls {
                    let k = i as f32 / (balls - 1).max(1) as f32 - 0.5;
                    pull.push((angle + arc * k, 0.0, traits.clone()));
                }
            }
            Weapon::Bow | Weapon::Crossbow => {
                // A tempered bow looses streams side by side.
                let lanes: &[f32] = match (self.weapon, temper) {
                    (Weapon::Bow, 0) => &[0.0],
                    (Weapon::Bow, 1) => &[-0.3, 0.3],
                    (Weapon::Bow, _) => &[-0.55, 0.0, 0.55],
                    _ => &[0.0],
                };
                let count = 1 + extra;
                for &lane in lanes {
                    let traits = ShotTraits {
                        // The outer streams of a thrice-tempered bow home.
                        homing: if temper >= 3 && lane != 0.0 {
                            base.homing.max(2)
                        } else {
                            base.homing
                        },
                        // A thrice-tempered rail chains.
                        chain: if self.weapon == Weapon::Crossbow && temper >= 3 {
                            base.chain.max(2)
                        } else {
                            base.chain
                        },
                        ..base.clone()
                    };
                    for i in 0..count {
                        let theta = angle + fan * (i as f32 - (count - 1) as f32 / 2.0);
                        pull.push((theta, lane, traits.clone()));
                    }
                }
            }
        }
        let count = pull.len() as u32;
        for (theta, lane, traits) in pull {
            let (ux, uy) = (theta.cos(), theta.sin());
            shots.push(Projectile {
                x: self.x + ux * 0.6 - ay * lane,
                y: self.y + uy * 0.6 + ax * lane,
                vx: ux * arms.speed,
                vy: uy * arms.speed,
                hostile: false,
                kind: match self.weapon {
                    Weapon::Fists => unreachable!("fists use the punch branch"),
                    Weapon::Bow | Weapon::Notes | Weapon::Stones => Shot::Arrow,
                    Weapon::Crossbow => Shot::Bolt,
                    Weapon::Handgonne => Shot::Ball,
                },
                look: dressed.clone(),
                damage: self.scaled(arms.damage) * drill.damage / 100,
                pierce: arms.pierce.saturating_add(self.bonus.pierce as u8),
                last_hit: None,
                empowered: drill.empowered,
                traits,
                ttl: arms.ttl,
            });
        }
        Some(Loosed::Arm(count, (ax, ay)))
    }
}

fn is_zero_u8(n: &u8) -> bool {
    *n == 0
}

fn is_zero_u64(n: &u64) -> bool {
    *n == 0
}
