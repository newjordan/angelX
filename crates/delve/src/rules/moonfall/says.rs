//! What the folk say: short, kind, a little odd. A person's line depends on
//! the hour, on what the party has done in this loop and on what it learned
//! in earlier ones. The words are static; the choice of them is
//! `Session::topic`.
use super::folk::Folk;
use serde::{Deserialize, Serialize};

/// What a villager is saying to a knight.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Topic {
    /// Small talk by the part of the day: 0 morning, 1 afternoon, 2 evening, 3 night.
    Hello(u8),
    /// The ask itself, heard for the first time or again.
    Request,
    /// They hand something over.
    Give,
    /// They take something and pay for it.
    Thanks,
    /// The favour is already done.
    Done,
    /// Not yet: the reason, by number.
    Nag(u8),
    /// A proclamation or the verse; by number.
    Cry(u8),
}

/// What the choice of words may lean on.
#[derive(Clone, Copy, Debug, Default)]
pub struct Ctx {
    /// Times the town has been rewound.
    pub loops: u32,
    /// Favours done beside the speaker's own.
    pub ticks: u32,
    /// Hours until a pass is ready, if one is being written.
    pub wait_hours: u32,
}

/// The part of the day an hour of the clock falls in.
pub fn band(hour: u32) -> u8 {
    match hour {
        6..=11 => 0,
        12..=16 => 1,
        17..=21 => 2,
        _ => 3,
    }
}

pub fn lines(folk: Folk, topic: Topic, ctx: Ctx) -> Vec<String> {
    let v: Vec<&str> = match (folk, topic) {
        // ---- Mira, who writes the town down --------------------------------
        (Folk::Mira, Topic::Hello(0)) => vec!["GOOD MORNING. I WRITE THE TOWN DOWN.", "YOU WRITE IT DOWN TOO; PRESS Q."],
        (Folk::Mira, Topic::Hello(1)) => vec!["THE LAMPS WAIT FOR DUSK.", "SO DO I. IT IS A GOOD WAIT."],
        (Folk::Mira, Topic::Hello(2)) => vec!["MIND THE WICKS. THE GLOAM LIKES AN OPEN FLAME.", "STAND NEAR A LIGHT AND IT STAYS LIT."],
        (Folk::Mira, Topic::Hello(_)) => vec!["THE THIRD NIGHT IS THE LONGEST.", "KEEP YOUR CANDLES, KNIGHT."],
        (Folk::Mira, Topic::Request) => vec![
            "I SEAL THE SOUTH DOOR FOR A KNIGHT WHO WEARS",
            "BOTH THE SWIFT AND THE STOUT MASK.",
            "COME AT DAWN, WHILE MY PEN IS FRESH.",
        ],
        (Folk::Mira, Topic::Give) => vec!["THERE: THE SEAL OF THE SOUTH DOOR.", "THE BELL TAKES THE DOORS. THE SEAL IS YOURS TO KEEP."],
        (Folk::Mira, Topic::Thanks) => vec![
            "PELLA'S HAND. THE SCROLL IS A RECIPE FOR LEMON CAKE.",
            "TELL HER YES. AND TAKE THE SWIFT MASK:",
            "LIGHT FEET, AND THE PLANKS WILL TRUST YOU.",
        ],
        (Folk::Mira, Topic::Done) => vec!["THE SEAL IS GIVEN; THE DOOR IS YOURS.", "A TICK ON MY PAGE, AND A SMILE."],
        (Folk::Mira, Topic::Nag(0)) => {
            return vec![
                format!("TWO MASKS, KNIGHT. I SEE {} IN YOUR LEDGER.", ctx.ticks),
                "COME BACK WHEN IT SAYS BOTH.".into(),
            ]
        }
        (Folk::Mira, Topic::Nag(1)) => vec!["MY PEN IS DRY. COME AT SIX BELLS,", "WHEN THE DAY IS NEW."],
        (Folk::Mira, Topic::Nag(_)) => vec!["I READ THE POST ALOUD AT THE WELL AT NINE,", "THE SECOND AND THIRD EVENINGS. BRING IT THEN."],
        (Folk::Mira, Topic::Cry(_)) => vec![
            "THE VERSE OF THREE LIGHTS, KNIGHTS:",
            "NORTH, SOUTH AND THE WELL, ALL AT ONCE.",
            "LIGHT THEM AT MIDNIGHT. THE MOON WILL THINK AGAIN.",
        ],
        // ---- Pella, who cries the hours ------------------------------------
        (Folk::Pella, Topic::Hello(0)) => vec!["BELLS AT THE HOUR, KNIGHT. I CRY THE REST.", "MIND THE PODIUM; IT HAS OPINIONS."],
        (Folk::Pella, Topic::Hello(1)) => vec!["NOON WAS A GOOD ONE.", "I HAD NO COLD AND THE PIGEONS KEPT QUIET."],
        (Folk::Pella, Topic::Hello(2)) => vec!["DUSK IS MY FAVOURITE CRY.", "EVERYONE LISTENS, AND NOBODY IS LATE."],
        (Folk::Pella, Topic::Hello(_)) => vec!["HUSH. EVEN THE ECHOES ARE ASLEEP."],
        (Folk::Pella, Topic::Request) => vec![
            "WOULD YOU CARRY A SCROLL TO MIRA QUILL?",
            "I SORT THE POST AT SEVEN, THE FIRST MORNING ONLY.",
            "SHE READS IT AT THE WELL AT NINE, LATER. A MASK FOR YOU.",
        ],
        (Folk::Pella, Topic::Give) => vec!["HERE. DO NOT BEND IT. THE LEMON IS FRAGILE."],
        (Folk::Pella, Topic::Thanks) => vec!["A KIND HAND. THANK YOU."],
        (Folk::Pella, Topic::Done) => vec!["MIRA SAID YES? THE CAKE WILL BE MAGNIFICENT.", "THE MASK LOOKS FAST ON YOU."],
        (Folk::Pella, Topic::Nag(0)) => vec!["YOU HAVE THE SCROLL ALREADY. OFF YOU GO."],
        (Folk::Pella, Topic::Nag(_)) => vec!["THE POST WENT OUT AT SEVEN THE FIRST MORNING.", "YOU ARE A LITTLE LATE, AND A LITTLE EARLY."],
        (Folk::Pella, Topic::Cry(0)) => vec!["NOON! THE BELLS ARE ON TIME.", "THE MOON IS NOT."],
        (Folk::Pella, Topic::Cry(1)) => vec![
            "HEAR YE! A LANTERN RIDER COMES FROM THE EAST,",
            "TOMORROW AT DUSK, WITH OIL FOR THE MOONWELL!",
        ],
        (Folk::Pella, Topic::Cry(2)) => vec![
            "THE RIDER IS AT THE GATE, AND HIS HORSE IS LAME!",
            "SOMEONE KIND SHOULD SEE TO IT.",
        ],
        (Folk::Pella, Topic::Cry(_)) => vec!["THREE TOWERS, ONE MIDNIGHT.", "THE MOON FALLS AT SIX. WE HAVE LONGER THAN WE FEAR."],
        // ---- Nessa, who bakes and forges -----------------------------------
        (Folk::Nessa, Topic::Hello(0)) => vec!["BREAD'S UP. THE OVEN IS COLD, THOUGH.", "LONG STORY. MOSTLY DAMP."],
        (Folk::Nessa, Topic::Hello(1)) => vec!["HAMMER OR LADLE, IT IS ALL THE SAME HEAT.", "ONE OF THEM TASTES BETTER."],
        (Folk::Nessa, Topic::Hello(2)) => vec!["BANKING THE OVEN. A LOAF FOR THE ROAD?", "NO? SENSIBLE. THE ROAD HAS ENOUGH LOAVES."],
        (Folk::Nessa, Topic::Hello(_)) => vec!["ZZZ... NOT AT HOME TO ANYONE WITHOUT BREAD."],
        (Folk::Nessa, Topic::Request) => vec![
            "MY OVEN AND FORGE WANT DRY REEDS.",
            "THE REED BANK, NORTH-WEST. CARRY THEM TWELVE HOURS,",
            "BRING THEM BETWEEN HALF PAST EIGHT AND TEN, DAY TWO.",
        ],
        (Folk::Nessa, Topic::Thanks) => vec![
            "REEDS! BLESSED, CRACKLING REEDS.",
            "HERE: THE STOUT MASK. CRATES WILL MIND THEIR MANNERS.",
        ],
        (Folk::Nessa, Topic::Give) => vec!["A SHOE FOR THE RIDER? TAKE ONE OFF THE CART.", "QUICK NOW. IT LEAVES AT EIGHT."],
        (Folk::Nessa, Topic::Done) => vec!["THE STOUT MASK LOOKS GOOD ON YOU.", "BEARS WOULD ENVY IT."],
        (Folk::Nessa, Topic::Nag(0)) => vec!["A HORSESHOE? WHO FOR? I CAN'T JUST", "GIVE IRON TO A STRANGER'S DREAM."],
        (Folk::Nessa, Topic::Nag(1)) => vec!["THE IRON CART COMES AT DAWN ON THE SECOND DAY.", "IT IS GONE BY EIGHT."],
        (Folk::Nessa, Topic::Nag(2)) => vec!["THESE ARE GREEN. THEY WANT TWELVE HOURS", "IN THE HAND TO GO DRY. TRY AGAIN LATER."],
        (Folk::Nessa, Topic::Nag(_)) => vec!["I TAKE KINDLING ON THE SECOND MORNING ONLY,", "BETWEEN HALF PAST EIGHT AND TEN."],
        (Folk::Nessa, Topic::Cry(_)) => vec!["MIND THE FORGE. IT FORGIVES NOBODY."],
        // ---- Orin, who lost his ink ----------------------------------------
        (Folk::Orin, Topic::Hello(0)) => vec!["MY INK POT... IT WAS RIGHT HERE.", "IT WAS RIGHT HERE YESTERDAY."],
        (Folk::Orin, Topic::Hello(1)) => vec!["A SCRIBE WITHOUT INK IS A MAN WITH OPINIONS.", "I HAVE MANY OPINIONS."],
        (Folk::Orin, Topic::Hello(2)) => vec!["SUPPER! THE BAKER NEVER ASKS WHAT I'VE LOST."],
        (Folk::Orin, Topic::Hello(_)) => vec!["SHH. THE PAGES ARE DREAMING."],
        (Folk::Orin, Topic::Request) => vec![
            "I LOST MY INK POT IN THE REEDS, NORTH-WEST.",
            "BRING IT BY HALF PAST EIGHT THE FIRST MORNING,",
            "AND I'LL WRITE YOU A PASS. FIVE HOURS' WORK.",
        ],
        (Folk::Orin, Topic::Thanks) => {
            return vec![
                "MY POT! MY BLESSED POT!".into(),
                "GIVE ME FIVE HOURS, AND A PASS SHALL BLOOM".into(),
                format!("FROM THIS PEN. ({} HOURS TO GO)", ctx.wait_hours.max(1)),
            ]
        }
        (Folk::Orin, Topic::Give) => vec!["THERE: THE PASS. THE BRIDGE-KEEPER WILL WEEP.", "OR SIGH. IT IS THE SAME FACE."],
        (Folk::Orin, Topic::Done) => vec!["THE PASS WAS A GOOD ONE.", "I DESERVE A SECOND POT. PERHAPS A THIRD."],
        (Folk::Orin, Topic::Nag(0)) => {
            return vec![
                "STILL WRITING. THE LETTERS ARE SHY TODAY.".into(),
                format!("{} HOURS MORE, I THINK.", ctx.wait_hours.max(1)),
            ]
        }
        (Folk::Orin, Topic::Nag(1)) => vec!["THE POT? I BOUGHT ANOTHER, AND A SPARE.", "THE FIRST MORNING WAS THE TIME FOR IT."],
        (Folk::Orin, Topic::Nag(_)) => vec!["THE PASS SMUDGED WHILE IT WAITED.", "A PASS KEEPS ITS INK TWO HOURS, NO MORE."],
        (Folk::Orin, Topic::Cry(_)) => vec!["I SMELL INK. A THEFT, OR A MIRACLE."],
        // ---- Tovin, who keeps the bridge -----------------------------------
        (Folk::Tovin, Topic::Hello(0)) => vec!["BRIDGE IS DOWN. DON'T LET IT GO UP", "WITH YOU ON IT."],
        (Folk::Tovin, Topic::Hello(1)) => vec!["THE NORTH TOWER'S KEY HANGS ON MY BELT.", "NOT FOR EVERYONE."],
        (Folk::Tovin, Topic::Hello(2)) => vec!["BRIDGE GOES UP AT EIGHT.", "ALWAYS HAS. ALWAYS WILL."],
        (Folk::Tovin, Topic::Hello(_)) => vec!["ZZZ. THE BRIDGE IS UP. GOOD NIGHT."],
        (Folk::Tovin, Topic::Request) => vec![
            "NO PASS, NO KEY. ORIN WRITES THEM, WHEN HE HAS HIS INK.",
            "I AIR THE KEYS ON THE SECOND DAY, FIVE TO EIGHT.",
        ],
        (Folk::Tovin, Topic::Thanks) => vec!["A PASS, IN ORIN'S OWN HAND!", "THE NORTH KEY IS YOURS. TURN IT AT THE TOWER."],
        (Folk::Tovin, Topic::Give) => vec!["ANOTHER KEY. I FOUND IT IN MY OTHER POCKET."],
        (Folk::Tovin, Topic::Done) => vec!["THE KEY IS YOURS TO KEEP.", "I'LL KEEP THE BRIDGE TILL EIGHT."],
        (Folk::Tovin, Topic::Nag(_)) => vec!["THE KEYS ARE AIRED ON THE SECOND DAY, AFTER FIVE.", "COME THEN, WITH THE PASS."],
        (Folk::Tovin, Topic::Cry(_)) => vec!["CLOSE UP. THE GLOAM LIKES AN OPEN GATE."],
        // ---- Aster, who rides with oil -------------------------------------
        (Folk::Aster, Topic::Hello(_)) | (Folk::Aster, Topic::Request) => vec![
            "MY HORSE HAS LOST A SHOE. THE OIL IS HEAVY",
            "AND THE WELL IS FAR. A HORSESHOE, IF YOU HAVE ONE?",
        ],
        (Folk::Aster, Topic::Thanks) => vec!["A SHOE! KIND KNIGHT.", "I RIDE FOR THE WELL. THE OIL WILL BE THERE SOON."],
        (Folk::Aster, Topic::Give) => vec!["THE OIL IS IN THE WELL.", "LIGHT IT AT THE THIRD MIDNIGHT, AND NOT BEFORE."],
        (Folk::Aster, Topic::Done) => vec!["THE OIL IS IN THE WELL, AS PROMISED.", "ONLY THE MIDNIGHT WAITS."],
        (Folk::Aster, Topic::Nag(_)) => vec!["I CAN'T CARRY IT FURTHER WITHOUT A SHOE.", "THE FORGE HAS ONE; I SAW IT AS I RODE IN."],
        (Folk::Aster, Topic::Cry(_)) => vec!["THE ROAD WAS EMPTY. THE SKY WAS NOT."],
    };
    let mut out: Vec<String> = v.into_iter().map(String::from).collect();
    // Round again, and they half remember.
    if matches!(topic, Topic::Hello(_)) && ctx.loops > 0 {
        out.push(again(folk).into());
    }
    out
}

fn again(folk: Folk) -> &'static str {
    match folk {
        Folk::Mira => "YOU HAVE THE LOOK OF SOMEONE WHO HAS DONE THIS BEFORE.",
        Folk::Pella => "I FEEL I HAVE CRIED THAT ONE ALREADY. HAVE I?",
        Folk::Nessa => "YOU AGAIN? THE BREAD IS FRESHER THAN YOU'D THINK.",
        Folk::Orin => "DID I ASK ABOUT MY INK BEFORE? I FEEL I DID.",
        Folk::Tovin => "DAYS REPEAT, KNIGHT. THE BRIDGE DOES NOT MIND.",
        Folk::Aster => "THE ROAD LOOKS FAMILIAR. SO DO YOU.",
    }
}

/// The words a knight reads for a thing they stand by (not a person).
pub fn sign(text: Sign) -> &'static str {
    match text {
        Sign::NoInk => "NOTHING IN THE REEDS BUT REEDS.",
        Sign::InkFound => "AN INK POT, SNUGGLED IN THE MUD.",
        Sign::ReedsFound => "A BUNDLE OF DRY REEDS.",
        Sign::HandsFull => "YOUR HANDS ARE FULL. PUT IT DOWN (X).",
        Sign::NorthLocked => "THE DOOR NEEDS ITS KEY.",
        Sign::NorthOpened => "THE KEY TURNS. THE NORTH TOWER STANDS OPEN.",
        Sign::SouthLocked => "THE DOOR NEEDS A SEAL.",
        Sign::SouthOpened => "THE SEAL TAKES. THE SOUTH TOWER STANDS OPEN.",
        Sign::TooHeavy => "TOO HEAVY. A STOUT MASK, PERHAPS (Z).",
        Sign::WellIdle => "THE MOONWELL WAITS FOR OIL AND MIDNIGHT.",
        Sign::WellOiled => "OIL IN THE WELL. IT WAITS FOR THE THIRD MIDNIGHT.",
        Sign::TowerIdle => "THE TOWER WAITS FOR THE THIRD MIDNIGHT.",
        Sign::TowerNeedsVerse => "YOU DO NOT KNOW THE VERSE. MIRA SAYS IT ON THE THIRD EVENING.",
        Sign::SealKept => "THE SOUTH SEAL IS KEPT.",
        Sign::KeyKept => "THE NORTH KEY IS KEPT.",
        Sign::Relit => "RELIT.",
        Sign::Dropped => "PUT DOWN.",
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Sign {
    NoInk,
    InkFound,
    ReedsFound,
    HandsFull,
    NorthLocked,
    NorthOpened,
    SouthLocked,
    SouthOpened,
    TooHeavy,
    WellIdle,
    WellOiled,
    TowerIdle,
    TowerNeedsVerse,
    SealKept,
    KeyKept,
    Relit,
    Dropped,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rules::moonfall::folk::CAST;

    fn clean(s: &str) -> bool {
        s.chars().all(|c| {
            c.is_ascii_uppercase()
                || c.is_ascii_digit()
                || " -.,:/%+!?'()#;".contains(c)
        })
    }

    #[test]
    fn every_line_the_folk_say_fits_the_font_and_the_box() {
        let ctx = Ctx { loops: 2, ticks: 3, wait_hours: 2 };
        for folk in CAST {
            let mut topics = vec![Topic::Request, Topic::Give, Topic::Thanks, Topic::Done];
            topics.extend((0..4).map(Topic::Hello));
            topics.extend((0..4).map(Topic::Nag));
            topics.extend((0..4).map(Topic::Cry));
            for t in topics {
                for line in lines(folk, t, ctx) {
                    assert!(clean(&line), "{folk:?} {t:?}: {line}");
                    assert!(line.len() <= 58, "{folk:?} {t:?} too long: {line}");
                }
            }
        }
        for s in [Sign::NoInk, Sign::NorthLocked, Sign::TowerNeedsVerse, Sign::Dropped] {
            assert!(clean(sign(s)));
        }
    }
}
