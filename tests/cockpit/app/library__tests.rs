use super::*;
use std::collections::HashSet;

const DISCIPLINES: [Discipline; 8] = [
    Discipline::ProbabilityStatistics,
    Discipline::LinearAlgebra,
    Discipline::GpuKernels,
    Discipline::Graphics3d,
    Discipline::GameDevelopment,
    Discipline::LlmSystems,
    Discipline::NaturalSciences,
    Discipline::Foundations,
];

#[test]
fn every_discipline_has_a_tutor_and_multiple_world_shelves_exist() {
    for discipline in DISCIPLINES {
        assert!(!tutor_for(discipline).name.is_empty());
        assert!(!primary_shelf(discipline).url.is_empty());
        assert!(
            CURRICULUM
                .iter()
                .any(|shelf| shelf.discipline == discipline),
            "{discipline:?} has no curriculum"
        );
    }
    assert!(CURRICULUM.len() >= 10);
}

#[test]
fn resident_faculty_and_curriculum_catalog_are_unique_and_actionable() {
    let mut tutor_ids = HashSet::new();
    let mut tutor_names = HashSet::new();
    for tutor in TUTORS {
        assert!(
            tutor_ids.insert(tutor.id),
            "duplicate tutor id {}",
            tutor.id
        );
        assert_eq!(tutor_by_id(tutor.id), Some(&tutor));
        assert!(
            tutor_names.insert(tutor.name),
            "duplicate tutor {}",
            tutor.name
        );
        assert!(!tutor.role.trim().is_empty());
        assert!(
            tutor.method.split_whitespace().count() >= 6,
            "{} needs a concrete teaching method",
            tutor.name
        );
    }

    let mut shelf_ids = HashSet::new();
    let mut urls = HashSet::new();
    let mut titles = HashSet::new();
    for shelf in CURRICULUM {
        assert!(
            shelf_ids.insert(shelf.id),
            "duplicate curriculum id {}",
            shelf.id
        );
        assert_eq!(shelf_by_id(shelf.id), Some(&shelf));
        assert!(
            shelf.url.starts_with("https://github.com/"),
            "{} must remain an inspectable upstream curriculum",
            shelf.title
        );
        assert!(
            urls.insert(shelf.url),
            "duplicate curriculum URL {}",
            shelf.url
        );
        let github_path = shelf
            .url
            .strip_prefix("https://github.com/")
            .expect("curriculum URL has a GitHub path");
        let parts = github_path.split('/').collect::<Vec<_>>();
        assert!(
            parts.len() == 2
                || (parts.len() >= 5
                    && parts[2] == "tree"
                    && parts[3..].iter().all(|part| !part.is_empty())),
            "{} has a malformed repository or tree URL: {}",
            shelf.title,
            shelf.url
        );
        assert!(
            !parts[0].is_empty() && !parts[1].is_empty(),
            "{} is missing a GitHub owner or repository",
            shelf.title
        );
        assert!(
            titles.insert(shelf.title),
            "duplicate curriculum title {}",
            shelf.title
        );
        assert!(!shelf.source.trim().is_empty());
        assert!(
            shelf.arc.len() >= 4,
            "{} needs a real learning sequence",
            shelf.title
        );
        assert!(
            shelf.arc.iter().all(|concept| !concept.trim().is_empty()),
            "{} contains a blank concept",
            shelf.title
        );
    }
    assert_eq!(catalog_tutors(), &TUTORS);
    assert_eq!(catalog_shelves(), &CURRICULUM);
}

#[test]
fn multiword_topics_normalize_into_immediate_offline_lessons() {
    let lesson = local_lesson("  ray\n tracing\twith   a BVH  ")
        .expect("a multiword topic produces a lesson");
    assert_eq!(lesson.topic, "ray tracing with a BVH");
    assert_eq!(lesson.discipline, Discipline::Graphics3d);
    assert_eq!(lesson.tutor.id, "vesper-raster");
    assert_eq!(lesson.shelf.id, "graphics-ray-tracing");
    assert_eq!(lesson.source_label, "Ray Tracing in One Weekend");
    assert!(lesson.next_arc.contains(&"light transport"));

    let rendered = lesson.render_offline();
    assert!(rendered.starts_with("LOCAL LESSON · READY OFFLINE"));
    assert!(rendered.contains("Objective ·"));
    assert!(rendered.contains("Exercise ·"));
    assert!(rendered.contains("Checkpoint ·"));
    assert!(rendered.contains(lesson.source_url));
    assert!(rendered.contains("course content has not been loaded"));
    assert!(!rendered.contains("Source · Wikipedia"));
}

#[test]
fn topic_normalization_is_bounded_unicode_safe_and_rejects_noise() {
    assert_eq!(normalize_topic(" \n\t "), None);
    assert_eq!(normalize_topic("+++"), None);
    assert_eq!(
        normalize_topic("χ²   distribution"),
        Some("χ² distribution".into())
    );

    let long = format!("{} final concept", "界".repeat(LOCAL_TOPIC_MAX_CHARS));
    let normalized = normalize_topic(&long).expect("long topic is clipped");
    assert_eq!(normalized.chars().count(), LOCAL_TOPIC_MAX_CHARS);
    assert!(normalized.ends_with('…'));
    assert!(local_lesson(&normalized).is_some());
}

#[test]
fn catalog_shelf_opens_by_stable_id_without_network_or_model_state() {
    let lesson =
        local_lesson_for_shelf("llm-systems-cs336").expect("stable shelf id opens a lesson");
    assert_eq!(lesson.topic, "Language models from scratch");
    assert_eq!(lesson.shelf.id, "llm-systems-cs336");
    assert_eq!(lesson.tutor.id, "ilex-tokenwright");
    assert_eq!(lesson.next_arc.first(), Some(&"tokenization"));
    assert!(local_lesson_for_shelf("missing-shelf").is_none());
}

#[test]
fn ask_tutor_handoff_is_editable_attributed_and_does_not_claim_source_access() {
    let lesson = local_lesson("Poisson distribution").unwrap();
    let prompt = lesson.ask_tutor_prompt();
    assert!(prompt.starts_with("Tutor me on “Poisson distribution”"));
    assert!(prompt.contains(lesson.tutor.name));
    assert!(prompt.contains(lesson.tutor.method));
    assert!(prompt.contains(lesson.source_label));
    assert!(prompt.contains(lesson.source_url));
    assert!(prompt.contains("do not claim you have read its content"));
    assert!(prompt.contains("wait for my answer before continuing"));
}

#[test]
fn every_discipline_builds_a_complete_local_lesson() {
    let topics = [
        "Poisson probability distribution",
        "eigenvalue of a matrix",
        "CUDA reduction kernel",
        "ray tracing BVH",
        "game engine gameplay loop",
        "transformer attention",
        "quantum particle physics",
        "recursion",
    ];
    let reached = topics
        .into_iter()
        .map(|topic| local_lesson(topic).expect("representative lesson"))
        .map(|lesson| {
            assert!(!lesson.objective.trim().is_empty());
            assert!(!lesson.exercise.trim().is_empty());
            assert!(!lesson.checkpoint.trim().is_empty());
            assert!(!lesson.source_url.trim().is_empty());
            assert!(!lesson.next_arc.is_empty());
            lesson.discipline
        })
        .collect::<HashSet<_>>();
    assert_eq!(reached, DISCIPLINES.into_iter().collect());
}

#[test]
fn living_catalog_exposes_every_resident_and_sourced_study_arc_once() {
    let catalog = catalog_text();
    assert!(catalog.starts_with("RESIDENT FACULTY · 6\n"));
    assert!(catalog.contains("CURRICULUM SHELVES · 16\n"));

    for tutor in TUTORS {
        assert_eq!(catalog.matches(tutor.name).count(), 1, "{}", tutor.name);
        assert_eq!(catalog.matches(tutor.role).count(), 1, "{}", tutor.name);
        assert_eq!(catalog.matches(tutor.method).count(), 1, "{}", tutor.name);
    }
    for shelf in CURRICULUM {
        assert_eq!(catalog.matches(shelf.title).count(), 1, "{}", shelf.title);
        assert_eq!(
            catalog
                .matches(&format!("     Source · {}", shelf.source))
                .count(),
            1,
            "{}",
            shelf.title
        );
        assert_eq!(
            catalog
                .matches(&format!("     Course · {}", shelf.url))
                .count(),
            1,
            "{}",
            shelf.title
        );
        assert_eq!(
            catalog
                .matches(&format!("     Arc · {}", shelf.arc.join(" → ")))
                .count(),
            1,
            "{}",
            shelf.title
        );
    }
}

#[test]
fn high_value_domains_route_to_their_resident_tutors() {
    assert_eq!(
        classify("eigenvalue of a matrix"),
        Discipline::LinearAlgebra
    );
    assert_eq!(classify("Triton CUDA reduction"), Discipline::GpuKernels);
    assert_eq!(classify("WebGPU shader"), Discipline::Graphics3d);
    assert_eq!(classify("Bevy game engine"), Discipline::GameDevelopment);
    assert_eq!(classify("transformer attention"), Discipline::LlmSystems);
    assert_eq!(
        classify("Poisson probability distribution"),
        Discipline::ProbabilityStatistics
    );
    assert_eq!(
        classify("quantum particle physics"),
        Discipline::NaturalSciences
    );
    assert_eq!(
        classify("chemical molecule reaction"),
        Discipline::NaturalSciences
    );
    assert_eq!(
        classify("cellular protein genetics"),
        Discipline::NaturalSciences
    );
    assert_eq!(
        classify("medical disease physiology"),
        Discipline::NaturalSciences
    );
}

#[test]
fn teaching_note_names_provenance_and_next_concepts() {
    let note = teaching_note("matrix", "Matrix", "A rectangular array in linear algebra.");
    assert!(note.contains("Sable Vector"));
    assert!(note.contains("MIT 18.06"));
    assert!(note.contains("https://github.com/mitmath/1806"));
    assert!(note.contains("Next · vectors"));
}

#[test]
fn every_lesson_closes_with_deterministic_recall_practice() {
    // Every shelf must deliver a lesson whose Recall section is
    // deterministic (same lesson, same prompt) and names that lesson's
    // resident tutor.
    for shelf in catalog_shelves() {
        let lesson = local_lesson_for_shelf(shelf.id).expect("shelf lesson");
        let once = lesson.recall_prompt();
        let twice = lesson.recall_prompt();
        assert_eq!(once, twice, "recall prompts are deterministic");
        assert!(
            once.contains(lesson.tutor.name),
            "recall names its tutor: {once}"
        );
        assert!(once.contains("Recall"), "{once}");
        let rendered = lesson.render_offline();
        assert!(
            rendered.contains("Recall · answer from memory, then check above"),
            "offline render embeds the recall section: {rendered}"
        );
        // The Ask Tutor handoff inherits the SAME recall practice, so the
        // tutor checks the answers instead of re-reading the material.
        let handoff = lesson.ask_tutor_prompt();
        assert!(
            handoff.contains("End the lesson with retrieval practice"),
            "handoff schedules the recall check: {handoff}"
        );
        assert!(
            handoff.contains(&lesson.recall_prompt()),
            "handoff reuses the single-sourced recall prompts: {handoff}"
        );
    }
}

#[test]
fn every_resident_can_deliver_a_personalized_sourced_lesson() {
    let samples = [
        (
            Discipline::ProbabilityStatistics,
            "Poisson",
            "probability distribution",
        ),
        (Discipline::LinearAlgebra, "matrix", "linear algebra matrix"),
        (Discipline::GpuKernels, "warp", "CUDA warp kernel"),
        (Discipline::Graphics3d, "shader", "WebGPU shader rendering"),
        (
            Discipline::GameDevelopment,
            "gameplay",
            "game engine gameplay loop",
        ),
        (Discipline::LlmSystems, "attention", "transformer attention"),
        (
            Discipline::NaturalSciences,
            "quantum",
            "quantum particle physics",
        ),
        (Discipline::Foundations, "recursion", "introductory concept"),
    ];
    for (discipline, term, context) in samples {
        assert_eq!(classify(context), discipline);
        let note = teaching_note(term, term, context);
        let tutor = tutor_for(discipline);
        assert!(note.contains(tutor.name), "{discipline:?}: {note}");
        assert!(note.contains(tutor.role), "{discipline:?}: {note}");
        assert!(note.contains(tutor.method), "{discipline:?}: {note}");
        assert!(note.contains("Prompt ·"), "{discipline:?}: {note}");
        assert!(
            note.contains(shelf_for(discipline, context).source),
            "{discipline:?}: {note}"
        );
        assert!(
            note.contains(shelf_for(discipline, context).url),
            "{discipline:?}: {note}"
        );
        assert!(note.contains("Next ·"), "{discipline:?}: {note}");
    }
}

#[test]
fn probability_and_natural_sciences_choose_relevant_college_shelves() {
    let poisson = teaching_note(
        "Poisson",
        "Poisson distribution",
        "A discrete probability distribution for event counts.",
    );
    assert!(poisson.contains("Probability and statistics"), "{poisson}");
    assert!(poisson.contains("OpenStax Statistics"), "{poisson}");
    assert!(poisson.contains("random variables"), "{poisson}");
    assert!(!poisson.contains("OSSU"), "{poisson}");

    let physics = teaching_note(
        "quantum",
        "Quantum mechanics",
        "A theory in physics describing particles and energy.",
    );
    assert!(physics.contains("OpenStax University Physics"), "{physics}");

    let chemistry = teaching_note(
        "molecule",
        "Molecule",
        "A chemical group of bonded atoms involved in reactions.",
    );
    assert!(chemistry.contains("OpenStax Chemistry"), "{chemistry}");

    let biology = teaching_note(
        "protein",
        "Protein",
        "A biological molecule encoded by genetic information in cells.",
    );
    assert!(biology.contains("OpenStax Biology"), "{biology}");
}

#[test]
fn every_curriculum_shelf_is_reachable_from_a_representative_lesson() {
    let cases = [
        (
            "recursion in introductory programming",
            "College foundations",
        ),
        (
            "computational thinking with a climate model and simulation",
            "Computational thinking",
        ),
        (
            "Poisson probability distribution",
            "Probability and statistics",
        ),
        ("quantum particle physics", "University physics"),
        ("chemical reaction and molecular bond", "College chemistry"),
        ("cellular protein genetics", "College biology"),
        ("eigenvalue of a matrix", "Linear algebra spine"),
        (
            "numerical linear algebra factorization stability",
            "Numerical linear algebra",
        ),
        ("CUDA warp reduction kernel", "GPU kernel forge"),
        ("GPU architecture roofline compiler", "GPU field references"),
        ("raster geometry shading", "Renderer from first pixels"),
        ("ray tracing BVH light transport", "Ray tracing shelf"),
        ("WebGPU render pipeline adapter", "WebGPU workshop"),
        ("game engine gameplay loop", "Game systems workshop"),
        (
            "transformer attention scaling",
            "Language models from scratch",
        ),
        (
            "small language model tokenizer SFT",
            "Small complete LLM laboratory",
        ),
    ];
    let mut reached = HashSet::new();
    for (context, expected_title) in cases {
        let discipline = classify(context);
        let shelf = shelf_for(discipline, context);
        assert_eq!(shelf.title, expected_title, "{context}");
        let note = teaching_note(context, context, context);
        assert!(
            note.contains(&format!("Course · {}", shelf.url)),
            "{} is reachable but its upstream URL is hidden\n{note}",
            shelf.title
        );
        reached.insert(shelf.title);
    }
    assert_eq!(
        reached.len(),
        CURRICULUM.len(),
        "every linked shelf must be reachable"
    );
    assert!(
        CURRICULUM.iter().all(|shelf| reached.contains(shelf.title)),
        "one or more cataloged shelves are decorative-only"
    );
}
