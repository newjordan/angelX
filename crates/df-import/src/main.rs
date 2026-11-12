//! `df-import <export-dir> <kingdom-dir> <game-assets-dir>`
//!
//! From the repository root:
//! `cargo run -p angel-df-import --release -- expansions/bomrekedan/df-export expansions/bomrekedan/kingdom crates/delve/assets/bomrekedan`

use std::path::PathBuf;
use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() != 3 {
        eprintln!("usage: df-import <export-dir> <kingdom-dir> <game-assets-dir>");
        eprintln!(
            "  e.g. df-import expansions/bomrekedan/df-export expansions/bomrekedan/kingdom crates/delve/assets/bomrekedan"
        );
        return ExitCode::from(2);
    }
    let (input, kingdom, game) = (PathBuf::from(&args[0]), PathBuf::from(&args[1]), PathBuf::from(&args[2]));
    match angel_df_import::run(&input, &kingdom, &game) {
        Ok(s) => {
            for l in s.lines {
                println!("{l}");
            }
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("df-import: {e}");
            ExitCode::FAILURE
        }
    }
}
