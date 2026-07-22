use std::time::{Duration, Instant};

/// Injectable monotonic presentation clock. Simulation time remains owned by
/// `game-app`; this clock only paces an explicit manual tick batch.
pub trait Clock {
    fn now(&self) -> Duration;
}

#[derive(Debug)]
pub struct MonotonicClock {
    origin: Instant,
}

impl Default for MonotonicClock {
    fn default() -> Self {
        Self {
            origin: Instant::now(),
        }
    }
}

impl Clock for MonotonicClock {
    fn now(&self) -> Duration {
        self.origin.elapsed()
    }
}

#[cfg(test)]
#[derive(Clone, Debug, Default)]
pub struct FakeClock(std::rc::Rc<std::cell::Cell<Duration>>);

#[cfg(test)]
impl FakeClock {
    pub fn advance(&self, duration: Duration) {
        self.0.set(self.0.get().saturating_add(duration));
    }
}

#[cfg(test)]
impl Clock for FakeClock {
    fn now(&self) -> Duration {
        self.0.get()
    }
}
