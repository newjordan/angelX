# Village renderer review

These captures use the checked-in `village_art_build.rs`, `village_art.rs`, `frontier_art.rs` and `Img` code in a small independent Rust fixture. They do not instantiate the economic simulation. They were reviewed at native size, with source transparency, dimensions and checksums audited separately.

| File | Evidence |
| --- | --- |
| contact.png | 96px buildings, shared-scale cottage stages, bridges and all eight 48px poses for each citizen role |
| construction.png | Four cottage, wall and gate stages at one fixed reference scale per structure |
| machines.png | Four distinct active phases for kiln fire, wheel rotor, shaft crank and press ram, over unchanged housings |
| wall-loops.png | Closed wall rectangles at 32px, 48px and 64px tile scale; true three-arm junctions and a composed four-arm crossing |
| workshop-loop.gif | 30 actual renderer frames at 15fps, two seconds, with four workshops and walking/working citizens |
| network-edges.png | Actual network/terrain drawing primitives in an isolated fixture: separate mechanical and arcane routes, active and stopped states, and irregular material borders |

The loop uses the game's 30Hz tick clock, sampling every second tick. Animation timing comes from the manifests. The preview restarts after two seconds; independent machine cycles do not all finish simultaneously. It is a review loop, not a claim that every clip shares one seamless global period.

Validation compared all four active outputs for each machine and all four walk outputs for each citizen; each was distinct. Paused machine outputs were identical at different ticks. Full-size source masters remain one directory above, with prompts and crop/anchor metadata. Actual village gameplay captures are documented in the main kingdom-works image index.
