# dotmax

Render anything in terminal braille. Images, GIFs, videos, webcam - one line of code.

[![Crates.io](https://img.shields.io/crates/v/dotmax.svg)](https://crates.io/crates/dotmax)
[![Documentation](https://docs.rs/dotmax/badge.svg)](https://docs.rs/dotmax)
[![License](https://img.shields.io/crates/l/dotmax.svg)](https://github.com/newjordan/dotmax#license)

## Gallery

### Color Rendering

| | |
|---|---|
| ![Color Tiger](docs/examples/color_tiger.png) | ![Viper Color](docs/examples/viper_ascII_art.png) |
| Full-color image rendering with braille dots | Color rendering with tuner overlay |
| ![Snake Closeup](docs/examples/snake_color_closeup.png) | ![Medium Tiger](docs/examples/med_tiger.png) |
| High-detail color closeup | Medium-resolution color render |

### Monochrome & Pixel Sizes

| | |
|---|---|
| ![8px ASCII Tiger](docs/examples/8px_ascii_tiger.png) | ![Small Pixel Tiger](docs/examples/small_pixel_tiger.png) |
| 8px monochrome braille | Small pixel density |
| ![Small Snake](docs/examples/small_px_snake.png) | ![Large Pixel Shading](docs/examples/large_pixel_shadingenabled.png) |
| Small pixel snake render | Large pixel with shading enabled |
| ![Snake Large Pixel](docs/examples/snake_large_pixel.png) | ![GIF Playback](docs/examples/gottem.png) |
| Large pixel monochrome | Animated GIF playback in terminal |

### 3D, Audio & More

| | |
|---|---|
| ![Sphere Wireframe](docs/examples/sphere.png) | ![Sphere IO](docs/examples/sphere_io.png) |
| 3D sphere wireframe (OBJ rendering) | Sphere with I/O overlay |
| ![Snake Head OBJ](docs/examples/snake_head_obj.png) | ![Audio Spectrograph](docs/examples/audio_spetrophasia.png) |
| 3D snake head model | Audio spectrograph visualization |
| ![Grid Formation](docs/examples/grid_formation.png) | |
| Grid/oscilloscope formation | |

## Install

```bash
cargo add dotmax --features image
```

## One-Line Usage

```rust
use dotmax::quick;

quick::show_file("photo.png")?;    // Any image
quick::show_file("cat.gif")?;      // Animated GIF (plays automatically)
quick::show_file("movie.mp4")?;    // Video (requires 'video' feature)
quick::show_webcam()?;             // Live webcam (requires 'video' feature)
```

## Features

| Feature | What it enables | Install |
|---------|-----------------|---------|
| `image` | PNG, JPG, GIF, APNG, BMP, WebP, TIFF | `cargo add dotmax --features image` |
| `svg` | SVG vector graphics | `cargo add dotmax --features svg` |
| `video` | Video + webcam (needs FFmpeg) | `cargo add dotmax --features video` |

```toml
# Cargo.toml - pick what you need
[dependencies]
dotmax = { version = "0.1", features = ["image"] }           # Images only
dotmax = { version = "0.1", features = ["image", "svg"] }    # Images + SVG
dotmax = { version = "0.1", features = ["video"] }           # Video + webcam
```

**Video feature requires FFmpeg installed on your system.**

## Quick API Reference

```rust
use dotmax::quick;

// Display (blocks until keypress or video ends)
quick::show_file("any.png")?;           // Auto-detect format
quick::show_image("photo.jpg")?;        // Static image only
quick::show_webcam()?;                  // Default webcam
quick::show_webcam_device(0)?;          // Webcam by index
quick::show_webcam_device("/dev/video1")?;  // Webcam by path

// Load without displaying
let grid = quick::load_image("photo.png")?;  // Returns BrailleGrid
quick::show(&grid)?;                         // Display manually

// Create empty grid
let mut grid = quick::grid()?;  // Terminal-sized
```

## Drawing Primitives

```rust
use dotmax::prelude::*;

let mut grid = grid()?;
draw_line(&mut grid, 0, 0, 100, 50)?;
draw_circle(&mut grid, 50, 25, 20)?;
draw_rectangle(&mut grid, 10, 10, 80, 40)?;
show(&grid)?;
```

## Animation Loop

```rust
use dotmax::animation::AnimationLoop;

AnimationLoop::new(80, 24)
    .fps(30)
    .on_frame(|frame, grid| {
        grid.clear();
        grid.set_dot((frame * 2) % 160, 48)?;
        Ok(true)  // Return false to stop
    })
    .run()?;
```

## Examples

```bash
# Basic (no features needed)
cargo run --example hello_braille
cargo run --example bouncing_ball
cargo run --example shapes_demo

# Images
cargo run --example load_image --features image
cargo run --example dither_comparison --features image

# Animated GIF/APNG
cargo run --example animated_gif --features image -- your.gif
cargo run --example animated_apng --features image -- your.apng

# Video (needs FFmpeg)
cargo run --example video_player --features video -- your.mp4

# Webcam (needs FFmpeg + camera)
cargo run --example webcam_viewer --features video
cargo run --example webcam_tuner --features video   # Interactive settings
```

## Tuners

Tuners let you **find the best render settings visually**. Adjust dithering, brightness, contrast, etc. in real-time and see the results instantly.

### Why use a tuner?

Different images/videos look best with different settings. Instead of guessing values in code, use the tuner to experiment live, then copy the settings you like.

### Webcam Tuner

```bash
cargo run --example webcam_tuner --features video
```

### Video/Image Tuner

```bash
cargo run --example render_tuner --features video -- your_video.mp4
cargo run --example render_tuner --features image -- your_image.png
```

### Tuner Controls

| Key | Action |
|-----|--------|
| D | Cycle dithering (Floyd/Bayer/Atkinson/None) |
| T | Toggle threshold (Auto/Manual) |
| +/- | Adjust threshold ±10 |
| [/] | Adjust threshold ±1 |
| B/b | Brightness +/- |
| C/c | Contrast +/- |
| G/g | Gamma +/- |
| M | Toggle color mode (Mono/Color) |
| R | Reset all settings |
| H | Help |
| Q | Quit |

### What the settings do

- **Dithering**: How dots are distributed. Floyd-Steinberg = smooth gradients, Bayer = patterned, Atkinson = high contrast, None = pure threshold
- **Threshold**: Brightness cutoff for black vs white. Auto (Otsu) calculates optimal value. Manual lets you pick 0-255
- **Brightness/Contrast/Gamma**: Standard image adjustments. Useful for dark or washed-out sources

## Performance

| Operation | Time |
|-----------|------|
| Frame render (80×24) | ~2μs |
| Image load + render | ~10ms |
| 60fps animation budget | 16.6ms (we use 1.6μs) |

## License

MIT OR Apache-2.0
