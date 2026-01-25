/// Input state tracking for window movement and mouse interaction

/// Tracks input state for interactivity features
#[derive(Debug, Default)]
pub struct InputState {
    // Window movement tracking
    window_pos: Option<(i32, i32)>,
    window_velocity: (f32, f32),

    // Mouse state
    mouse_pos: Option<(f32, f32)>, // Normalized clip space [-1, 1]
    mouse_velocity: (f32, f32),    // Mouse velocity in clip space
    mouse_active: bool,
}

impl InputState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Update window position and calculate velocity
    /// Call this when WindowEvent::Moved is received
    pub fn update_window_pos(&mut self, x: i32, y: i32) {
        if let Some((prev_x, prev_y)) = self.window_pos {
            // Calculate raw delta (pixels moved)
            let dx = (x - prev_x) as f32;
            let dy = (y - prev_y) as f32;

            // Smooth velocity with exponential moving average
            // Higher weight = more responsive, lower = smoother
            const SMOOTHING: f32 = 0.3;
            self.window_velocity.0 = self.window_velocity.0 * (1.0 - SMOOTHING) + dx * SMOOTHING;
            self.window_velocity.1 = self.window_velocity.1 * (1.0 - SMOOTHING) + dy * SMOOTHING;
        }
        self.window_pos = Some((x, y));
    }

    /// Update mouse position in clip space and calculate velocity
    /// x, y should be in range [-1, 1]
    pub fn update_mouse_pos(&mut self, x: f32, y: f32) {
        if let Some((prev_x, prev_y)) = self.mouse_pos {
            // Calculate velocity (change in position)
            let dx = x - prev_x;
            let dy = y - prev_y;

            // Smooth velocity with exponential moving average
            const SMOOTHING: f32 = 0.5;
            self.mouse_velocity.0 = self.mouse_velocity.0 * (1.0 - SMOOTHING) + dx * SMOOTHING;
            self.mouse_velocity.1 = self.mouse_velocity.1 * (1.0 - SMOOTHING) + dy * SMOOTHING;
        }
        self.mouse_pos = Some((x, y));
        self.mouse_active = true;
    }

    /// Get mouse velocity in clip space
    pub fn mouse_velocity(&self) -> (f32, f32) {
        self.mouse_velocity
    }

    /// Decay mouse velocity when not moving
    pub fn decay_mouse_velocity(&mut self) {
        const DECAY: f32 = 0.8;
        self.mouse_velocity.0 *= DECAY;
        self.mouse_velocity.1 *= DECAY;
    }

    /// Mark mouse as inactive (cursor left window)
    pub fn set_mouse_inactive(&mut self) {
        self.mouse_active = false;
    }

    /// Get current window velocity for physics impulse
    /// Returns (x, y) velocity in pixels/frame (smoothed)
    pub fn window_velocity(&self) -> (f32, f32) {
        self.window_velocity
    }

    /// Decay window velocity when window is stationary
    /// Call this each frame
    pub fn decay_velocity(&mut self) {
        const DECAY: f32 = 0.9;
        self.window_velocity.0 *= DECAY;
        self.window_velocity.1 *= DECAY;
    }

    /// Get mouse position in clip space if active
    pub fn mouse_pos(&self) -> Option<(f32, f32)> {
        if self.mouse_active {
            self.mouse_pos
        } else {
            None
        }
    }

}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_window_velocity_calculation() {
        let mut input = InputState::new();

        // First position establishes baseline
        input.update_window_pos(100, 100);
        assert_eq!(input.window_velocity(), (0.0, 0.0));

        // Move window 10 pixels right, 5 pixels down
        input.update_window_pos(110, 105);
        let (vx, vy) = input.window_velocity();
        assert!(vx > 0.0, "Should have positive x velocity");
        assert!(vy > 0.0, "Should have positive y velocity");
    }

    #[test]
    fn test_velocity_decay() {
        let mut input = InputState::new();
        input.update_window_pos(0, 0);
        input.update_window_pos(100, 0); // Large movement

        let (initial_vx, _) = input.window_velocity();
        input.decay_velocity();
        let (decayed_vx, _) = input.window_velocity();

        assert!(decayed_vx < initial_vx, "Velocity should decay");
    }

    #[test]
    fn test_mouse_inactive() {
        let mut input = InputState::new();
        input.update_mouse_pos(0.5, 0.5);
        assert!(input.mouse_pos().is_some());

        input.set_mouse_inactive();
        assert!(input.mouse_pos().is_none());
    }
}
