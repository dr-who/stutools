//! Rate limits in Mbit/s, the unit `--cctv` uses for each camera and `--ratelimitmbps` uses for a fill.

/// Bytes per second for a rate in decimal Mbit/s, as network speeds are quoted.
pub fn bytes_per_second(mbps: f64) -> f64 {
    mbps * 1e6 / 8.
}
