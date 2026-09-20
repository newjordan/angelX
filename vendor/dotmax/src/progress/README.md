# `dotmax::progress` — modular terminal progress styles

~500 loading bars, spinners, gauges, borders, screen-wipes and animated
visualizations for braille/block terminals, built to be **lifted out and dropped
into other programs**. Every style is a stateless pure function of
`(progress, time)`; everything a style needs arrives through one immutable
[`BarContext`], and every style implements one trait, [`ProgressStyle`].

## Inject into any program (3 lines)

```rust
use dotmax::progress::{styles_for_theme, BarContext, render_string};

let style = &styles_for_theme("space")[0];
let frame = render_string(style.as_ref(), &BarContext::new(0.42, t_seconds, 40, 3))?;
println!("{frame}");   // a braille/block string you reprint each frame
```

`render_string` / `render_lines` are renderer-agnostic — no TUI required. If you
*are* using dotmax's `TerminalRenderer`, render the `BrailleGrid` the style
draws into directly instead.

## Write your own (no registration needed)

```rust
use dotmax::progress::{BarContext, ProgressStyle, draw};
use dotmax::{BrailleGrid, DotmaxError};

struct MyBar;
impl ProgressStyle for MyBar {
    fn name(&self) -> &str { "my-bar" }
    fn theme(&self) -> &str { "custom" }
    fn render(&self, g: &mut BrailleGrid, c: &BarContext) -> Result<(), DotmaxError> {
        let (w, h) = draw::dot_dims(g);
        draw::fill_rect(g, 0, 0, (c.eased * w as f32) as usize, h);
        Ok(())
    }
}
```

## What's in the box

- `easing` — 31 tween curves (`Easing::*`, the Penner set), dependency-free `f32 -> f32`.
- `BarContext` — per-frame inputs: `progress`, `eased`, `time`, `width`/`height` (cells), `palette`, `label`.
- `draw` — bounds-safe braille (`dot`/`dot_i`/`hline`/`vline`/`fill_rect`/…) **and**
  block-element helpers (`hbar` smooth eighths, `vblock`, `shade`, `glyph`).
- `all_styles()` / `styles_for_theme(name)` / `themes()` — the registry.
- `render_lines` / `render_string` — headless one-shot rendering.

## Themes (52)

`classic` `animals` `tech` `nature` `space` `retro` `ocean` `food` ·
`fractal` `chaos` `waves` `cellular` `geometry` `noise` `numbertheory` `topology` ·
`atari` `nintendo` `gameboy` `physics` `chemistry` `biology` `electronics` ·
`perspective` `sinewave` `blocks` · `border` `wipe` `spinner` `meter` ·
`fruits` `plants` `wildlife` `cultures` `cosmos` `gadgets` `surf` `cars` `lasers` ·
`floweroflife` `platonic` `yantra` `goldenratio` `penrose` ·
`mythology` `weather` `music` `sports` `transit` `medieval` `quantum` `architecture`

## Browse them

```bash
cargo run --example loading_bars        # arrows = style, [ ] = theme, t = easing
cargo run --example loading_bar_sheet   # writes docs/progress_loader_sheet.html
```

## Extracting the module wholesale

Copy `src/progress/` into another project. The only dotmax types it depends on
are `BrailleGrid`, `Color`, and `DotmaxError`; swap those for your own grid by
re-pointing the `use crate::{…}` lines and the `draw` helpers.
