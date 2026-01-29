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

mod input;

use fiberlamp_core::{
    ExternalForces, FiberLamp, FiberVertex, Renderer, RendererResources, NODES,
    body_color, generate_palette, setup_wgpu_device_and_config, tip_color, triangulate_segment,
};
use input::InputState;

// ============================================================================
// INTERACTIVITY TUNING CONSTANTS
// ============================================================================

/// Mouse collision sphere radius in clip space [-1, 1]
const COLLISION_RADIUS: f32 = 0.25;

/// Collision impulse strength multiplier
const COLLISION_STRENGTH: f32 = 200.0;

/// Number of segments for the debug collision circle
const DEBUG_CIRCLE_SEGMENTS: usize = 32;

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

    /// Enable debug visualizations (collision sphere, etc.)
    #[arg(long)]
    pub debug: bool,
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
    input_state: InputState,
    /// Pre-allocated buffer for collision impulses (one per fiber)
    collision_impulses: Vec<f32>,
}

impl App {
    fn new(args: Args) -> Self {
        let palette = generate_palette(args.ncolors as usize);
        let fiber_count = args.count as usize;

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
            input_state: InputState::new(),
            collision_impulses: vec![0.0; fiber_count],
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

        // Scale factor to fit fibers on screen nicely
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

    /// Calculate collision impulses for all fibers based on mouse position and velocity
    fn calculate_collision_impulses(&mut self, mouse_pos: (f32, f32), mouse_velocity: (f32, f32)) {
        let Some(lamp) = &self.fiber_lamp else {
            return;
        };
        let Some(renderer) = &self.renderer else {
            return;
        };

        let (width, height) = renderer.size();
        let aspect = width as f32 / height as f32;
        let scale = 1.5;

        let cos_t = lamp.theta.cos();
        let sin_t = lamp.theta.sin();

        // Mouse speed determines impulse strength
        let mouse_speed = (mouse_velocity.0 * mouse_velocity.0 + mouse_velocity.1 * mouse_velocity.1).sqrt();

        for (i, fiber) in lamp.fibers.iter().enumerate() {
            let tip = fiber.tip();

            // Apply geometric rotation (same as in generate_fiber_vertices)
            let tip_rx = tip.x * cos_t - tip.z * sin_t;
            // Also compute rotated z for depth check
            let tip_rz = tip.x * sin_t + tip.z * cos_t;

            // Convert to clip space
            let tip_screen = (
                tip_rx * scale / aspect,
                -tip.y * scale - 0.7,
            );

            // Check if tip is within collision radius
            let dx = tip_screen.0 - mouse_pos.0;
            let dy = tip_screen.1 - mouse_pos.1;
            let dist_sq = dx * dx + dy * dy;
            let radius_sq = COLLISION_RADIUS * COLLISION_RADIUS;

            let impulse = if dist_sq < radius_sq && mouse_speed > 0.001 && tip_rz > -0.1 {
                let dist = dist_sq.sqrt();
                let penetration = 1.0 - (dist / COLLISION_RADIUS);
                -mouse_velocity.0 * penetration * COLLISION_STRENGTH
            } else {
                0.0
            };

            if i < self.collision_impulses.len() {
                self.collision_impulses[i] = impulse;
            }
        }
    }

    /// Generate debug visualization vertices for the collision sphere
    fn generate_debug_collision_sphere(&self, mouse_pos: (f32, f32)) -> Vec<FiberVertex> {
        use std::f32::consts::PI;

        let mut vertices = Vec::with_capacity(DEBUG_CIRCLE_SEGMENTS * 6);

        let line_width = 0.005;
        let color = [1.0, 0.0, 0.0, 0.8];

        for i in 0..DEBUG_CIRCLE_SEGMENTS {
            let angle0 = (i as f32 / DEBUG_CIRCLE_SEGMENTS as f32) * 2.0 * PI;
            let angle1 = ((i + 1) as f32 / DEBUG_CIRCLE_SEGMENTS as f32) * 2.0 * PI;

            let p0 = Vec2::new(
                mouse_pos.0 + COLLISION_RADIUS * angle0.cos(),
                mouse_pos.1 + COLLISION_RADIUS * angle0.sin(),
            );
            let p1 = Vec2::new(
                mouse_pos.0 + COLLISION_RADIUS * angle1.cos(),
                mouse_pos.1 + COLLISION_RADIUS * angle1.sin(),
            );

            let segment = triangulate_segment(p0, p1, line_width, line_width, color);
            vertices.extend_from_slice(&segment);
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

        // Create wgpu instance and surface from winit window
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::PRIMARY,
            flags: wgpu::InstanceFlags::empty(),
            ..Default::default()
        });

        let surface = instance
            .create_surface(window.clone())
            .expect("Failed to create surface");

        let size = window.inner_size();
        let (device, queue, config) = pollster::block_on(
            setup_wgpu_device_and_config(&instance, &surface, size.width, size.height),
        );

        let resources = RendererResources {
            surface,
            device,
            queue,
            config,
        };
        let renderer = Renderer::new(resources, self.args.msaa);

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

            WindowEvent::CursorMoved { position, .. } => {
                // Convert pixel coordinates to clip space [-1, 1]
                if let Some(window) = &self.window {
                    let size = window.inner_size();
                    let clip_x = (position.x as f32 / size.width as f32) * 2.0 - 1.0;
                    let clip_y = -((position.y as f32 / size.height as f32) * 2.0 - 1.0);
                    self.input_state.update_mouse_pos(clip_x, clip_y);
                }
            }

            WindowEvent::CursorLeft { .. } => {
                self.input_state.set_mouse_inactive();
            }

            WindowEvent::RedrawRequested => {
                self.frame_count += 1;

                // Calculate collision impulses if mouse is active
                if let Some(mouse_pos) = self.input_state.mouse_pos() {
                    let mouse_velocity = self.input_state.mouse_velocity();
                    self.calculate_collision_impulses(mouse_pos, mouse_velocity);

                    // Decay mouse velocity for next frame
                    self.input_state.decay_mouse_velocity();
                } else {
                    // Clear collision impulses when mouse is inactive
                    self.collision_impulses.fill(0.0);
                }

                // Build external forces for physics
                let forces = ExternalForces::new()
                    .with_collision_impulses(&self.collision_impulses);

                // Calculate delta time for fixed-timestep physics
                let now = Instant::now();
                if let Some(last) = self.last_frame {
                    let delta = (now - last).as_secs_f32();
                    self.physics_accumulator += delta;

                    // Step physics at fixed 60Hz, capped to prevent spiral of death
                    if let Some(lamp) = &mut self.fiber_lamp {
                        let mut steps = 0;
                        while self.physics_accumulator >= PHYSICS_DT && steps < MAX_PHYSICS_STEPS {
                            lamp.step(&mut self.rng, &forces);
                            self.physics_accumulator -= PHYSICS_DT;
                            steps += 1;
                        }
                    }
                }
                self.last_frame = Some(now);

                // Generate vertices from fiber positions
                let mut vertices = self.generate_fiber_vertices();

                // Add debug collision sphere visualization if enabled via --debug flag
                if self.args.debug {
                    if let Some(mouse_pos) = self.input_state.mouse_pos() {
                        let debug_vertices = self.generate_debug_collision_sphere(mouse_pos);
                        vertices.extend(debug_vertices);
                    }
                }

                // Render
                if let Some(renderer) = &mut self.renderer {
                    renderer.update_vertices(&vertices);

                    match renderer.render() {
                        Ok(_) => {}
                        Err(wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated) => {
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
