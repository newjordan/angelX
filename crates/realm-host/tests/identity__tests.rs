use super::*;

#[test]
fn landmark_table_rows_match_pre_extraction_literals() {
    let expected = [
        (
            Building::Keep,
            "Keep courtyard",
            (255, 207, 92),
            0,
            "resting in the keep",
            4,
            2,
            1.8,
            (0.50, 0.55),
        ),
        (
            Building::Gatehouse,
            "Gatehouse",
            (255, 103, 126),
            1,
            "at the gatehouse",
            4,
            2,
            1.15,
            (0.08, 0.50),
        ),
        (
            Building::Rookery,
            "Rookery",
            (91, 190, 255),
            2,
            "sealing scrolls at the rookery",
            3,
            1,
            1.6,
            (0.30, 0.50),
        ),
        (
            Building::Scriptorium,
            "Scriptorium",
            (184, 228, 255),
            3,
            "studying in the scriptorium",
            3,
            1,
            1.2,
            (0.55, 0.18),
        ),
        (
            Building::Smithy,
            "Smithy",
            (255, 207, 92),
            4,
            "at work in the smithy",
            3,
            1,
            1.25,
            (0.80, 0.45),
        ),
        (
            Building::Chapel,
            "Chapel",
            (255, 103, 126),
            5,
            "praying in the chapel",
            4,
            1,
            1.5,
            (0.68, 0.80),
        ),
        (
            Building::RoundTable,
            "Round Table",
            (99, 241, 169),
            6,
            "council at the Round Table",
            4,
            2,
            1.1,
            (0.35, 0.82),
        ),
        (
            Building::Observatory,
            "Observatory",
            (86, 232, 255),
            7,
            "charting the heavens at the observatory",
            4,
            2,
            1.3,
            (0.18, 0.20),
        ),
    ];
    for (row, expected) in LANDMARKS.iter().zip(expected) {
        assert_eq!(
            (
                row.kind,
                row.display_name,
                row.rgb,
                row.index,
                row.arrive_activity,
                row.facade_material,
                row.facade_span,
                row.landmark_height,
                row.district_fraction
            ),
            expected
        );
    }
}
