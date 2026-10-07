//! Lossless scroll distances from Android's timestamped gesture owner.
pub struct ScrollDistance {
    epoch: u64,
    active: bool,
    pending: f32,
}
impl ScrollDistance {
    pub const fn new() -> Self {
        Self {
            epoch: 1,
            active: false,
            pending: 0.,
        }
    }
    pub fn active(&mut self, active: bool) {
        self.active = active;
        if !active {
            self.invalidate();
        }
    }
    pub fn invalidate(&mut self) {
        self.epoch = self.epoch.wrapping_add(1).max(1);
        self.pending = 0.;
    }
    pub fn token(&self, screen: u8) -> u64 {
        if self.active && screen == 8 {
            self.epoch
        } else {
            0
        }
    }
    pub fn push(&mut self, screen: u8, epoch: u64, delta: f32) -> bool {
        if epoch == 0 || epoch != self.token(screen) || !delta.is_finite() {
            return false;
        }
        let sum = self.pending + delta;
        if !sum.is_finite() {
            return false;
        }
        self.pending = sum;
        true
    }
    pub fn stop(&mut self, epoch: u64) {
        if epoch == self.epoch {
            // Keep already travelled distance; reject only subsequent coast callbacks.
            self.epoch = self.epoch.wrapping_add(1).max(1);
        }
    }
    pub fn take(&mut self, screen: u8) -> f32 {
        let distance = std::mem::take(&mut self.pending);
        if self.token(screen) != 0 {
            distance
        } else {
            0.
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn batching_preserves_distance_and_direction_changes() {
        let mut scroll = ScrollDistance::new();
        scroll.active(true);
        let epoch = scroll.token(8);
        for delta in [-12., -30., 5.] {
            assert!(scroll.push(8, epoch, delta));
        }
        assert_eq!(scroll.take(8), -37.);
        assert_eq!(scroll.take(8), 0.);
    }
    #[test]
    fn catching_fling_keeps_travel_but_rejects_late_coast() {
        let mut scroll = ScrollDistance::new();
        scroll.active(true);
        let epoch = scroll.token(8);
        scroll.push(8, epoch, -15.);
        scroll.stop(epoch);
        assert!(!scroll.push(8, epoch, -100.));
        assert_eq!(scroll.take(8), -15.);
        assert!(scroll.push(8, scroll.token(8), 4.));
        assert_eq!(scroll.take(8), 4.);
    }
    #[test]
    fn navigation_background_and_invalid_values_cannot_scroll() {
        let mut scroll = ScrollDistance::new();
        scroll.active(true);
        let epoch = scroll.token(8);
        scroll.push(8, epoch, -15.);
        scroll.invalidate();
        assert!(!scroll.push(8, epoch, -100.));
        assert_eq!(scroll.take(8), 0.);
        let epoch = scroll.token(8);
        assert!(!scroll.push(8, epoch, f32::NAN));
        assert!(!scroll.push(2, epoch, 5.));
        scroll.push(8, epoch, -20.);
        scroll.active(false);
        assert_eq!(scroll.take(8), 0.);
        assert_eq!(scroll.token(8), 0);
    }
}
