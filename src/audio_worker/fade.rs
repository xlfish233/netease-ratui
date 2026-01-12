use rodio::Sink;
use std::sync::Arc;
use std::time::{Duration, Instant};

pub(super) struct Crossfade {
    from: Arc<Sink>,
    to: Arc<Sink>,
    start: Instant,
    duration: Duration,
    paused_at: Option<Instant>,
    paused_total: Duration,
    last_ratio: f32,
}

impl Crossfade {
    pub(super) fn new(from: Arc<Sink>, to: Arc<Sink>, duration_ms: u64) -> Self {
        let duration = Duration::from_millis(duration_ms.max(1));
        Self {
            from,
            to,
            start: Instant::now(),
            duration,
            paused_at: None,
            paused_total: Duration::ZERO,
            last_ratio: 0.0,
        }
    }

    pub(super) fn pause(&mut self) {
        if self.paused_at.is_none() {
            self.paused_at = Some(Instant::now());
        }
    }

    pub(super) fn resume(&mut self) {
        if let Some(at) = self.paused_at.take() {
            self.paused_total = self.paused_total.saturating_add(at.elapsed());
        }
    }

    pub(super) fn pause_sinks(&self) {
        self.from.pause();
        self.to.pause();
    }

    pub(super) fn resume_sinks(&self) {
        self.from.play();
        self.to.play();
    }

    pub(super) fn apply(&mut self, base_volume: f32) -> bool {
        let now = self.paused_at.unwrap_or_else(Instant::now);
        let elapsed = now
            .duration_since(self.start)
            .saturating_sub(self.paused_total);
        let t = (elapsed.as_secs_f32() / self.duration.as_secs_f32()).clamp(0.0, 1.0);
        self.last_ratio = t;
        self.from.set_volume(base_volume * (1.0 - t));
        self.to.set_volume(base_volume * t);
        if t >= 1.0 {
            self.from.stop();
            return true;
        }
        false
    }

    pub(super) fn stop(self) {
        self.from.stop();
    }
}
