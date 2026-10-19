//! Repaint the recorded four-seat impact ticks through the cockpit cells.
#[test]
#[ignore = "exports recorded hit-stop frames and the production terminal cells"]
fn write_hitstop_terminal_shots() {
    use std::io::Read;
    let input = std::path::PathBuf::from(std::env::var_os("ANGEL_HITSTOP_SNAPSHOTS").unwrap());
    let out = std::path::PathBuf::from(std::env::var_os("ANGEL_HITSTOP_CELLS").unwrap());
    std::fs::create_dir_all(&out).unwrap();
    for id in ["waxen-warden", "the-bone-choir", "busy-fight"] {
        let mut selected = std::collections::BTreeMap::<u32, Vec<String>>::new();
        for entry in std::fs::read_dir(&input).unwrap() {
            let path = entry.unwrap().path();
            if path.extension().and_then(|e| e.to_str()) != Some("ppm") {
                continue;
            }
            let name = path.file_stem().unwrap().to_str().unwrap();
            if !name.starts_with(id) {
                continue;
            }
            let tick = name.rsplit('-').next().unwrap().parse().unwrap();
            selected.entry(tick).or_default().push(name.into());
        }
        let mut stream = std::fs::File::open(input.join(format!("{id}.snapshots"))).unwrap();
        for tick in 0..1800 {
            let mut length = [0; 8];
            stream.read_exact(&mut length).unwrap();
            let mut bytes = vec![0; u64::from_le_bytes(length) as usize];
            stream.read_exact(&mut bytes).unwrap();
            if let Some(names) = selected.get(&tick) {
                let run: angel_delve::rules::Run = serde_json::from_slice(&bytes).unwrap();
                assert_eq!(run.players.len(), 4);
                for name in names {
                    super::tests::capture(&out, name, &run, true);
                }
            }
        }
    }
}
