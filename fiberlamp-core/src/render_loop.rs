//! Common render loop logic shared between all platform frontends.
//!
//! `FiberlampSession` encapsulates the renderer, physics simulation, palette,
//! and vertex generation so that platform-specific code only needs to drive
//! `animate()` and `resize()`.

use glam::Vec2;
use rand::rngs::ThreadRng;

use crate::physics::{ExternalForces, FiberLamp, NODES};
use crate::renderer::Renderer;
use crate::vertex::{body_color, generate_palette, tip_color, triangulate_segment, FiberVertex};

/// Configuration for creating a FiberlampSession.
#[derive(Debug, Clone)]
pub struct FiberlampConfig {
    pub fiber_count: usize,
    pub ncolors: usize,
}

/// Encapsulates the full fiberlamp render session: simulation + rendering.
pub struct FiberlampSession {
    renderer: Renderer,
    lamp: FiberLamp,
    palette: Vec<[f32; 3]>,
    rng: ThreadRng,
}

impl FiberlampSession {
    /// Create a new session from a pre-built Renderer and config.
    pub fn new(renderer: Renderer, config: FiberlampConfig) -> Self {
        let mut rng = rand::thread_rng();
        let lamp = FiberLamp::new(config.fiber_count, &mut rng);
        let palette = generate_palette(config.ncolors);
        Self {
            renderer,
            lamp,
            palette,
            rng,
        }
    }

    /// Step physics and render one frame.
    ///
    /// Returns the result of the wgpu surface present operation.
    pub fn animate(&mut self, forces: &ExternalForces<'_>) -> Result<(), wgpu::SurfaceError> {
        self.lamp.step(&mut self.rng, forces);

        let vertices = self.generate_fiber_vertices();
        self.renderer.update_vertices(&vertices);
        self.renderer.render()
    }

    /// Resize the renderer surface.
    pub fn resize(&mut self, width: u32, height: u32) {
        self.renderer.resize(width, height);
    }

    /// Access the underlying renderer (e.g. for size queries).
    pub fn renderer(&self) -> &Renderer {
        &self.renderer
    }

    /// Access the lamp simulation state.
    pub fn lamp(&self) -> &FiberLamp {
        &self.lamp
    }

    /// Access the lamp simulation state mutably.
    pub fn lamp_mut(&mut self) -> &mut FiberLamp {
        &mut self.lamp
    }

    /// Access the RNG.
    pub fn rng_mut(&mut self) -> &mut ThreadRng {
        &mut self.rng
    }

    /// Access the palette.
    pub fn palette(&self) -> &[[f32; 3]] {
        &self.palette
    }

    /// Generate vertices from all fibers for rendering.
    /// This is the shared vertex generation previously duplicated in main.rs and ffi.rs.
    pub fn generate_fiber_vertices(&self) -> Vec<FiberVertex> {
        generate_fiber_vertices_from(&self.lamp, &self.renderer, &self.palette)
    }
}

/// Standalone vertex generation function for use when you have separate
/// references to lamp, renderer, and palette (e.g., from the bin crate's App).
pub fn generate_fiber_vertices_from(
    lamp: &FiberLamp,
    renderer: &Renderer,
    palette: &[[f32; 3]],
) -> Vec<FiberVertex> {
    let (width, height) = renderer.size();
    let aspect = width as f32 / height as f32;

    // Scale factor to fit fibers on screen
    let scale = 1.5;

    // Precompute rotation for the entire lamp
    let cos_t = lamp.theta.cos();
    let sin_t = lamp.theta.sin();

    let mut vertices = Vec::with_capacity(lamp.fibers.len() * (NODES - 1) * 6);

    for fiber in &lamp.fibers {
        // Skip first segment so fibers start spread apart (not from single point)
        for i in 2..NODES {
            let parent = &fiber.nodes[i - 1];
            let node = &fiber.nodes[i];

            // Apply geometric rotation around vertical axis (rotate x/z plane)
            let parent_rx = parent.x * cos_t - parent.z * sin_t;
            let node_rx = node.x * cos_t - node.z * sin_t;

            // Convert 3D position to 2D screen space
            let p0 = Vec2::new(parent_rx * scale / aspect, -parent.y * scale - 0.7);
            let p1 = Vec2::new(node_rx * scale / aspect, -node.y * scale - 0.7);

            // Line width: thicker at base, thinner at tips
            let base_width = 6.0 / width as f32;
            let tip_width = 2.0 / width as f32;
            let t0 = (i - 1) as f32 / (NODES - 1) as f32;
            let t1 = i as f32 / (NODES - 1) as f32;
            let width0 = base_width * (1.0 - t0) + tip_width * t0;
            let width1 = base_width * (1.0 - t1) + tip_width * t1;

            // Color based on segment type
            let color = if i < NODES - 2 {
                body_color(node.z, t1)
            } else {
                let tip = fiber.tip();
                tip_color(tip.x, tip.y, lamp.psi(), palette)
            };

            let segment = triangulate_segment(p0, p1, width0, width1, color);
            vertices.extend_from_slice(&segment);
        }
    }

    vertices
}
