use std::sync::Arc;
use std::time::Instant;

use clap::Parser;
use glam::Vec2;
use rand::rngs::ThreadRng;
use winit::{
    application::ApplicationHandler,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    window::{Fullscreen, Window, WindowId},
};

mod physics;
mod renderer;
mod vertex;

use physics::{FiberLamp, NODES};
use renderer::Renderer;
use vertex::{body_color, generate_palette, tip_color, triangulate_segment, FiberVertex};

/// Fixed physics timestep (60Hz)
const PHYSICS_DT: f32 = 1.0 / 60.0;
/// Maximum physics steps per frame to prevent spiral of death
const MAX_PHYSICS_STEPS: u32 = 4;

/// Modern fiber optic lamp screensaver - Rust/wgpu reimplementation of xscreensaver fiberlamp
#[derive(Parser, Debug, Clone)]
#[command(author, version, about)]
pub struct Args {
    /// Number of fibers (10-500)
    #[arg(short, long, default_value_t = 500)]
    pub count: u32,

    /// Frame delay in milliseconds
    #[arg(short, long, default_value_t = 16)]
    pub delay: u32,

    /// Number of colors in the palette
    #[arg(short, long, default_value_t = 64)]
    pub ncolors: u32,

    /// Run in windowed mode instead of fullscreen
    #[arg(short, long)]
    pub windowed: bool,

    /// MSAA sample count for anti-aliasing (1=off, 2, 4, or 8)
    #[arg(short, long, default_value_t = 1, value_parser = clap::value_parser!(u32).range(1..=8))]
    pub msaa: u32,
}

struct App {
    args: Args,
    window: Option<Arc<Window>>,
    renderer: Option<Renderer>,
    fiber_lamp: Option<FiberLamp>,
    rng: ThreadRng,
    palette: Vec<[f32; 3]>,
    frame_count: u64,
    last_frame: Option<Instant>,
    physics_accumulator: f32,
}

impl App {
    fn new(args: Args) -> Self {
        let palette = generate_palette(args.ncolors as usize);

        Self {
            args,
            window: None,
            renderer: None,
            fiber_lamp: None,
            rng: rand::thread_rng(),
            palette,
            frame_count: 0,
            last_frame: None,
            physics_accumulator: 0.0,
        }
    }

    /// Generate vertices from all fibers for rendering
    fn generate_fiber_vertices(&self) -> Vec<FiberVertex> {
        let Some(lamp) = &self.fiber_lamp else {
            return Vec::new();
        };
        let Some(renderer) = &self.renderer else {
            return Vec::new();
        };

        let (width, height) = renderer.size();
        let aspect = width as f32 / height as f32;

        // Scale factor to fit fibers on screen
        let scale = 1.5; // Scale up to fill screen nicely

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
                // Base at bottom, fibers spray upward (like original xscreensaver)
                // Physics: y=0 at base, y<0 toward tips (upward)
                // Screen: y=-1 bottom, y=+1 top
                let p0 = Vec2::new(
                    parent_rx * scale / aspect,
                    -parent.y * scale - 0.7,
                );
                let p1 = Vec2::new(
                    node_rx * scale / aspect,
                    -node.y * scale - 0.7,
                );

                // Line width: thicker at base, thinner at tips
                let base_width = 6.0 / width as f32; // 6 pixels at base
                let tip_width = 2.0 / width as f32;  // 2 pixels at tip
                let t0 = (i - 1) as f32 / (NODES - 1) as f32;
                let t1 = i as f32 / (NODES - 1) as f32;
                let width0 = base_width * (1.0 - t0) + tip_width * t0;
                let width1 = base_width * (1.0 - t1) + tip_width * t1;

                // Color based on segment type
                let color = if i < NODES - 2 {
                    // Body: color based on z-depth and segment position
                    body_color(node.z, t1)
                } else {
                    // Tip segments: color from rotating palette
                    let tip = fiber.tip();
                    tip_color(tip.x, tip.y, lamp.psi(), &self.palette)
                };

                let segment = triangulate_segment(p0, p1, width0, width1, color);
                vertices.extend_from_slice(&segment);
            }
        }

        vertices
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }

        let mut attrs = Window::default_attributes().with_title("Fiberlamp");

        if !self.args.windowed {
            attrs = attrs.with_fullscreen(Some(Fullscreen::Borderless(None)));
        }

        let window = Arc::new(
            event_loop
                .create_window(attrs)
                .expect("Failed to create window"),
        );

        // Initialize wgpu renderer
        let renderer = pollster::block_on(Renderer::new(window.clone(), self.args.msaa));

        // Initialize fiber lamp simulation
        let fiber_lamp = FiberLamp::new(self.args.count as usize, &mut self.rng);

        self.window = Some(window);
        self.renderer = Some(renderer);
        self.fiber_lamp = Some(fiber_lamp);

        log::info!(
            "Window created, renderer initialized with {} fibers",
            self.args.count
        );
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        match event {
            WindowEvent::CloseRequested => {
                log::info!("Close requested, exiting");
                event_loop.exit();
            }

            WindowEvent::KeyboardInput { event, .. } => {
                // Exit on Escape or Q
                if event.state.is_pressed() {
                    if let winit::keyboard::PhysicalKey::Code(key) = event.physical_key {
                        if matches!(
                            key,
                            winit::keyboard::KeyCode::Escape | winit::keyboard::KeyCode::KeyQ
                        ) {
                            event_loop.exit();
                        }
                    }
                }
            }

            WindowEvent::Resized(size) => {
                if let Some(renderer) = &mut self.renderer {
                    renderer.resize(size.width, size.height);
                }
            }

            WindowEvent::RedrawRequested => {
                self.frame_count += 1;

                // Calculate delta time for fixed-timestep physics
                let now = Instant::now();
                if let Some(last) = self.last_frame {
                    let delta = (now - last).as_secs_f32();
                    self.physics_accumulator += delta;

                    // Step physics at fixed 60Hz, capped to prevent spiral of death
                    if let Some(lamp) = &mut self.fiber_lamp {
                        let mut steps = 0;
                        while self.physics_accumulator >= PHYSICS_DT && steps < MAX_PHYSICS_STEPS {
                            lamp.step(&mut self.rng);
                            self.physics_accumulator -= PHYSICS_DT;
                            steps += 1;
                        }
                    }
                }
                self.last_frame = Some(now);

                // Generate vertices from fiber positions
                let vertices = self.generate_fiber_vertices();

                // Render
                if let Some(renderer) = &mut self.renderer {
                    renderer.update_vertices(&vertices);

                    match renderer.render() {
                        Ok(_) => {}
                        Err(wgpu::SurfaceError::Lost) => {
                            if let Some(window) = &self.window {
                                let size = window.inner_size();
                                renderer.resize(size.width, size.height);
                            }
                        }
                        Err(wgpu::SurfaceError::OutOfMemory) => {
                            log::error!("Out of memory, exiting");
                            event_loop.exit();
                        }
                        Err(e) => {
                            log::warn!("Render error: {:?}", e);
                        }
                    }
                }

                // Request next frame
                if let Some(window) = &self.window {
                    window.request_redraw();
                }
            }

            _ => {}
        }
    }
}

fn main() {
    env_logger::init();

    let args = Args::parse();
    log::info!("Starting fiberlamp with {:?}", args);

    let event_loop = EventLoop::new().expect("Failed to create event loop");
    event_loop.set_control_flow(ControlFlow::Poll);

    let mut app = App::new(args);
    event_loop.run_app(&mut app).expect("Event loop error");
}
