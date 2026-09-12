//! GBOS device agent.
//!
//! The binary entrypoint is `main.rs`; `video.rs` is exposed as a library
//! module so diagnostics under `src/bin/` can exercise the real streaming
//! pipeline (capture → encode → NAL parsing) end to end.

pub mod video;
