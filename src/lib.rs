//! GBOS device agent.
//!
//! The binary entrypoint is `main.rs`; `video.rs` is exposed as a library
//! module so diagnostics under `src/bin/` can exercise the real streaming
//! pipeline (capture → encode → NAL parsing) end to end.
//!
//! `logging` lives here rather than in the binary for the same reason: the
//! diagnostics link against this library, and declaring the module in both
//! targets would compile two independent loggers.

pub mod logging;
pub mod video;
