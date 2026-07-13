// Copyright The SimpleGameEngine Contributors

use std::time::Duration;

pub const DEFAULT_FIXED_STEP: Duration = Duration::from_nanos(16_666_667);

#[derive(Debug, PartialEq, Eq)]
pub struct Time {
    delta: Duration,
    elapsed: Duration,
    frame_index: u64,
}

impl Time {
    pub(crate) const fn new() -> Self {
        Self {
            delta: Duration::ZERO,
            elapsed: Duration::ZERO,
            frame_index: 0,
        }
    }

    pub(crate) fn advance(&mut self, delta: Duration) {
        self.delta = delta;
        self.elapsed = self.elapsed.saturating_add(delta);
        self.frame_index = self.frame_index.saturating_add(1);
    }

    #[must_use]
    pub const fn delta(&self) -> Duration {
        self.delta
    }

    #[must_use]
    pub const fn elapsed(&self) -> Duration {
        self.elapsed
    }

    #[must_use]
    pub const fn frame_index(&self) -> u64 {
        self.frame_index
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct FixedTime {
    step: Duration,
    elapsed: Duration,
    tick_index: u64,
}

impl FixedTime {
    pub(crate) const fn new(step: Duration) -> Self {
        Self {
            step,
            elapsed: Duration::ZERO,
            tick_index: 0,
        }
    }

    pub(crate) fn advance(&mut self) {
        self.elapsed = self.elapsed.saturating_add(self.step);
        self.tick_index = self.tick_index.saturating_add(1);
    }

    #[must_use]
    pub const fn step(&self) -> Duration {
        self.step
    }

    #[must_use]
    pub const fn elapsed(&self) -> Duration {
        self.elapsed
    }

    #[must_use]
    pub const fn tick_index(&self) -> u64 {
        self.tick_index
    }
}
