//! Lightweight frame profiler.
//!
//! GPUI draws on demand: while cards animate the window redraws every frame,
//! and when nothing moves it stays idle. The profiler therefore measures
//! *bursts* of continuous frames (gaps over 250 ms start a new burst) and
//! reports, per burst window:
//!
//! * frames per second,
//! * frame interval average / 95th percentile / worst (smoothness),
//! * time spent building the element tree in `render` (CPU-side UI cost),
//! * dropped frames (interval over 25 ms, i.e. below 40 fps).
//!
//! It deliberately cannot see GPU time. For that, use the platform tools
//! listed in PROFILING.md (Metal HUD, RenderDoc, Vulkan overlay, PIX).
//!
//! Set `SOLITARITE_PROFILE=1` to also log a summary line to stderr about once
//! per second of continuous animation.

use std::collections::VecDeque;
use std::time::{Duration, Instant};

const WINDOW: usize = 180;
const BURST_GAP: Duration = Duration::from_millis(250);
const DROPPED_FRAME_MS: f32 = 25.0;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Summary {
    pub fps: f32,
    pub interval_avg_ms: f32,
    pub interval_p95_ms: f32,
    pub interval_max_ms: f32,
    pub build_avg_ms: f32,
    pub build_max_ms: f32,
    pub dropped: u32,
    pub frames: u32,
}

pub struct PerfStats {
    last_frame: Option<Instant>,
    intervals: VecDeque<f32>,
    builds: VecDeque<f32>,
    dropped_in_window: u32,
    log: bool,
    last_log: Instant,
}

impl PerfStats {
    pub fn new() -> Self {
        Self {
            last_frame: None,
            intervals: VecDeque::with_capacity(WINDOW),
            builds: VecDeque::with_capacity(WINDOW),
            dropped_in_window: 0,
            log: std::env::var_os("SOLITARITE_PROFILE").is_some_and(|value| value != "0"),
            last_log: Instant::now(),
        }
    }

    /// Record one rendered frame. `build` is how long `render` spent building
    /// the element tree.
    pub fn record(&mut self, now: Instant, build: Duration) {
        if let Some(last) = self.last_frame {
            let gap = now.duration_since(last);
            if gap > BURST_GAP {
                // Idle gap: start a fresh burst so old data does not skew it.
                self.intervals.clear();
                self.builds.clear();
                self.dropped_in_window = 0;
            } else {
                let ms = gap.as_secs_f32() * 1000.0;
                push_capped(&mut self.intervals, ms);
                if ms > DROPPED_FRAME_MS {
                    self.dropped_in_window += 1;
                }
            }
        }
        push_capped(&mut self.builds, build.as_secs_f32() * 1000.0);
        self.last_frame = Some(now);

        if self.log && now.duration_since(self.last_log) >= Duration::from_secs(1) {
            self.last_log = now;
            let s = self.summary();
            if s.frames > 3 {
                eprintln!(
                    "[solitarite perf] {:.0} fps | interval avg {:.1} p95 {:.1} max {:.1} ms | build avg {:.2} max {:.2} ms | dropped {}",
                    s.fps,
                    s.interval_avg_ms,
                    s.interval_p95_ms,
                    s.interval_max_ms,
                    s.build_avg_ms,
                    s.build_max_ms,
                    s.dropped,
                );
            }
        }
    }

    pub fn summary(&self) -> Summary {
        let frames = self.intervals.len() as u32;
        if self.intervals.is_empty() {
            return Summary {
                build_avg_ms: average(&self.builds),
                build_max_ms: maximum(&self.builds),
                ..Summary::default()
            };
        }
        let avg = average(&self.intervals);
        let mut sorted: Vec<f32> = self.intervals.iter().copied().collect();
        sorted.sort_by(|a, b| a.total_cmp(b));
        let p95 = sorted[((sorted.len() as f32 * 0.95) as usize).min(sorted.len() - 1)];
        Summary {
            fps: if avg > 0.0 { 1000.0 / avg } else { 0.0 },
            interval_avg_ms: avg,
            interval_p95_ms: p95,
            interval_max_ms: maximum(&self.intervals),
            build_avg_ms: average(&self.builds),
            build_max_ms: maximum(&self.builds),
            dropped: self.dropped_in_window,
            frames,
        }
    }

    /// True when frames have been arriving continuously (an animation burst).
    pub fn is_active(&self, now: Instant) -> bool {
        self.last_frame
            .is_some_and(|last| now.duration_since(last) <= BURST_GAP)
    }
}

fn push_capped(queue: &mut VecDeque<f32>, value: f32) {
    if queue.len() == WINDOW {
        queue.pop_front();
    }
    queue.push_back(value);
}

fn average(queue: &VecDeque<f32>) -> f32 {
    if queue.is_empty() {
        0.0
    } else {
        queue.iter().sum::<f32>() / queue.len() as f32
    }
}

fn maximum(queue: &VecDeque<f32>) -> f32 {
    queue.iter().copied().fold(0.0, f32::max)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn steady_sixty_fps_reports_sixty_fps_and_no_drops() {
        let mut stats = PerfStats::new();
        let start = Instant::now();
        for i in 0..120u64 {
            stats.record(start + Duration::from_micros(i * 16_667), Duration::from_micros(900));
        }
        let s = stats.summary();
        assert!((s.fps - 60.0).abs() < 1.0, "fps {}", s.fps);
        assert_eq!(s.dropped, 0);
        assert!(s.build_avg_ms > 0.8 && s.build_avg_ms < 1.0);
    }

    #[test]
    fn slow_frames_are_counted_as_dropped() {
        let mut stats = PerfStats::new();
        let start = Instant::now();
        let mut t = start;
        for i in 0..40u64 {
            t += Duration::from_millis(if i % 10 == 9 { 60 } else { 16 });
            stats.record(t, Duration::from_millis(1));
        }
        let s = stats.summary();
        assert_eq!(s.dropped, 4);
        assert!(s.interval_max_ms >= 59.0);
        assert!(s.interval_p95_ms >= s.interval_avg_ms);
    }

    #[test]
    fn idle_gap_starts_a_new_burst() {
        let mut stats = PerfStats::new();
        let start = Instant::now();
        stats.record(start, Duration::from_millis(1));
        stats.record(start + Duration::from_millis(16), Duration::from_millis(1));
        stats.record(start + Duration::from_secs(5), Duration::from_millis(1));
        assert_eq!(stats.summary().frames, 0);
    }
}
