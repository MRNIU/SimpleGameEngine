// Copyright The SimpleGameEngine Contributors

use std::{collections::VecDeque, time::Duration};

use crate::SkippedSurfaceFrame;

const DEFAULT_SAMPLE_CAPACITY: usize = 240;
const SIXTY_FPS_BUDGET: Duration = Duration::from_nanos(16_666_667);
const THIRTY_FPS_BUDGET: Duration = Duration::from_nanos(33_333_333);

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct FramePhaseDurations {
    advance: Option<Duration>,
    extract: Option<Duration>,
    render: Option<Duration>,
}

impl FramePhaseDurations {
    #[must_use]
    pub const fn new(advance: Duration, extract: Duration, render: Duration) -> Self {
        Self {
            advance: Some(advance),
            extract: Some(extract),
            render: Some(render),
        }
    }

    #[must_use]
    pub const fn play(advance: Duration, extract: Duration) -> Self {
        Self {
            advance: Some(advance),
            extract: Some(extract),
            render: None,
        }
    }

    #[must_use]
    pub const fn render(render: Duration) -> Self {
        Self {
            advance: None,
            extract: None,
            render: Some(render),
        }
    }

    #[must_use]
    pub const fn advance(self) -> Option<Duration> {
        self.advance
    }

    #[must_use]
    pub const fn extract(self) -> Option<Duration> {
        self.extract
    }

    #[must_use]
    pub const fn render_duration(self) -> Option<Duration> {
        self.render
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct FramePerformanceSample {
    frame_time: Duration,
    phases: FramePhaseDurations,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FrameTimeSummary {
    p50: Duration,
    p95: Duration,
    max: Duration,
}

impl FrameTimeSummary {
    #[must_use]
    pub const fn p50(self) -> Duration {
        self.p50
    }

    #[must_use]
    pub const fn p95(self) -> Duration {
        self.p95
    }

    #[must_use]
    pub const fn max(self) -> Duration {
        self.max
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SurfaceSkipCounters {
    zero_size: u64,
    timeout: u64,
    occluded: u64,
    outdated: u64,
}

impl SurfaceSkipCounters {
    fn record(&mut self, reason: SkippedSurfaceFrame) {
        let counter = match reason {
            SkippedSurfaceFrame::ZeroSize => &mut self.zero_size,
            SkippedSurfaceFrame::Timeout => &mut self.timeout,
            SkippedSurfaceFrame::Occluded => &mut self.occluded,
            SkippedSurfaceFrame::Outdated => &mut self.outdated,
        };
        *counter = counter.saturating_add(1);
    }

    #[must_use]
    pub const fn zero_size(self) -> u64 {
        self.zero_size
    }

    #[must_use]
    pub const fn timeout(self) -> u64 {
        self.timeout
    }

    #[must_use]
    pub const fn occluded(self) -> u64 {
        self.occluded
    }

    #[must_use]
    pub const fn outdated(self) -> u64 {
        self.outdated
    }

    #[must_use]
    pub const fn total(self) -> u64 {
        self.zero_size
            .saturating_add(self.timeout)
            .saturating_add(self.occluded)
            .saturating_add(self.outdated)
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct FramePerformanceSummary {
    sample_count: usize,
    frames_per_second: Option<u32>,
    frame_time: Option<FrameTimeSummary>,
    average_advance: Option<Duration>,
    average_extract: Option<Duration>,
    average_render: Option<Duration>,
    frames_over_60_fps_budget: usize,
    frames_over_30_fps_budget: usize,
    surface_skips: SurfaceSkipCounters,
}

impl FramePerformanceSummary {
    #[must_use]
    pub const fn sample_count(self) -> usize {
        self.sample_count
    }

    #[must_use]
    pub const fn frames_per_second(self) -> Option<u32> {
        self.frames_per_second
    }

    #[must_use]
    pub const fn frame_time(self) -> Option<FrameTimeSummary> {
        self.frame_time
    }

    #[must_use]
    pub const fn average_advance(self) -> Option<Duration> {
        self.average_advance
    }

    #[must_use]
    pub const fn average_extract(self) -> Option<Duration> {
        self.average_extract
    }

    #[must_use]
    pub const fn average_render(self) -> Option<Duration> {
        self.average_render
    }

    #[must_use]
    pub const fn frames_over_60_fps_budget(self) -> usize {
        self.frames_over_60_fps_budget
    }

    #[must_use]
    pub const fn frames_over_30_fps_budget(self) -> usize {
        self.frames_over_30_fps_budget
    }

    #[must_use]
    pub const fn surface_skips(self) -> SurfaceSkipCounters {
        self.surface_skips
    }
}

#[derive(Debug)]
pub struct FramePerformanceMonitor {
    capacity: usize,
    samples: VecDeque<FramePerformanceSample>,
    last_completed_at: Option<std::time::Instant>,
    surface_skips: SurfaceSkipCounters,
}

impl Default for FramePerformanceMonitor {
    fn default() -> Self {
        Self::new()
    }
}

impl FramePerformanceMonitor {
    #[must_use]
    pub fn new() -> Self {
        Self::with_capacity(DEFAULT_SAMPLE_CAPACITY)
    }

    fn with_capacity(capacity: usize) -> Self {
        debug_assert!(capacity > 0);
        Self {
            capacity: capacity.max(1),
            samples: VecDeque::with_capacity(capacity),
            last_completed_at: None,
            surface_skips: SurfaceSkipCounters::default(),
        }
    }

    pub fn record_completed(&mut self, phases: FramePhaseDurations) {
        self.record_completed_at(std::time::Instant::now(), phases);
    }

    fn record_completed_at(&mut self, now: std::time::Instant, phases: FramePhaseDurations) {
        let Some(previous) = self.last_completed_at.replace(now) else {
            return;
        };
        if self.samples.len() == self.capacity {
            let _ = self.samples.pop_front();
        }
        self.samples.push_back(FramePerformanceSample {
            frame_time: now.saturating_duration_since(previous),
            phases,
        });
    }

    pub fn record_surface_skip(&mut self, reason: SkippedSurfaceFrame) {
        self.surface_skips.record(reason);
    }

    pub fn reset(&mut self) {
        self.samples.clear();
        self.last_completed_at = None;
        self.surface_skips = SurfaceSkipCounters::default();
    }

    #[must_use]
    pub fn frames_per_second(&self) -> Option<u32> {
        let elapsed = self
            .samples
            .iter()
            .map(|sample| sample.frame_time)
            .fold(Duration::ZERO, Duration::saturating_add);
        (!elapsed.is_zero())
            .then(|| (self.samples.len() as f64 / elapsed.as_secs_f64()).round() as u32)
    }

    #[must_use]
    pub fn summary(&self) -> FramePerformanceSummary {
        let sample_count = self.samples.len();
        if sample_count == 0 {
            return FramePerformanceSummary {
                surface_skips: self.surface_skips,
                ..FramePerformanceSummary::default()
            };
        }
        let mut frame_times = self
            .samples
            .iter()
            .map(|sample| sample.frame_time)
            .collect::<Vec<_>>();
        frame_times.sort_unstable();
        FramePerformanceSummary {
            sample_count,
            frames_per_second: self.frames_per_second(),
            frame_time: Some(FrameTimeSummary {
                p50: percentile(&frame_times, 50),
                p95: percentile(&frame_times, 95),
                max: frame_times[sample_count - 1],
            }),
            average_advance: average_duration(
                self.samples
                    .iter()
                    .filter_map(|sample| sample.phases.advance),
            ),
            average_extract: average_duration(
                self.samples
                    .iter()
                    .filter_map(|sample| sample.phases.extract),
            ),
            average_render: average_duration(
                self.samples
                    .iter()
                    .filter_map(|sample| sample.phases.render),
            ),
            frames_over_60_fps_budget: frame_times
                .iter()
                .filter(|duration| **duration > SIXTY_FPS_BUDGET)
                .count(),
            frames_over_30_fps_budget: frame_times
                .iter()
                .filter(|duration| **duration > THIRTY_FPS_BUDGET)
                .count(),
            surface_skips: self.surface_skips,
        }
    }
}

fn percentile(sorted: &[Duration], percentage: usize) -> Duration {
    let rank = sorted.len().saturating_mul(percentage).div_ceil(100);
    sorted[rank.saturating_sub(1)]
}

fn average_duration(values: impl Iterator<Item = Duration>) -> Option<Duration> {
    let (total, count) = values.fold((Duration::ZERO, 0_u32), |(total, count), duration| {
        (total.saturating_add(duration), count.saturating_add(1))
    });
    (count > 0).then(|| total / count)
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use crate::SkippedSurfaceFrame;

    use super::*;

    #[test]
    fn completed_frames_report_fps_percentiles_and_phase_averages() {
        let started = Instant::now();
        let mut monitor = FramePerformanceMonitor::with_capacity(8);
        monitor.record_completed_at(started, FramePhaseDurations::default());
        for (index, millis) in [10, 20, 30, 40].into_iter().enumerate() {
            monitor.record_completed_at(
                started + Duration::from_millis(millis),
                FramePhaseDurations::new(
                    Duration::from_millis((index + 1) as u64),
                    Duration::from_millis(2),
                    Duration::from_millis(3),
                ),
            );
        }

        let summary = monitor.summary();
        assert_eq!(summary.sample_count(), 4);
        assert_eq!(summary.frames_per_second(), Some(100));
        assert_eq!(
            summary.frame_time(),
            Some(FrameTimeSummary {
                p50: Duration::from_millis(10),
                p95: Duration::from_millis(10),
                max: Duration::from_millis(10),
            })
        );
        assert_eq!(
            summary.average_advance(),
            Some(Duration::from_micros(2_500))
        );
        assert_eq!(summary.average_extract(), Some(Duration::from_millis(2)));
        assert_eq!(summary.average_render(), Some(Duration::from_millis(3)));
    }

    #[test]
    fn rolling_window_evicts_old_samples_and_tracks_long_frames() {
        let started = Instant::now();
        let mut monitor = FramePerformanceMonitor::with_capacity(3);
        monitor.record_completed_at(started, FramePhaseDurations::default());
        for elapsed in [10, 30, 70, 110] {
            monitor.record_completed_at(
                started + Duration::from_millis(elapsed),
                FramePhaseDurations::default(),
            );
        }

        let summary = monitor.summary();
        assert_eq!(summary.sample_count(), 3);
        assert_eq!(
            summary.frame_time(),
            Some(FrameTimeSummary {
                p50: Duration::from_millis(40),
                p95: Duration::from_millis(40),
                max: Duration::from_millis(40),
            })
        );
        assert_eq!(summary.frames_over_60_fps_budget(), 3);
        assert_eq!(summary.frames_over_30_fps_budget(), 2);
    }

    #[test]
    fn skipped_surface_frames_do_not_become_completed_samples() {
        let mut monitor = FramePerformanceMonitor::with_capacity(4);
        monitor.record_surface_skip(SkippedSurfaceFrame::Timeout);
        monitor.record_surface_skip(SkippedSurfaceFrame::Outdated);
        monitor.record_surface_skip(SkippedSurfaceFrame::Occluded);
        monitor.record_surface_skip(SkippedSurfaceFrame::ZeroSize);

        let summary = monitor.summary();
        assert_eq!(summary.sample_count(), 0);
        assert_eq!(summary.frames_per_second(), None);
        assert_eq!(
            summary.surface_skips(),
            SurfaceSkipCounters {
                zero_size: 1,
                timeout: 1,
                occluded: 1,
                outdated: 1,
            }
        );
    }

    #[test]
    fn unmeasured_phases_remain_absent_in_the_summary() {
        let started = Instant::now();
        let mut monitor = FramePerformanceMonitor::with_capacity(4);
        monitor.record_completed_at(started, FramePhaseDurations::default());
        monitor.record_completed_at(
            started + Duration::from_millis(16),
            FramePhaseDurations::play(Duration::from_millis(2), Duration::from_millis(1)),
        );

        let summary = monitor.summary();
        assert_eq!(summary.average_advance(), Some(Duration::from_millis(2)));
        assert_eq!(summary.average_extract(), Some(Duration::from_millis(1)));
        assert_eq!(summary.average_render(), None);
    }

    #[test]
    fn reset_separates_backend_sessions() {
        let started = Instant::now();
        let mut monitor = FramePerformanceMonitor::with_capacity(4);
        monitor.record_completed_at(started, FramePhaseDurations::default());
        monitor.record_completed_at(
            started + Duration::from_millis(16),
            FramePhaseDurations::default(),
        );
        monitor.record_surface_skip(SkippedSurfaceFrame::Timeout);

        monitor.reset();

        assert_eq!(monitor.summary(), FramePerformanceSummary::default());
    }
}
