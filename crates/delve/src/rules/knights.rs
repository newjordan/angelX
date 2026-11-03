//! The knights a player may take into the Delve: who they are, what they
//! carry in, and the colours they wear. Chosen on the Delve's intro.
//!
//! Each entry is a kit — weapon, guard, hand, ultimate, colours — and the
//! knight who wears it comes from a model house's castle
//! (`crate::world::houses`): the house serving the party's seat names him,
//! in its own words. With no house serving (the stub route), the Keep's own
//! household takes the field under the kits' old names.

use super::{Card, Hero, Pack, Run};
use super::map::houses::{self, HouseId};

/// One knight of the company.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Knight {
    pub id: &'static str,
    pub name: &'static str,
    pub title: &'static str,
    pub about: &'static str,
    /// Cards they start with: weapon, guard, and anything in hand.
    pub arm: &'static str,
    pub guard: &'static str,
    pub hand: &'static [&'static str],
    /// Spells they know in their slots, cast with keys 1 to 4.
    pub spells: &'static [&'static str],
    pub bombs: u32,
    pub max_hp: u32,
    /// Ink swaps on the realm's knight: plume and tabard, shield charge,
    /// and for a knight in coloured plate the steel (the structure greys of
    /// the older drawings, then the hero's stone ramp).
    pub colours: &'static [(char, char)],
}

pub const COMPANY: &[Knight] = &[
    Knight {
        id: "percival",
        name: "Sir Percival",
        title: "the Seeker",
        about: "Asks every question. A good bow, a quick roll, and a potion for the road.",
        arm: "bow",
        guard: "dodge-roll",
        hand: &["potion"],
        spells: &[],
        bombs: 2,
        max_hp: 100,
        colours: &[],
    },
    Knight {
        id: "lynette",
        name: "Dame Lynette",
        title: "the Shieldmaiden",
        about: "Says what she thinks and holds the line. Crossbow and kite shield.",
        arm: "crossbow",
        guard: "kite-shield",
        hand: &[],
        spells: &[],
        bombs: 2,
        max_hp: 100,
        colours: &[('7', '1'), ('8', '0'), ('5', '3'), ('4', '2')],
    },
    Knight {
        id: "gareth",
        name: "Sir Gareth",
        title: "the Kitchen Knight",
        about: "Started in the kitchens; loud and proud of it. Handgonne and extra bombs.",
        arm: "handgonne",
        guard: "dodge-roll",
        hand: &[],
        spells: &[],
        bombs: 4,
        max_hp: 100,
        colours: &[('7', 'A'), ('8', 'l'), ('5', '6'), ('4', '@')],
    },
    Knight {
        id: "galahad",
        name: "Sir Galahad",
        title: "the Pure",
        about: "The one who found it. Close work with a bright blade, and a stout heart.",
        arm: "knights-blade",
        guard: "dodge-roll",
        hand: &[],
        spells: &[],
        bombs: 1,
        max_hp: 130,
        colours: &[('7', 'c'), ('8', '$'), ('5', '9'), ('4', '6')],
    },
    // The posse: each lane of the harness, knighted.
    Knight {
        id: "composer",
        name: "the Composer",
        title: "First on the Scene",
        about: "Parses what you type and submits it clean. Big blade, kite shield, a gold chain. Every third stroke of a chain cuts all the way round, half again as hard.",
        arm: "knights-blade",
        guard: "kite-shield",
        hand: &[],
        spells: &[],
        bombs: 1,
        max_hp: 140,
        colours: &[
            ('H', 'z'), ('i', 'Q'), ('h', 'Q'), ('J', 'q'), ('G', 'S'), ('g', 'x'), ('5', '4'),
            ('W', 'z'), ('V', 'z'), ('v', 'Q'), ('U', 'Q'), ('u', 'q'),
        ],
    },
    Knight {
        id: "dispatcher",
        name: "the Dispatcher",
        title: "Tool In, Receipt Out",
        about: "Every tool on the belt and a capsule for each. Crossbow, quick roll, extra bombs.",
        arm: "crossbow",
        guard: "dodge-roll",
        hand: &["potion"],
        spells: &[],
        bombs: 3,
        max_hp: 100,
        colours: &[
            ('H', 't'), ('i', 'o'), ('h', 'o'), ('J', 'R'), ('G', 'r'), ('g', 'B'), ('7', '3'),
            ('W', 'T'), ('V', 't'), ('v', 'O'), ('U', 'o'), ('u', 'R'), ('x', 'B'), ('s', 'b'),
        ],
    },
    Knight {
        id: "loop",
        name: "the Loop Knight",
        title: "of the Ouroboros",
        about: "Sets the goal, keeps the loop, never skips a beat. Bow and a steady roll. Keep loosing and the string quickens; rest and the beat is lost.",
        arm: "bow",
        guard: "dodge-roll",
        hand: &[],
        spells: &[],
        bombs: 2,
        max_hp: 110,
        colours: &[
            ('H', 'T'), ('i', '6'), ('h', '5'), ('J', '4'), ('G', '@'), ('g', 'a'),
            ('W', 'T'), ('V', '6'), ('v', '5'), ('U', '4'), ('u', '@'), ('x', 'a'),
        ],
    },
    Knight {
        id: "money",
        name: "the Money Knight",
        title: "Cha-Ching",
        about: "Fewer tokens, faster runs, coins everywhere. Handgonne and a sack of bombs. The gold you carry weighs the shot, up to half again as hard.",
        arm: "handgonne",
        guard: "kite-shield",
        hand: &[],
        spells: &[],
        bombs: 4,
        max_hp: 100,
        colours: &[
            ('H', '6'), ('i', '5'), ('h', '4'), ('J', 'C'), ('G', 'e'), ('g', 'F'), ('7', 'y'),
            ('W', '6'), ('V', '5'), ('v', '4'), ('U', 'C'), ('u', 'e'), ('x', 'F'),
        ],
    },
    Knight {
        id: "competition",
        name: "the Competition Knight",
        title: "Leaderboard in Sight",
        about: "Kernels shaved, hashes verified, submitted fast. Chrome plate, crossbow. Rest the bolt a full measure and it strikes two and a half times as hard.",
        arm: "crossbow",
        guard: "dodge-roll",
        hand: &[],
        spells: &[],
        bombs: 2,
        max_hp: 100,
        colours: &[('7', 'C'), ('8', 'E'), ('5', 'y'), ('4', 'C')],
    },
    Knight {
        id: "scryglass",
        name: "the Scryglass Knight",
        title: "Pictures in the Shell",
        about: "Casts the pictures into your terminal. A staff that sparks, a kite shield, a potion for the road; as the Mage, weaves its spells: 1 Ember, 2 Frost, 3 Storm, 4 casts the mix.",
        arm: "spark-staff",
        guard: "kite-shield",
        hand: &["potion"],
        spells: &[],
        bombs: 2,
        max_hp: 100,
        colours: &[
            ('H', '3'), ('i', '2'), ('h', '2'), ('J', '0'), ('G', '0'), ('g', 's'), ('7', 'w'),
            ('W', '3'), ('V', '2'), ('v', '2'), ('U', '0'), ('u', '0'), ('x', 's'),
        ],
    },
    // A dwarf of the deep halls: her own body in the arena, two axes, and a
    // guard that spins them (`spin.rs`).
    Knight {
        id: "ironbraid",
        name: "Dame Thrudi",
        title: "the Ironbraid",
        about: "A dwarf of the deep halls, her beard braided for war. Two axes, and a spin that cuts all round her.",
        arm: "twin-axes",
        guard: "axe-spin",
        hand: &["potion"],
        spells: &[],
        bombs: 3,
        max_hp: 130,
        colours: &[],
    },
    Knight {
        id: "bard",
        name: "the Bard",
        title: "of the Quiet Verse",
        about: "Hold F for a cone of notes. R sings for two seconds: nearby foes sleep, even under fire.",
        arm: "note-stream",
        guard: "dodge-roll",
        hand: &["potion"],
        spells: &[],
        bombs: 2,
        max_hp: 100,
        colours: &[],
    },
    Knight {
        id: "peasant",
        name: "the Peasant",
        title: "the Deserter",
        about: "Brown shorts, bare feet and thrown stones. R leaves a scarecrow and bolts for two seconds.",
        arm: "thrown-stones",
        guard: "dodge-roll",
        hand: &[],
        spells: &[],
        bombs: 0,
        max_hp: 70,
        colours: &[],
    },
    // The champions at the bar: each a big body of their own, a great arm,
    // three spells on keys 1 to 3 and an ultimate (`champions`). Appended,
    // so the company's slots keep.
    Knight {
        id: "hammerer",
        name: "Sir Brannoc",
        title: "the Hammerer",
        about: "A smith's son grown to a giant. A great maul, a thrown hammer, a slam that shakes the floor.",
        arm: "great-maul",
        guard: "dodge-roll",
        hand: &["potion"],
        spells: &[],
        bombs: 1,
        max_hp: 170,
        colours: &[],
    },
    Knight {
        id: "butcher",
        name: "Gutch",
        title: "the Butcher",
        about: "Came up out of the low kitchens with his cleaver and his hook, and never went back.",
        arm: "cleaver",
        guard: "dodge-roll",
        hand: &[],
        spells: &[],
        bombs: 1,
        max_hp: 190,
        colours: &[],
    },
    Knight {
        id: "alchemist",
        name: "Master Orrin",
        title: "the Alchemist",
        about: "Twenty flasks on a belt and a still on his back. Fire, foul air and the one elixir.",
        arm: "flask-bandolier",
        guard: "dodge-roll",
        hand: &["potion"],
        spells: &[],
        bombs: 3,
        max_hp: 120,
        colours: &[],
    },
    Knight {
        id: "reaver",
        name: "Hrothgar Ashcleave",
        title: "the Reaver",
        about: "A raider out of the north with an axe in each fist. He throws one, bounds after it, and the blood keeps him going.",
        arm: "reaver-axes",
        guard: "dodge-roll",
        hand: &["potion"],
        spells: &[],
        bombs: 1,
        max_hp: 160,
        colours: &[],
    },
    Knight {
        id: "lancer",
        name: "Dame Isolde Farreach",
        title: "the Lancer",
        about: "A tourney champion who kept the lance when she lost the horse. Long reach, a thrown javelin, and a charge nothing stands in front of.",
        arm: "war-lance",
        guard: "dodge-roll",
        hand: &["potion"],
        spells: &[],
        bombs: 1,
        max_hp: 140,
        colours: &[],
    },
    // A hero of the barrows, head and shoulders over the company: he keeps
    // his own dead at his side (`minions`), with a spell for each kind on
    // keys 1 to 3, a soul rip on 4, and the Tombstone on R.
    Knight {
        id: "gravecaller",
        name: "the Gravecaller",
        title: "Lord of the Barrow",
        about: "1-3 raise dead; 4 tears souls. His aura pulses. Space dodges and raises a dancing Skeleton Wall. Tap G for heel/vigil; hold G to spend corpses on a golem. R plants a Tombstone.",
        arm: "grave-staff",
        guard: "dodge-roll",
        hand: &["potion"],
        spells: &["raise-ghoul", "barrow-warden", "bone-totem", "soul-rip"],
        bombs: 1,
        max_hp: 150,
        colours: &[],
    },
    Knight {
        id: "farwatch", name: "Iona Farwatch", title: "the Copper Horizon",
        about: "A fragile rifle scout. Every fourth shot cracks twice as hard. 1 sightline, 2 brass rain, 3 steady sights. R Last Light marks one distant foe.",
        arm: "horizon-rifle", guard: "dodge-roll", hand: &["potion"],
        spells: &["far-sightline", "far-brass-rain", "far-steady"], bombs: 1, max_hp: 75,
        colours: &[('7', 'O'), ('8', 'I'), ('5', 't')],
    },
    Knight {
        id: "galeweaver", name: "Tavi Reedwake", title: "the Galeweaver",
        about: "Hold 1 to charge, release to pierce. 2 binds two foes at their feet, not their hands. 3 runs on wind and evades blows. R Needle Squall focuses one foe.",
        arm: "reedwake-bow", guard: "dodge-roll", hand: &["potion"],
        spells: &["gale-power", "gale-shackle", "gale-windrun"], bombs: 1, max_hp: 100,
        colours: &[('7', 'l'), ('8', 'Q'), ('5', 'V')],
    },
    Knight {
        id: "cinderheart", name: "Sera Kilnheart", title: "the Emberwright",
        about: "1 sends a fire wave; 2 marks a delayed stunning pillar; 3 kindles a patch of flame. Each cast stacks attack speed, up to five. R Furnace Verdict burns one foe.",
        arm: "kilnheart-staff", guard: "dodge-roll", hand: &["potion"],
        spells: &["ember-wave", "ember-pillar", "ember-kindle"], bombs: 1, max_hp: 95,
        colours: &[('7', 'R'), ('8', 'O'), ('5', 'B')],
    },
    Knight { id: "saffra", name: "Saffra Noonveil", title: "the Sandglass Duelist", about: "An amber-masked handgonne duelist. Noon Shatter bursts through a crowded room; Borrowed Minute wards and heals between volleys.", arm: "handgonne", guard: "dodge-roll", hand: &["dialkeepers-ring"], spells: &["noon-shatter", "borrowed-minute"], bombs: 1, max_hp: 95, colours: &[('7', 'Y'), ('8', 'h'), ('5', 'C')] },
    Knight { id: "rill", name: "Rill Copperkite", title: "the Storm Courier", about: "A swift crossbow skirmisher. Storm Receipt clears a crowd and wards you; Eye of the Squall shelters and heals. Keep moving in fulgurite spurs.", arm: "crossbow", guard: "dodge-roll", hand: &["fulgurite-spurs"], spells: &["storm-receipt", "eye-of-the-squall"], bombs: 1, max_hp: 90, colours: &[('7', 'C'), ('8', 'W'), ('5', 'Y')] },
    Knight { id: "brine", name: "Captain Brine", title: "the Drowned Bell", about: "A salvage gunner in a sealed diving helm. Break a crowded deck, then shelter and recover before the next volley.", arm: "handgonne", guard: "dodge-roll", hand: &["undertow-coil"], spells: &["keelbreaker", "pressure-seal"], bombs: 1, max_hp: 110, colours: &[('7', 'C'), ('8', 'Y'), ('5', 'W')] },
    Knight { id: "vesper", name: "Dame Vesper", title: "the Bellkeeper", about: "A room-clearing toll and a guarded recovery. Slow, sturdy, deliberate.", arm: "crossbow", guard: "dodge-roll", hand: &["wayfarer-wick"], spells: &["eclipse-toll", "mercy-lantern"], bombs: 2, max_hp: 115, colours: &[('7', 'e'), ('8', 'C'), ('5', 'Y')] },
    Knight { id: "orrin", name: "Orrin Ashglass", title: "the Comet Notary", about: "Writes a sentence across the room, then burns to recover. Keep moving.", arm: "handgonne", guard: "dodge-roll", hand: &["wayfarer-wick"], spells: &["starfall-writ", "cinder-tithe"], bombs: 2, max_hp: 90, colours: &[('7', 'h'), ('8', 'W'), ('5', '7')] },
    Knight { id: "iona", name: "Iona Wick", title: "the Last Lamplighter", about: "Carries light into hostile fire. Shelter, heal, and pick your next opening.", arm: "bow", guard: "dodge-roll", hand: &["wayfarer-wick"], spells: &["mercy-lantern", "last-light"], bombs: 2, max_hp: 100, colours: &[('7', 'U'), ('8', 'V'), ('5', 'c')] },
    Knight { id: "vesperhost", name: "Bastien Brasservice", title: "the Siege Maître d’", about: "Bastien seats every disaster before serving it. A crossbow and kite shield hold the aisle. Widowglass Hourglass grants ten percent faster shooting and swinging. Deploy Siege Samovar's stationary turret every sixteen seconds; Borrowed Regular creates a decoy on a nineteen-second cooldown. Break Last Call Saucers once for three shot-intercepting guards, or Saint's Saltcellar once for twelve healing and three seconds of ward. Eighty-five base health, no roll and no bombs: keep the service exit clear.", arm: "crossbow", guard: "kite-shield", hand: &["widowglass-hourglass", "last-call-saucers", "saints-saltcellar"], spells: &["siege-samovar", "borrowed-regular"], bombs: 0, max_hp: 85, colours: &[('7', 'W'), ('8', 'C'), ('5', 'Y')] },
    Knight { id: "waxwarden", name: "Mirelle Wick", title: "the Candlewick Exorcist", about: "Mirelle collects the things that gather around a dying candle. A handgonne and kite shield anchor her position; Needlewake Spool adds one shot pierce. Open Vesper Apiary for a frost-support moth or break Sanguine Ampoule for a leech, each once. Gallows Sun deals twenty-two room-wide damage and wards for two seconds on a twenty-three-second cooldown. Rookstep blinks four steps then heals twelve on an eighteen-second cooldown. Ninety base health, no roll and no bombs: keep an empty landing behind your shield.", arm: "handgonne", guard: "kite-shield", hand: &["needlewake-spool", "vesper-apiary", "sanguine-ampoule"], spells: &["gallows-sun", "rookstep-sigil"], bombs: 0, max_hp: 90, colours: &[('7', 'Y'), ('8', 'C'), ('5', 'h')] },
    Knight { id: "pearlrunner", name: "Iona Wakeglass", title: "the Comet Ferrier", about: "Iona ferries the last patrons across the flooded causeway. A crossbow and dodge roll keep her mobile: Comet Siphon casts a bolt then blinks four steps on a sixteen-second cooldown. Last Call Censer scatters a ring and heals six on a twenty-two-second cooldown. Crack Oyster Aegis once for twelve healing and three seconds of ward. Eighty base health and no bombs: aim for an empty landing before paying the crossing.", arm: "crossbow", guard: "dodge-roll", hand: &["oyster-aegis"], spells: &["comet-siphon", "last-call-censer"], bombs: 0, max_hp: 80, colours: &[('7', 'C'), ('8', 'Y'), ('5', 'W')] },
    Knight { id: "cuckoosmith", name: "Tamsin Lockjaw", title: "the Closing-Time Gunsmith", about: "Tamsin repairs the tavern clocks with confiscated crossbow parts. Lobster Caliper adds one pierce and seven damage; Cuckoo Breach sends a ring and blinks two along her aim on a twenty-one-second cooldown, without granting ward. Powderplum Tart restores fourteen life and deals twenty-four room-wide damage once. Seventy-five base health, a dodge roll and no bombs: line up the shot, then leave before the clock answers.", arm: "crossbow", guard: "dodge-roll", hand: &["lobster-caliper", "powderplum-tart"], spells: &["cuckoo-breach"], bombs: 0, max_hp: 75, colours: &[('7', 'Y'), ('8', 'R'), ('5', 'W')] },
    Knight { id: "mothporter", name: "Neris Lamplow", title: "the Afterhours Porter", about: "Neris keeps a room for every traveller the marsh has swallowed. Her mothlight censer sends out a ring and grants three seconds of ward on a twenty-four-second cooldown. Oyster Mortgage adds one pierce and five damage to her bow; Firefly Sorbet restores fifteen life and deals twenty room-wide damage once. Eighty base health, a dodge roll and no bombs: survive between lantern rounds rather than trusting the light to last.", arm: "bow", guard: "dodge-roll", hand: &["oyster-mortgage", "firefly-sorbet"], spells: &["mothlight-censer"], bombs: 0, max_hp: 80, colours: &[('7', 'Y'), ('8', 'C'), ('5', 'W')] },
    Knight { id: "osric", name: "Osric Wickwake", title: "the Last Ferry Stoker", about: "He ferries the tavern's night staff home through flooded furnace tunnels, charging one unburnt match per crossing. His handgonne gains one pierce and eight projectile velocity from Cinder Seahorse. Matchstick Ferry fires a bolt and blinks two steps on an eighteen-second cooldown; it grants no ward. Mooncrab Cracker restores eighteen life and wards for two seconds. Seventy base health, a dodge roll and no bombs: choose the landing before lighting the match.", arm: "handgonne", guard: "dodge-roll", hand: &["cinder-seahorse", "mooncrab-cracker"], spells: &["matchstick-ferry"], bombs: 0, max_hp: 70, colours: &[('7', 'R'), ('8', 'Y'), ('5', 'C')] },
    Knight { id: "soundglass", name: "Ivara Soundglass", title: "the Drowned Bellkeeper", about: "She rings the cellar's service bell for patrons who never came back from the tide. Pearl Diver Fork lends her crossbow one fork and six projectile velocity. Undertow Bell fires a ring and grants two seconds of ward on a twenty-second cooldown; it does not heal. Mooncrab Cracker restores eighteen life with two seconds of ward. Seventy-five base health, a dodge roll and no bombs: the bell buys a brief firing window, not permanent shelter.", arm: "crossbow", guard: "dodge-roll", hand: &["pearl-diver-fork", "mooncrab-cracker"], spells: &["undertow-bell"], bombs: 0, max_hp: 75, colours: &[('7', 'C'), ('8', 'W'), ('5', 'Y')] },
    Knight { id: "nerissa", name: "Nerissa Sounding", title: "the Blacktide Quartermaster", about: "She provisioned a fleet after its burial and still keeps a place for every missing sailor. The Abyssal Compass quickens her handgonne and lends eight maximum life; it does not steer bullets. Blacktide Censer bursts for thirty-six beneath one second of ward. A Brineheart Fig restores twelve and bursts for twenty. Seventy base health, a dodge roll and no bombs: count the long censer cooldown before committing to the boarding action.", arm: "handgonne", guard: "dodge-roll", hand: &["abyssal-compass", "brineheart-fig"], spells: &["blacktide-censer"], bombs: 0, max_hp: 70, colours: &[('7', 'b'), ('8', 'C'), ('5', 'Y')] },
    Knight { id: "maribel", name: "Maribel Fusewick", title: "the Last Revel Marshal", about: "She evacuated a city through its own victory parade. Plant the Rook Mortar as a decoy gun line, then blink along your aim with the matchbox or ticket. The matchbox grants one second of ward; the ticket grants status immunity, not bullet immunity. Confetti antidote heals eight. Seventy health, no held projectile relic and no bombs: the fireworks are exits, not armour.", arm: "handgonne", guard: "dodge-roll", hand: &["masquerade-matchbox", "velvet-rope-ticket", "confetti-antidote"], spells: &["rook-mortar"], bombs: 0, max_hp: 70, colours: &[('7', 'Y'), ('8', 'C'), ('5', 'h')] },
    Knight { id: "bront", name: "Bront Stillhammer", title: "the Zero-Hour Millwright", about: "He shut the glacier engine down with his own shield and still hears its flywheel turning. Bank crossbow bolts once off a wall to chill the pursuit; Zero Hour Bell spreads those traits in a ring under one second of ward. The confetti antidote restores eight life and grants four seconds of status immunity, not damage immunity. Eighty-five health, no roll and no bombs: face the kite shield toward the reply and plan each corner before ringing the bell.", arm: "crossbow", guard: "kite-shield", hand: &["permafrost-flywheel", "confetti-antidote"], spells: &["zero-hour-bell"], bombs: 0, max_hp: 85, colours: &[('7', 'u'), ('8', 'Y'), ('5', 'C')] },
    Knight { id: "vespra", name: "Vespra Copperveil", title: "the Storm Apiarist", about: "She keeps thunderbees in the abandoned cathedral organ. Thundercomb bends her handgonne rounds toward prey and chains them to one neighbour; Queenstorm Decree carries those traits in a ring under a one-second ward. Her single jar restores six life and bursts for twenty nova damage. Eighty health, no bombs: roll before the swarm answers, and never mistake the veil for armour.", arm: "handgonne", guard: "dodge-roll", hand: &["thundercomb", "bottled-squall-honey"], spells: &["queenstorm-decree"], bombs: 0, max_hp: 80, colours: &[('7', 'Y'), ('8', 'C'), ('5', 'h')] },
    Knight { id: "nocturne", name: "Tamsin Nightfare", title: "the Eclipse Conductor", about: "She still checks tickets on the railway swallowed by the eclipse. Umbra Switchfork sends crossbow bolts through one foe and chains to another; Last Platform Signal spreads those traits in a ring under two seconds of ward. Her punchcard blinks eight steps along the aim and grants three seconds of status immunity, not protection from damage. Seventy-five health, a roll and no medicine or bombs: keep the platform clear and reserve a route home.", arm: "crossbow", guard: "dodge-roll", hand: &["umbra-switchfork", "eclipse-punchcard"], spells: &["last-platform-signal"], bombs: 0, max_hp: 75, colours: &[('7', 'C'), ('8', 'Y'), ('5', 'h')] },
    Knight { id: "fenella", name: "Fenella Glassvein", title: "the Mothwake Apothecary", about: "A plague-house pharmacist who taught luminous moths to carry the names of the missing. Her crossbow drives widowglass venom through the front rank; Mothwake Lantern spreads those projectile traits in a ring. Its one-second ward is time to turn, not permission to linger. Roll into clean air and save the Moonmilk for recovery. Seventy-five health, no bombs: measure the dose, then leave the room.", arm: "crossbow", guard: "dodge-roll", hand: &["widowglass-ampoule", "moonmilk-flask"], spells: &["mothwake-lantern"], bombs: 0, max_hp: 75, colours: &[('7', 'G'), ('8', 'Y'), ('5', 'h')] },
    Knight { id: "astrolabe", name: "Iona Nightindex", title: "the Last Astronomer", about: "A planetarium keeper who carried the smallest sun out of a collapsing sky. Her Orrery Heart guides shots and swings a single parrying satellite nearby; it does not make her invulnerable. Whiteout Parasol opens a trait-carrying frost ring under two seconds of ward. Spend the Eventide Key to blink along your aim, not to heal. Seventy-five health and no bombs: chart an exit before the heavens fall.", arm: "handgonne", guard: "dodge-roll", hand: &["orrery-heart", "eventide-key"], spells: &["whiteout-parasol"], bombs: 0, max_hp: 75, colours: &[('7', 'C'), ('8', 'Y'), ('5', 'h')] },
    Knight { id: "brasswrit", name: "Vesper Brasswrit", title: "the Stormrail Magistrate", about: "A circuit judge who held court aboard a runaway lightning train. Her crossbow punches a ticket through the front rank and arcs into the next passenger. Thunderhead Gavel calls a meteor beneath a one-second ward: a verdict, not lasting shelter. Roll clear of the reply and save the Moonmilk for recovery. Seventy-five health and no bombs; keep a clear platform behind you.", arm: "crossbow", guard: "dodge-roll", hand: &["stormrail-ticket", "moonmilk-flask"], spells: &["thunderhead-gavel"], bombs: 0, max_hp: 75, colours: &[('7', 'C'), ('8', 'Y'), ('5', 'h')] },
    Knight { id: "linet", name: "Linet Snowstitch", title: "the Whiteout Concierge", about: "A mountain hotel porter who led the guests out through a shattered conservatory. Her handgonne banks chilling shots off the walls; Whiteout Parasol sends those same traits outward beneath a two-second ward. The canopy is a departure window, not permanent shelter: roll before it fades. Moonmilk is one recovery drink. Seventy-five health and no bombs: make the room cold, then leave it empty.", arm: "handgonne", guard: "dodge-roll", hand: &["rimewheel-bearing", "moonmilk-flask"], spells: &["whiteout-parasol"], bombs: 0, max_hp: 75, colours: &[('7', 'C'), ('8', 'W'), ('5', 'Y')] },
    Knight { id: "oren", name: "Oren Brassmeasure", title: "the Redoubt Second", about: "A fencing second who carried a tuning fork instead of a starting pistol. His crossbow knocks attackers back and returns a little life on impact; Velvet Rebuttal sends those same traits down a single lane beneath a one-second ward. Roll sideways after the answer, not forward into the next blade. Last-Call Thunder is a single close nova with a brief ward, not a healing drink. Seventy-five health and no bombs: keep the measure, break the rush, leave standing.", arm: "crossbow", guard: "dodge-roll", hand: &["redoubt-tuning-fork", "last-call-thunder"], spells: &["velvet-rebuttal"], bombs: 0, max_hp: 75, colours: &[('7', 'r'), ('8', 'Y'), ('5', 'h')] },
    Knight { id: "sablewire", name: "Ysra Sablewire", title: "the Quarantine Signaler", about: "A rooftop telegraph keeper who rang the evacuation before the plague bells. Her switchboard chains poisoned bolts between nearby enemies; Quarantine Star carries those same traits into an aimed impact. Raise the kite shield during the twenty-second recharge: the impact ward lasts only one second. Moonmilk is one emergency recovery. Eighty health, no roll and no bombs: mark the crowded crossing, then face the answering fire.", arm: "crossbow", guard: "kite-shield", hand: &["plague-switchboard", "moonmilk-flask"], spells: &["quarantine-star"], bombs: 0, max_hp: 80, colours: &[('7', 'g'), ('8', 'Y'), ('5', 'h')] },
    Knight { id: "coalglass", name: "Petra Coalglass", title: "the Furnace Usher", about: "A foundry evacuation captain who learned to shoot around collapsed doors. Cinderpin banks burning rounds off walls; Furnace Curtain sends those traits outward and grants a one-second ward. Roll across the reply before that brief protection fades. Moonmilk is a single recovery, not a second curtain. Eighty health and no bombs: the last worker leaves before the furnace goes dark.", arm: "handgonne", guard: "dodge-roll", hand: &["cinderpin-escapement", "moonmilk-flask"], spells: &["furnace-curtain"], bombs: 0, max_hp: 80, colours: &[('7', 'r'), ('8', 'Y'), ('5', 'h')] },
    Knight { id: "mirelle", name: "Mirelle Redseam", title: "the Velvet Breacher", about: "A theatre armourer who stitched the evacuation curtains from her own duelling coat. Widow's Spindle lets handgonne rounds pierce a second body and grants two vamp. Roll across the firing lane, not down it; ring Vesper Knell when the audience crowds the stage. Last-Call Thunder is a single emergency ward and close-range blast, not a refillable spell. Seventy-five health and no bombs: the standing ovation is getting everyone out alive.", arm: "handgonne", guard: "dodge-roll", hand: &["widows-spindle", "last-call-thunder"], spells: &["vesper-knell"], bombs: 0, max_hp: 75, colours: &[('7', 'r'), ('8', 'Y'), ('5', 'h')] },
    Knight { id: "corvin", name: "Corvin Noonfall", title: "the Venom Bombardier", about: "An orchard siege engineer who measures range by the shadow of a ruined clocktower. Adder Hourglass sends piercing venom through a firing lane; Sundial Mortar drops an aimed impact carrying those same traits. Keep the kite shield toward return fire while the nineteen-second mortar reloads. Moonmilk steadies a bruised gunner. Eighty health, no dodge and no bombs: choose your emplacement before the crowd arrives.", arm: "crossbow", guard: "kite-shield", hand: &["adder-hourglass", "moonmilk-flask"], spells: &["sundial-mortar"], bombs: 0, max_hp: 80, colours: &[('7', 'Y'), ('8', 'G'), ('5', 'h')] },
    Knight { id: "bellweather", name: "Elian Bellweather", title: "the Drowned Watch", about: "A rescue diver who counts the living by lanternlight. Wake Lantern guides crossbow shots home and chills pursuers; Undertow Benediction restores eight life beneath a brief ward. Dodge through the gap, not into the crowd. A Brineheart Fig restores twelve life and bursts in a close nova if the exit closes. Seventy-five health, no bombs: bring everyone back before the bell falls silent.", arm: "crossbow", guard: "dodge-roll", hand: &["wake-lantern", "brineheart-fig"], spells: &["undertow-benediction"], bombs: 0, max_hp: 75, colours: &[('7', 'C'), ('8', 'Y'), ('5', 'b')] },
    Knight { id: "nera", name: "Nera Coilwake", title: "the Stormline Ferrier", about: "A flood-rescue ferrier carrying a lightning rod instead of an oar. Fulgurite Fork carries each crossbow hit to one nearby foe; Arcwell Salute sends that chain-bearing fire around her. A kite shield covers the approach, but offers no dodge roll. One Moonmilk Flask restores ten life with a brief ward. Eighty health and no bombs: find a crowded crossing, cast, then face the return fire.", arm: "crossbow", guard: "kite-shield", hand: &["fulgurite-fork", "moonmilk-flask"], spells: &["arcwell-salute"], bombs: 0, max_hp: 80, colours: &[('7', 'C'), ('8', 'Y'), ('5', 'h')] },
    Knight { id: "maelis", name: "Maelis Sablehour", title: "the Orrery Warden", about: "An observatory evacuation marshal who keeps the last stars in her coat. Whiteout chills and pushes with each shot; Orrery Collapse sends those traits around her in a ring. Votive Aegis restores eight life and wards the retreat. One Hearthglass Pear restores eighteen life with a brief ward. Eighty health, a dodge roll, no bombs: step close for the orbit, then leave before it closes.", arm: "crossbow", guard: "dodge-roll", hand: &["whiteout-muzzle", "hearthglass-pear"], spells: &["orrery-collapse", "votive-aegis"], bombs: 0, max_hp: 80, colours: &[('7', 'C'), ('8', 'W'), ('5', 'Y')] },
    Knight { id: "ivo", name: "Ivo Brasswake", title: "the Last-Call Ricochet", about: "A shuttered shooting-gallery keeper who collects impossible angles. His handgonne banks quickened shots with Bankshot Medallion; Vesper Knell bursts nearby foes for twenty-four beneath a brief ward. One Moonmilk Flask restores ten life. Eighty-five health, a dodge roll and no bombs: play the wall, not the crowd.", arm: "handgonne", guard: "dodge-roll", hand: &["bankshot-medallion", "moonmilk-flask"], spells: &["vesper-knell"], bombs: 0, max_hp: 85, colours: &[('7', 'Y'), ('8', '5'), ('5', 'h')] },
    Knight { id: "sabine", name: "Sabine Wickward", title: "the Reliquary Ranger", about: "A lighthouse mortician who guides lost shots home. Needle Cathedra lends her crossbow piercing, seeking bolts; Stormglass Writ carries those traits into an aimed thunderbolt. Votive Aegis restores eight life beneath a brief ward. Seventy-five health, a dodge roll and no bombs: survive by choosing a lane, not by standing in it.", arm: "crossbow", guard: "dodge-roll", hand: &["needle-cathedra"], spells: &["stormglass-writ", "votive-aegis"], bombs: 0, max_hp: 75, colours: &[('7', 'C'), ('8', 'Y'), ('5', 'h')] },
    Knight { id: "lys", name: "Lys Firefly", title: "the Lantern Duelist", about: "A glasshouse fencing tutor carrying a shield of smoked brass. Firefly Prism splits her crossbow shots into seeking sparks; Copperhead Volley presses the attack while Undertow Crown restores breathing room. Eighty health, no bombs and no roll: face the firing lane, never the encirclement.", arm: "crossbow", guard: "kite-shield", hand: &["firefly-prism"], spells: &["copperhead-volley", "undertow-crown"], bombs: 0, max_hp: 80, colours: &[('7', 'G'), ('8', 'C'), ('5', 'h')] },
    Knight { id: "orren", name: "Orren Bellrime", title: "the Glacier Sexton", about: "A frostbound siege-chapel custodian. A kite shield holds the doorway while Whiteout shots slow the advance. Ring Bastion Bell to establish a turret; Mercy Procession catches the answer and restores health. Heavy protection, no bombs, no dodge roll.", arm: "handgonne", guard: "kite-shield", hand: &["whiteout-muzzle"], spells: &["bastion-bell", "mercy-procession"], bombs: 0, max_hp: 105, colours: &[('7', 'W'), ('8', 'C'), ('5', 'h')] },
    Knight { id: "mariel", name: "Mariel Undertow", title: "the Breakwater Corsair", about: "A salvage captain who fights under an emberproof parasol. Stormrail shots bank off stone and arc into crowded boarding lanes; Undertow Crown buys breathing room, while Cinder Parasol wards a meteor strike. Seventy-five health and no bombs: recover between volleys, never trade broadsides.", arm: "handgonne", guard: "dodge-roll", hand: &["stormrail-conductor"], spells: &["undertow-crown", "cinder-parasol"], bombs: 0, max_hp: 75, colours: &[('7', 'R'), ('8', 'C'), ('5', 'h')] },
    Knight { id: "veska", name: "Veska Rimehook", title: "the Avalanche Marshal", about: "A mountain rescue gunner who marks evacuation lanes with ivory harpoons. Piercing chill shots slow the front rank; Starfall Writ carries the same shot traits into an impact, while Halo Breach answers an encirclement. Eighty health and no bombs: control the approach, not the whole mountain.", arm: "crossbow", guard: "dodge-roll", hand: &["winterfang-bayonet"], spells: &["starfall-writ", "halo-breach"], bombs: 0, max_hp: 80, colours: &[('7', 'W'), ('8', 'C'), ('5', 'h')] },
    Knight { id: "sorel", name: "Sorel Choirwire", title: "the Circuit Cantor", about: "A siege-chapel electrician who conducts by muzzle flash. Chain shots punish clustered foes; Bastion Bell anchors a defended firing lane and Mercy Procession catches the reply. Slow feet, steady hands: shelter before you sing.", arm: "handgonne", guard: "dodge-roll", hand: &["choirwire-coil"], spells: &["bastion-bell", "mercy-procession"], bombs: 0, max_hp: 90, colours: &[('7', 'Y'), ('8', 'C'), ('5', 'h')] },
    Knight { id: "iole", name: "Iole Mothwake", title: "the Night Entomancer", about: "A plague-lantern naturalist hunting by wingbeat. Guided venom bolts seek stragglers; Copperhead Volley delivers a deliberate sting and Pallbearer Lanterns shelter the fragile collector. Keep moving while the poison works.", arm: "crossbow", guard: "dodge-roll", hand: &["mothglass-lens", "vipers-hourglass"], spells: &["copperhead-volley", "pallbearer-lanterns"], bombs: 0, max_hp: 75, colours: &[('7', 'G'), ('8', 'C'), ('5', 'h')] },
    Knight { id: "elian", name: "Elian Needlewake", title: "the Linebreaker", about: "A whalebone surveyor who sights through a crowd. Piercing crossbow bolts reward lined-up shots; Halo Breach clears encirclement and Borrowed Minute shelters a careful reload.", arm: "crossbow", guard: "dodge-roll", hand: &["needlewake-sight"], spells: &["halo-breach", "borrowed-minute"], bombs: 1, max_hp: 85, colours: &[('7', 'W'), ('8', 'C'), ('5', 'h')] },
    Knight { id: "nyra", name: "Nyra Switchglass", title: "the Stormrunner", about: "A rooftop courier with a handgonne and no safe route home. Foxstep breaks bindings and crosses firing lanes; Stormglass Vespers punishes pursuit beneath a ward. Save the Glassway Key for the escape.", arm: "handgonne", guard: "dodge-roll", hand: &["switchglass-escapement", "foxstep-spindle", "glassway-key"], spells: &["stormglass-vespers"], bombs: 1, max_hp: 85, colours: &[('7', 'C'), ('8', 'Y'), ('5', 'h')] },
    Knight { id: "tamsin", name: "Tamsin Brasswake", title: "the Wake Engineer", about: "A funeral artillery engineer who builds cover from grief. Set a Rook Mortar, call Pallbearer Lanterns against incoming fire, then reposition with the crossbow.", arm: "crossbow", guard: "dodge-roll", hand: &["wake-engine"], spells: &["rook-mortar", "pallbearer-lanterns"], bombs: 1, max_hp: 90, colours: &[('7', 'h'), ('8', 'Y'), ('5', 'C')] },
    Knight { id: "morrow", name: "Morrow Briarglass", title: "the Widowseed Keeper", about: "A brass-masked crossbow apothecary. Venom shots wear foes down; Thorn Testament wounds the whole room. Glasshouse Mercy heals beneath a ward.", arm: "crossbow", guard: "dodge-roll", hand: &["widowseed-cage"], spells: &["thorn-testament", "glasshouse-mercy"], bombs: 1, max_hp: 90, colours: &[('7', 'G'), ('8', 'Y'), ('5', 'h')] },
];

pub fn knight(id: &str) -> Option<&'static Knight> {
    COMPANY.iter().find(|k| k.id == id)
}

/// Whether spell `id` is one a kit of the company carries in.
pub fn kit_spell(id: &str) -> bool {
    COMPANY.iter().any(|k| k.spells.contains(&id))
}

impl Knight {
    /// Which of a house's knights wears this kit.
    pub fn slot(&self) -> usize {
        COMPANY.iter().position(|k| k.id == self.id).unwrap_or(0)
    }

    /// The kit as a calling, for a house knight who wears it.
    pub fn class(&self) -> &'static str {
        if self.name.starts_with("the ") {
            self.name
        } else {
            self.title
        }
    }

    /// The house knight who wears this kit, named by the house's model.
    pub fn of_house(&self, house: Option<HouseId>) -> Option<&'static houses::Knight> {
        houses::get(house?).knight(self.slot())
    }

    /// The wearer's name: the house's knight, or the Keep's own.
    pub fn name_for(&self, house: Option<HouseId>) -> String {
        self.of_house(house)
            .map_or_else(|| self.name.to_string(), |k| k.name.clone())
    }

    /// The wearer's name with his castle: "Ardent of Lanternmere".
    pub fn full_name(&self, house: Option<HouseId>) -> String {
        match (self.of_house(house), house) {
            (Some(k), Some(id)) => {
                let castle = &houses::get(id).castle;
                let castle = castle
                    .strip_prefix("The ")
                    .map_or_else(|| castle.clone(), |rest| format!("the {rest}"));
                format!("{} of {castle}", k.name)
            }
            _ => self.name.to_string(),
        }
    }

    /// Name and calling for the intro: "Ardent of Lanternmere, the Seeker".
    pub fn styled(&self, house: Option<HouseId>) -> String {
        if self.of_house(house).is_some() {
            format!("{}, {}", self.full_name(house), self.class())
        } else {
            format!("{}, {}", self.name, self.title)
        }
    }
}

/// The house a seat's knight would ride for, if dressed now.
pub fn house_for_seat(seat: u32) -> Option<HouseId> {
    houses::for_seat(&houses::serving(), seat)
}

impl Run {
    /// Dress knight `id` as `who`: their weapon, guard, hand, bombs and colours.
    pub fn outfit(&mut self, id: u32, who: &Knight) {
        self.heroes.forget(id);
        let card = |id: &str| -> Option<Card> { self.book.get(id).cloned() };
        let (arm, guard) = (card(who.arm), card(who.guard));
        let Some(hero) = self.players.get_mut(&id) else {
            return;
        };
        if let Some(arm) = arm {
            hero.equip(&arm);
        }
        if let Some(guard) = guard {
            hero.guard = guard.guard.unwrap_or_default();
            hero.guard_card = Some(guard.id.clone());
        }
        // Dressed as the kit's knight, harness and helm, never left bare.
        hero.owned.body = true;
        hero.owned.helmet = true;
        hero.hand = who.hand.iter().map(|c| c.to_string()).collect();
        // A kit's own spells go with the kit: every kit's come out of the
        // slots, and this one's go in.
        for slot in hero.spells.iter_mut() {
            if slot.as_deref().is_some_and(kit_spell) {
                *slot = None;
            }
        }
        for &spell in who.spells {
            if let Some(slot) = hero.spells.iter_mut().find(|s| s.is_none()) {
                *slot = Some(spell.to_string());
            }
        }
        hero.bombs = who.bombs;
        // The hearth's warmth comes on top of the knight's own health.
        hero.max_hp = who.max_hp + hero.home_hp;
        hero.hp = hero.max_hp;
        hero.knight = Some(who.id.to_string());
        hero.flee_ticks = 0;
        hero.hero_kit = Default::default();
        if hero.peasant() { hero.armor = 0; }
        // The seat's house names the knight: the serving house for the host,
        // the formation's other houses and then the March for friends.
        hero.house = house_for_seat(id).map(|h| houses::get(h).key.to_string());
        // A naked start keeps only what the knight brought home; the
        // identity start keeps the name, the house and the colours, and
        // begins the knight as a shape (`identity`).
        if self.naked_start || self.identity || self.home.wardrobe.knights.contains_key(&hero.name) {
            self.restore_gear(id);
        }
    }

    /// Begin the delve in `pack` instead of the one the dice chose.
    pub fn begin_in(&mut self, pack: Pack) {
        if self.dungeon.depth == 0 {
            // In the Undercroft, the stair will go down into it.
            self.dungeon.pack = pack;
            return;
        }
        if self.dungeon.pack == pack || self.dungeon.depth != 1 {
            return;
        }
        self.settlement_site = None;
        self.settlement_exhibits.clear();
        self.dungeon = super::layout::floor_party(1, pack, self.party_size(), &mut self.rng);
        self.stock_gear_cache();
        self.populate_boss_gates();
        self.cues.retain(|c| c != "run_start");
        self.enter(0, None);
        self.cues.insert(0, "run_start".into());
    }
}

impl Run {
    /// The music this moment wants: a guardian's battle cue, a calm place's
    /// own score, or the delve's own, each a playlist that moves on to its
    /// next track as one ends (`music::play`, by the run's clock). A floor's
    /// list runs on from room to room, with a quiet after each track
    /// (`music::play_sparse`: `None` then); a place up in the realm and a
    /// Sanctuary begin their lists at a point of their own.
    pub fn music(&self) -> Option<&'static str> {
        use super::music::{self, play};
        if !self.active() {
            return None;
        }
        let depth = self.dungeon.depth as u64;
        // A village of the Deep sings its people's own songs.
        if let Some(list) = self.village_music() {
            return Some(play(list, self.raid_id ^ u64::from(self.floor()), self.tick));
        }
        let delving = !self.at_home_now()
            && self.room().kind != super::RoomKind::Sanctuary
            && !self.enemies.iter().any(|e| e.boss.is_some() || e.kind == super::EnemyKind::Dragon);
        if delving {
            // Down a floor the score comes and goes: a track, then the dark
            // on its own a while, in an order of the delve's own.
            let pack = match self.dungeon.pack.kin() {
                Pack::Crypt => 0,
                Pack::Cavern => 1,
                _ => 2,
            };
            return music::play_sparse(music::DUNGEON, self.raid_id ^ (depth * 3 + pack), self.tick);
        }
        Some(if self.at_home_now() {
            play(music::HOME, self.raid_id ^ self.at as u64, self.tick)
        } else if self.room().kind == super::RoomKind::Sanctuary {
            play(music::REST, self.raid_id ^ (depth * 64 + self.at as u64), self.tick)
        } else {
            play(music::BOSS, self.raid_id ^ depth, self.tick)
        })
    }
}

/// Which part of a kit a Sanctuary reforge changes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Part {
    /// The weapon: an arm card.
    Offense,
    /// The guard key: a guard card.
    Defense,
}

impl Part {
    pub fn word(self) -> &'static str {
        match self {
            Part::Offense => "offence",
            Part::Defense => "defence",
        }
    }

    pub fn from_word(word: &str) -> Option<Part> {
        match word.trim().to_ascii_lowercase().as_str() {
            "offense" | "offence" | "weapon" | "attack" => Some(Part::Offense),
            "defense" | "defence" | "guard" | "defend" => Some(Part::Defense),
            _ => None,
        }
    }
}

impl Run {
    /// Knight `id` stands in a Sanctuary and has not reforged on this floor.
    pub fn can_reforge(&self, id: u32) -> bool {
        self.room().kind == super::RoomKind::Sanctuary
            && self
                .players
                .get(&id)
                .is_some_and(|h| h.hp > 0 && h.reforge_job.is_none() && h.reforged_on != Some(self.dungeon.depth))
    }

    /// Equip a reforged card on knight `id`: a weapon for offence, a guard
    /// for defence. The card joins the run's book.
    pub fn reforge(&mut self, id: u32, part: Part, card: Card) -> Result<String, String> {
        let wanted = match part {
            Part::Offense => super::cards::Kind::Arm,
            Part::Defense => super::cards::Kind::Guard,
        };
        if card.kind != wanted {
            return Err(format!(
                "a {} reforge needs `kind {}`, not `kind {}`",
                part.word(),
                wanted.word(),
                card.kind.word()
            ));
        }
        if !self.can_reforge(id) {
            return Err("only once per Sanctuary, while standing in it".into());
        }
        let name = card.name.clone();
        if !self.book.insert(card.clone()) {
            return Err("the card book is full".into());
        }
        let depth = self.dungeon.depth;
        let hero = self.players.get_mut(&id).ok_or("no such knight")?;
        match part {
            Part::Offense => hero.equip(&card),
            Part::Defense => {
                hero.guard = card.guard.unwrap_or_default();
                hero.guard_card = Some(card.id.clone());
            }
        }
        hero.reforged_on = Some(depth);
        self.cues.push("reforged".into());
        self.sounds.push("card_pickup");
        Ok(name)
    }
}

impl Hero {
    /// The house this knight rides for.
    pub fn house_id(&self) -> Option<HouseId> {
        self.house.as_deref().and_then(houses::by_key)
    }

    /// The knight's own name, from their house (or the Keep's household).
    pub fn knight_name(&self) -> Option<String> {
        let kit = self.knight.as_deref().and_then(knight)?;
        Some(kit.name_for(self.house_id()))
    }

    /// The name shown for this knight: their player's, the one they joined
    /// by (the operator, 2026-10-18: "the names need to carry over to the
    /// avatar"); the knight's own only for a player who gave none.
    pub fn shown_name(&self) -> String {
        if !self.look.name.is_empty() { return self.look.name.clone(); }
        let player = self.name.trim();
        let unnamed = player.is_empty()
            || ["you", "host"].iter().any(|n| player.eq_ignore_ascii_case(n))
            || player.strip_prefix("Knight ").is_some_and(|n| n.parse::<u32>().is_ok());
        if unnamed { self.knight_name().unwrap_or_else(|| self.name.clone()) } else { self.name.clone() }
    }

    /// The colours this knight wears over the realm's red.
    pub fn colours(&self) -> &'static [(char, char)] {
        self.knight
            .as_deref()
            .and_then(knight)
            .map_or(&[], |k| k.colours)
    }
}

#[cfg(test)]
#[path = "../../tests/rules/together_knights__tests.rs"]
mod tests;
