use std::time::Duration;

pub fn hertz(hz: f64) -> Duration {
    Duration::from_secs_f64(1.0 / hz)
}
