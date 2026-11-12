//! Prints the kingdom economy's design checks and the full requirement tree
//! for a castle at a pace, as JSON.
//!
//!     cargo run -p angel-kingdom --example economy_tree -- [castle] [stones per minute] [out.json] [--powered]
//!
//! With no arguments it solves the great tower at 30 stones a minute and
//! writes /work/artifacts/brick-castles-20261108/sim/tree.json.
use angel_kingdom::design::{check, solve, Severity, SolveOptions};
use angel_kingdom::catalog;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).filter(|a| a != "--powered").collect();
    let powered = std::env::args().any(|a| a == "--powered");
    let castle = args.first().map_or("great-tower", String::as_str);
    let rate: f64 = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(30.0);
    let out = args
        .get(2)
        .map_or("/work/artifacts/brick-castles-20261108/sim/tree.json", String::as_str);
    let c = catalog();
    let findings = check(c);
    let errors = findings.iter().filter(|f| f.severity == Severity::Error).count();
    for f in &findings {
        println!("{:?} {} {}: {}", f.severity, f.code, f.subject, f.message);
    }
    println!("{} findings, {} errors", findings.len(), errors);
    let mut o = SolveOptions::new(castle, rate);
    o.powered = powered;
    let req = solve(c, &o).expect("the castle solves");
    let json = req.json(c);
    std::fs::write(out, serde_json::to_string_pretty(&json).unwrap()).expect("write tree");
    println!(
        "{castle} at {rate}/min: {} workers ({} masons in {} lodges), {} carts in {} carter's yards, {} power units, shaft {:.1} units/shift ({:?}), {} beds",
        req.workers, req.masons, req.lodges, req.vehicles, req.yards, req.power_units, req.shaft_per_shift, req.shaft_plan, req.beds
    );
    for (plan, n) in &req.buildings {
        println!("  {plan}: {n}");
    }
    println!("wrote {out}");
}
