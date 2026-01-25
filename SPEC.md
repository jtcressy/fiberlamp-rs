# Fiberlamp-rs: Modern Fiber Optic Lamp Screensaver

## Goal
Reimplement the classic xscreensaver `fiberlamp` in Rust with modern GPU rendering. Must work as a native Wayland client for use with KDE's plasma-wallpaper-application plugin.

## Original Behavior (from xscreensaver fiberlamp.c)
- Simulates a fiber optic lamp with ~500 flexible fibers
- Each fiber is a chain of discrete nodes (cantilever beam physics)
- Base oscillates periodically (lamp gets "bumped" every ~10000 cycles)
- Fiber tips glow with cycling colors
- Parameters: count (10-500), cycles (100-10000), delay, ncolors (64)

## Physics Model
```
Fiber = chain of N nodes connected by rigid segments
- Node 0 (base) is pinned to lamp center
- Nodes 1..N are free, affected by:
  - Gravity (upward, since fibers rise)
  - Damping (velocity decay)
  - Length constraints (segments maintain fixed length)
- Use Verlet integration + iterative constraint solving
- Periodic "bump" applies impulse to base position
```

## Tech Stack
- **wgpu**: GPU rendering (Vulkan/Metal backend, works on Wayland)
- **winit**: Windowing with native Wayland support
- **glam**: Vector math
- **rand**: RNG for initialization and bumps

## Architecture
```
src/
  main.rs         - Window setup, event loop
  physics.rs      - Fiber/FiberLamp simulation
  renderer.rs     - wgpu pipeline, vertex generation
  shaders/
    fiber.wgsl    - Vertex/fragment shaders with glow
```

## Rendering Approach
1. Each fiber → line strip of vertices
2. Vertex attributes: position, color, distance-from-tip
3. Fragment shader: additive blending, glow falloff toward base
4. Black background, no depth testing needed

## Key Requirements
1. **Native Wayland client** - no X11 dependencies, no XWayland
2. **Fullscreen by default** - fill whatever surface it's given
3. **Smooth 60fps** on integrated GPUs
4. **CLI args** matching original: `--count`, `--delay`, `--ncolors`

## Stretch Goals
- WGSL compute shader for physics (GPU-side simulation)
- Config file support
- Multiple color palette options

## Reference
- Original source: https://github.com/Zygo/xscreensaver/blob/master/hacks/fiberlamp.c
- Man page: `fiberlamp(6x)`

## Testing
Run with: `WAYLAND_DISPLAY=wayland-0 cargo run --release`
For wallpaper plugin: configure plasma-wallpaper-application to launch the binary
