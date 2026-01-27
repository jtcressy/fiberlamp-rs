//! Fiberlamp library - fiber optic lamp simulation
//!
//! This library provides the core simulation and rendering for the fiberlamp screensaver.

pub mod physics;
pub mod renderer;
pub mod vertex;

#[cfg(feature = "macos-screensaver")]
pub mod ffi;

// Re-export commonly used types
pub use physics::{Fiber, FiberLamp, Node, NODES};
pub use renderer::Renderer;
pub use vertex::{FiberVertex, body_color, generate_palette, tip_color, triangulate_segment};
