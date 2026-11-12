use super::*;

fn scores(list: &[(u32, u16)]) -> Vec<(u32, u16)> {
    list.to_vec()
}
fn cries(calls: &[Call]) -> Vec<Cry> {
    calls.iter().map(|c| c.cry).collect()
}
#[test]
fn weight_follows_the_share_of_health_taken() {
    assert_eq!(Weight::of(5, 100, false, false), Weight::Graze);
    assert_eq!(Weight::of(20, 100, false, false), Weight::Solid);
    assert_eq!(Weight::of(50, 100, false, false), Weight::Crushing);
    assert_eq!(Weight::of(1, 100, true, false), Weight::Kill);
    assert_eq!(Weight::of(1, 100, true, true), Weight::Finale);
    assert!(Weight::Finale.shake() > Weight::Solid.shake());
}
#[test]
fn first_blood_chains_sprees_and_shutdowns_are_called() {
    let mut j = Juice::default();
    let c = j.kill(10, 1, 2, &scores(&[(1, 1), (2, 0)]), 10);
    assert_eq!(cries(&c), vec![Cry::FirstBlood], "the first kill opens, nothing else");
    let c = j.kill(40, 1, 3, &scores(&[(1, 2), (2, 0), (3, 0)]), 10);
    assert!(cries(&c).contains(&Cry::Chain(2)) && !cries(&c).contains(&Cry::FirstBlood));
    let c = j.kill(70, 1, 2, &scores(&[(1, 3), (2, 0), (3, 0)]), 10);
    assert!(cries(&c).contains(&Cry::Chain(3)) && cries(&c).contains(&Cry::Spree(3)));
    assert_eq!(j.streak(1), 3);
    let c = j.kill(400, 2, 1, &scores(&[(1, 3), (2, 1), (3, 0)]), 10);
    assert!(cries(&c).contains(&Cry::Shutdown(3)) && !cries(&c).contains(&Cry::Chain(2)));
    assert_eq!((j.streak(1), j.best(1)), (0, 3));
}
#[test]
fn revenge_lead_and_match_point_are_called_once() {
    let mut j = Juice::default();
    j.kill(10, 1, 2, &scores(&[(1, 1), (2, 0)]), 3);
    let c = j.kill(300, 2, 1, &scores(&[(1, 1), (2, 1)]), 3);
    assert!(cries(&c).contains(&Cry::Revenge), "{c:?}");
    let c = j.kill(600, 2, 3, &scores(&[(1, 1), (2, 2), (3, 0)]), 3);
    assert!(cries(&c).contains(&Cry::Lead) && cries(&c).contains(&Cry::Point), "{c:?}");
    let c = j.kill(900, 2, 1, &scores(&[(1, 1), (2, 3)]), 3);
    assert!(!cries(&c).contains(&Cry::Point), "match point is called once");
}
#[test]
fn old_blows_and_calls_go_and_the_books_stay_bounded() {
    let mut j = Juice::default();
    for n in 0..40 {
        j.blow(Blow { owner: 1, target: 2, x: 1., y: 1., from: (0., 0.), lost: 10, weight: Weight::Solid, born: n });
    }
    assert!(j.blows.len() <= 24 && j.valid());
    j.kill(5, 1, 2, &scores(&[(1, 1), (2, 0)]), 10);
    j.tick(5 + CALL_LIFE);
    assert!(j.calls.is_empty() && j.blows.is_empty());
    let back: Juice = serde_json::from_str(&serde_json::to_string(&j).unwrap()).unwrap();
    assert_eq!(back, j);
    assert!(Juice::default().quiet());
}
