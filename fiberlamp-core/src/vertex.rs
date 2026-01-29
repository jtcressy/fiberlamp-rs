use bytemuck::{Pod, Zeroable};
use glam::Vec2;

/// Vertex data for fiber rendering
/// Each fiber segment is triangulated into a quad (2 triangles, 6 vertices)
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct FiberVertex {
    /// Screen-space position in clip coordinates [-1, 1]
    pub position: [f32; 2],
    /// RGBA color with alpha for glow falloff
    pub color: [f32; 4],
}

impl FiberVertex {
    pub fn new(position: Vec2, color: [f32; 4]) -> Self {
        Self {
            position: position.into(),
            color,
        }
    }

    pub fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<FiberVertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                // position
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float32x2,
                },
                // color
                wgpu::VertexAttribute {
                    offset: 8,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float32x4,
                },
            ],
        }
    }
}

/// Triangulate a line segment into a quad (2 triangles = 6 vertices)
///
/// Creates a screen-space quad with the specified width at each endpoint.
/// This is necessary because wgpu doesn't support line width.
pub fn triangulate_segment(
    p0: Vec2,         // Start point (clip coords)
    p1: Vec2,         // End point (clip coords)
    width0: f32,      // Width at start (clip coords)
    width1: f32,      // Width at end (clip coords)
    color: [f32; 4],  // RGBA color
) -> [FiberVertex; 6] {
    let dir = (p1 - p0).normalize_or_zero();
    let perp = Vec2::new(-dir.y, dir.x);

    // Four corners of the quad
    let v0 = p0 - perp * width0 * 0.5;
    let v1 = p0 + perp * width0 * 0.5;
    let v2 = p1 - perp * width1 * 0.5;
    let v3 = p1 + perp * width1 * 0.5;

    // Two triangles: (v0, v1, v2) and (v1, v3, v2)
    [
        FiberVertex::new(v0, color),
        FiberVertex::new(v1, color),
        FiberVertex::new(v2, color),
        FiberVertex::new(v1, color),
        FiberVertex::new(v3, color),
        FiberVertex::new(v2, color),
    ]
}

/// Convert HSV to RGB
/// h: hue in degrees (0-360)
/// s: saturation (0-1)
/// v: value (0-1)
pub fn hsv_to_rgb(h: f32, s: f32, v: f32) -> [f32; 3] {
    let h = h % 360.0;
    let c = v * s;
    let x = c * (1.0 - ((h / 60.0) % 2.0 - 1.0).abs());
    let m = v - c;

    let (r, g, b) = if h < 60.0 {
        (c, x, 0.0)
    } else if h < 120.0 {
        (x, c, 0.0)
    } else if h < 180.0 {
        (0.0, c, x)
    } else if h < 240.0 {
        (0.0, x, c)
    } else if h < 300.0 {
        (x, 0.0, c)
    } else {
        (c, 0.0, x)
    };

    [r + m, g + m, b + m]
}

/// Generate a color palette with ncolors evenly spaced hues
pub fn generate_palette(ncolors: usize) -> Vec<[f32; 3]> {
    (0..ncolors)
        .map(|i| {
            let hue = i as f32 / ncolors as f32 * 360.0;
            hsv_to_rgb(hue, 1.0, 1.0)
        })
        .collect()
}

/// Body color based on z-depth and segment position
/// seg_t: 0.0 = base, 1.0 = tip - used to dim base where fibers overlap
pub fn body_color(z: f32, seg_t: f32) -> [f32; 4] {
    // z ranges from about -1 to 1
    // Map z to normalized depth: 0 = back, 1 = front
    let depth_t = ((z + 1.0) / 2.0).clamp(0.0, 1.0);

    // Inverse-square falloff for realistic light attenuation
    // Map depth to distance: front (depth_t=1) -> d=1, back (depth_t=0) -> d=8
    // front: 1/1² = 1.0 (100%), back: 1/8² = 0.016 (1.6%)
    let distance = 1.0 + (1.0 - depth_t) * 7.0;
    let inv_sq = 1.0 / (distance * distance);

    // Interpolate from very dark olive (back) to bright white (front)
    // Very dark olive base: ~#1a1a0d = (0.1, 0.1, 0.05)
    // Bright white/cream at front: (1.0, 1.0, 0.9)
    let r = 0.1 + inv_sq * 0.9;
    let g = 0.1 + inv_sq * 0.9;
    let b = 0.05 + inv_sq * 0.85;

    // Dim base segments heavily to compensate for additive overlap
    let seg_brightness = 0.1 + seg_t * seg_t * 0.9;

    [r * seg_brightness, g * seg_brightness, b * seg_brightness, 0.4]
}

/// Tip color from rotating palette
pub fn tip_color(x: f32, y: f32, psi: f32, palette: &[[f32; 3]]) -> [f32; 4] {
    let ncolors = palette.len();
    if ncolors == 0 {
        return [1.0, 1.0, 1.0, 1.0];
    }

    let angle = y.atan2(x) + psi;
    let normalized = (angle / (2.0 * std::f32::consts::PI)).rem_euclid(1.0);
    let idx = (normalized * ncolors as f32) as usize % ncolors;
    let [r, g, b] = palette[idx];
    [r, g, b, 1.0] // Full opacity for bright tips
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_triangulate_segment() {
        let vertices = triangulate_segment(
            Vec2::new(0.0, 0.0),
            Vec2::new(1.0, 0.0),
            0.1,
            0.1,
            [1.0, 0.0, 0.0, 1.0],
        );
        assert_eq!(vertices.len(), 6);
    }

    #[test]
    fn test_hsv_to_rgb() {
        // Red
        let [r, g, b] = hsv_to_rgb(0.0, 1.0, 1.0);
        assert!((r - 1.0).abs() < 0.001);
        assert!(g.abs() < 0.001);
        assert!(b.abs() < 0.001);

        // Green
        let [r, g, b] = hsv_to_rgb(120.0, 1.0, 1.0);
        assert!(r.abs() < 0.001);
        assert!((g - 1.0).abs() < 0.001);
        assert!(b.abs() < 0.001);

        // Blue
        let [r, g, b] = hsv_to_rgb(240.0, 1.0, 1.0);
        assert!(r.abs() < 0.001);
        assert!(g.abs() < 0.001);
        assert!((b - 1.0).abs() < 0.001);
    }
}
