//! C FFI interface for macOS screensaver integration
//!
//! This module provides a C-compatible interface for the Objective-C shim to call into
//! the Rust fiberlamp simulation and renderer.

use std::ffi::c_void;
use std::panic::{self, AssertUnwindSafe};

use crate::physics::{ExternalForces, FiberLamp};
use crate::renderer::Renderer;
use crate::vertex::{body_color, generate_palette, tip_color, triangulate_segment, FiberVertex};
use crate::NODES;
use glam::Vec2;
use rand::rngs::ThreadRng;

/// Opaque handle to the fiberlamp state
pub struct FiberlampHandle {
    renderer: Renderer,
    fiber_lamp: FiberLamp,
    rng: ThreadRng,
    palette: Vec<[f32; 3]>,
}

/// Configuration constants (defaults)
const DEFAULT_FIBER_COUNT: u32 = 500;
const DEFAULT_NCOLORS: u32 = 64;
const DEFAULT_MSAA_SAMPLES: u32 = 4;

/// Bounds for fiber count
const MIN_FIBERS: u32 = 10;
const MAX_FIBERS: u32 = 500;

/// C-compatible configuration struct for fiberlamp initialization
#[repr(C)]
pub struct FiberlampConfig {
    pub fiber_count: u32,
    pub ncolors: u32,
    pub msaa_samples: u32,
}

/// Animation interval in seconds (60 FPS)
const ANIMATION_INTERVAL: f64 = 1.0 / 60.0;

/// Initialize the fiberlamp renderer and simulation
///
/// # Safety
/// - `ns_view` must be a valid pointer to an NSView with `wantsLayer = YES`
/// - Must be called from the main thread
/// - The NSView must remain valid until `fiberlamp_destroy` is called
#[unsafe(no_mangle)]
pub extern "C" fn fiberlamp_init(
    ns_view: *mut c_void,
    width: u32,
    height: u32,
) -> *mut FiberlampHandle {
    let result = panic::catch_unwind(AssertUnwindSafe(|| {
        if ns_view.is_null() {
            log::error!("fiberlamp_init: ns_view is null");
            return std::ptr::null_mut();
        }

        // Initialize logging (safe to call multiple times)
        let _ = env_logger::try_init();

        log::info!(
            "fiberlamp_init: ns_view={:?}, size={}x{}",
            ns_view,
            width,
            height
        );

        let mut rng = rand::thread_rng();

        // Create renderer from raw NSView
        let renderer = pollster::block_on(Renderer::from_raw_view(
            ns_view,
            width,
            height,
            DEFAULT_MSAA_SAMPLES,
        ));

        // Create fiber lamp simulation
        let fiber_lamp = FiberLamp::new(DEFAULT_FIBER_COUNT as usize, &mut rng);

        // Generate color palette
        let palette = generate_palette(DEFAULT_NCOLORS as usize);

        let handle = Box::new(FiberlampHandle {
            renderer,
            fiber_lamp,
            rng,
            palette,
        });

        Box::into_raw(handle)
    }));

    match result {
        Ok(ptr) => ptr,
        Err(e) => {
            log::error!("fiberlamp_init panicked: {:?}", e);
            std::ptr::null_mut()
        }
    }
}

/// Animate one frame
///
/// Returns 0 on success, non-zero on error
///
/// # Safety
/// - `handle` must be a valid pointer returned by `fiberlamp_init`
/// - Must be called from the main thread
#[unsafe(no_mangle)]
pub extern "C" fn fiberlamp_animate(handle: *mut FiberlampHandle) -> i32 {
    if handle.is_null() {
        return -1;
    }

    let handle = unsafe { &mut *handle };

    let result = panic::catch_unwind(AssertUnwindSafe(|| {
        // Step physics
        let forces = ExternalForces::new();
        handle.fiber_lamp.step(&mut handle.rng, &forces);

        // Generate vertices
        let vertices = generate_fiber_vertices(
            &handle.fiber_lamp,
            &handle.renderer,
            &handle.palette,
        );

        // Update and render
        handle.renderer.update_vertices(&vertices);

        match handle.renderer.render() {
            Ok(()) => 0,
            Err(wgpu::SurfaceError::Lost) => {
                // Surface lost, needs resize
                1
            }
            Err(wgpu::SurfaceError::OutOfMemory) => {
                log::error!("Out of GPU memory");
                -2
            }
            Err(e) => {
                log::warn!("Render error: {:?}", e);
                0 // Non-fatal errors
            }
        }
    }));

    match result {
        Ok(code) => code,
        Err(e) => {
            log::error!("fiberlamp_animate panicked: {:?}", e);
            -3
        }
    }
}

/// Resize the renderer
///
/// # Safety
/// - `handle` must be a valid pointer returned by `fiberlamp_init`
/// - Must be called from the main thread
#[unsafe(no_mangle)]
pub extern "C" fn fiberlamp_resize(handle: *mut FiberlampHandle, width: u32, height: u32) {
    if handle.is_null() {
        return;
    }

    let handle = unsafe { &mut *handle };

    let result = panic::catch_unwind(AssertUnwindSafe(|| {
        log::info!("fiberlamp_resize: {}x{}", width, height);
        handle.renderer.resize(width, height);
    }));

    if let Err(e) = result {
        log::error!("fiberlamp_resize panicked: {:?}", e);
    }
}

/// Destroy the fiberlamp handle and free resources
///
/// # Safety
/// - `handle` must be a valid pointer returned by `fiberlamp_init`, or null
/// - After this call, `handle` must not be used again
/// - Must be called from the main thread
#[unsafe(no_mangle)]
pub extern "C" fn fiberlamp_destroy(handle: *mut FiberlampHandle) {
    if handle.is_null() {
        return;
    }

    let result = panic::catch_unwind(AssertUnwindSafe(|| {
        log::info!("fiberlamp_destroy");
        let _ = unsafe { Box::from_raw(handle) };
    }));

    if let Err(e) = result {
        log::error!("fiberlamp_destroy panicked: {:?}", e);
    }
}

/// Get the recommended animation interval in seconds
#[unsafe(no_mangle)]
pub extern "C" fn fiberlamp_animation_interval() -> f64 {
    ANIMATION_INTERVAL
}

/// Get the default configuration
#[unsafe(no_mangle)]
pub extern "C" fn fiberlamp_default_config() -> FiberlampConfig {
    FiberlampConfig {
        fiber_count: DEFAULT_FIBER_COUNT,
        ncolors: DEFAULT_NCOLORS,
        msaa_samples: DEFAULT_MSAA_SAMPLES,
    }
}

/// Get the minimum allowed fiber count
#[unsafe(no_mangle)]
pub extern "C" fn fiberlamp_min_fibers() -> u32 {
    MIN_FIBERS
}

/// Get the maximum allowed fiber count
#[unsafe(no_mangle)]
pub extern "C" fn fiberlamp_max_fibers() -> u32 {
    MAX_FIBERS
}

/// Initialize the fiberlamp renderer and simulation with custom configuration
///
/// # Safety
/// - `ns_view` must be a valid pointer to an NSView with `wantsLayer = YES`
/// - Must be called from the main thread
/// - The NSView must remain valid until `fiberlamp_destroy` is called
#[unsafe(no_mangle)]
pub extern "C" fn fiberlamp_init_with_config(
    ns_view: *mut c_void,
    width: u32,
    height: u32,
    config: FiberlampConfig,
) -> *mut FiberlampHandle {
    let result = panic::catch_unwind(AssertUnwindSafe(|| {
        if ns_view.is_null() {
            log::error!("fiberlamp_init_with_config: ns_view is null");
            return std::ptr::null_mut();
        }

        // Initialize logging (safe to call multiple times)
        let _ = env_logger::try_init();

        // Validate and clamp configuration
        let fiber_count = config.fiber_count.clamp(MIN_FIBERS, MAX_FIBERS) as usize;
        let ncolors = config.ncolors as usize;
        let msaa_samples = config.msaa_samples;

        log::info!(
            "fiberlamp_init_with_config: ns_view={:?}, size={}x{}, fibers={}, colors={}, msaa={}",
            ns_view,
            width,
            height,
            fiber_count,
            ncolors,
            msaa_samples
        );

        let mut rng = rand::thread_rng();

        // Create renderer from raw NSView
        let renderer = pollster::block_on(Renderer::from_raw_view(
            ns_view,
            width,
            height,
            msaa_samples,
        ));

        // Create fiber lamp simulation
        let fiber_lamp = FiberLamp::new(fiber_count, &mut rng);

        // Generate color palette
        let palette = generate_palette(ncolors);

        let handle = Box::new(FiberlampHandle {
            renderer,
            fiber_lamp,
            rng,
            palette,
        });

        Box::into_raw(handle)
    }));

    match result {
        Ok(ptr) => ptr,
        Err(e) => {
            log::error!("fiberlamp_init_with_config panicked: {:?}", e);
            std::ptr::null_mut()
        }
    }
}

/// Generate vertices from all fibers for rendering
/// (Duplicated from main.rs to avoid dependency on winit types)
fn generate_fiber_vertices(
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
        // Skip first segment so fibers start spread apart
        for i in 2..NODES {
            let parent = &fiber.nodes[i - 1];
            let node = &fiber.nodes[i];

            // Apply geometric rotation around vertical axis
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
