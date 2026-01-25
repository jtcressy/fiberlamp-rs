# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Rust/wgpu reimplementation of the xscreensaver `fiberlamp` - a fiber optic lamp screensaver. Designed to work as a native Wayland client for KDE's plasma-wallpaper-application plugin.

## Build and Run Commands

```bash
# Build (debug)
cargo build

# Build (release - recommended for smooth 60fps)
cargo run --release

# Run with Wayland (native)
WAYLAND_DISPLAY=wayland-0 cargo run --release

# Run tests
cargo test

# Run single test
cargo test test_fiber_creation

# CLI options
cargo run --release -- --count 500 --ncolors 64 --windowed --msaa 4
```

## Architecture

The codebase follows a clear separation:

```
src/
├── main.rs      - Event loop, window management (winit), fixed-timestep physics driver
├── physics.rs   - Fiber simulation: cantilever beam physics, Euler integration
├── renderer.rs  - wgpu pipeline setup, MSAA support, additive blending
├── vertex.rs    - Vertex triangulation, color palette generation
└── shaders/
    └── fiber.wgsl - Simple passthrough shader (vertices pre-transformed to clip space)
```

### Data Flow

1. **Physics** (`FiberLamp::step`): Updates angular velocities/positions of fiber nodes using cantilever beam simulation with periodic "bumps"
2. **Vertex Generation** (`App::generate_fiber_vertices`): Converts 3D node positions to 2D clip-space quads (triangulated segments with varying width)
3. **Rendering**: Uploads vertices to GPU, renders with additive blending for glow effect

### Key Constants

- `NODES = 20`: Each fiber is a chain of 20 connected nodes
- `PHYSICS_DT = 1/60`: Fixed 60Hz physics timestep
- `MAX_VERTICES = 500 * 19 * 6`: Pre-allocated vertex buffer capacity

### Physics Model

Each fiber is a cantilever beam with:
- Node 0 pinned at lamp center
- Angular spring forces toward parent angles
- Load accumulation from downstream nodes
- Periodic "bump" every ~10000 cycles applies impulse to base
- Geometric rotation at ~10°/minute for visual variety

### Rendering Approach

- Fibers rendered as triangulated quads (wgpu doesn't support line width)
- Body segments: 3-tier depth coloring (dim/medium/bright based on z)
- Tip segments: Rotating HSV color palette
- Additive blending creates glow accumulation effect
- Optional MSAA (1/2/4/8 samples) via `--msaa` flag
