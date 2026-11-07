//! Nintendo BNS (Banner Sound) audio parser.
//! BNS is the official Nintendo audio container format used in opening.bnr,
//! sound archives, and channel title audio on Wii and GameCube.

use serde::{Deserialize, Serialize};

pub const BNS_MAGIC: &[u8; 4] = b"BNS\0";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AudioCodec {
    Pcm8,
    Pcm16,
    DspAdpcm,
    Unknown(u16),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BnsHeader {
    pub codec: AudioCodec,
    pub loop_flag: bool,
    pub channels: u8,
    pub sample_rate: u32,
    pub loop_start_sample: u32,
    pub total_samples: u32,
    pub data_offset: u32,
    pub data_size: u32,
}

/// Parses the BNS header from raw bytes.
pub fn parse_bns_header(data: &[u8]) -> Result<BnsHeader, &'static str> {
    if data.len() < 0x20 {
        return Err("data slice too small for BNS header");
    }
    if &data[0..4] != BNS_MAGIC {
        return Err("invalid BNS magic identifier (expected 'BNS\\0')");
    }

    // Standard BNS structure:
    // 0x00: b"BNS\0"
    // 0x04: Endianness (0xFEFF = Big Endian)
    // 0x06: Version
    // 0x08: File size
    // 0x0C: Header size / info block offset
    let info_offset = if data.len() >= 0x10 {
        u32::from_be_bytes(data[0x0C..0x10].try_into().unwrap()) as usize
    } else {
        0x20
    };

    let target = if info_offset + 0x18 <= data.len() {
        &data[info_offset..]
    } else {
        &data[0x10..]
    };

    if target.len() < 0x18 {
        return Err("insufficient data for BNS stream information");
    }

    let codec_raw = u16::from_be_bytes(target[0..2].try_into().unwrap());
    let codec = match codec_raw {
        0 => AudioCodec::Pcm8,
        1 => AudioCodec::Pcm16,
        2 => AudioCodec::DspAdpcm,
        other => AudioCodec::Unknown(other),
    };

    let loop_flag = target[2] != 0;
    let channels = target[3].max(1);
    let sample_rate = u32::from_be_bytes([0, target[4], target[5], target[6]]); // or 24-bit/32-bit depending on rev
    let sample_rate = if sample_rate < 4000 || sample_rate > 96000 {
        // Fallback to standard 32-bit sample rate offset
        u32::from_be_bytes(target[4..8].try_into().unwrap())
    } else {
        sample_rate
    };

    let loop_start_sample = u32::from_be_bytes(target[8..12].try_into().unwrap());
    let total_samples = u32::from_be_bytes(target[12..16].try_into().unwrap());
    let data_offset = if target.len() >= 20 {
        u32::from_be_bytes(target[16..20].try_into().unwrap())
    } else {
        0x40
    };
    let data_size = if target.len() >= 24 {
        u32::from_be_bytes(target[20..24].try_into().unwrap())
    } else {
        (data.len() as u32).saturating_sub(data_offset)
    };

    Ok(BnsHeader {
        codec,
        loop_flag,
        channels,
        sample_rate,
        loop_start_sample,
        total_samples,
        data_offset,
        data_size,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bns_header_parsing() {
        let mut sample = vec![0u8; 0x60];
        sample[0..4].copy_from_slice(b"BNS\0");
        sample[0x0C..0x10].copy_from_slice(&0x20u32.to_be_bytes()); // info at 0x20

        // Info at 0x20:
        sample[0x20..0x22].copy_from_slice(&2u16.to_be_bytes()); // DSP ADPCM
        sample[0x22] = 1; // loop enabled
        sample[0x23] = 2; // stereo 2 channels
        sample[0x24..0x28].copy_from_slice(&32000u32.to_be_bytes()); // 32 kHz
        sample[0x28..0x2C].copy_from_slice(&64000u32.to_be_bytes()); // loop start
        sample[0x2C..0x30].copy_from_slice(&256000u32.to_be_bytes()); // total samples
        sample[0x30..0x34].copy_from_slice(&0x40u32.to_be_bytes()); // data offset
        sample[0x34..0x38].copy_from_slice(&128000u32.to_be_bytes()); // data size

        let bns = parse_bns_header(&sample).expect("parse valid BNS");
        assert_eq!(bns.codec, AudioCodec::DspAdpcm);
        assert!(bns.loop_flag);
        assert_eq!(bns.channels, 2);
        assert_eq!(bns.sample_rate, 32000);
        assert_eq!(bns.total_samples, 256000);
    }
}
