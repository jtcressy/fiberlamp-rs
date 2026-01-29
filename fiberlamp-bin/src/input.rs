/// Input state tracking for mouse interaction

/// Tracks input state for mouse collision
#[derive(Debug, Default)]
pub struct InputState {
    // Mouse state
    mouse_pos: Option<(f32, f32)>, // Normalized clip space [-1, 1]
    mouse_velocity: (f32, f32),    // Mouse velocity in clip space
    mouse_active: bool,
}

impl InputState {
    pub fn new() -> Self {
        Self::default()
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
    fn test_mouse_inactive() {
        let mut input = InputState::new();
        input.update_mouse_pos(0.5, 0.5);
        assert!(input.mouse_pos().is_some());

        input.set_mouse_inactive();
        assert!(input.mouse_pos().is_none());
    }

    #[test]
    fn test_mouse_velocity() {
        let mut input = InputState::new();

        // First position establishes baseline
        input.update_mouse_pos(0.0, 0.0);
        assert_eq!(input.mouse_velocity(), (0.0, 0.0));

        // Move mouse
        input.update_mouse_pos(0.1, 0.05);
        let (vx, vy) = input.mouse_velocity();
        assert!(vx > 0.0, "Should have positive x velocity");
        assert!(vy > 0.0, "Should have positive y velocity");
    }
}
