//! Reproduction test for the production screen-stream pipeline: real screen
//! capture → RGBA→I420 → OpenH264, comparing OpenH264's authoritative
//! frame_type() against the NAL-scan keyframe detection used in video.rs.
//!
//! Requires Screen Recording permission on macOS.

use openh264::encoder::FrameType;
use openh264::formats::RgbaSliceU8;

use agent::video::{annexb_nals, capture_frame_rgba, create_encoder, nal_type};

const NAL_IDR: u8 = 5;
const FPS: u32 = 10;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("capturing real screen…");

    let (w, h, rgba) = capture_frame_rgba()?;
    println!("screen: {}x{}", w, h);

    let mut active = create_encoder(w, h)?;

    println!("frame | openh264 frame_type | nal-scan types      | nal->key | match");
    println!("------+----------------------+---------------------+----------+------");

    let mut mismatches = 0usize;

    for frame in 0..60u32 {
        let (w2, h2, rgba) = capture_frame_rgba()?;

        // Resolution change → recreate encoder, exactly like the stream loop.
        if (w2, h2) != (w, h) {
            println!("resolution changed to {}x{} — recreating encoder", w2, h2);
            active = create_encoder(w2, h2)?;
            continue;
        }

        active.yuv.read_rgba8(RgbaSliceU8::new(&rgba, (w2, h2)));

        let ts = openh264::Timestamp::from_millis((frame * 1000 / FPS) as u64);
        let bitstream = active.encoder.encode_at(&active.yuv, ts)?;
        let frame_type = bitstream.frame_type();
        let raw = bitstream.to_vec();

        let nals = annexb_nals(&raw);
        let types: Vec<u8> = nals.iter().map(|nal| nal_type(nal)).collect();
        let nal_is_key = types.contains(&NAL_IDR);

        let openh264_is_key = matches!(
            frame_type,
            FrameType::IDR | FrameType::I | FrameType::IPMixed
        );

        let pretty: Vec<String> = types.iter().map(|t| t.to_string()).collect();

        let ok = openh264_is_key == nal_is_key;
        if !ok {
            mismatches += 1;
        }

        if frame % 2 == 0 || !ok {
            println!(
                "{:>5} | {:<20?} | {:19} | {:<8} | {}",
                frame,
                frame_type,
                pretty.join(","),
                nal_is_key,
                if ok { "OK" } else { "*** MISMATCH ***" }
            );
        }

        if !ok {
            // Dump the raw frame for a mismatch so it can be inspected.
            println!(
                "  raw len={} first 40: {:02x?}",
                raw.len(),
                &raw[..raw.len().min(40)]
            );
        }
    }

    println!("\nmismatches: {}", mismatches);

    Ok(())
}
