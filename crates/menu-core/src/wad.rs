//! Nintendo WAD (Wii Title Archive) & TMD (Title Metadata) format parser.
//! Based on authentic Nintendo RVL-WAD and libogc / wadlib specifications.

use serde::{Deserialize, Serialize};

pub const WAD_HEADER_SIZE: usize = 0x20;
pub const WAD_TYPE_IB: [u8; 2] = [b'i', b'b'];
pub const WAD_TYPE_IS: [u8; 2] = [b'I', b's'];

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WadHeader {
    pub header_size: u32,
    pub wad_type: [u8; 2],
    pub certs_size: u32,
    pub crl_size: u32,
    pub ticket_size: u32,
    pub tmd_size: u32,
    pub data_size: u32,
    pub footer_size: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TmdContent {
    pub content_id: u32,
    pub index: u16,
    pub content_type: u16,
    pub size: u64,
    pub sha1: [u8; 20],
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Tmd {
    pub title_id: String,
    pub title_type: u32,
    pub group_id: u16,
    pub access_rights: u32,
    pub title_version: u16,
    pub num_contents: u16,
    pub boot_index: u16,
    pub contents: Vec<TmdContent>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WadTicket {
    pub ticket_id: u64,
    pub console_id: u32,
    pub title_id: String,
    pub access_mask: u16,
    pub common_key_index: u8,
}

/// Parses the 32-byte header of a Nintendo WAD file.
pub fn parse_wad_header(data: &[u8]) -> Result<WadHeader, &'static str> {
    if data.len() < WAD_HEADER_SIZE {
        return Err("data too short for WAD header");
    }
    let header_size = u32::from_be_bytes(data[0..4].try_into().unwrap());
    let wad_type = [data[4], data[5]];
    if wad_type != WAD_TYPE_IB && wad_type != WAD_TYPE_IS {
        return Err("unsupported WAD type identifier (expected 'Is' or 'ib')");
    }
    let certs_size = u32::from_be_bytes(data[8..12].try_into().unwrap());
    let crl_size = u32::from_be_bytes(data[12..16].try_into().unwrap());
    let ticket_size = u32::from_be_bytes(data[16..20].try_into().unwrap());
    let tmd_size = u32::from_be_bytes(data[20..24].try_into().unwrap());
    let data_size = u32::from_be_bytes(data[24..28].try_into().unwrap());
    let footer_size = u32::from_be_bytes(data[28..32].try_into().unwrap());

    Ok(WadHeader {
        header_size,
        wad_type,
        certs_size,
        crl_size,
        ticket_size,
        tmd_size,
        data_size,
        footer_size,
    })
}

/// Parses Title Metadata (TMD) from bytes.
pub fn parse_tmd(data: &[u8]) -> Result<Tmd, &'static str> {
    // A standard TMD has a signature (0x140 or 0x200 bytes) followed by header:
    // Issuer: 0x40 bytes
    // Version / CA / Signer: bytes
    // Title ID is at offset 0x18C in standard RSA-2048 TMD
    let sig_type = if data.len() >= 4 {
        u32::from_be_bytes(data[0..4].try_into().unwrap())
    } else {
        return Err("data too short for TMD");
    };

    let base_offset = match sig_type {
        0x00010001 => 0x240, // RSA-2048
        0x00010002 => 0x140, // RSA-4096
        0x00010004 => 0x140, // ECDSA
        _ => {
            // Assume 0x240 standard offset if unrecognized
            0x240
        }
    };

    if data.len() < base_offset + 0x4C {
        return Err("data slice too small for TMD body");
    }

    let body = &data[base_offset..];
    let title_id_bytes = &body[0x4C..0x54];
    let title_id = title_id_bytes
        .iter()
        .map(|b| format!("{:02X}", b))
        .collect::<String>();

    let title_type = u32::from_be_bytes(body[0x54..0x58].try_into().unwrap());
    let group_id = u16::from_be_bytes(body[0x58..0x5A].try_into().unwrap());
    let access_rights = u32::from_be_bytes(body[0x98..0x9C].try_into().unwrap());
    let title_version = u16::from_be_bytes(body[0x9C..0x9E].try_into().unwrap());
    let num_contents = u16::from_be_bytes(body[0x9E..0xA0].try_into().unwrap());
    let boot_index = u16::from_be_bytes(body[0xA0..0xA2].try_into().unwrap());

    let mut contents = Vec::with_capacity(num_contents as usize);
    let mut content_offset = base_offset + 0xB4;

    for _ in 0..num_contents {
        if content_offset + 0x24 > data.len() {
            break;
        }
        let chunk = &data[content_offset..content_offset + 0x24];
        let cid = u32::from_be_bytes(chunk[0..4].try_into().unwrap());
        let index = u16::from_be_bytes(chunk[4..6].try_into().unwrap());
        let content_type = u16::from_be_bytes(chunk[6..8].try_into().unwrap());
        let size = u64::from_be_bytes(chunk[8..16].try_into().unwrap());
        let mut sha1 = [0u8; 20];
        sha1.copy_from_slice(&chunk[16..36]);

        contents.push(TmdContent {
            content_id: cid,
            index,
            content_type,
            size,
            sha1,
        });
        content_offset += 0x24;
    }

    Ok(Tmd {
        title_id,
        title_type,
        group_id,
        access_rights,
        title_version,
        num_contents,
        boot_index,
        contents,
    })
}

/// Parses a Wii Ticket.
pub fn parse_ticket(data: &[u8]) -> Result<WadTicket, &'static str> {
    if data.len() < 0x200 {
        return Err("data too short for Ticket");
    }
    // Standard RSA-2048 ticket offset is 0x1D0 for ticket body
    let sig_type = u32::from_be_bytes(data[0..4].try_into().unwrap());
    let base_offset = match sig_type {
        0x00010001 => 0x140,
        _ => 0x140,
    };
    if data.len() < base_offset + 0xD0 {
        return Err("insufficient data for ticket payload");
    }
    let body = &data[base_offset..];
    let ticket_id = u64::from_be_bytes(body[0x50..0x58].try_into().unwrap());
    let console_id = u32::from_be_bytes(body[0x58..0x5C].try_into().unwrap());
    let title_id = body[0x5C..0x64]
        .iter()
        .map(|b| format!("{:02X}", b))
        .collect::<String>();
    let access_mask = u16::from_be_bytes(body[0x64..0x66].try_into().unwrap());
    let common_key_index = body[0xB1];

    Ok(WadTicket {
        ticket_id,
        console_id,
        title_id,
        access_mask,
        common_key_index,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wad_header_parsing() {
        let mut sample = vec![0u8; 0x40];
        sample[0..4].copy_from_slice(&0x20u32.to_be_bytes()); // header size
        sample[4..6].copy_from_slice(&WAD_TYPE_IB);
        sample[8..12].copy_from_slice(&0xA00u32.to_be_bytes()); // certs size
        sample[16..20].copy_from_slice(&0x2A4u32.to_be_bytes()); // ticket size
        sample[20..24].copy_from_slice(&0x520u32.to_be_bytes()); // tmd size
        sample[24..28].copy_from_slice(&0x400000u32.to_be_bytes()); // data size

        let hdr = parse_wad_header(&sample).expect("parse valid WAD header");
        assert_eq!(hdr.header_size, 0x20);
        assert_eq!(hdr.wad_type, WAD_TYPE_IB);
        assert_eq!(hdr.ticket_size, 0x2A4);
        assert_eq!(hdr.tmd_size, 0x520);
        assert_eq!(hdr.data_size, 0x400000);
    }
}
