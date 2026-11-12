# Frontier terrain

The retained `ground-atlas.png` contains meadow grass, forest floor, gray stone, and teal water. `props-atlas.png` contains a fir grove, rock outcrop, crystal vein, and oak grove. `workers-atlas.png` contains two walk poses each for a loaded handcart hauler and a crate porter. Full built-in imagegen prompts and source provenance are in `manifest.json`.

The build helper compiles ground materials into 128-pixel textures and environmental props into 48- and 64-pixel transparent sprites. World-anchored mirrored sampling prevents texture seams and keeps scrolling stable; water drifts slowly. Tile patches can use different materials without repeating one grass clump in every tile. The rendering code draws connected, contoured cobblestone lanes above the generated materials.

Prop coordinates are bottom-centre world anchors. Place groves behind production sites and sort props with structures by ground-contact Y. The source props remain cosmetic; movement and harvesting rules belong to the kingdom simulation.

`paint_carrier` renders the hauler or porter at 28–32 pixels, alternates their walking poses, mirrors leftward travel, adds a subtle step bob, and colours the cargo strap to match the resource in transit. The source workers retain their painted wood, cloth, faces and loads. Use the shipment's interpolated world position as the bottom-centre anchor. The simulation determines actual route, delivery and contents.

`ore-resources-atlas.png` adds raw iron ore, coal, moon crystal and living ember. Render them through `paint_prop` using IDs `ore-iron`, `ore-coal`, `ore-crystal`, and `ore-ember`. Their materials match the four mineral enemies in `../monsters/ore-enemies-atlas.png`, making expedition forecasts readable through pictures as well as text. Ordinary coal stays dark and unlit; living ember has a molten heart. The manifest records the reviewed crop gutters rather than assuming that the generated sheet divides precisely in half.

The atlas PNGs and manifest describe full-resolution source artwork. Runtime textures and sprites are build outputs; they are not additional generated masters. Integrated game captures, isolated art fixtures, and combat captures are distinguished in [the screenshot index](https://github.com/newjordan/angelX/blob/main/docs/images/kingdom-works/README.md).
