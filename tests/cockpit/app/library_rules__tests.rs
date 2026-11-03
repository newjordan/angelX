use super::*;

#[test]
fn compiled_original_routing_golden_covers_term_combinations() {
    // Digest captured from the compiled pre-migration classifier and shelf
    // selector, with all individual terms, every ordered pair, and all eight
    // disciplines (including the two Unicode/nonmatching cases).
    let routing = capture(None).unwrap();
    let terms = routing
        .classify
        .iter()
        .chain(&routing.shelves)
        .flat_map(|rule| rule.terms.iter().map(String::as_str))
        .collect::<Vec<_>>();
    let mut samples = vec![
        String::new(),
        "unrelated subject".into(),
        "a GPU compute question".into(),
        "cellular biology and chemistry".into(),
        "eigen matrix 3d\u{212a}".into(),
    ];
    for a in &terms {
        samples.push((*a).to_string());
        for b in &terms {
            samples.push(format!("{a} mixed {b}"));
        }
    }
    let mut hash = 0xcbf29ce484222325u64;
    let mut feed = |value: &str| {
        for byte in value.as_bytes() {
            hash ^= u64::from(*byte);
            hash = hash.wrapping_mul(0x100000001b3);
        }
        hash ^= 0xff;
        hash = hash.wrapping_mul(0x100000001b3);
    };
    for sample in &samples {
        feed(sample);
        feed(&format!("{:?}", routing.classify(sample)));
        for discipline in [
            Discipline::ProbabilityStatistics,
            Discipline::LinearAlgebra,
            Discipline::GpuKernels,
            Discipline::Graphics3d,
            Discipline::GameDevelopment,
            Discipline::LlmSystems,
            Discipline::NaturalSciences,
            Discipline::Foundations,
        ] {
            feed(routing.shelf(discipline, sample).title);
        }
    }
    assert_eq!(samples.len(), 18_095);
    assert_eq!(hash, 0xed96d66e830eddaf);
}

#[test]
fn reload_pins_each_capture_and_rejects_invalid_sources() {
    let temp = crate::tests::TestGitWorkspace::new("lesson-routing-reload");
    let path = temp.path().join("routing.json");
    std::fs::write(&path, EMBEDDED).unwrap();
    let old = capture(Some(&path)).unwrap();
    let mut value: serde_json::Value = serde_json::from_str(EMBEDDED).unwrap();
    value["classify"][0]["terms"] = serde_json::json!(["newsearchterm"]);
    std::fs::write(&path, serde_json::to_vec(&value).unwrap()).unwrap();
    let new = capture(Some(&path)).unwrap();
    assert_eq!(old.classify("cuda"), Discipline::GpuKernels);
    assert_eq!(new.classify("cuda"), Discipline::Foundations);
    assert_eq!(new.classify("newsearchterm"), Discipline::GpuKernels);
    value["shelves"][0]["shelf"] = "not a real shelf".into();
    std::fs::write(&path, serde_json::to_vec(&value).unwrap()).unwrap();
    assert!(capture(Some(&path)).is_err());
    assert!(capture(Some(Path::new("relative.json"))).is_err());
    std::fs::write(&path, vec![b'x'; MAX_BYTES as usize + 1]).unwrap();
    assert!(capture(Some(&path)).is_err());
}

#[cfg(unix)]
#[test]
fn override_rejects_symlinks() {
    let temp = crate::tests::TestGitWorkspace::new("lesson-routing-symlink");
    let path = temp.path().join("routing.json");
    let link = temp.path().join("link.json");
    std::fs::write(&path, EMBEDDED).unwrap();
    std::os::unix::fs::symlink(path, &link).unwrap();
    assert!(capture(Some(&link)).is_err());
}
