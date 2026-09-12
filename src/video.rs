//! H.264 screen streaming for the GBOS device agent.
//!
//! Replaces the old periodic full-screen PNG grab with a real video pipeline:
//!
//!   1. capture the primary display as RGBA (via the `screenshots` crate);
//!   2. convert RGBA -> I420 and encode with OpenH264 (H.264 Annex-B output);
//!   3. re-package each encoded frame's NAL units as AVCC (4-byte length
//!      prefixes) — the format WebCodecs expects for `avc1` chunks;
//!   4. ship it to the backend over the existing device WebSocket as framed
//!      binary messages, where the backend already relays every binary message
//!      to authorized monitoring sessions.
//!
//! The capture + encode work is blocking CPU work, so it runs on a dedicated
//! `std::thread` that pushes [`StreamPacket`]s over a `tokio` unbounded channel.
//! The async websocket loop only ever forwards packets — it never captures.
//!
//! Binary wire format (every message):
//!
//! | offset | size | field                                          |
//! |--------|------|------------------------------------------------|
//! | 0      | 1    | kind: 1 = stream config, 2 = encoded video     |
//! | 1      | 1    | flags: bit 0 = keyframe                        |
//! | 2      | 4    | timestamp ms, big-endian, relative to stream   |
//! | 6      | ..   | payload (config: UTF-8 JSON; video: AVCC NALs) |
//!
//! `avc1` parameter sets live in the avcC config payload, not in video
//! samples; SPS/PPS NALs are therefore removed during AVCC conversion.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64;
use screenshots::Screen;
use tokio::sync::mpsc::UnboundedSender;

use openh264::OpenH264API;
use openh264::encoder::{
    BitRate, Complexity, Encoder, EncoderConfig, FrameRate, FrameType,
    IntraFramePeriod, Profile, RateControlMode, UsageType, VuiConfig,
};
use openh264::formats::{RgbaSliceU8, YUVBuffer};

// ---------------------------------------------------------------------------
// Wire protocol
// ---------------------------------------------------------------------------

/// Stream config message (payload is UTF-8 JSON).
pub const STREAM_KIND_CONFIG: u8 = 1;

/// Encoded video frame message (payload is one frame of AVCC H.264 NAL units).
pub const STREAM_KIND_VIDEO: u8 = 2;

/// Flag bit: the video payload starts a decodable point (IDR).
pub const STREAM_FLAG_KEYFRAME: u8 = 0x01;

/// H.264 NAL unit types we care about.
const NAL_IDR: u8 = 5;
const NAL_SPS: u8 = 7;
const NAL_PPS: u8 = 8;

const FPS: u32 = 10;
const FRAME_INTERVAL: Duration = Duration::from_millis(1000 / FPS as u64);

/// A single binary message the websocket loop forwards verbatim.
pub struct StreamPacket {
    pub kind: u8,
    pub keyframe: bool,
    pub timestamp_ms: u32,
    pub payload: Vec<u8>,
}

impl StreamPacket {
    /// Serialize to the on-wire binary form: 6-byte header + payload.
    pub fn into_binary(self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(self.payload.len() + 6);

        bytes.push(self.kind);

        let flags = if self.keyframe {
            STREAM_FLAG_KEYFRAME
        } else {
            0
        };

        bytes.push(flags);
        bytes.extend_from_slice(&self.timestamp_ms.to_be_bytes());
        bytes.extend_from_slice(&self.payload);

        bytes
    }
}

// ---------------------------------------------------------------------------
// NAL unit helpers
// ---------------------------------------------------------------------------

/// Split an Annex-B H.264 stream into NAL payloads (start codes stripped).
/// Handles 4- and 3-byte start codes and strips trailing zero bytes.
pub fn annexb_nals(data: &[u8]) -> Vec<&[u8]> {
    // Store both the payload start and the start-code offset. The latter is
    // the end of the preceding NAL. Using the next payload offset instead
    // accidentally appends that NAL's `00 00 {00} 01` marker to the previous
    // AVCC unit, which makes the browser's H.264 decoder fail.
    let mut starts: Vec<(usize, usize)> = Vec::new();
    let mut j = 0usize;

    while j + 3 < data.len() {
        let is3 = data[j] == 0 && data[j + 1] == 0 && data[j + 2] == 1;
        let is4 = j + 4 <= data.len()
            && data[j] == 0
            && data[j + 1] == 0
            && data[j + 2] == 0
            && data[j + 3] == 1;

        if is4 {
            starts.push((j + 4, j));
            j += 4;
        } else if is3 {
            starts.push((j + 3, j));
            j += 3;
        } else {
            j += 1;
        }
    }

    let mut nals = Vec::with_capacity(starts.len());

    for (idx, &(start, _)) in starts.iter().enumerate() {
        let raw_end = starts
            .get(idx + 1)
            .map(|&(_, marker_start)| marker_start)
            .unwrap_or(data.len());

        // Trim trailing zero bytes (cabac_zero_word padding).
        let mut end = raw_end;

        while end > start && data[end - 1] == 0 {
            end -= 1;
        }

        if end > start {
            nals.push(&data[start..end]);
        }
    }

    nals
}

pub fn nal_type(nal: &[u8]) -> u8 {
    nal[0] & 0x1f
}

/// Re-package NAL units into AVCC (4-byte big-endian lengths), the chunk format
/// WebCodecs requires for `avc1` codecs.
fn to_avcc(nals: &[&[u8]], out: &mut Vec<u8>) {
    for nal in nals {
        out.extend_from_slice(&(nal.len() as u32).to_be_bytes());
        out.extend_from_slice(nal);
    }
}

// ---------------------------------------------------------------------------
// Capture
// ---------------------------------------------------------------------------

/// Capture the primary display as (width, height, RGBA bytes). Width/height are
/// guaranteed even, which the encoder requires.
pub fn capture_frame_rgba() -> Result<(usize, usize, Vec<u8>), Box<dyn std::error::Error>> {
    let screens = Screen::all()?;

    let screen = screens.first().ok_or("No display found")?;

    let image = screen.capture()?;

    let (w, h) = (image.width() as usize, image.height() as usize);
    let raw = image.into_raw();

    // Crop the last column / row when a dimension is odd so the encoder (and the
    // YUV 4:2:0 conversion) never sees an unrepresentable size.
    if w % 2 == 0 && h % 2 == 0 {
        Ok((w, h, raw))
    } else {
        let w2 = w - (w % 2);
        let h2 = h - (h % 2);
        let mut cropped = Vec::with_capacity(w2 * h2 * 4);

        for y in 0..h2 {
            let start = (y * w) * 4;
            cropped.extend_from_slice(&raw[start..start + w2 * 4]);
        }

        Ok((w2, h2, cropped))
    }
}

// ---------------------------------------------------------------------------
// Encoder
// ---------------------------------------------------------------------------

pub struct ActiveEncoder {
    pub width: usize,
    pub height: usize,
    pub encoder: Encoder,
    pub yuv: YUVBuffer,
}

/// Describes the stream in the JSON config message the viewer needs before it
/// can configure its decoder (codec string + avcC `description`).
struct StreamConfig {
    width: usize,
    height: usize,
    codec: String,
    description_b64: String,
    description_len: usize,
}

impl StreamConfig {
    fn to_json(&self) -> String {
        let value = serde_json::json!({
            "w": self.width,
            "h": self.height,
            "fps": FPS,
            "codec": self.codec,
            "description": self.description_b64,
        });

        value.to_string()
    }
}

/// Pick a sensible bitrate for the captured resolution (scaled by area, capped).
fn bitrate_for(width: usize, height: usize) -> u32 {
    let reference_area = (1280 * 720) as f64;
    let area = (width * height) as f64;

    let scaled = 2_500_000.0 * (area / reference_area);

    scaled.clamp(1_000_000.0, 8_000_000.0) as u32
}

pub fn create_encoder(
    width: usize,
    height: usize,
) -> Result<ActiveEncoder, Box<dyn std::error::Error>> {
    let api = OpenH264API::from_source();

    let config = EncoderConfig::new()
        .bitrate(BitRate::from_bps(bitrate_for(width, height)))
        .max_frame_rate(FrameRate::from_hz(FPS as f32))
        .usage_type(UsageType::ScreenContentRealTime)
        .rate_control_mode(RateControlMode::Bitrate)
        .profile(Profile::Baseline)
        .complexity(Complexity::Low)
        // Screen-content usage forces these off internally; set them off up
        // front so OpenH264 doesn't log parameter-validation warnings.
        .adaptive_quantization(false)
        .background_detection(false)
        // Scene-change detection is enabled by default for screen content; that
        // plus the periodic intra period below guarantees regular keyframes.
        .intra_frame_period(IntraFramePeriod::from_num_frames(FPS * 2))
        // VideoToolbox-style full-range BT.709 for computer graphics.
        .vui(VuiConfig::bt709_full());

    let encoder = Encoder::with_api_config(api, config)?;

    let yuv = YUVBuffer::new(width, height);

    Ok(ActiveEncoder {
        width,
        height,
        encoder,
        yuv,
    })
}

/// Build the codec string (`avc1.PPCCLL`) and avcC description bytes from the
/// SPS/PPS NAL payloads of an IDR frame.
fn describe_stream(sps: &[u8], pps: &[u8]) -> StreamConfig {
    debug_assert!(sps.len() >= 4, "SPS is shorter than its header");
    debug_assert!(sps[0] & 0x1f == NAL_SPS, "not an SPS NAL");
    debug_assert!(pps.len() >= 2, "PPS is shorter than its header");
    debug_assert!(pps[0] & 0x1f == NAL_PPS, "not a PPS NAL");

    // NAL header byte is sps[0]; profile/constraint/level follow it.
    let codec = format!("avc1.{:02x}{:02x}{:02x}", sps[1], sps[2], sps[3]);

    // avcC stores complete SPS/PPS NAL units, including their one-byte NAL
    // headers (0x67 / 0x68). Omitting those headers produces a malformed
    // decoder description: Chromium then cannot validate the IDR NAL in a
    // chunk marked as a key frame and rejects it with DataError.
    let mut avcc = Vec::with_capacity(6 + sps.len() + 3 + pps.len());

    avcc.push(0x01); // configurationVersion
    avcc.push(sps[1]); // AVCProfileIndication
    avcc.push(sps[2]); // profile_compatibility
    avcc.push(sps[3]); // AVCLevelIndication
    avcc.push(0xff); // 6 reserved bits + lengthSizeMinusOne = 3 (4-byte lengths)
    avcc.push(0xe1); // 3 reserved bits + numOfSequenceParameterSets = 1
    avcc.extend_from_slice(&(sps.len() as u16).to_be_bytes());
    avcc.extend_from_slice(sps);
    avcc.push(0x01); // numOfPictureParameterSets = 1
    avcc.extend_from_slice(&(pps.len() as u16).to_be_bytes());
    avcc.extend_from_slice(pps);

    StreamConfig {
        width: 0,
        height: 0,
        codec,
        description_len: avcc.len(),
        description_b64: BASE64.encode(&avcc),
    }
}

// ---------------------------------------------------------------------------
// Streaming thread
// ---------------------------------------------------------------------------

/// Handle to a running screen-stream thread.
///
/// Dropping or calling [`stop`](VideoStream::stop) signals the thread to exit on
/// its next loop iteration (at most ~100 ms later). The thread is joined by a
/// small helper thread so a stop never blocks the async websocket loop.
pub struct VideoStream {
    stop: Arc<AtomicBool>,
    join: Option<std::thread::JoinHandle<()>>,
}

impl VideoStream {
    /// Start capturing + encoding the screen and pushing packets onto `sender`.
    pub fn start(
        sender: UnboundedSender<StreamPacket>,
    ) -> std::io::Result<VideoStream> {
        let stop = Arc::new(AtomicBool::new(false));
        let thread_stop = Arc::clone(&stop);

        let join = std::thread::Builder::new()
            .name("gbos-screen-stream".to_string())
            .spawn(move || run_stream(thread_stop, sender))?;

        Ok(VideoStream {
            stop,
            join: Some(join),
        })
    }

    /// Signal the streaming thread to stop and detach (never blocks the caller).
    pub fn stop(&mut self) {
        if self.join.is_none() {
            return;
        }

        self.stop.store(true, Ordering::Relaxed);

        if let Some(join) = self.join.take() {
            // Join off the async runtime: the encoder thread may be mid-capture.
            std::thread::spawn(move || {
                let _ = join.join();
            });
        }
    }
}

impl Drop for VideoStream {
    fn drop(&mut self) {
        self.stop();
    }
}

fn run_stream(stop: Arc<AtomicBool>, sender: UnboundedSender<StreamPacket>) {
    let started = Instant::now();

    // (width, height, encoder state) for the current capture resolution.
    let mut active: Option<ActiveEncoder> = None;

    // (width, height, config payload) of the config message currently in
    // effect. Re-sent on every periodic keyframe so a viewer joining or
    // reconnecting mid-stream can configure its decoder within one intra
    // period (~2 s), not just at stream start.
    let mut stored_config: Option<(usize, usize, Vec<u8>)> = None;

    // Frame index the last config message was emitted for.
    let mut last_config_frame: u64 = 0;

    let mut frame_index: u64 = 0;

    while !stop.load(Ordering::Relaxed) {
        let tick_start = Instant::now();

        // 1) Capture.
        let (w, h, rgba) = match capture_frame_rgba() {
            Ok(frame) => frame,
            Err(error) => {
                println!("Screen capture failed ❌");
                println!("Error: {}", error);

                // Screen Recording permission may be pending; retry slowly.
                std::thread::sleep(Duration::from_millis(500));
                continue;
            }
        };

        // 2) (Re)create the encoder when the display size changes.
        if active
            .as_ref()
            .map(|encoder| (encoder.width, encoder.height))
            != Some((w, h))
        {
            match create_encoder(w, h) {
                Ok(encoder) => {
                    println!(
                        "Screen encoder ready 🖥️ {}x{} @ {}fps (H.264)",
                        w, h, FPS
                    );

                    active = Some(encoder);
                    stored_config = None;
                }
                Err(error) => {
                    println!("Screen encoder init failed ❌");
                    println!("Error: {}", error);

                    std::thread::sleep(Duration::from_millis(500));
                    continue;
                }
            }
        }

        let encoder = match active.as_mut() {
            Some(encoder) => encoder,
            None => continue,
        };

        // 3) RGBA -> I420, then encode.
        encoder.yuv.read_rgba8(RgbaSliceU8::new(&rgba, (w, h)));

        let timestamp_ms = ((frame_index * 1000) / FPS as u64) as u32;
        let ts = openh264::Timestamp::from_millis(timestamp_ms as u64);

        let bitstream = match encoder.encoder.encode_at(&encoder.yuv, ts) {
            Ok(bitstream) => bitstream,
            Err(error) => {
                println!("Screen encode failed ❌");
                println!("Error: {:?}", error);
                continue;
            }
        };

        frame_index += 1;

        // OpenH264's frame_type() is the authoritative record of what the
        // encoder produced; the NAL scan below is a cross-check of what is
        // actually in the payload (which is what Chrome's WebCodecs decoder
        // verifies). A chunk is only flagged KEY when both signals agree an
        // IDR is present — flagging a chunk key without an IDR NAL makes the
        // browser reject it with "marked as type key but wasn't a key frame".
        let frame_type = bitstream.frame_type();

        let annexb = bitstream.to_vec();
        let nals = annexb_nals(&annexb);

        if nals.is_empty() {
            continue;
        }

        let types: Vec<u8> = nals.iter().map(|nal| nal_type(nal)).collect();
        let nals_contain_idr = types.contains(&NAL_IDR);

        let openh264_is_key = matches!(
            frame_type,
            FrameType::IDR | FrameType::I | FrameType::IPMixed
        );
        let keyframe = openh264_is_key && nals_contain_idr;

        if openh264_is_key != nals_contain_idr {
            println!(
                "⚠️  frame-type disagreement: openh264={:?} nal-scan={} → \
                 classifying as {}",
                frame_type,
                nals_contain_idr,
                if keyframe { "KEY" } else { "delta" }
            );
        }

        // 4) Config emission.
        //
        // Emit a fresh config message whenever an IDR keyframe carries SPS/PPS
        // for a new resolution (encoder restart). Additionally, re-send the
        // current config ahead of every ~2 s periodic keyframe, so a viewer
        // that joins or reconnects mid-stream receives the avcC description in
        // time to decode that keyframe — not just at stream start.
        if keyframe {
            let has_parameters =
                types.contains(&NAL_SPS) && types.contains(&NAL_PPS);

            let resolution_changed = stored_config
                .as_ref()
                .map(|stored| (stored.0, stored.1))
                != Some((w, h));

            let rebuild = has_parameters
                && (stored_config.is_none() || resolution_changed);

            let resend = !rebuild
                && stored_config.is_some()
                && frame_index.saturating_sub(last_config_frame)
                    >= FPS as u64;

            if rebuild {
                let sps = nals
                    .iter()
                    .find(|nal| nal_type(nal) == NAL_SPS)
                    .copied();

                let pps = nals
                    .iter()
                    .find(|nal| nal_type(nal) == NAL_PPS)
                    .copied();

                if let (Some(sps), Some(pps)) = (sps, pps) {
                    let mut config = describe_stream(sps, pps);
                    config.width = w;
                    config.height = h;

                    let payload = config.to_json().into_bytes();

                    let packet = StreamPacket {
                        kind: STREAM_KIND_CONFIG,
                        keyframe: false,
                        timestamp_ms: 0,
                        payload: payload.clone(),
                    };

                    if sender.send(packet).is_err() {
                        break; // websocket loop is gone
                    }

                    stored_config = Some((w, h, payload));

                    println!(
                        "Screen stream config sent 📋 ({}x{} {}, avcC {} bytes)",
                        w, h, config.codec, config.description_len
                    );
                }
            } else if resend {
                if let Some((_, _, payload)) = stored_config.as_ref() {
                    let packet = StreamPacket {
                        kind: STREAM_KIND_CONFIG,
                        keyframe: false,
                        timestamp_ms: 0,
                        payload: payload.clone(),
                    };

                    if sender.send(packet).is_err() {
                        break; // websocket loop is gone
                    }

                    println!("Screen stream config re-sent 🔁 (periodic)");
                }
            }

            if rebuild || resend {
                last_config_frame = frame_index;
            }
        }

        // 5) AVCC conversion + ship the frame.
        //
        // `avc1` uses an out-of-band avcC description for SPS/PPS. OpenH264
        // repeats those parameter sets before IDRs in its Annex-B output, but
        // keeping them in an `avc1` sample makes Chromium's decoder reject
        // the access unit. The description emitted above remains available to
        // every viewer before its corresponding keyframe.
        let video_nals: Vec<&[u8]> = nals
            .iter()
            .copied()
            .filter(|nal| {
                let kind = nal_type(nal);
                kind != NAL_SPS && kind != NAL_PPS
            })
            .collect();

        if video_nals.is_empty() {
            continue;
        }

        let mut avcc = Vec::with_capacity(annexb.len() + 8);
        to_avcc(&video_nals, &mut avcc);

        let packet = StreamPacket {
            kind: STREAM_KIND_VIDEO,
            keyframe,
            timestamp_ms,
            payload: avcc,
        };

        if sender.send(packet).is_err() {
            break;
        }

        if frame_index % (FPS as u64 * 2) == 0 {
            let kind = if keyframe { "KEY" } else { "P  " };

            println!(
                "Screen frame sent 🖥️ [{}] {} ms ({:.1} KB)",
                kind,
                timestamp_ms,
                annexb.len() as f64 / 1024.0
            );
        }

        // 6) Pace to the target frame rate (sleep-corrected by elapsed time).
        let elapsed = tick_start.elapsed();

        if elapsed < FRAME_INTERVAL {
            std::thread::sleep(FRAME_INTERVAL - elapsed);
        }
    }

    println!(
        "Screen stream stopped ({} frames over {:.1}s)",
        frame_index,
        started.elapsed().as_secs_f32()
    );
}

#[cfg(test)]
mod tests {
    use super::annexb_nals;

    #[test]
    fn splits_nals_without_copying_the_next_start_code() {
        let stream = [
            0, 0, 0, 1, 0x67, 0xaa, // four-byte Annex-B marker + SPS
            0, 0, 1, 0x68, 0xbb, // three-byte Annex-B marker + PPS
            0, 0, 0, 1, 0x65, 0xcc, // four-byte Annex-B marker + IDR
        ];

        assert_eq!(annexb_nals(&stream), vec![&[0x67, 0xaa][..], &[0x68, 0xbb], &[0x65, 0xcc]]);
    }
}
