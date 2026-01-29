//! Fiberlamp core library - fiber optic lamp simulation and rendering.
//!
//! This crate provides the platform-agnostic core: physics simulation,
//! vertex generation, GPU rendering pipeline, and a shared render loop.

pub mod physics;
pub mod render_loop;
pub mod renderer;
pub mod vertex;

// Re-export commonly used types
pub use physics::{ExternalForces, Fiber, FiberLamp, Node, NODES};
pub use render_loop::{FiberlampConfig, FiberlampSession};
pub use renderer::{Renderer, RendererResources, setup_wgpu_device_and_config};
pub use vertex::{FiberVertex, body_color, generate_palette, tip_color, triangulate_segment};
