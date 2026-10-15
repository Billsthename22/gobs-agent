//! Inert stand-in for location services on unsupported platforms.

pub const GPS_SUPPORTED: bool = false;
pub const GPS_UNAVAILABLE_REASON: &str = "GPS location is not supported on this platform";

pub struct LocationManager;

impl LocationManager {
    pub fn new() -> Self {
        Self
    }

    pub fn start(&self) {}

    pub fn current_location(&self) -> Option<(f64, f64)> {
        None
    }

    pub fn stop(&self) {}
}
