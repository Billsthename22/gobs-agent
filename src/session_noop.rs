//! The per-session helper interface on platforms that do not have one.
//!
//! Only Windows needs this indirection. On macOS the agent is a single process
//! that either captures its own screen and location or, as a root LaunchDaemon,
//! captures neither — there is no user session to reach into, so there is
//! nothing to delegate to.
//!
//! This exists so [`SessionBridge`] can be named unconditionally in the event
//! loop. The alternative — `#[cfg]` on every call site — would spread the
//! platform question across the loop instead of keeping it in one place, and the
//! loop is the part that has to stay readable.
//!
//! Every method here is inert, and [`SessionBridge::delegates`] returns `false`,
//! so the loop's "a helper owns this" branches are never taken and the
//! in-process capture path runs exactly as it did before the bridge existed.

use tokio::sync::mpsc;

use crate::video;

/// Inert stand-in for the Windows session bridge.
pub struct SessionBridge;

impl SessionBridge {
    /// Never delegates on this platform.
    pub fn new(_screen_tx: mpsc::UnboundedSender<video::StreamPacket>) -> Self {
        Self
    }

    /// Never delegates on this platform.
    pub fn delegates(&self) -> bool {
        false
    }

    /// Does nothing: there is no helper to keep in step with the session.
    pub fn tick(&mut self) {}

    /// Does nothing: the event loop starts the in-process stream itself.
    pub fn set_screen(&mut self, _enabled: bool) {}

    /// Does nothing: the event loop drives the in-process location manager.
    pub fn set_gps(&mut self, _enabled: bool) {}

    /// Never has a fix: the event loop reads the in-process manager.
    pub fn take_fix(&self) -> Option<(f64, f64)> {
        None
    }

    /// Does nothing: there is no helper to stop.
    pub fn stop(&mut self) {}
}
