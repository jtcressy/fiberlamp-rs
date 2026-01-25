use rand::Rng;
use std::f32::consts::PI;

/// Number of nodes per fiber
pub const NODES: usize = 20;

/// Physics constants (from original fiberlamp.c)
const DT: f32 = 0.5;
const PY: f32 = 0.12;
const DAMPING: f32 = 0.055;
const SPREAD: f32 = 30.0;

/// Calculate segment length for node i
#[inline]
fn len(i: usize) -> f32 {
    if i < NODES - 3 {
        1.0 / (NODES as f32 - 2.5)
    } else {
        0.25 / (NODES as f32 - 2.5)
    }
}

/// Single node in a fiber chain
#[derive(Clone, Copy, Debug, Default)]
pub struct Node {
    pub phi: f32,
    pub phidash: f32,
    pub eta: f32,
    pub etadash: f32,
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

/// A single fiber
pub struct Fiber {
    pub nodes: [Node; NODES],
}

impl Fiber {
    /// Create a new fiber with random initial angles (matches original init_fiberlamp)
    pub fn new<R: Rng>(rng: &mut R) -> Self {
        let phi = (SPREAD * rng.gen_range(0.0_f32..1.0)).to_radians();
        let eta = rng.gen_range(0.0_f32..2.0 * PI) - PI;

        let mut nodes = [Node::default(); NODES];

        // Initialize all nodes with same angles
        for node in &mut nodes {
            node.phi = phi;
            node.phidash = 0.0;
            node.eta = eta;
            node.etadash = 0.0;
        }

        // Initial perturbation on base node
        nodes[0].etadash = 0.002 / DT;
        nodes[0].y = 0.0;
        nodes[0].z = 0.0;

        let mut fiber = Self { nodes };
        // Initial position calculation
        fiber.update_positions_initial();
        fiber
    }

    /// Initial position calculation (before simulation starts)
    fn update_positions_initial(&mut self) {
        self.nodes[0].x = 0.0;
        self.nodes[0].y = 0.0;
        self.nodes[0].z = 0.0;

        for i in 1..NODES {
            let p = &self.nodes[i - 1];
            let sp = p.phi.sin();
            let cp = p.phi.cos();
            let se = p.eta.sin();
            let ce = p.eta.cos();

            let px = p.x;
            let py = p.y;
            let pz = p.z;
            let segment_len = len(i - 1);

            self.nodes[i].x = px + segment_len * ce * sp;
            self.nodes[i].y = py - segment_len * cp;
            self.nodes[i].z = pz + segment_len * se * sp;
        }
    }

    /// Simulate one step - matches original draw_fiberlamp physics loop exactly
    pub fn step(&mut self, cx: f32) {
        // Apply bump to base node eta
        self.nodes[0].eta += cx * 0.05;

        // Process nodes from 1 to NODES (FORWARD, not reverse!)
        for i in 1..NODES {
            let n_phi;
            let n_eta;
            let n_phidash;
            let n_etadash;
            let n_x;
            let n_z;

            // Read current node values
            {
                let n = &self.nodes[i];
                n_phi = n.phi;
                n_eta = n.eta;
                n_phidash = n.phidash;
                n_etadash = n.etadash;
                n_x = n.x;
                n_z = n.z;
            }

            // Read parent node values
            let p_phi;
            let p_eta;
            let p_x;
            let p_y;
            let p_z;
            {
                let p = &self.nodes[i - 1];
                p_phi = p.phi;
                p_eta = p.eta;
                p_x = p.x;
                p_y = p.y;
                p_z = p.z;
            }

            // Stress: spring force toward parent angles
            let pstress = (n_phi - p_phi) * PY;
            let estress = (n_eta - p_eta) * PY;

            // Direction from parent to current node
            let dxi = n_x - p_x;
            let dzi = n_z - p_z;

            // Normalized horizontal distance
            let li_raw = (dxi * dxi + dzi * dzi).sqrt();
            let li = if li_raw > 0.0001 { li_raw / len(i) } else { 0.0001 };

            // Drag term
            let drag = DAMPING * len(i) * len(i) * (NODES * NODES) as f32;

            // Load: accumulated from downstream nodes
            let mut pload = 0.0_f32;
            let mut eload = 0.0_f32;

            if li > 0.0001 {
                for j in (i + 1)..NODES {
                    let nn = &self.nodes[j];
                    let dxj = nn.x - n_x;
                    let dzj = nn.z - n_z;

                    // Dot product for phi load
                    pload += len(j) * (dxi * dxj + dzi * dzj) / li;
                    // Cross product for eta load
                    eload += len(j) * (dxi * dzj - dzi * dxj) / li;
                }
            }

            // Update angular velocities and angles (Euler integration)
            let new_phidash = n_phidash + DT * (pload - pstress - drag * n_phidash) / len(i);
            let new_phi = n_phi + DT * new_phidash;

            let new_etadash = n_etadash + DT * (eload - estress - drag * n_etadash) / len(i);
            let new_eta = n_eta + DT * new_etadash;

            self.nodes[i].phidash = new_phidash;
            self.nodes[i].phi = new_phi;
            self.nodes[i].etadash = new_etadash;
            self.nodes[i].eta = new_eta;

            // Update position using PARENT's angles (key insight from original!)
            let sp = p_phi.sin();
            let cp = p_phi.cos();
            let se = p_eta.sin();
            let ce = p_eta.cos();
            let segment_len = len(i - 1);

            self.nodes[i].x = p_x + segment_len * ce * sp;
            self.nodes[i].y = p_y - segment_len * cp;
            self.nodes[i].z = p_z + segment_len * se * sp;
        }
    }

    pub fn tip(&self) -> &Node {
        &self.nodes[NODES - 1]
    }
}

/// Geometric rotation speed: ~20 degrees/minute at 60fps
/// 20° / 60s / 60fps ≈ 0.00058 rad/frame
const DTHETA: f32 = 0.0006;

/// The complete fiber lamp simulation
pub struct FiberLamp {
    pub fibers: Vec<Fiber>,
    pub cx: f32,
    pub psi: f32,
    pub dpsi: f32,
    pub bump_counter: u32,
    pub cycles: u32,
    /// Current geometric rotation angle (radians)
    pub theta: f32,
}

impl FiberLamp {
    pub fn new<R: Rng>(fiber_count: usize, rng: &mut R) -> Self {
        let fibers = (0..fiber_count).map(|_| Fiber::new(rng)).collect();

        Self {
            fibers,
            cx: 0.0,
            psi: 0.0,
            dpsi: 0.01,
            bump_counter: 0,
            cycles: 10000,
            theta: 0.0,
        }
    }

    pub fn step<R: Rng>(&mut self, rng: &mut R) {
        // Bump check
        self.bump_counter += 1;
        if self.bump_counter > self.cycles {
            self.bump_counter = 0;
            self.cx = rng.gen_range(-0.125_f32..0.125);
        }

        // Color wheel rotation
        self.psi += self.dpsi;
        if self.psi > 2.0 * PI {
            self.psi -= 2.0 * PI;
        }

        // Geometric rotation (~10 degrees/minute)
        self.theta += DTHETA;
        if self.theta > 2.0 * PI {
            self.theta -= 2.0 * PI;
        }

        // Update fibers
        for fiber in &mut self.fibers {
            fiber.step(self.cx);
        }

        // Decay bump
        self.cx *= 0.99;
    }

    pub fn psi(&self) -> f32 {
        self.psi
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fiber_creation() {
        let mut rng = rand::thread_rng();
        let fiber = Fiber::new(&mut rng);

        for node in &fiber.nodes {
            assert!(node.x.is_finite());
            assert!(node.y.is_finite());
            assert!(node.z.is_finite());
        }

        // Tip should be above base (negative y is up)
        assert!(fiber.nodes[NODES - 1].y < fiber.nodes[0].y);
    }

    #[test]
    fn test_fiber_lamp_step() {
        let mut rng = rand::thread_rng();
        let mut lamp = FiberLamp::new(10, &mut rng);

        for _ in 0..100 {
            lamp.step(&mut rng);
        }

        for fiber in &lamp.fibers {
            for node in &fiber.nodes {
                assert!(node.x.is_finite());
                assert!(node.y.is_finite());
                assert!(node.z.is_finite());
                assert!(node.phi.is_finite());
                assert!(node.eta.is_finite());
            }
        }
    }

    #[test]
    fn test_segment_length() {
        let body_len = len(0);
        let tip_len = len(NODES - 1);
        assert!(body_len > tip_len);
    }
}
