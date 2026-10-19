use super::*;

fn rows(text: &str) -> Vec<String> {
    text.lines().map(|l| l.trim().to_string()).filter(|l| !l.is_empty()).collect()
}

/// A shaded, outlined orb: dark rim, mid body, a light core and a glint.
const ORB: &str = "
    ....jjjjjj....
    ...jJhhhhJj...
    ..jJhh44hhJj..
    ..jJh4@@4hJj..
    ..jJh4@6@4hJ..
    ..jJh4@@4hJj..
    ..jJhh44hhJj..
    ...jJhhhhJj...
    ....jjjjjj....
";

/// One ink, a filled square: a blob.
const BLOB: &str = "
    77777777
    77777777
    77777777
    77777777
    77777777
    77777777
";

/// A scatter of specks in two inks.
const SPECKS: &str = "
    7.....2.....
    ....7.....2.
    ..2.....7...
    ......2...7.
";

#[test]
fn a_one_ink_blob_is_crude() {
    let (grade, score) = grade([rows(BLOB).as_slice()]);
    assert_eq!(grade, Grade::Crude, "scored {score}");
}

#[test]
fn a_scatter_of_specks_is_crude() {
    let m = marks(&rows(SPECKS));
    assert_eq!(m.silhouette, 0, "{m:?}");
    assert_eq!(Grade::of_score(m.score()), Grade::Crude, "{m:?}");
}

#[test]
fn a_shaded_outlined_orb_is_fine_or_better_and_beats_a_blob() {
    let m = marks(&rows(ORB));
    assert!(m.outline > 0 && m.shading > 0 && m.detail > 0 && m.silhouette == SILHOUETTE, "{m:?}");
    assert!(Grade::of_score(m.score()) >= Grade::Fine, "{m:?} = {}", m.score());
    assert!(m.score() > score(&rows(BLOB)) + 30);
}

#[test]
fn an_empty_picture_scores_nothing_and_no_picture_is_crude() {
    assert_eq!(score(&rows("....\n....")), 0);
    assert_eq!(grade(std::iter::empty::<&[String]>()).0, Grade::Crude);
}

#[test]
fn the_grade_is_the_mean_of_a_wishs_pictures() {
    let (orb, blob) = (rows(ORB), rows(BLOB));
    let (_, both) = grade([orb.as_slice(), blob.as_slice()]);
    assert_eq!(both, (score(&orb) + score(&blob)) / 2);
}

#[test]
fn a_graded_number_keeps_at_least_one_and_a_legend_gives_a_little_over() {
    assert_eq!(graded(1, Grade::Crude), 1);
    assert_eq!(graded(0, Grade::Legendary), 0);
    assert_eq!(graded(20, Grade::Crude), 12);
    assert_eq!(graded(20, Grade::Fine), 16);
    assert_eq!(graded(20, Grade::Masterwork), 20);
    assert_eq!(graded(20, Grade::Legendary), 23);
    assert_eq!(graded(3, Grade::Crude), 2);
}

#[test]
fn grades_climb_with_the_score() {
    assert_eq!(Grade::of_score(0), Grade::Crude);
    assert_eq!(Grade::of_score(FINE), Grade::Fine);
    assert_eq!(Grade::of_score(MASTERWORK), Grade::Masterwork);
    assert_eq!(Grade::of_score(LEGENDARY), Grade::Legendary);
    assert!(Grade::Crude.power() < Grade::Fine.power());
    assert!(Grade::Fine.power() < Grade::Masterwork.power());
    assert!(Grade::Masterwork.power() < Grade::Legendary.power());
}
