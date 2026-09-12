//! Diagnostic: inspect what OpenH264 actually emits with the agent's exact
//! encoder config, and compare OpenH264's authoritative `frame_type()` against
//! the NAL-unit scan used in video.rs for keyframe detection.

use openh264::encoder::{
    BitRate, Complexity, Encoder, EncoderConfig, FrameRate, IntraFramePeriod,
    Profile, RateControlMode, UsageType, VuiConfig,
};
use openh264::formats::YUVBuffer;

const FPS: u32 = 10;

const NAL_IDR: u8 = 5;
const NAL_SPS: u8 = 7;
const NAL_PPS: u8 = 8;

fn annexb_nals(data: &[u8]) -> Vec<&[u8]> {
    let mut starts: Vec<usize> = Vec::new();
    let mut j = 0usize;

    while j + 3 < data.len() {
        let is3 = data[j] == 0 && data[j + 1] == 0 && data[j + 2] == 1;
        let is4 = j + 4 <= data.len()
            && data[j] == 0
            && data[j + 1] == 0
            && data[j + 2] == 0
            && data[j + 3] == 1;

        if is4 {
            starts.push(j + 4);
            j += 4;
        } else if is3 {
            starts.push(j + 3);
            j += 3;
        } else {
            j += 1;
        }
    }

    let mut nals = Vec::with_capacity(starts.len());

    for (idx, &start) in starts.iter().enumerate() {
        let raw_end = starts.get(idx + 1).copied().unwrap_or(data.len());
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

fn nal_type(nal: &[u8]) -> u8 {
    nal[0] & 0x1f
}

fn make_yuv(w: usize, h: usize, frame: u32, moving: bool) -> YUVBuffer {
    let mut data = vec![0u8; w * h];

    for y in 0..h {
        for x in 0..w {
            let value = if moving {
                (x as u32 / 2 + y as u32 + frame * 7) % 255
            } else {
                128
            };
            data[y * w + x] = value as u8;
        }
    }

    data.extend(std::iter::repeat_n(128u8, w * h / 2));

    YUVBuffer::from_vec(data, w, h)
}

fn run(moving: bool, w: usize, h: usize) -> Result<(), Box<dyn std::error::Error>> {
    let config = EncoderConfig::new()
        .bitrate(BitRate::from_bps(2_500_000))
        .max_frame_rate(FrameRate::from_hz(FPS as f32))
        .usage_type(UsageType::ScreenContentRealTime)
        .rate_control_mode(RateControlMode::Bitrate)
        .profile(Profile::Baseline)
        .complexity(Complexity::Low)
        .adaptive_quantization(false)
        .background_detection(false)
        .intra_frame_period(IntraFramePeriod::from_num_frames(FPS * 2))
        .vui(VuiConfig::bt709_full());

    let mut encoder = Encoder::with_api_config(openh264::OpenH264API::from_source(), config)?;

    println!(
        "\n===== content: {} | {}x{} =====",
        if moving { "MOVING" } else { "STATIC" },
        w,
        h
    );
    println!("frame | openh264 frame_type | nal-scan types        | nal->key | match");
    println!("------+----------------------+-----------------------+----------+------");

    for frame in 0..30u32 {
        let yuv = make_yuv(w, h, frame, moving);

        let ts = openh264::Timestamp::from_millis((frame * 1000 / FPS) as u64);
        let bitstream = encoder.encode_at(&yuv, ts)?;
        let frame_type = bitstream.frame_type();
        let raw = bitstream.to_vec();

        let nals = annexb_nals(&raw);
        let types: Vec<u8> = nals.iter().map(|nal| nal_type(nal)).collect();
        let has_idr = types.contains(&NAL_IDR);

        let openh264_is_key = matches!(frame_type, openh264::encoder::FrameType::IDR);
        let nal_is_key = has_idr;

        let pretty_types: Vec<String> = types.iter().map(|t| t.to_string()).collect();

        println!(
            "{:>5} | {:<20?} | {:32} | {:<8} | {}",
            frame,
            frame_type,
            pretty_types.join(","),
            nal_is_key,
            if openh264_is_key == nal_is_key {
                "OK"
            } else {
                "MISMATCH"
            }
        );

        if frame == 0 {
            let idr_idx = types.iter().position(|&t| t == NAL_IDR);
            println!("\n--- frame 0 hex dump (first 64 bytes) ---");
            for (i, b) in raw.iter().take(64).enumerate() {
                print!("{:02x} ", b);
                if (i + 1) % 16 == 0 {
                    println!();
                }
            }
            println!();
            println!("sps={} pps={} idr_nals={:?}", types.contains(&NAL_SPS), types.contains(&NAL_PPS), idr_idx);
        }
    }

    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    run(true, 320, 240)?;
    run(false, 320, 240)?;
    // Production-like Retina screen size — large frames get split into
    // multiple slices, which exercises the NAL boundary handling harder.
    run(true, 1512, 982)?;
    Ok(())
}
