use super::*;
use crate::agent::club::Club;
use crate::delve::together_grants::Scripted;
use crate::delve::together_shooter::layout::Pack;
use crate::delve::together_shooter::salvage::{Grade, Place, Salvage};

trait Start {
    fn start(&mut self, club: Arc<dyn Club>, plan: Plan, fleet: Vec<Vessel>);
}

impl Start for Shipwright {
    /// Ask `club` for build `plan` on a thread of its own.
    fn start(&mut self, club: Arc<dyn Club>, plan: Plan, fleet: Vec<Vessel>) {
        self.start_oracle(crate::app::control::realm::club_oracle(club), plan, fleet);
    }
}

/// One build asked of `club`: the brief, the checker, one mend.
fn design(club: &dyn Club, plan: &Plan, fleet: &[Vessel]) -> Result<Design, String> {
    design_with(plan, fleet, |prompt| crate::agent::club::with_read_timeout_at_least(GRANT_CALL, || club.respond(prompt)))
}

fn plan() -> Plan {
    let mut yard = Salvage::default();
    for place in Place::CORE {
        let id = yard.add(place, Grade::Fine, "You", Pack::Archive);
        yard.pieces.iter_mut().find(|p| p.id == id).unwrap().wish = Some(format!("a purple drank {}", place.word()));
    }
    yard.plan().expect("the core four")
}

const GOOD: &str = "vessel The Sizzurp Belle\nsays A swaggering purple bore with a syrup heart and a cup for a stack.\ntraits drunk, vain\nquirks lush\nvoice launch :: Hold my cup.\nvoice breach :: Surprise, rock.\nvoice ore :: Ooh. Shiny.\nvoice hurt :: Not the paint!\nvoice gnawer :: That worm is not invited.\nvoice home :: Somebody polish me.\nvoice fallen :: Up, sugar.\nvoice idle :: Are we digging or what?\n";

#[test]
fn the_model_reads_every_piece_and_its_checked_answer_is_the_vessel() {
    let club = Scripted::new(&[GOOD], Duration::ZERO);
    let made = design(club.as_ref(), &plan(), &[]).expect("checks");
    assert_eq!(made.name, "The Sizzurp Belle");
    let asked = club.asked.lock().unwrap();
    assert_eq!(asked.len(), 1);
    assert!(asked[0].contains("a purple drank prow"), "the brief reads the wishes");
}

#[test]
fn an_answer_the_checker_turns_away_is_mended_once() {
    let bad = GOOD.replace("traits drunk, vain", "traits drunk, sparkly");
    let club = Scripted::new(&[&bad, GOOD], Duration::ZERO);
    assert!(design(club.as_ref(), &plan(), &[]).is_ok());
    let asked = club.asked.lock().unwrap();
    assert_eq!(asked.len(), 2);
    assert!(asked[1].contains("`sparkly` is not one of"), "the mend says why");
    let club = Scripted::new(&[&bad], Duration::ZERO);
    assert!(design(club.as_ref(), &plan(), &[]).is_err(), "a second bad answer is not taken");
}

#[test]
fn a_build_is_never_lost_the_fallback_makes_her_when_the_model_fails() {
    let mut wright = Shipwright::default();
    let club: Arc<dyn Club> = Scripted::new(&["ERR: the link dropped"], Duration::ZERO);
    wright.start(club, plan(), Vec::new());
    let mut landed = None;
    for _ in 0..200 {
        landed = wright.landed();
        if landed.is_some() {
            break;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    let (job, made, by_model) = landed.expect("the job lands");
    assert!(!by_model);
    assert_eq!(job.ids().len(), 4);
    assert!(salvage::check(&format!(
        "vessel {}\nsays {}\ntraits {}\nquirks {}\n{}",
        made.name,
        made.says,
        made.traits.join(", "),
        made.quirks.join(", "),
        made.voice.iter().map(|(m, l)| format!("voice {m} :: {l}\n")).collect::<String>()
    ), &[])
    .is_ok());
    assert!(wright.job.is_none());
}
