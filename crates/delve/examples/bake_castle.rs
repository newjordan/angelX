//! Bake a castle for the game: `angel-castle/v1` JSON in, the compact asset
//! out (`world::works`).
//!
//! `cargo run -p angel-delve --example bake_castle -- <in.castle.json> <plan-id> <out.castle>`
//!
//! The asset the game ships is `crates/delve/assets/works/great-tower.castle`,
//! baked from Blocks' `great-tower.castle.json`; run this again when Blocks
//! makes a better one. It prints what it baked.

use angel_delve::world::works::Castle;

fn main() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [input, plan, output] = args.as_slice() else {
        return Err("usage: bake_castle <in.castle.json> <plan-id> <out.castle>".into());
    };
    let text = std::fs::read_to_string(input).map_err(|e| format!("{input}: {e}"))?;
    let castle = Castle::from_json(plan, &text)?;
    let bytes = castle.encode();
    // What the game reads must be what was baked.
    let back = Castle::decode(&bytes)?;
    if back != castle {
        let first = castle.stones.iter().zip(&back.stones).position(|(a, b)| a != b);
        return Err(format!(
            "the baked castle does not read back the same (first stone off: {first:?}; tables {} falsework {} earth {} solid {} bounds {})",
            castle.materials == back.materials && castle.roles == back.roles && castle.stages == back.stages,
            castle.falsework == back.falsework,
            castle.earth == back.earth,
            castle.solid == back.solid,
            castle.bounds == back.bounds
        ));
    }
    std::fs::write(output, &bytes).map_err(|e| format!("{output}: {e}"))?;
    let blocked = castle.solid.iter().filter(|&&n| n != angel_delve::world::works::NEVER).count();
    println!(
        "{plan}: {} stones, {} stages, {} falsework, {} earth, {blocked} blocked tiles of {}; {} bytes ({} of the JSON)",
        castle.stones.len(),
        castle.stages.len(),
        castle.falsework.len(),
        castle.earth.len(),
        castle.solid.len(),
        bytes.len(),
        text.len()
    );
    Ok(())
}
