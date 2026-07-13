// Copyright The SimpleGameEngine Contributors

use std::time::{Duration, Instant};

const SAMPLE_INTERVAL: Duration = Duration::from_millis(500);

#[derive(Debug)]
pub struct FrameRateCounter {
    sample_started: Instant,
    sampled_frames: u32,
    frames_per_second: Option<f32>,
}

impl Default for FrameRateCounter {
    fn default() -> Self {
        Self::new()
    }
}

impl FrameRateCounter {
    #[must_use]
    pub fn new() -> Self {
        Self::new_at(Instant::now())
    }

    /// Records one completed frame and returns whether the displayed sample changed.
    pub fn record_frame(&mut self) -> bool {
        self.record_frame_at(Instant::now())
    }

    #[must_use]
    pub fn rounded_frames_per_second(&self) -> Option<u32> {
        self.frames_per_second.map(|value| value.round() as u32)
    }

    fn new_at(now: Instant) -> Self {
        Self {
            sample_started: now,
            sampled_frames: 0,
            frames_per_second: None,
        }
    }

    fn record_frame_at(&mut self, now: Instant) -> bool {
        self.sampled_frames = self.sampled_frames.saturating_add(1);
        let elapsed = now.saturating_duration_since(self.sample_started);
        if elapsed < SAMPLE_INTERVAL {
            return false;
        }
        self.frames_per_second = Some(self.sampled_frames as f32 / elapsed.as_secs_f32());
        self.sample_started = now;
        self.sampled_frames = 0;
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn samples_presented_frames_over_a_fixed_interval() {
        let started = Instant::now();
        let mut counter = FrameRateCounter::new_at(started);
        for frame in 1..=30 {
            let changed =
                counter.record_frame_at(started + Duration::from_millis(frame * 500 / 30));
            assert_eq!(changed, frame == 30);
        }
        assert_eq!(counter.rounded_frames_per_second(), Some(60));
    }
}
