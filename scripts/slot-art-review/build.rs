fn main() {
    let path = "../../cockpit/src/stage/world_viz/art.rs";
    println!("cargo:rerun-if-changed={path}");
    let source = std::fs::read_to_string(path).unwrap();
    let cells_path = "../../cockpit/src/stage/world_viz/art/cells.rs";
    println!("cargo:rerun-if-changed={cells_path}");
    let painter = std::fs::read_to_string(cells_path).unwrap();
    let painter = painter.trim_start_matches("//! Shared half-block and braille painter for the cockpit and art exporters.\n");
    // Extract the function by balanced braces, retaining the actual cockpit
    // implementation without linking unrelated services into this reviewer.
    let mut panels = String::new();
    for game in [
        "cinder",
        "latch",
        "mossbell",
        "brasswake",
        "rafterwake",
        "pennon",
    ] {
        let start = source
            .find(&format!("pub(crate) fn paint_{game}_panels("))
            .unwrap();
        let function = &source[start..];
        let end = function.find('{').unwrap();
        let mut depth = 1;
        let mut length = end + 1;
        for c in function[end + 1..].chars() {
            length += c.len_utf8();
            if c == '{' {
                depth += 1;
            }
            if c == '}' {
                depth -= 1;
            }
            if depth == 0 {
                break;
            }
        }
        panels.push_str(&function[..length]);
        panels.push('\n');
    }
    let panels = panels
        .replace(
            "crate::drive::together_shooter::Run",
            "angel_delve::rules::Run",
        )
        .replace(
            "crate::drive::together_shooter::HZ",
            "angel_delve::rules::HZ",
        );
    let out = format!(
        "{painter}\n{panels}\n"
    );
    std::fs::write(
        std::path::Path::new(&std::env::var("OUT_DIR").unwrap()).join("painter.rs"),
        out,
    )
    .unwrap();
}
