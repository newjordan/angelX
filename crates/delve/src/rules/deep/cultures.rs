//! The deep people: who lives in the villages of the Deep, as a wheel that
//! turns a place each chapter. Each people keeps its own town names, its
//! own six who speak (an elder, three with errands, one who keeps the
//! wishing place and one who only talks), its own songs, and the old delve
//! whose stone its caverns are cut from.
//!
//! The wheel never sets one people two villages running: chapter `c` takes
//! the wheel's place `start + c * STEP`, and `STEP` shares no factor with
//! the wheel's length, so every people comes round before any comes again.

use super::super::music::Track;
use super::super::{EnemyKind, Pack};
use super::missions::Errand;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Culture {
    /// Mushroom-cap folk: cottages under giant caps, spore-lamps, the gills
    /// glowing blue in the dark.
    Capfolk,
    /// Crystal miners: halls hewn in the crystal, rails and lodestones, a
    /// waterfall down the cavern's face.
    Glimmerkin,
    /// Lava forgers: basalt halls round a forge-heart, channels of slow
    /// fire, chimneys smoking up into the dark.
    Emberfolk,
    /// The drowned lamplighters: stilt-houses over black water, a lamp on
    /// every post, mist on the lake.
    Lampwick,
    /// The bat-riders: woven nests hung from the roof of the dark on ropes,
    /// rope bridges between them, red lanterns the bats steer by.
    Roostfolk,
    /// The root-kin: knotted houses in the roots of a tree too big to see
    /// the top of, its sap glowing gold where it runs.
    Rootkin,
    /// The drowned bell-ringers: a half-flooded cathedral of stalagmite
    /// spires, a bell in every spire, the water ringing back.
    Chimefolk,
    /// The salt-miners: white terraces cut in the salt, brine pools in
    /// steps, salt-lamps that glow the colour of dawn.
    Saltkin,
}

/// The wheel, in its order. The four found first keep their places; the
/// four after were met later, and come round between them.
pub const WHEEL: [Culture; 8] = [
    Culture::Capfolk,
    Culture::Glimmerkin,
    Culture::Emberfolk,
    Culture::Lampwick,
    Culture::Roostfolk,
    Culture::Rootkin,
    Culture::Chimefolk,
    Culture::Saltkin,
];
/// How far the wheel turns a chapter: coprime with its length.
pub const STEP: u32 = 3;

/// The people of chapter `chapter`'s village, for a realm whose wheel
/// starts at `wheel`.
pub fn turn(wheel: u32, chapter: u32) -> Culture {
    let n = WHEEL.len() as u32;
    WHEEL[((wheel % n + chapter.saturating_sub(1) % n * STEP) % n) as usize]
}

/// One of a village's six who speak.
pub struct Folk {
    pub name: &'static str,
    /// What they say as a knight first comes up to them.
    pub hello: &'static str,
    /// What they ask, if they ask anything: the errand, its title, and why.
    pub asks: Option<Ask>,
    /// What they say once it is asked, or all they ever say.
    pub after: &'static str,
}

/// An errand as a villager asks it.
pub struct Ask {
    pub errand: Errand,
    pub title: &'static str,
    pub why: &'static str,
    pub need: u16,
}

/// The six who speak, by their place in the village (`village::SPEAKERS`):
/// the elder, three who ask errands, one who only talks, the wishkeeper.
pub const ELDER: usize = 0;
pub const TALKER: usize = 4;
pub const WISHKEEPER: usize = 5;

impl Culture {
    /// The people, as the boards name them.
    pub fn people(self) -> &'static str {
        match self {
            Culture::Capfolk => "THE CAPFOLK",
            Culture::Glimmerkin => "THE GLIMMERKIN",
            Culture::Emberfolk => "THE EMBERFOLK",
            Culture::Lampwick => "THE LAMPWICK",
            Culture::Roostfolk => "THE ROOSTFOLK",
            Culture::Rootkin => "THE ROOTKIN",
            Culture::Chimefolk => "THE CHIMEFOLK",
            Culture::Saltkin => "THE SALTKIN",
        }
    }

    /// The town on chapter `chapter`'s floor: each people has three, and a
    /// deeper chapter comes to the next.
    pub fn town(self, chapter: u32) -> &'static str {
        let names: [&str; 3] = match self {
            Culture::Capfolk => ["MOTHERCAP", "SPORESEND", "GILLHOLLOW"],
            Culture::Glimmerkin => ["GLIMMERDEEP", "SHARDHOLM", "QUARTZWAY"],
            Culture::Emberfolk => ["EMBERHOLD", "CINDERWICK", "ASHFORGE"],
            Culture::Lampwick => ["STILLWATER", "WICKMERE", "LANTERN REACH"],
            Culture::Roostfolk => ["HOLLOWROOST", "WINGWELL", "DUSKPERCH"],
            Culture::Rootkin => ["TAPROOT", "DEEPBOLE", "HEARTWOOD"],
            Culture::Chimefolk => ["BELLDROWN", "TOLLMERE", "CHIMEWATER"],
            Culture::Saltkin => ["WHITEBRINE", "SALTSTAIR", "BRINEHOLLOW"],
        };
        names[(chapter.saturating_sub(1) / WHEEL.len() as u32 % 3) as usize]
    }

    /// The old delve whose stone the people's cavern is cut from.
    pub fn kin(self) -> Pack {
        match self {
            Culture::Capfolk => Pack::Fungal,
            Culture::Glimmerkin => Pack::Cavern,
            Culture::Emberfolk => Pack::Hellforge,
            Culture::Lampwick => Pack::Archive,
            Culture::Roostfolk => Pack::Crypt,
            Culture::Rootkin => Pack::Fungal,
            Culture::Chimefolk => Pack::Archive,
            Culture::Saltkin => Pack::Cavern,
        }
    }

    /// What the village sings while the party is there: the calm places'
    /// own tunes, a people's own few.
    pub fn music(self) -> &'static [Track] {
        match self {
            Culture::Capfolk => &[
                Track { name: "bluebells", secs: 43 },
                Track { name: "pear-tree", secs: 50 },
                Track { name: "heather", secs: 40 },
                Track { name: "village-green", secs: 139 },
            ],
            Culture::Glimmerkin => &[
                Track { name: "library-rain", secs: 53 },
                Track { name: "small-hours", secs: 89 },
                Track { name: "candles", secs: 70 },
                Track { name: "quiet-hills", secs: 53 },
            ],
            Culture::Emberfolk => &[
                Track { name: "fireside-cup", secs: 58 },
                Track { name: "keepers-cottage", secs: 72 },
                Track { name: "village-wakes", secs: 72 },
                Track { name: "last-light", secs: 62 },
            ],
            Culture::Lampwick => &[
                Track { name: "roadside-lantern", secs: 123 },
                Track { name: "sanctuary", secs: 69 },
                Track { name: "library-rain", secs: 53 },
                Track { name: "small-hours", secs: 89 },
            ],
            Culture::Roostfolk => &[
                Track { name: "heather", secs: 40 },
                Track { name: "qr-p-minimax-021", secs: 65 },
                Track { name: "quiet-hills", secs: 53 },
                Track { name: "qr-p-minimax-033", secs: 62 },
            ],
            Culture::Rootkin => &[
                Track { name: "village-wakes", secs: 72 },
                Track { name: "pear-tree", secs: 50 },
                Track { name: "keepers-cottage", secs: 72 },
                Track { name: "qr-p-minimax-028", secs: 72 },
            ],
            Culture::Chimefolk => &[
                Track { name: "sanctuary", secs: 69 },
                Track { name: "candles", secs: 70 },
                Track { name: "qr-p-minimax-049", secs: 88 },
                Track { name: "library-rain", secs: 53 },
            ],
            Culture::Saltkin => &[
                Track { name: "last-light", secs: 62 },
                Track { name: "qr-p-minimax-053", secs: 81 },
                Track { name: "bluebells", secs: 43 },
                Track { name: "qr-p-minimax-030", secs: 63 },
            ],
        }
    }

    /// What the lost are called, the relics gathered and the lamps lit, on
    /// the floors below: the tracker's words.
    pub fn sign_words(self) -> (&'static str, &'static str, &'static str) {
        match self {
            Culture::Capfolk => ("SPORE-LAMP", "GLOWCAP", "LOST CAPLING"),
            Culture::Glimmerkin => ("LODESTONE", "SKY-CRYSTAL", "LOST DELVER"),
            Culture::Emberfolk => ("BRAZIER", "STAR-IRON", "LOST APPRENTICE"),
            Culture::Lampwick => ("DROWNED LAMP", "PEARL WICK", "LOST LAMPLIGHTER"),
            Culture::Roostfolk => ("MOON-LANTERN", "MOTH-SILK", "LOST RIDER"),
            Culture::Rootkin => ("SAP-LAMP", "HEARTSEED", "LOST SEEDLING"),
            Culture::Chimefolk => ("DROWNED BELL", "BELL-SILVER", "LOST CHORISTER"),
            Culture::Saltkin => ("SALT-LAMP", "STAR-SALT", "LOST SALTER"),
        }
    }

    /// The one the party finds below and sends home: their name, and what
    /// the village says the next time the party comes, with them home.
    pub fn rescued(self) -> (&'static str, &'static str) {
        match self {
            Culture::Capfolk => ("POPPY", "POPPY IS HOME, RED CAP AND ALL. TRUFFLE HAS NOT LET GO OF HER SINCE."),
            Culture::Glimmerkin => ("FELDSPAR", "FELDSPAR CAME UP THE SEAM WITH HIS BLUE LAMP STILL LIT. MICA HAS FORGIVEN HIM. NEARLY."),
            Culture::Emberfolk => ("CLINKER", "CLINKER IS HOME, AND STILL HOLDING HIS HAMMER. SOOTY HAS STOPPED SHOUTING AT HIM. MOSTLY."),
            Culture::Lampwick => ("TAPER", "TAPER ROWED HOME BY YOUR LIGHT. THE SOUP WAS STILL ON."),
            Culture::Roostfolk => ("SWIFT", "SWIFT IS HOME. HER BAT HAS NOT LEFT HER SIDE, AND NEITHER HAS TWITCH."),
            Culture::Rootkin => ("ACORN", "ACORN IS HOME. SHE BROUGHT A STONE BACK. SHE SAYS IT TALKED THE WHOLE WAY."),
            Culture::Chimefolk => ("CHIME", "CHIME IS HOME, STILL SINGING. THE CATHEDRAL SANG IT BACK TO HIM."),
            Culture::Saltkin => ("CRUST", "CRUST IS HOME. OLD FLAKE SAYS SHE IS NOT ANGRY. SHE IS. SHE IS ALSO VERY GLAD."),
        }
    }

    /// The six who speak.
    pub fn folk(self) -> [Folk; 6] {
        match self {
            Culture::Capfolk => [
                Folk {
                    name: "OLD MOREL",
                    hello: "A STAR FELL INTO OUR DARK, AND IT WAS YOU. WE ARE THE CAPFOLK. OUR LAMPS WENT OUT WHEN THE DEEP THINGS CAME.",
                    asks: Some(Ask { errand: Errand::Beacons, title: "LIGHT THE SPORE-LAMPS", why: "FIVE SPORE-LAMPS STAND ON THE FLOORS BELOW, COLD THIS HUNDRED YEARS. LIGHT THEM, AND OUR YOUNG WILL SEE THE WAY HOME.", need: 5 }),
                    after: "EVERY LAMP YOU LIGHT DOWN THERE, WE SEE FROM HERE. LIKE STARS COMING BACK.",
                },
                Folk {
                    name: "PUFFLING",
                    hello: "YOU GLOW! IS EVERYONE FROM UP THERE SO BRIGHT?",
                    asks: Some(Ask { errand: Errand::Gather, title: "GATHER GLOWCAPS", why: "THE GLOWCAPS GROW WHERE WE DARE NOT GO. BRING BACK SIX AND WE'LL PLANT ONE OVER EVERY DOOR.", need: 6 }),
                    after: "SIX GLOWCAPS. I'VE CHOSEN WHICH DOORS ALREADY.",
                },
                Folk {
                    name: "NETTLECAP",
                    hello: "THE BATS DRINK THE GLOW RIGHT OUT OF OUR CAPS. WE HAVE NO ARROWS LEFT.",
                    asks: Some(Ask { errand: Errand::Slay(EnemyKind::Bat), title: "DRIVE OFF THE BATS", why: "FELL FIFTEEN OF THE GLOW-DRINKERS ON THE WAY DOWN, AND THE REST WILL KEEP AWAY.", need: 15 }),
                    after: "MIND THEIR TEETH. THEY GO FOR THE EYES, WHICH IS WHERE THE LIGHT IS.",
                },
                Folk {
                    name: "TRUFFLE",
                    hello: "LOST, ARE YOU? NO. NOBODY IS LOST WHO COMES DOWN ON PURPOSE.",
                    asks: Some(Ask { errand: Errand::Rescue, title: "FIND THE LOST CAPLING", why: "MY LITTLE ONE WANDERED DOWN AFTER A SPORE-MOTH. SHE WEARS A RED CAP. BRING HER HOME.", need: 1 }),
                    after: "A RED CAP. SHE'LL BE THE ONLY RED THING DOWN THERE THAT ISN'T ANGRY.",
                },
                Folk {
                    name: "INKCAP",
                    hello: "WE COUNT THE YEARS BY CAPS. THIS IS THE FOUR HUNDREDTH CAP SINCE ANYONE CAME DOWN FROM ABOVE.",
                    asks: None,
                    after: "IS IT TRUE THE SKY IS A CEILING YOU CANNOT TOUCH? I SHOULD LIKE TO SEE SUCH A THING.",
                },
                Folk {
                    name: "MOTHER AMANITA",
                    hello: "THIS WELL HEARS WISHES. A VISITOR'S WISH IS THE STRONGEST KIND: IT COMES FROM OUTSIDE THE DARK.",
                    asks: None,
                    after: "WISH SOMETHING FOR OUR CITY, STAR-KNIGHT. WE WILL KEEP IT AS LONG AS THERE ARE CAPS.",
                },
            ],
            Culture::Glimmerkin => [
                Folk {
                    name: "FOREMAN GEODE",
                    hello: "YOU CAME DOWN LIKE A SHARD OF THE OLD SUN. WE ARE THE GLIMMERKIN. WE DIG THE CRYSTAL THAT HOLDS THE LIGHT.",
                    asks: Some(Ask { errand: Errand::Beacons, title: "RELIGHT THE LODESTONES", why: "FIVE LODESTONES LIT THE OLD SEAMS BELOW. THE DARK PUT THEM OUT ONE BY ONE. STRIKE THEM AGAIN.", need: 5 }),
                    after: "A LIT LODESTONE SINGS. WE'LL HEAR IT UP HERE THROUGH THE ROCK.",
                },
                Folk {
                    name: "MICA",
                    hello: "MY BROTHER FELDSPAR WENT DOWN THE SEAM AND NEVER CAME UP.",
                    asks: Some(Ask { errand: Errand::Rescue, title: "FIND THE LOST DELVER", why: "HE CARRIES A BLUE LAMP AND HE'S TOO STUBBORN TO DIE. FIND HIM AND SEND HIM HOME.", need: 1 }),
                    after: "A BLUE LAMP. AND A BEARD. MOSTLY BEARD.",
                },
                Folk {
                    name: "GARNET",
                    hello: "THE BEST CRYSTAL FELL FROM THE SKY, LONG AGO. THE SKY-CRYSTALS. THEY'RE ALL BELOW US NOW.",
                    asks: Some(Ask { errand: Errand::Gather, title: "BRING BACK SKY-CRYSTALS", why: "SIX OF THEM, AND I CAN CUT A WINDOW THAT SHOWS THE SURFACE. IMAGINE IT.", need: 6 }),
                    after: "DON'T DROP THEM. THEY REMEMBER BEING DROPPED.",
                },
                Folk {
                    name: "FLINT",
                    hello: "SOMETHING IN THE LOWER SEAMS SWALLOWS OUR CARTS WHOLE. SLIMES. BIG ONES.",
                    asks: Some(Ask { errand: Errand::Slay(EnemyKind::Slime), title: "CLEAR THE SLIMES", why: "FELL FIFTEEN OF THE SEAM-SWALLOWERS AND WE CAN RUN THE RAILS AGAIN.", need: 15 }),
                    after: "IF ONE SPLITS, IT STILL COUNTS AS ONE. FOREMAN'S RULE.",
                },
                Folk {
                    name: "OPAL",
                    hello: "EVERY CRYSTAL HERE HOLDS A LITTLE OF THE LIGHT THAT FELL ON IT. YOU'VE GIVEN THEM PLENTY TODAY.",
                    asks: None,
                    after: "LISTEN. THE WALLS ARE HUMMING. THEY DO THAT WHEN THEY'RE HAPPY.",
                },
                Folk {
                    name: "OLD BERYL",
                    hello: "THE WISHING STONE TAKES ONE WISH FROM EACH WHO COMES FROM ABOVE. WE HAVEN'T HAD ONE IN AN AGE.",
                    asks: None,
                    after: "SPEAK INTO THE STONE, STAR-KNIGHT. WHAT SHOULD GLIMMERDEEP BECOME?",
                },
            ],
            Culture::Emberfolk => [
                Folk {
                    name: "FORGEMOTHER BRAND",
                    hello: "A SPARK FELL FROM ABOVE AND WOKE THE FORGE-HALL. WE ARE THE EMBERFOLK. OUR FIRES ARE DYING.",
                    asks: Some(Ask { errand: Errand::Slay(EnemyKind::Imp), title: "DRIVE OUT THE COAL-THIEVES", why: "THE IMPS STEAL OUR COALS AND LAUGH ABOUT IT. FELL FIFTEEN AND THE FORGE STAYS LIT.", need: 15 }),
                    after: "THEY GIGGLE WHEN THEY STEAL. YOU'LL KNOW THEM BY THE GIGGLE.",
                },
                Folk {
                    name: "TONGS",
                    hello: "THE OLD BRAZIERS RAN ALL THE WAY DOWN. A ROAD OF FIRE. NOW IT'S A ROAD OF NOTHING.",
                    asks: Some(Ask { errand: Errand::Beacons, title: "REKINDLE THE BRAZIERS", why: "FIVE BRAZIERS BELOW US. LIGHT THEM AND OUR TRADERS CAN WALK THE DEEP AGAIN.", need: 5 }),
                    after: "A BRAZIER LIT IS A PROMISE KEPT. MY GRANDMOTHER SAID THAT. SHE KEPT A LOT OF THEM.",
                },
                Folk {
                    name: "SOOTY",
                    hello: "OUR APPRENTICE CLINKER WENT DOWN FOR STAR-IRON AND HASN'T COME BACK.",
                    asks: Some(Ask { errand: Errand::Rescue, title: "BRING HOME THE APPRENTICE", why: "HE'S YOUNG AND HE'S BRAVE AND HE'S AN IDIOT. BRING HIM HOME.", need: 1 }),
                    after: "IF HE'S HOLDING A HAMMER, HE'S FINE. IF HE ISN'T, HURRY.",
                },
                Folk {
                    name: "CINDER",
                    hello: "STAR-IRON FALLS THROUGH THE ROCK, THEY SAY. FROM WHERE YOU CAME FROM.",
                    asks: Some(Ask { errand: Errand::Gather, title: "GATHER STAR-IRON", why: "SIX LUMPS OF IT. I'LL FORGE A BELL THAT RINGS ALL THE WAY TO THE SURFACE.", need: 6 }),
                    after: "IT'S WARM WHEN YOU FIND IT. IT'S ALWAYS WARM.",
                },
                Folk {
                    name: "ASHLING",
                    hello: "I'VE NEVER SEEN A FIRE YOU DIDN'T HAVE TO FEED. IS THE SUN LIKE THAT?",
                    asks: None,
                    after: "THE FORGE SINGS DIFFERENT WITH YOU HERE. HIGHER.",
                },
                Folk {
                    name: "OLD BELLOWS",
                    hello: "THE WISHING ANVIL. STRIKE IT WITH A WISH AND THE CITY REMEMBERS THE BLOW.",
                    asks: None,
                    after: "ONE WISH EACH, VISITOR. MAKE IT A GOOD STRONG ONE.",
                },
            ],
            Culture::Lampwick => [
                Folk {
                    name: "KEEPER WICKLOW",
                    hello: "A LIGHT ON THE WATER, AND IT FELL FROM ABOVE. WE ARE THE LAMPWICK. WE KEEP THE LAMPS SO THE DROWNED CAN FIND THEIR WAY.",
                    asks: Some(Ask { errand: Errand::Beacons, title: "LIGHT THE DROWNED LAMPS", why: "FIVE LAMPS STAND IN THE BLACK WATER BELOW. LIGHT THEM AND THE LOST WILL SEE THE SHORE.", need: 5 }),
                    after: "A LAMP ON THE WATER IS TWO LAMPS. THE ONE, AND ITS REFLECTION.",
                },
                Folk {
                    name: "MOTH",
                    hello: "THE WRAITHS COME UP OUT OF THE LAKE AT NIGHT. WE THINK IT'S NIGHT. IT'S ALWAYS NIGHT.",
                    asks: Some(Ask { errand: Errand::Slay(EnemyKind::Wraith), title: "LAY THE WRAITHS", why: "FIFTEEN OF THEM, ON THE WAY DOWN, AND THE LAKE WILL BE QUIET AGAIN.", need: 15 }),
                    after: "THEY WERE LAMPLIGHTERS ONCE. BE KIND, IF YOU CAN.",
                },
                Folk {
                    name: "PERCH",
                    hello: "PEARL WICKS BURN A HUNDRED YEARS. THEY GROW IN THE MUD OF THE DEEPER LAKES.",
                    asks: Some(Ask { errand: Errand::Gather, title: "BRING BACK PEARL WICKS", why: "SIX OF THEM AND EVERY HOUSE IN STILLWATER HAS LIGHT TILL WE'RE ALL GREY.", need: 6 }),
                    after: "THEY LOOK LIKE NOTHING. THEY LOOK LIKE STRING. THEY'RE NOT STRING.",
                },
                Folk {
                    name: "OLD TALLOW",
                    hello: "MY GIRL ROWED OUT TO LIGHT THE FAR LAMP AND THE MIST TOOK HER BOAT.",
                    asks: Some(Ask { errand: Errand::Rescue, title: "FIND THE LOST LAMPLIGHTER", why: "SHE'LL HAVE HER POLE AND HER LAMP. SHE'LL BE WAITING. SHE'S GOOD AT WAITING.", need: 1 }),
                    after: "TELL HER THE SOUP'S ON. SHE'LL COME FOR THE SOUP.",
                },
                Folk {
                    name: "RUSHLIGHT",
                    hello: "WE NAME OUR CHILDREN AFTER LIGHTS. RUSHLIGHT, TAPER, GLIM. I WAS NEARLY CALLED BONFIRE.",
                    asks: None,
                    after: "THE WATER'S BRIGHTER SINCE YOU CAME. LOOK AT IT.",
                },
                Folk {
                    name: "MOTHER MERE",
                    hello: "THE WISHING LAMP. LIGHT IT WITH A WISH, AND THE WHOLE LAKE WILL HOLD IT.",
                    asks: None,
                    after: "ONE WISH EACH, FROM ABOVE. WHAT SHOULD STILLWATER KEEP?",
                },
            ],
            Culture::Roostfolk => [
                Folk {
                    name: "ROOSTMOTHER ASH",
                    hello: "WE WATCHED YOU FALL FROM OUR NESTS: A STAR, AND NOT ONE BAT DARED CHASE IT. WE ARE THE ROOSTFOLK. WE RIDE THE DARK, AND THE DARK HAS TURNED ON US.",
                    asks: Some(Ask { errand: Errand::Beacons, title: "LIGHT THE MOON-LANTERNS", why: "FIVE MOON-LANTERNS HANG ON THE FLOORS BELOW, WHERE OUR RIDERS TURNED FOR HOME. LIGHT THEM, AND THE BATS WILL STEER BY THEM AGAIN.", need: 5 }),
                    after: "A RIDER STEERS BY LANTERNS. NO LANTERNS, NO RIDERS. YOU SEE HOW IT IS.",
                },
                Folk {
                    name: "TWITCH",
                    hello: "YOU DIDN'T EVEN HAVE A BAT. YOU JUST FELL. THAT'S THE BRAVEST THING I EVER SAW.",
                    asks: Some(Ask { errand: Errand::Rescue, title: "FIND THE LOST RIDER", why: "MY SISTER SWIFT FLEW DOWN, AND HER BAT CAME HOME WITHOUT HER. SHE'S DOWN THERE ON FOOT. RIDERS HATE BEING ON FOOT.", need: 1 }),
                    after: "SHE'LL BE CROSS AND SHE'LL BE HUNGRY. MOSTLY CROSS.",
                },
                Folk {
                    name: "LOOM",
                    hello: "EVERY ROPE IN HOLLOWROOST IS MY KNOTTING, AND EVERY ONE OF THEM IS FRAYING.",
                    asks: Some(Ask { errand: Errand::Gather, title: "GATHER MOTH-SILK", why: "THE MOONMOTHS SPIN SILK ON THE FLOORS BELOW, STRONGER THAN ANY ROPE. SIX SKEINS, AND I'LL MEND EVERY BRIDGE WE HAVE.", need: 6 }),
                    after: "SILK HOLDS. STRING FRAYS. I'VE BEEN SAYING IT FOR FORTY YEARS.",
                },
                Folk {
                    name: "CAPTAIN SOOTWING",
                    hello: "THE SILKMOTHERS SPIN SNARES ACROSS THE DEEP ROADS. WE LOST THREE BATS LAST MONTH. GOOD BATS.",
                    asks: Some(Ask { errand: Errand::Slay(EnemyKind::Silkmother), title: "CUT DOWN THE SILKMOTHERS", why: "FELL FOUR OF THE SNARE-SPINNERS ON THE WAY DOWN, AND OUR RIDERS CAN FLY THE DEEP ROADS AGAIN.", need: 4 }),
                    after: "KEEP YOUR BLADE CLEAN. THE SILK GETS EVERYWHERE.",
                },
                Folk {
                    name: "PIP",
                    hello: "I'M LEARNING TO HANG UPSIDE DOWN. IT'S HARD. HOW DO YOU SLEEP, UP THERE?",
                    asks: None,
                    after: "THE BATS SAY YOU SMELL OF WIND. THEY LIKE IT.",
                },
                Folk {
                    name: "WARDEN FELL",
                    hello: "THE WISHING PERCH. TELL YOUR WISH TO OLD DUSK, HANGING THERE, AND HE CARRIES IT UP TO WHERE THE STARS ARE.",
                    asks: None,
                    after: "ONE WISH EACH, FROM ABOVE. DUSK IS OLD, BUT HE'LL FLY FOR A STAR.",
                },
            ],
            Culture::Rootkin => [
                Folk {
                    name: "ELDER GNARL",
                    hello: "THE ROOTS FELT YOU LAND. WE FEEL EVERYTHING THROUGH THE ROOTS. WE ARE THE ROOTKIN, AND THE GREAT TREE OVER US IS DYING FROM THE BOTTOM UP.",
                    asks: Some(Ask { errand: Errand::Beacons, title: "WAKE THE SAP-LAMPS", why: "FIVE SAP-LAMPS GROW ON THE ROOTS BELOW US. THEY HAVE GONE DARK AND HARD. WAKE THEM, AND THE SAP WILL RUN AGAIN.", need: 5 }),
                    after: "A LAMP WAKES AND THE WHOLE TREE SIGHS. YOU'LL HEAR IT, IF YOU LISTEN.",
                },
                Folk {
                    name: "BRAMBLE",
                    hello: "MY LITTLE ONE, ACORN, WENT DOWN THE ROOT-PATHS TO FIND THE RAIN. THERE ISN'T ANY RAIN DOWN THERE.",
                    asks: Some(Ask { errand: Errand::Rescue, title: "FIND THE LOST SEEDLING", why: "SHE HAS A GREEN SPROUT ON HER HEAD AND SHE TALKS TO STONES. BRING HER HOME BEFORE SHE TAKES ROOT SOMEWHERE SHE SHOULDN'T.", need: 1 }),
                    after: "IF SHE'S STANDING VERY STILL, SHE'S LISTENING. DON'T TAKE IT TO HEART.",
                },
                Folk {
                    name: "OLD TANNIN",
                    hello: "EVERY HUNDRED YEARS THE TREE DROPS ITS HEARTSEEDS INTO THE DEEP. WE NEVER KNEW WHERE THEY FELL.",
                    asks: Some(Ask { errand: Errand::Gather, title: "GATHER HEARTSEEDS", why: "SIX HEARTSEEDS, AND WE CAN PLANT A NEW TREE BESIDE THE OLD ONE. IN CASE.", need: 6 }),
                    after: "THEY'RE WARM. A SEED SHOULDN'T BE WARM. THESE ARE.",
                },
                Folk {
                    name: "THORN",
                    hello: "THE LEECHES COME UP THE ROOTS AT NIGHT AND DRINK THE SAP. THE TREE GROWS PALER EVERY MONTH.",
                    asks: Some(Ask { errand: Errand::Slay(EnemyKind::Leech), title: "DRIVE OFF THE SAP-DRINKERS", why: "FELL TEN OF THE LEECHES ON THE FLOORS BELOW, AND THE REST WILL LEARN.", need: 10 }),
                    after: "THEY LATCH ON. PULL HARD. THEY DON'T LIKE BEING PULLED.",
                },
                Folk {
                    name: "MOSS",
                    hello: "I'M FOUR HUNDRED RINGS OLD. THAT'S YOUNG, HERE. WHAT ARE YOU, A HUNDRED?",
                    asks: None,
                    after: "THE SAP RUNS WARMER SINCE YOU CAME. EVERYONE'S NOTICED.",
                },
                Folk {
                    name: "MOTHER BURL",
                    hello: "THE LISTENING KNOT. SPEAK A WISH INTO IT, AND THE ROOTS CARRY IT TO EVERY CORNER OF THE TREE.",
                    asks: None,
                    after: "ONE WISH EACH, FROM ABOVE. WHAT SHOULD TAPROOT GROW?",
                },
            ],
            Culture::Chimefolk => [
                Folk {
                    name: "PRECENTOR VESPER",
                    hello: "WE HEARD YOU BEFORE WE SAW YOU. A STAR MAKES A SOUND, FALLING, LIKE A BELL STRUCK SOFTLY. WE ARE THE CHIMEFOLK. THE WATER ROSE, AND OUR BELLS WENT UNDER ONE BY ONE.",
                    asks: Some(Ask { errand: Errand::Beacons, title: "RING THE DROWNED BELLS", why: "FIVE BELLS STAND ON THE FLOORS BELOW, SILENT SINCE THE FLOOD. RING THEM, AND WE'LL HEAR THAT THE DEEP IS STILL OURS.", need: 5 }),
                    after: "EVERY BELL YOU RING DOWN THERE, OUR BELLS ANSWER. LISTEN.",
                },
                Folk {
                    name: "CANTOR REED",
                    hello: "SOMETHING STOLE OUR HANDBELLS. NOW THEY RING THEM IN THE DARK, WRONG, AND THE DEEP THINGS COME RUNNING.",
                    asks: Some(Ask { errand: Errand::Slay(EnemyKind::Ringer), title: "SILENCE THE FALSE RINGERS", why: "FELL FOUR OF THE RINGERS ON THE WAY DOWN, AND TAKE OUR BELLS BACK OFF THEM.", need: 4 }),
                    after: "A BELL RUNG WRONG IS WORSE THAN NO BELL AT ALL.",
                },
                Folk {
                    name: "SEXTON MARL",
                    hello: "THE OLD BELLS WERE CAST IN BELL-SILVER. IT SINGS IN THE ROCK BELOW, IF YOU KNOW HOW TO LISTEN.",
                    asks: Some(Ask { errand: Errand::Gather, title: "BRING BACK BELL-SILVER", why: "SIX LUMPS OF IT, AND I'LL CAST A NEW BELL FOR THE CATHEDRAL. A BIG ONE.", need: 6 }),
                    after: "TAP IT. IF IT HUMS, IT'S BELL-SILVER. IF IT DOESN'T, IT'S A ROCK.",
                },
                Folk {
                    name: "SISTER NONE",
                    hello: "OUR YOUNGEST CHORISTER WANDERED OFF AFTER AN ECHO. ECHOES LEAD YOU WRONG, DOWN HERE.",
                    asks: Some(Ask { errand: Errand::Rescue, title: "FIND THE LOST CHORISTER", why: "HIS NAME IS CHIME. HE'LL BE SINGING, SO HE WON'T FEEL ALONE. FOLLOW THE SINGING.", need: 1 }),
                    after: "HE ONLY KNOWS THE ONE HYMN. YOU'LL KNOW IT TOO, BY THE END.",
                },
                Folk {
                    name: "TREBLE",
                    hello: "WE RING THE HOURS SO WE KNOW THERE ARE HOURS. OTHERWISE IT'S JUST ONE LONG DARK.",
                    asks: None,
                    after: "THE WATER'S GONE DOWN A FINGER SINCE YOU CAME. WE MARKED IT ON THE PILLAR.",
                },
                Folk {
                    name: "ABBESS TOLL",
                    hello: "THE WISHING BELL. RING IT ONCE WITH A WISH, AND THE CATHEDRAL KEEPS THE NOTE FOREVER.",
                    asks: None,
                    after: "ONE WISH EACH, FROM ABOVE. WHAT SHOULD BELLDROWN SING OF?",
                },
            ],
            Culture::Saltkin => [
                Folk {
                    name: "GRANDMOTHER HALITE",
                    hello: "A STAR FELL INTO WHITEBRINE AND THE SALT LIT UP LIKE NOON. WE ARE THE SALTKIN. WE RAKE THE BRINE AND CUT THE WHITE STONE, AND THE BRINE HAS BEEN GOING DOWN.",
                    asks: Some(Ask { errand: Errand::Beacons, title: "LIGHT THE SALT-LAMPS", why: "FIVE SALT-LAMPS MARK THE OLD BRINE ROADS BELOW. LIGHT THEM, AND OUR CARAVANS CAN WALK THE DEEP AGAIN.", need: 5 }),
                    after: "SALT-LIGHT IS PINK, IF YOU LOOK CLOSE. WE THINK IT'S THE PRETTIEST LIGHT THERE IS.",
                },
                Folk {
                    name: "RAKER BRACK",
                    hello: "THE SPITTERS FOUL OUR BRINE-PANS. ONE GLOB, AND A WHOLE PAN GOES BLACK.",
                    asks: Some(Ask { errand: Errand::Slay(EnemyKind::Spitter), title: "DRIVE OUT THE SPITTERS", why: "FELL EIGHT OF THEM ON THE FLOORS BELOW, AND THE PANS WILL RUN CLEAN.", need: 8 }),
                    after: "DON'T STAND IN WHAT THEY SPIT. YOU'LL SMELL IT FOR A WEEK.",
                },
                Folk {
                    name: "CRYSTAL",
                    hello: "STAR-SALT FALLS THROUGH THE ROCK LIKE SNOW. THE OLD ONES SAY IT'S WHAT'S LEFT OF STARS THAT FELL BEFORE YOU.",
                    asks: Some(Ask { errand: Errand::Gather, title: "GATHER STAR-SALT", why: "SIX HANDFULS, AND I'LL SALT THE TASTING POOL SO EVERY WISH IN IT KEEPS.", need: 6 }),
                    after: "TASTE IT. GO ON. IT TASTES LIKE A COLD MORNING.",
                },
                Folk {
                    name: "OLD FLAKE",
                    hello: "MY BOY CRUST TOOK A CART DOWN THE BRINE ROAD TO TRADE. THE CART CAME BACK. HE DIDN'T.",
                    asks: Some(Ask { errand: Errand::Rescue, title: "FIND THE LOST SALTER", why: "HE'LL HAVE A RAKE OVER HIS SHOULDER AND SALT IN HIS BEARD. HE'S TOO YOUNG FOR A BEARD. IT'S SALT.", need: 1 }),
                    after: "TELL HIM I'M NOT ANGRY. I AM, BUT TELL HIM I'M NOT.",
                },
                Folk {
                    name: "BRINE",
                    hello: "WE CUT A NEW TERRACE EVERY YEAR. THE CITY CLIMBS. ONE DAY IT WILL REACH THE TOP.",
                    asks: None,
                    after: "THE POOLS ARE CLEARER SINCE YOU CAME. YOU CAN SEE YOUR FACE IN THEM.",
                },
                Folk {
                    name: "MOTHER SALINE",
                    hello: "THE TASTING POOL. DROP IN A GRAIN OF SALT WITH YOUR WISH, AND THE POOL KEEPS IT AS LONG AS IT IS SALT.",
                    asks: None,
                    after: "ONE WISH EACH, FROM ABOVE. WHAT SHOULD WHITEBRINE KEEP?",
                },
            ],
        }
    }
}
