//! C FFI interface for macOS screensaver integration
//!
//! This module provides a C-compatible interface for the Objective-C shim to call into
//! the Rust fiberlamp simulation and renderer via FiberlampSession.

use std::ffi::c_void;
use std::panic::{self, AssertUnwindSafe};

use fiberlamp_core::{ExternalForces, FiberlampSession};

#[cfg(feature = "macos-screensaver")]
use fiberlamp_core::{FiberlampConfig, Renderer, RendererResources, setup_wgpu_device_and_config};

/// Opaque handle to the fiberlamp state
pub struct FiberlampHandle {
    session: FiberlampSession,
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
pub struct FiberlampCConfig {
    pub fiber_count: u32,
    pub ncolors: u32,
    pub msaa_samples: u32,
}

/// Animation interval in seconds (60 FPS)
const ANIMATION_INTERVAL: f64 = 1.0 / 60.0;

/// Create RendererResources from a raw NSView pointer (macOS-specific surface creation).
#[cfg(feature = "macos-screensaver")]
async fn create_resources_from_nsview(
    ns_view: *mut c_void,
    width: u32,
    height: u32,
) -> RendererResources {
    use raw_window_handle::{AppKitWindowHandle, RawWindowHandle};
    use std::ptr::NonNull;

    let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
        backends: wgpu::Backends::METAL,
        flags: wgpu::InstanceFlags::empty(),
        ..Default::default()
    });

    let surface = unsafe {
        let handle = AppKitWindowHandle::new(
            NonNull::new(ns_view).expect("NSView pointer must not be null"),
        );
        let raw_handle = RawWindowHandle::AppKit(handle);

        let target = wgpu::SurfaceTargetUnsafe::RawHandle {
            raw_display_handle: raw_window_handle::RawDisplayHandle::AppKit(
                raw_window_handle::AppKitDisplayHandle::new(),
            ),
            raw_window_handle: raw_handle,
        };

        instance
            .create_surface_unsafe(target)
            .expect("Failed to create surface from NSView")
    };

    let (device, queue, config) =
        setup_wgpu_device_and_config(&instance, &surface, width, height).await;

    RendererResources {
        surface,
        device,
        queue,
        config,
    }
}

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
    let config = FiberlampCConfig {
        fiber_count: DEFAULT_FIBER_COUNT,
        ncolors: DEFAULT_NCOLORS,
        msaa_samples: DEFAULT_MSAA_SAMPLES,
    };
    fiberlamp_init_with_config(ns_view, width, height, config)
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
        let forces = ExternalForces::new();

        match handle.session.animate(&forces) {
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
        handle.session.resize(width, height);
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
pub extern "C" fn fiberlamp_default_config() -> FiberlampCConfig {
    FiberlampCConfig {
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
    config: FiberlampCConfig,
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

        #[cfg(feature = "macos-screensaver")]
        {
            let resources =
                pollster::block_on(create_resources_from_nsview(ns_view, width, height));
            let renderer = Renderer::new(resources, msaa_samples);

            let session_config = FiberlampConfig {
                fiber_count,
                ncolors,
            };
            let session = FiberlampSession::new(renderer, session_config);

            let handle = Box::new(FiberlampHandle { session });
            Box::into_raw(handle)
        }

        #[cfg(not(feature = "macos-screensaver"))]
        {
            log::error!("fiberlamp_init_with_config: macos-screensaver feature not enabled");
            std::ptr::null_mut()
        }
    }));

    match result {
        Ok(ptr) => ptr,
        Err(e) => {
            log::error!("fiberlamp_init_with_config panicked: {:?}", e);
            std::ptr::null_mut()
        }
    }
}
