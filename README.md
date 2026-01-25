# fiberlamp-rs

A modern Rust reimplementation of the classic xscreensaver `fiberlamp` using wgpu for GPU-accelerated rendering. Simulates a fiber optic lamp with realistic physics and glowing fiber tips.

![Rust](https://img.shields.io/badge/rust-stable-orange)
![License](https://img.shields.io/badge/license-MIT-blue)
![Platform](https://img.shields.io/badge/platform-Linux%20%7C%20macOS%20%7C%20Windows-lightgrey)

## Features

- **GPU-accelerated rendering** via wgpu (Vulkan, Metal, DX12 backends)
- **Native Wayland support** - works with KDE plasma-wallpaper-application
- **Realistic physics simulation** - cantilever beam model with periodic perturbations
- **Smooth 60fps** on integrated GPUs
- **MSAA anti-aliasing** - optional 2x, 4x, or 8x multisampling
- **Configurable parameters** - fiber count, color palette size, MSAA level

## Installation

### From Source

Requires Rust 1.85+ (2024 edition).

```bash
git clone https://github.com/jtcressy/fiberlamp-rs.git
cd fiberlamp-rs
cargo build --release
```

The binary will be at `target/release/fiberlamp-rs`.

### Dependencies

On Linux, you may need to install Vulkan drivers:

```bash
# Fedora/RHEL
sudo dnf install vulkan-loader mesa-vulkan-drivers

# Ubuntu/Debian
sudo apt install libvulkan1 mesa-vulkan-drivers

# Arch
sudo pacman -S vulkan-icd-loader vulkan-mesa-layers
```

## Usage

```bash
# Run fullscreen (default)
./fiberlamp-rs

# Run in a window
./fiberlamp-rs --windowed

# Custom fiber count (10-500)
./fiberlamp-rs --count 300

# Enable 4x MSAA anti-aliasing
./fiberlamp-rs --msaa 4

# All options
./fiberlamp-rs --help
```

### Command Line Options

| Option | Short | Default | Description |
|--------|-------|---------|-------------|
| `--count` | `-c` | 500 | Number of fibers (10-500) |
| `--delay` | `-d` | 16 | Frame delay in milliseconds |
| `--ncolors` | `-n` | 64 | Number of colors in the palette |
| `--windowed` | `-w` | false | Run in windowed mode |
| `--msaa` | `-m` | 1 | MSAA sample count (1=off, 2, 4, or 8) |

### Controls

- **Escape** or **Q** - Exit

### Wayland Usage

For native Wayland rendering:

```bash
WAYLAND_DISPLAY=wayland-0 ./fiberlamp-rs
```

### KDE Plasma Wallpaper

To use as an animated wallpaper with KDE's plasma-wallpaper-application:

1. Build the release binary
2. Configure plasma-wallpaper-application to launch the binary
3. The application will fill whatever surface it's given

## How It Works

### Physics Simulation

Each fiber is modeled as a chain of 20 connected nodes using cantilever beam physics:

- **Node 0** is pinned at the lamp's center
- **Nodes 1-19** move freely with angular spring forces
- **Euler integration** updates angular velocities and positions
- **Periodic bumps** (~every 10,000 frames) perturb the base, causing natural swaying
- **Geometric rotation** slowly rotates the entire lamp (~10°/minute)

### Rendering

- Fibers are triangulated into screen-space quads (wgpu doesn't support line width)
- **Body segments** use 3-tier depth coloring (dim/medium/bright based on z-depth)
- **Tip segments** use a rotating HSV color palette
- **Additive blending** creates the characteristic glow effect
- Fixed 60Hz physics with variable render rate

## Building for Development

```bash
# Debug build
cargo build

# Run with logging
RUST_LOG=info cargo run

# Run tests
cargo test

# Run specific test
cargo test test_fiber_creation
```

## Credits

### Original xscreensaver fiberlamp

This project is a reimplementation of the original `fiberlamp` screensaver:

- **Author:** Tim Auckland (tda10.geo@yahoo.com)
- **Copyright:** 2005
- **Source:** [xscreensaver/hacks/fiberlamp.c](https://github.com/Zygo/xscreensaver/blob/master/hacks/fiberlamp.c)

The original was written in C using Xlib. This Rust version faithfully recreates the physics simulation while using modern GPU rendering via wgpu.

### xscreensaver

[xscreensaver](https://www.jwz.org/xscreensaver/) is a collection of screen savers by Jamie Zawinski and many contributors. It has been the standard screen saver collection on Unix systems since 1992.

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

The original xscreensaver fiberlamp by Tim Auckland was released under a permissive license allowing use, modification, and distribution.
