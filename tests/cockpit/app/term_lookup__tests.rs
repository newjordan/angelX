use super::*;

static TEST_LOOKUP_IN_FLIGHT: AtomicBool = AtomicBool::new(false);

#[test]
fn busy_lookup_returns_the_complete_offline_lesson() {
    let lease = acquire_lookup(&TEST_LOOKUP_IN_FLIGHT).unwrap();
    let lookup = QuickLookup::spawn_local_with_admission(
        lesson_for_lookup("Poisson distribution"),
        &TEST_LOOKUP_IN_FLIGHT,
    );

    assert_eq!(lookup.reference_status(), ReferenceStatus::Offline);
    assert!(!lookup.is_loading());
    assert!(lookup.visible_text().contains("Poisson distribution"));

    drop(lease);
    assert!(acquire_lookup(&TEST_LOOKUP_IN_FLIGHT).is_some());
}

#[test]
fn selection_candidate_accepts_bounded_multiword_topics_and_rejects_garbage() {
    assert_eq!(selected_term("Poisson,"), Some("Poisson".to_string()));
    assert_eq!(selected_term("p-value"), Some("p-value".to_string()));
    assert_eq!(selected_term("χ2"), Some("χ2".to_string()));
    assert_eq!(
        selected_term("  Poisson   distribution,  "),
        Some("Poisson distribution".to_string())
    );
    assert_eq!(
        selected_term("ray tracing (computer graphics)"),
        Some("ray tracing (computer graphics)".to_string())
    );
    assert_eq!(
        selected_term("(Poisson distribution)"),
        Some("Poisson distribution".to_string())
    );
    assert_eq!(
        selected_term("C++ memory model"),
        Some("C++ memory model".to_string())
    );
    assert_eq!(selected_term("++++"), None);
    assert_eq!(selected_term("ray\ntracing"), None);
    assert_eq!(selected_term("ray\ttracing"), None);
    assert_eq!(
        selected_term(&"x".repeat(crate::library::LOCAL_TOPIC_MAX_CHARS + 1)),
        None
    );
}

#[test]
fn parser_prefers_the_scientific_concept_over_the_eponym() {
    let body = r#"{
          "query": {"pages": [
            {"pageid": 111, "index": 1, "title": "Siméon Denis Poisson",
             "extract": "Siméon Denis Poisson was a French mathematician and physicist."},
            {"pageid": 222, "index": 2, "title": "Poisson distribution",
             "extract": "In probability theory and statistics, the Poisson distribution is a discrete probability distribution. It gives the probability of a number of events in a fixed interval."}
          ]}
        }"#;
    let definition = parse_definition("poisson", body.as_bytes())
        .expect("valid response")
        .expect("STEM result");
    assert_eq!(definition.title, "Poisson distribution");
    assert!(definition.summary.contains("probability theory"));
    assert!(definition.summary.contains("fixed interval."));
    assert_eq!(definition.source_url, "https://en.wikipedia.org/?curid=222");
}

#[test]
fn parser_rejects_unrelated_results_and_disambiguation() {
    let body = r#"{
          "query": {"pages": [
            {"index": 1, "title": "Mercury", "extract": "Mercury may refer to:"},
            {"index": 2, "title": "Mercury Records", "extract": "Mercury Records is a record label."}
          ]}
        }"#;
    assert_eq!(parse_definition("mercury", body.as_bytes()).unwrap(), None);
}

#[test]
fn parser_rejects_a_fuzzy_stem_bystander_unrelated_to_the_selected_word() {
    let body = r#"{
          "query": {"pages": [
            {"pageid": 333, "index": 1, "title": "Isabel Bevier",
             "extract": "Isabel Bevier pioneered the scientific study of home economics."}
          ]}
        }"#;
    assert_eq!(parse_definition("bevy", body.as_bytes()).unwrap(), None);
}

#[test]
fn parser_prefers_the_foundational_vector_space_over_a_trendy_database_match() {
    let body = r#"{
          "query": {"pages": [
            {"pageid": 444, "index": 1, "title": "Vector database",
             "extract": "A vector database is a machine learning data structure using an approximate nearest-neighbor algorithm."},
            {"pageid": 555, "index": 2, "title": "Vector space",
             "extract": "In mathematics and linear algebra, a vector space is a set of vectors that may be added and scaled."}
          ]}
        }"#;
    let definition = parse_definition("vector", body.as_bytes())
        .expect("valid response")
        .expect("STEM result");
    assert_eq!(definition.title, "Vector space");
    assert_eq!(
        crate::library::classify(&format!(
            "vector {} {}",
            definition.title, definition.summary
        )),
        crate::library::Discipline::LinearAlgebra
    );
}

#[test]
fn roll_never_slices_a_multibyte_character() {
    let mut lookup = QuickLookup::ready("χ", "χ²", "A mathematical symbol.");
    lookup.shown = 0;
    while lookup.roll_pending() {
        lookup.roll();
        assert!(lookup.text.is_char_boundary(lookup.shown));
        let _ = lookup.visible_text();
    }
}

#[test]
fn completed_lookup_rolls_definition_source_then_tutor() {
    let (sender, receiver) = mpsc::channel();
    let mut lookup = QuickLookup::loading(lesson_for_lookup("matrix"), receiver);
    sender
        .send(Ok(Some(Definition {
            title: "Matrix (mathematics)".to_string(),
            summary: "A rectangular array used in linear algebra.".to_string(),
            source_url: "https://en.wikipedia.org/?curid=123".to_string(),
        })))
        .unwrap();

    while lookup.is_loading() || lookup.roll_pending() {
        lookup.poll_and_roll(true);
    }
    let definition = lookup.text.find("A rectangular array").unwrap();
    let source = lookup.text.find("Source · Wikipedia").unwrap();
    let tutor = lookup.text.find("Tutor · Sable Vector").unwrap();
    let curriculum = lookup
        .text
        .find("Course · https://github.com/mitmath/1806")
        .unwrap();
    assert!(definition < source && source < tutor && tutor < curriculum);
    assert!(lookup.text.contains("https://en.wikipedia.org/?curid=123"));
}

#[test]
fn loading_feedback_is_immediate_and_motion_off_settles_in_one_tick() {
    let outcome = || TestLookupOutcome::Success {
        title: "Poisson distribution",
        summary: "A discrete probability distribution for event counts.",
        source_url: "https://en.wikipedia.org/?curid=24268",
    };

    let mut animated = QuickLookup::queued("Poisson", outcome());
    assert!(animated.is_loading());
    assert!(animated.visible_text().starts_with("Checking “Poisson”"));
    assert!(
        !animated.roll_pending(),
        "the short loading acknowledgement should not animate"
    );
    animated.poll_and_roll(true);
    assert!(!animated.is_loading());
    assert!(animated.roll_pending());
    assert!(!animated.visible_text().is_empty());

    let mut motion_off = QuickLookup::queued("Poisson", outcome());
    motion_off.poll_and_roll(false);
    assert!(!motion_off.is_loading());
    assert!(!motion_off.roll_pending());
    assert!(motion_off.visible_text().contains("Source · Wikipedia"));
    assert!(motion_off.visible_text().contains("Tutor · Rowan Compass"));
}

#[test]
fn empty_and_disconnected_lookups_finish_as_bounded_world_messages() {
    let (sender, receiver) = mpsc::channel();
    let mut empty = QuickLookup::loading(lesson_for_lookup("notascienceterm"), receiver);
    sender.send(Ok(None)).unwrap();
    empty.poll_and_roll(true);
    assert!(!empty.is_loading());
    assert!(empty.text.contains("No concise STEM or computing entry"));
    assert!(empty.text.contains("LOCAL LESSON · READY OFFLINE"));
    assert_eq!(empty.reference_status, ReferenceStatus::NotFound);

    let (sender, receiver) = mpsc::channel();
    let mut disconnected = QuickLookup::loading(lesson_for_lookup("poisson"), receiver);
    drop(sender);
    disconnected.poll_and_roll(true);
    assert!(!disconnected.is_loading());
    assert!(disconnected.text.starts_with("Quick lookup unavailable"));
    assert!(disconnected.text.contains("LOCAL LESSON · READY OFFLINE"));
    assert_eq!(disconnected.reference_status, ReferenceStatus::Unavailable);

    let mut explicit_error =
        QuickLookup::queued("poisson", TestLookupOutcome::Error("service unavailable"));
    explicit_error.poll_and_roll(false);
    assert!(
        explicit_error
            .visible_text()
            .starts_with("Quick lookup unavailable")
    );
    assert!(
        explicit_error
            .visible_text()
            .contains("LOCAL LESSON · READY OFFLINE")
    );
    assert!(explicit_error.visible_text().contains(
        "Quick lookup unavailable · optional Wikipedia enrichment · service unavailable"
    ));

    let mut explicit_empty = QuickLookup::queued("unknown", TestLookupOutcome::Empty);
    explicit_empty.poll_and_roll(false);
    assert!(
        explicit_empty
            .visible_text()
            .contains("No concise STEM or computing entry")
    );
}

#[test]
fn offline_lookup_is_immediately_useful_and_exposes_safe_handoff_apis() {
    let lookup = QuickLookup::offline("ray tracing with a BVH".to_string());
    assert!(!lookup.is_loading());
    assert!(!lookup.roll_pending());
    assert_eq!(lookup.term(), "ray tracing with a BVH");
    assert_eq!(lookup.reference_status, ReferenceStatus::Offline);
    assert!(
        lookup
            .visible_text()
            .starts_with("LOCAL LESSON · READY OFFLINE")
    );
    assert!(lookup.visible_text().contains("Vesper Raster"));
    assert_eq!(
        lookup.source_url(),
        "https://github.com/RayTracing/raytracing.github.io"
    );
    let prompt = lookup.ask_prompt();
    assert!(prompt.contains("Tutor me on “ray tracing with a BVH”"));
    assert!(prompt.contains(lookup.source_url()));
    assert!(prompt.contains("wait for my answer"));
}

#[test]
fn stem_filter_accepts_every_teaching_library_subject_family() {
    let cases = [
        (
            "poisson",
            "Poisson distribution",
            "A probability distribution used in statistics.",
        ),
        (
            "matrix",
            "Matrix",
            "A rectangular array studied in linear algebra.",
        ),
        (
            "cuda",
            "CUDA",
            "A parallel computing platform for a graphics processing unit.",
        ),
        (
            "shader",
            "Shader",
            "A computer program used in a computer graphics pipeline.",
        ),
        (
            "bevy",
            "Bevy",
            "A data-driven video game engine using an entity component system.",
        ),
        (
            "transformer",
            "Transformer",
            "A deep learning architecture used for a language model.",
        ),
        (
            "protein",
            "Protein",
            "A biological macromolecule encoded by genetic information.",
        ),
        (
            "recursion",
            "Recursion",
            "A method used in mathematics and computer science.",
        ),
    ];
    for (needle, title, extract) in cases {
        assert!(
            stem_score(needle, title, extract) > 0,
            "{title} was rejected: {extract}"
        );
    }
}

#[test]
fn source_urls_are_canonical_and_never_accept_foreign_hosts() {
    let by_id = serde_json::json!({
        "pageid": 42,
        "fullurl": "https://attacker.invalid/poisson"
    });
    assert_eq!(source_url(&by_id), "https://en.wikipedia.org/?curid=42");

    let trusted =
        serde_json::json!({"fullurl": "https://en.wikipedia.org/wiki/Poisson_distribution"});
    assert_eq!(
        source_url(&trusted),
        "https://en.wikipedia.org/wiki/Poisson_distribution"
    );

    let foreign = serde_json::json!({"fullurl": "https://attacker.invalid/poisson"});
    assert_eq!(source_url(&foreign), "https://en.wikipedia.org/");
}

#[test]
#[ignore = "live MediaWiki curriculum-family contract check"]
fn live_lookup_routes_each_teaching_family_to_its_curriculum() {
    let cases = [
        ("poisson", crate::library::Discipline::ProbabilityStatistics),
        ("vector", crate::library::Discipline::LinearAlgebra),
        ("eigenvalue", crate::library::Discipline::LinearAlgebra),
        ("cuda", crate::library::Discipline::GpuKernels),
        ("shader", crate::library::Discipline::Graphics3d),
        ("gameplay", crate::library::Discipline::GameDevelopment),
        ("tokenization", crate::library::Discipline::LlmSystems),
        ("protein", crate::library::Discipline::NaturalSciences),
        ("recursion", crate::library::Discipline::Foundations),
    ];
    for (term, expected) in cases {
        let definition = lookup_definition(term)
            .unwrap_or_else(|error| panic!("{term}: Wikipedia request failed: {error}"))
            .unwrap_or_else(|| panic!("{term}: no STEM or computing result"));
        assert!(
            stem_score(term, &definition.title, &definition.summary) >= 2,
            "{term}: {} — {}",
            definition.title,
            definition.summary
        );
        assert_eq!(
            crate::library::classify(&format!(
                "{term} {} {}",
                definition.title, definition.summary
            )),
            expected,
            "{term}: {} — {}",
            definition.title,
            definition.summary
        );
        assert!(
            definition
                .source_url
                .starts_with("https://en.wikipedia.org/")
        );
    }
}
