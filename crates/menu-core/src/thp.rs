//! Nintendo Wii THP video format parser.
//! THP is the proprietary video format used by Nintendo for game intros, cutscenes,
//! and channel preview animations. It wraps motion JPEG-like video with DSP ADPCM audio.

use serde::{Deserialize, Serialize};

pub const THP_MAGIC: &[u8; 4] = b"THP\0";

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThpHeader {
    pub version: u32,
    pub max_buffer_size: u32,
    pub max_audio_samples: u32,
    pub fps: f32,
    pub num_frames: u32,
    pub first_frame_size: u32,
    pub data_size: u32,
    pub comp_info_offset: u32,
    pub offset_data_offset: u32,
    pub movie_data_offset: u32,
    pub final_frame_data_offset: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThpVideoInfo {
    pub width: u32,
    pub height: u32,
    pub video_format: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThpAudioInfo {
    pub channels: u32,
    pub sample_rate: u32,
    pub num_samples: u32,
    pub track_count: u32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThpMovie {
    pub header: ThpHeader,
    pub video: Option<ThpVideoInfo>,
    pub audio: Option<ThpAudioInfo>,
}

/// Parses the THP header and component descriptions from raw slice.
pub fn parse_thp_header(data: &[u8]) -> Result<ThpMovie, &'static str> {
    if data.len() < 0x30 {
        return Err("data slice too small for THP header");
    }
    if &data[0..4] != THP_MAGIC {
        return Err("invalid THP magic identifier (expected 'THP\\0')");
    }

    let version = u32::from_be_bytes(data[4..8].try_into().unwrap());
    let max_buffer_size = u32::from_be_bytes(data[8..12].try_into().unwrap());
    let max_audio_samples = u32::from_be_bytes(data[12..16].try_into().unwrap());
    let fps = f32::from_be_bytes(data[16..20].try_into().unwrap());
    let num_frames = u32::from_be_bytes(data[20..24].try_into().unwrap());
    let first_frame_size = u32::from_be_bytes(data[24..28].try_into().unwrap());
    let data_size = u32::from_be_bytes(data[28..32].try_into().unwrap());
    let comp_info_offset = u32::from_be_bytes(data[32..36].try_into().unwrap());
    let offset_data_offset = u32::from_be_bytes(data[36..40].try_into().unwrap());
    let movie_data_offset = u32::from_be_bytes(data[40..44].try_into().unwrap());
    let final_frame_data_offset = u32::from_be_bytes(data[44..48].try_into().unwrap());

    let header = ThpHeader {
        version,
        max_buffer_size,
        max_audio_samples,
        fps,
        num_frames,
        first_frame_size,
        data_size,
        comp_info_offset,
        offset_data_offset,
        movie_data_offset,
        final_frame_data_offset,
    };

    let mut video = None;
    let mut audio = None;

    let comp_off = comp_info_offset as usize;
    if comp_off > 0 && comp_off + 4 <= data.len() {
        let num_components = u32::from_be_bytes(data[comp_off..comp_off + 4].try_into().unwrap()) as usize;
        let mut info_ptr = comp_off + 20;

        for i in 0..num_components.min(4) {
            let comp_type_idx = comp_off + 4 + i;
            if comp_type_idx >= data.len() {
                break;
            }
            let comp_type = data[comp_type_idx];
            if comp_type == 0 {
                // Video component
                let stride = if version >= 0x00011000 { 12 } else { 8 };
                if info_ptr + stride <= data.len() {
                    let width = u32::from_be_bytes(data[info_ptr..info_ptr + 4].try_into().unwrap());
                    let height = u32::from_be_bytes(data[info_ptr + 4..info_ptr + 8].try_into().unwrap());
                    let video_format = if stride >= 12 {
                        u32::from_be_bytes(data[info_ptr + 8..info_ptr + 12].try_into().unwrap())
                    } else {
                        0
                    };
                    video = Some(ThpVideoInfo {
                        width,
                        height,
                        video_format,
                    });
                    info_ptr += stride;
                }
            } else if comp_type == 1 {
                // Audio component
                let stride = if version >= 0x00011000 { 16 } else { 12 };
                if info_ptr + stride <= data.len() {
                    let channels = u32::from_be_bytes(data[info_ptr..info_ptr + 4].try_into().unwrap());
                    let sample_rate = u32::from_be_bytes(data[info_ptr + 4..info_ptr + 8].try_into().unwrap());
                    let num_samples = u32::from_be_bytes(data[info_ptr + 8..info_ptr + 12].try_into().unwrap());
                    let track_count = if stride >= 16 {
                        u32::from_be_bytes(data[info_ptr + 12..info_ptr + 16].try_into().unwrap())
                    } else {
                        1
                    };
                    audio = Some(ThpAudioInfo {
                        channels,
                        sample_rate,
                        num_samples,
                        track_count,
                    });
                    info_ptr += stride;
                }
            }
        }
    }

    Ok(ThpMovie {
        header,
        video,
        audio,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_thp_header_parsing() {
        let mut sample = vec![0u8; 0x100];
        sample[0..4].copy_from_slice(b"THP\0");
        sample[4..8].copy_from_slice(&0x00011000u32.to_be_bytes()); // version 1.1
        sample[16..20].copy_from_slice(&29.97f32.to_be_bytes()); // fps
        sample[20..24].copy_from_slice(&1200u32.to_be_bytes()); // 1200 frames
        sample[32..36].copy_from_slice(&0x30u32.to_be_bytes()); // comp info offset at 0x30

        // Comp info at 0x30:
        sample[0x30..0x34].copy_from_slice(&2u32.to_be_bytes()); // 2 components
        sample[0x34] = 0; // component 0: video
        sample[0x35] = 1; // component 1: audio

        // Info ptr starts at 0x30 + 20 = 0x44:
        // Video: width 640, height 480, format 0 (12 bytes)
        sample[0x44..0x48].copy_from_slice(&640u32.to_be_bytes());
        sample[0x48..0x4C].copy_from_slice(&480u32.to_be_bytes());
        sample[0x4C..0x50].copy_from_slice(&0u32.to_be_bytes());

        // Audio: channels 2, sample_rate 32000, num_samples 1280000, tracks 1 (16 bytes)
        sample[0x50..0x54].copy_from_slice(&2u32.to_be_bytes());
        sample[0x54..0x58].copy_from_slice(&32000u32.to_be_bytes());
        sample[0x58..0x5C].copy_from_slice(&1280000u32.to_be_bytes());
        sample[0x5C..0x60].copy_from_slice(&1u32.to_be_bytes());

        let movie = parse_thp_header(&sample).expect("parse valid THP");
        assert_eq!(movie.header.num_frames, 1200);
        let v = movie.video.expect("video track");
        assert_eq!(v.width, 640);
        assert_eq!(v.height, 480);
        let a = movie.audio.expect("audio track");
        assert_eq!(a.channels, 2);
        assert_eq!(a.sample_rate, 32000);
    }
}
