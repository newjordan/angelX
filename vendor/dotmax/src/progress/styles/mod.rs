//! Registry of every bundled progress-bar style, grouped by theme.
//!
//! Each theme lives in its own file and exposes `pub fn styles() ->
//! Vec<Box<dyn ProgressStyle>>`. Adding a theme is two lines here plus one
//! file — nothing else in the crate needs to change.

use super::ProgressStyle;

pub use aurora::RibbonFlow;
pub use glitch::SyncLock;

// Wave 1 — concrete / playful themes.
mod animals;
mod classic;
mod food;
mod nature;
mod ocean;
mod retro;
mod space;
mod tech;

// Live catalog v2 — motion-led themes added by the 0.1.8 experience.
mod aurora;
mod fireworks;
mod glitch;
mod inferno;
mod matrix;

// Wave 2 — esoteric mathematics & visual algorithms.
mod cellular;
mod chaos;
mod fractal;
mod geometry;
mod noise;
mod numbertheory;
mod topology;
mod waves;

// Wave 3 — retro games, science, and rendering-technique aesthetics.
mod atari;
mod biology;
mod blocks;
mod chemistry;
mod electronics;
mod gameboy;
mod nintendo;
mod perspective;
mod physics;
mod sinewave;

// Wave 4 — complementary families: beyond the bar.
mod border;
mod meter;
mod spinner;
mod wipe;

// Wave 5 — wide-spread variety.
mod cars;
mod cosmos;
mod cultures;
mod fruits;
mod gadgets;
mod lasers;
mod plants;
mod surf;
mod wildlife;

// Wave 6 — sacred geometry families.
mod floweroflife;
mod goldenratio;
mod penrose;
mod platonic;
mod yantra;

// Wave 7 — fresh mechanics.
mod architecture;
mod medieval;
mod music;
mod mythology;
mod quantum;
mod sports;
mod transit;
mod weather;

/// All themes bundled with dotmax, in display order.
pub const THEMES: [&str; 57] = [
    "classic",
    "matrix",
    "aurora",
    "inferno",
    "glitch",
    "fireworks",
    "animals",
    "tech",
    "nature",
    "space",
    "retro",
    "ocean",
    "food",
    "fractal",
    "chaos",
    "waves",
    "cellular",
    "geometry",
    "noise",
    "numbertheory",
    "topology",
    "atari",
    "nintendo",
    "gameboy",
    "physics",
    "chemistry",
    "biology",
    "electronics",
    "perspective",
    "sinewave",
    "blocks",
    "border",
    "wipe",
    "spinner",
    "meter",
    "fruits",
    "plants",
    "wildlife",
    "cultures",
    "cosmos",
    "gadgets",
    "surf",
    "cars",
    "lasers",
    "floweroflife",
    "platonic",
    "yantra",
    "goldenratio",
    "penrose",
    "mythology",
    "weather",
    "music",
    "sports",
    "transit",
    "medieval",
    "quantum",
    "architecture",
];

/// The list of bundled theme names.
#[must_use]
pub fn themes() -> &'static [&'static str] {
    &THEMES
}

/// Collect every bundled style across all themes.
#[must_use]
pub fn all_styles() -> Vec<Box<dyn ProgressStyle>> {
    let mut v = Vec::new();
    v.extend(classic::styles());
    v.extend(matrix::styles());
    v.extend(aurora::styles());
    v.extend(inferno::styles());
    v.extend(glitch::styles());
    v.extend(fireworks::styles());
    v.extend(animals::styles());
    v.extend(tech::styles());
    v.extend(nature::styles());
    v.extend(space::styles());
    v.extend(retro::styles());
    v.extend(ocean::styles());
    v.extend(food::styles());
    v.extend(fractal::styles());
    v.extend(chaos::styles());
    v.extend(waves::styles());
    v.extend(cellular::styles());
    v.extend(geometry::styles());
    v.extend(noise::styles());
    v.extend(numbertheory::styles());
    v.extend(topology::styles());
    v.extend(atari::styles());
    v.extend(nintendo::styles());
    v.extend(gameboy::styles());
    v.extend(physics::styles());
    v.extend(chemistry::styles());
    v.extend(biology::styles());
    v.extend(electronics::styles());
    v.extend(perspective::styles());
    v.extend(sinewave::styles());
    v.extend(blocks::styles());
    v.extend(border::styles());
    v.extend(wipe::styles());
    v.extend(spinner::styles());
    v.extend(meter::styles());
    v.extend(fruits::styles());
    v.extend(plants::styles());
    v.extend(wildlife::styles());
    v.extend(cultures::styles());
    v.extend(cosmos::styles());
    v.extend(gadgets::styles());
    v.extend(surf::styles());
    v.extend(cars::styles());
    v.extend(lasers::styles());
    v.extend(floweroflife::styles());
    v.extend(platonic::styles());
    v.extend(yantra::styles());
    v.extend(goldenratio::styles());
    v.extend(penrose::styles());
    v.extend(mythology::styles());
    v.extend(weather::styles());
    v.extend(music::styles());
    v.extend(sports::styles());
    v.extend(transit::styles());
    v.extend(medieval::styles());
    v.extend(quantum::styles());
    v.extend(architecture::styles());
    v
}

/// Collect only the styles belonging to `theme` (case-insensitive).
#[must_use]
pub fn styles_for_theme(theme: &str) -> Vec<Box<dyn ProgressStyle>> {
    let theme = theme.to_lowercase();
    all_styles()
        .into_iter()
        .filter(|s| s.theme() == theme)
        .collect()
}
