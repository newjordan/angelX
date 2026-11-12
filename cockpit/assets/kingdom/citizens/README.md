# Village citizens

`workers-concept.png` is the reviewed first-stage character sheet, generated
with built-in image generation using the village vision as the art-direction
reference. The builder, farmer, porter and arcanist share cream clothing,
weathered leather, slate, terracotta, ochre and teal accents.

The four transparent `*-sheet.png` masters contain four walking frames in the
upper row and four working frames in the lower row. `manifest.json` records
all source rectangles and measured ground anchors; `prompts.json` retains the
exact generation instructions. Each character uses one scale across all eight
frames. The anchors correct source-layout drift without resizing a raised
tool, bending worker or walking step independently.

The builder spreads mortar onto bricks, the farmer hoes, the porter lowers
and lifts a crate, and the arcanist channels a small crystal. Walking loops
use 140 ms frames and work loops use 180 ms frames. Runtime banks are prepared
by the village art helper at 32 and 48 pixels, with a common bottom-centre
foot pivot. Do not split these sheets by a rounded uniform cell width: the
1774-pixel source width alternates 443- and 444-pixel cells.

The population simulation lives in `crates/kingdom/src/population.rs`. Named
citizens retain their identities and move between real building jobs and
homes. Food, shelter and rest affect their work efficiency; new cottages
provide room for gradual migration rather than instantly creating labour.
The default settlement starts with six people, a meal costs one food per
person every 90 seconds, and a supplied spare bed admits one new citizen
after 120 seconds of healthy conditions. Fully staffed workplaces receive
25–100% work credit from their workers' needs; a workplace that cannot staff
its complete crew leaves those citizens available for smaller jobs.

`crates/kingdom/src/citizen_motion.rs` keeps each citizen's current and previous
plot, destination and a bounded route. Routes prefer roads, use owned land for
construction access, and change when geometry or the assignment changes.
Citizens stop at entrances selected by the settlement adapter. Walking and
working are separate animation states; travel is currently visual and does
not add a second labour penalty on top of hunger, rest and staffing.

Simulation clocks are authoritative at one movement step per second. Render
animation receives Knights Wish's 30 Hz game ticks and interpolates the saved
previous/current plots. The actor ledger is capped at 256 people and each
cached route at 512 adjacent steps; longer routes replan at the end of their
cached prefix.
