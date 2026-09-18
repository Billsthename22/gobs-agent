//! Fallback for platforms the agent does not manage as a service.
//!
//! The agent targets macOS and Windows. On anything else it still runs in the
//! foreground, but it has no service manager to register with.

pub const MESSAGE: &str = "This platform has no supported background-service integration; \
                           run the agent in the foreground instead.";
