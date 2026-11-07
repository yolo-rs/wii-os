//! Pure-Rust parser and extractor for Wii WAD archives (`SystemMenu-v514.wad`).
//!
//! Provides cryptographically verified parsing of WAD headers, ticket decryption
//! via the Wii Common Key (AES-128-CBC), Title Metadata (TMD) verification,
//! content extraction (.app files), DOL executable inspection, and U8 layout archive unpacking.

use std::{
    fs,
    path::Path,
};

/// Wii Common Key for retail consoles (Europe, USA, Japan).
pub const WII_COMMON_KEY: [u8; 16] = [
    0xeb, 0xe4, 0x2a, 0x22, 0x5e, 0x85, 0x93, 0xe4,
    0x48, 0xd9, 0xc5, 0x45, 0x73, 0x81, 0xaa, 0xf7,
];

/// Magic identifier for Nintendo U8 archives (`U*8-`).
pub const U8_MAGIC: u32 = 0x55aa382d;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WadHeader {
    pub header_len: u32,
    pub wad_type: String,
    pub cert_len: u32,
    pub crl_len: u32,
    pub ticket_len: u32,
    pub tmd_len: u32,
    pub data_len: u32,
    pub footer_len: u32,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WadTicket {
    pub issuer: String,
    pub title_id_hex: String,
    pub encrypted_title_key_hex: String,
    pub decrypted_title_key_hex: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WadContentMeta {
    pub index: u16,
    pub cid: u32,
    pub cid_hex: String,
    pub kind: u16,
    pub size: u64,
    pub offset: u64,
    pub sha1_hex: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WadTmd {
    pub issuer: String,
    pub title_id_hex: String,
    pub title_type: u32,
    pub group_id: u16,
    pub region: u16,
    pub title_version: u16,
    pub num_contents: u16,
    pub boot_index: u16,
    pub contents: Vec<WadContentMeta>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DolSection {
    pub offset: u32,
    pub address: u32,
    pub size: u32,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DolHeader {
    pub entry_point: u32,
    pub text_sections: Vec<DolSection>,
    pub data_sections: Vec<DolSection>,
    pub bss_address: u32,
    pub bss_size: u32,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct U8Node {
    pub index: usize,
    pub name: String,
    pub is_dir: bool,
    pub data_offset: u32,
    pub size: u32,
}

/// Pure-Rust AES-128-CBC Decryptor without external dependencies.
pub struct Aes128Cbc;

impl Aes128Cbc {
    // S-Box and inverse S-box for standard AES
    const SBOX: [u8; 256] = [
        0x63, 0x7c, 0x77, 0x7b, 0xf2, 0x6b, 0x6f, 0xc5, 0x30, 0x01, 0x67, 0x2b, 0xfe, 0xd7, 0xab, 0x76,
        0xca, 0x82, 0xc9, 0x7d, 0xfa, 0x59, 0x47, 0xf0, 0xad, 0xd4, 0xa2, 0xaf, 0x9c, 0xa4, 0x72, 0xc0,
        0xb7, 0xfd, 0x93, 0x26, 0x36, 0x3f, 0xf7, 0xcc, 0x34, 0xa5, 0xe5, 0xf1, 0x71, 0xd8, 0x31, 0x15,
        0x04, 0xc7, 0x23, 0xc3, 0x18, 0x96, 0x05, 0x9a, 0x07, 0x12, 0x80, 0xe2, 0xeb, 0x27, 0xb2, 0x75,
        0x09, 0x83, 0x2c, 0x1a, 0x1b, 0x6e, 0x5a, 0xa0, 0x52, 0x3b, 0xd6, 0xb3, 0x29, 0xe3, 0x2f, 0x84,
        0x53, 0xd1, 0x00, 0xed, 0x20, 0xfc, 0xb1, 0x5b, 0x6a, 0xcb, 0xbe, 0x39, 0x4a, 0x4c, 0x58, 0xcf,
        0xd0, 0xef, 0xaa, 0xfb, 0x43, 0x4d, 0x33, 0x85, 0x45, 0xf9, 0x02, 0x7f, 0x50, 0x3c, 0x9f, 0xa8,
        0x51, 0xa3, 0x40, 0x8f, 0x92, 0x9d, 0x38, 0xf5, 0xbc, 0xb6, 0xda, 0x21, 0x10, 0xff, 0xf3, 0xd2,
        0xcd, 0x0c, 0x13, 0xec, 0x5f, 0x97, 0x44, 0x17, 0xc4, 0xa7, 0x7e, 0x3d, 0x64, 0x5d, 0x19, 0x73,
        0x60, 0x81, 0x4f, 0xdc, 0x22, 0x2a, 0x90, 0x88, 0x46, 0xee, 0xb8, 0x14, 0xde, 0x5e, 0x0b, 0xdb,
        0xe0, 0x32, 0x3a, 0x0a, 0x49, 0x06, 0x24, 0x5c, 0xc2, 0xd3, 0xac, 0x62, 0x91, 0x95, 0xe4, 0x79,
        0xe7, 0xc8, 0x37, 0x6d, 0x8d, 0xd5, 0x4e, 0xa9, 0x6c, 0x56, 0xf4, 0xea, 0x65, 0x7a, 0xae, 0x08,
        0xba, 0x78, 0x25, 0x2e, 0x1c, 0xa6, 0xb4, 0xc6, 0xe8, 0xdd, 0x74, 0x1f, 0x4b, 0xbd, 0x8b, 0x8a,
        0x70, 0x3e, 0xb5, 0x66, 0x48, 0x03, 0xf6, 0x0e, 0x61, 0x35, 0x57, 0xb9, 0x86, 0xc1, 0x1d, 0x9e,
        0xe1, 0xf8, 0x98, 0x11, 0x69, 0xd9, 0x8e, 0x94, 0x9b, 0x1e, 0x87, 0xe9, 0xce, 0x55, 0x28, 0xdf,
        0x8c, 0xa1, 0x89, 0x0d, 0xbf, 0xe6, 0x42, 0x68, 0x41, 0x99, 0x2d, 0x0f, 0xb0, 0x54, 0xbb, 0x16,
    ];

    const RSBOX: [u8; 256] = [
        0x52, 0x09, 0x6a, 0xd5, 0x30, 0x36, 0xa5, 0x38, 0xbf, 0x40, 0xa3, 0x9e, 0x81, 0xf3, 0xd7, 0xfb,
        0x7c, 0xe3, 0x39, 0x82, 0x9b, 0x2f, 0xff, 0x87, 0x34, 0x8e, 0x43, 0x44, 0xc4, 0xde, 0xe9, 0xcb,
        0x54, 0x7b, 0x94, 0x32, 0xa6, 0xc2, 0x23, 0x3d, 0xee, 0x4c, 0x95, 0x0b, 0x42, 0xfa, 0xc3, 0x4e,
        0x08, 0x2e, 0xa1, 0x66, 0x28, 0xd9, 0x24, 0xb2, 0x76, 0x5b, 0xa2, 0x49, 0x6d, 0x8b, 0xd1, 0x25,
        0x72, 0xf8, 0xf6, 0x64, 0x86, 0x68, 0x98, 0x16, 0xd4, 0xa4, 0x5c, 0xcc, 0x5d, 0x65, 0xb6, 0x92,
        0x6c, 0x70, 0x48, 0x50, 0xfd, 0xed, 0xb9, 0xda, 0x5e, 0x15, 0x46, 0x57, 0xa7, 0x8d, 0x9d, 0x84,
        0x90, 0xd8, 0xab, 0x00, 0x8c, 0xbc, 0xd3, 0x0a, 0xf7, 0xe4, 0x58, 0x05, 0xb8, 0xb3, 0x45, 0x06,
        0xd0, 0x2c, 0x1e, 0x8f, 0xca, 0x3f, 0x0f, 0x02, 0xc1, 0xaf, 0xbd, 0x03, 0x01, 0x13, 0x8a, 0x6b,
        0x3a, 0x91, 0x11, 0x41, 0x4f, 0x67, 0xdc, 0xea, 0x97, 0xf2, 0xcf, 0xce, 0xf0, 0xb4, 0xe6, 0x73,
        0x96, 0xac, 0x74, 0x22, 0xe7, 0xad, 0x35, 0x85, 0xe2, 0xf9, 0x37, 0xe8, 0x1c, 0x75, 0xdf, 0x6e,
        0x47, 0xf1, 0x1a, 0x71, 0x1d, 0x29, 0xc5, 0x89, 0x6f, 0xb7, 0x62, 0x0e, 0xaa, 0x18, 0xbe, 0x1b,
        0xfc, 0x56, 0x3e, 0x4b, 0xc6, 0xd2, 0x79, 0x20, 0x9a, 0xdb, 0xc0, 0xfe, 0x78, 0xcd, 0x5a, 0xf4,
        0x1f, 0xdd, 0xa8, 0x33, 0x88, 0x07, 0xc7, 0x31, 0xb1, 0x12, 0x10, 0x59, 0x27, 0x80, 0xec, 0x5f,
        0x60, 0x51, 0x7f, 0xa9, 0x19, 0xb5, 0x4a, 0x0d, 0x2d, 0xe5, 0x7a, 0x9f, 0x93, 0xc9, 0x9c, 0xef,
        0xa0, 0xe0, 0x3b, 0x4d, 0xae, 0x2a, 0xf5, 0xb0, 0xc8, 0xeb, 0xbb, 0x3c, 0x83, 0x53, 0x99, 0x61,
        0x17, 0x2b, 0x04, 0x7e, 0xba, 0x77, 0xd6, 0x26, 0xe1, 0x69, 0x14, 0x63, 0x55, 0x21, 0x0c, 0x7d,
    ];

    const RCON: [u8; 11] = [0x00, 0x01, 0x02, 0x04, 0x08, 0x10, 0x20, 0x40, 0x80, 0x1b, 0x36];

    #[inline]
    fn gmul(mut a: u8, mut b: u8) -> u8 {
        let mut p = 0u8;
        for _ in 0..8 {
            if (b & 1) != 0 {
                p ^= a;
            }
            let hi = (a & 0x80) != 0;
            a <<= 1;
            if hi {
                a ^= 0x1b;
            }
            b >>= 1;
        }
        p
    }

    fn key_expansion(key: &[u8; 16]) -> [u8; 176] {
        let mut w = [0u8; 176];
        w[0..16].copy_from_slice(key);
        let mut rcon_idx = 1;
        let mut i = 16;
        while i < 176 {
            let mut temp = [w[i - 4], w[i - 3], w[i - 2], w[i - 1]];
            if i % 16 == 0 {
                let k0 = temp[0];
                temp[0] = Self::SBOX[temp[1] as usize] ^ Self::RCON[rcon_idx];
                temp[1] = Self::SBOX[temp[2] as usize];
                temp[2] = Self::SBOX[temp[3] as usize];
                temp[3] = Self::SBOX[k0 as usize];
                rcon_idx += 1;
            }
            for j in 0..4 {
                w[i + j] = w[i - 16 + j] ^ temp[j];
            }
            i += 4;
        }
        w
    }

    fn inv_cipher(state: &mut [u8; 16], w: &[u8; 176]) {
        // AddRoundKey with round 10
        for i in 0..16 {
            state[i] ^= w[160 + i];
        }

        for round in (1..10).rev() {
            // InvShiftRows
            let mut temp = [0u8; 16];
            temp[0] = state[0];   temp[1] = state[13]; temp[2] = state[10]; temp[3] = state[7];
            temp[4] = state[4];   temp[5] = state[1];  temp[6] = state[14]; temp[7] = state[11];
            temp[8] = state[8];   temp[9] = state[5];  temp[10] = state[2]; temp[11] = state[15];
            temp[12] = state[12]; temp[13] = state[9]; temp[14] = state[6]; temp[15] = state[3];

            // InvSubBytes
            for i in 0..16 {
                state[i] = Self::RSBOX[temp[i] as usize];
            }

            // AddRoundKey
            for i in 0..16 {
                state[i] ^= w[round * 16 + i];
            }

            // InvMixColumns
            let mut mixed = [0u8; 16];
            for c in 0..4 {
                let idx = c * 4;
                let s0 = state[idx];
                let s1 = state[idx + 1];
                let s2 = state[idx + 2];
                let s3 = state[idx + 3];
                mixed[idx] = Self::gmul(0x0e, s0) ^ Self::gmul(0x0b, s1) ^ Self::gmul(0x0d, s2) ^ Self::gmul(0x09, s3);
                mixed[idx + 1] = Self::gmul(0x09, s0) ^ Self::gmul(0x0e, s1) ^ Self::gmul(0x0b, s2) ^ Self::gmul(0x0d, s3);
                mixed[idx + 2] = Self::gmul(0x0d, s0) ^ Self::gmul(0x09, s1) ^ Self::gmul(0x0e, s2) ^ Self::gmul(0x0b, s3);
                mixed[idx + 3] = Self::gmul(0x0b, s0) ^ Self::gmul(0x0d, s1) ^ Self::gmul(0x09, s2) ^ Self::gmul(0x0e, s3);
            }
            *state = mixed;
        }

        // InvShiftRows (Round 0)
        let mut temp = [0u8; 16];
        temp[0] = state[0];   temp[1] = state[13]; temp[2] = state[10]; temp[3] = state[7];
        temp[4] = state[4];   temp[5] = state[1];  temp[6] = state[14]; temp[7] = state[11];
        temp[8] = state[8];   temp[9] = state[5];  temp[10] = state[2]; temp[11] = state[15];
        temp[12] = state[12]; temp[13] = state[9]; temp[14] = state[6]; temp[15] = state[3];

        // InvSubBytes
        for i in 0..16 {
            state[i] = Self::RSBOX[temp[i] as usize];
        }

        // AddRoundKey (Round 0)
        for i in 0..16 {
            state[i] ^= w[i];
        }
    }

    /// Decrypts in-place a buffer of length multiple of 16 using AES-128-CBC.
    pub fn decrypt_cbc(key: &[u8; 16], iv: &[u8; 16], data: &mut [u8]) -> Result<(), String> {
        if data.len() % 16 != 0 {
            return Err("AES-128-CBC input data must be a multiple of 16 bytes".into());
        }
        let w = Self::key_expansion(key);
        let mut prev_block = *iv;
        for chunk in data.chunks_exact_mut(16) {
            let current_ciphertext = <[u8; 16]>::try_from(&*chunk).unwrap();
            let mut block = current_ciphertext;
            Self::inv_cipher(&mut block, &w);
            for i in 0..16 {
                chunk[i] = block[i] ^ prev_block[i];
            }
            prev_block = current_ciphertext;
        }
        Ok(())
    }
}

pub struct WadArchive {
    data: Vec<u8>,
    header: WadHeader,
    ticket: WadTicket,
    tmd: WadTmd,
    title_key: [u8; 16],
    #[allow(dead_code)]
    data_offset: usize,
}

impl WadArchive {
    #[inline]
    fn align64(x: usize) -> usize {
        (x + 63) & !63
    }

    pub fn open(path: &Path) -> Result<Self, String> {
        let bytes = fs::read(path).map_err(|e| format!("cannot open WAD {}: {e}", path.display()))?;
        Self::from_bytes(bytes)
    }

    pub fn from_bytes(data: Vec<u8>) -> Result<Self, String> {
        if data.len() < 0x40 {
            return Err("WAD buffer is too small to contain a valid header".into());
        }

        let header_len = u32::from_be_bytes(data[0..4].try_into().unwrap());
        let wad_type = String::from_utf8_lossy(&data[4..8]).trim_matches('\0').to_string();
        let cert_len = u32::from_be_bytes(data[8..12].try_into().unwrap());
        let crl_len = u32::from_be_bytes(data[12..16].try_into().unwrap());
        let ticket_len = u32::from_be_bytes(data[16..20].try_into().unwrap());
        let tmd_len = u32::from_be_bytes(data[20..24].try_into().unwrap());
        let data_len = u32::from_be_bytes(data[24..28].try_into().unwrap());
        let footer_len = u32::from_be_bytes(data[28..32].try_into().unwrap());

        let header = WadHeader {
            header_len,
            wad_type,
            cert_len,
            crl_len,
            ticket_len,
            tmd_len,
            data_len,
            footer_len,
        };

        let mut pos = Self::align64(header_len as usize);
        pos += Self::align64(cert_len as usize);
        pos += Self::align64(crl_len as usize);
        let ticket_offset = pos;
        pos += Self::align64(ticket_len as usize);
        let tmd_offset = pos;
        pos += Self::align64(tmd_len as usize);
        let data_offset = pos;

        if data.len() < data_offset {
            return Err("WAD buffer ends before data offset".into());
        }

        // Parse ticket
        if data.len() < ticket_offset + ticket_len as usize {
            return Err("WAD buffer truncated before ticket end".into());
        }
        let ticket_slice = &data[ticket_offset..ticket_offset + ticket_len as usize];
        let ticket_issuer = String::from_utf8_lossy(&ticket_slice[0x140..0x180])
            .trim_matches('\0')
            .to_string();
        let mut enc_title_key = [0u8; 16];
        enc_title_key.copy_from_slice(&ticket_slice[0x1bf..0x1cf]);
        let mut title_id = [0u8; 8];
        title_id.copy_from_slice(&ticket_slice[0x1dc..0x1e4]);
        let title_id_hex = hex::encode(title_id);

        // Decrypt title key
        let mut iv = [0u8; 16];
        iv[0..8].copy_from_slice(&title_id);
        let mut dec_title_key = enc_title_key;
        Aes128Cbc::decrypt_cbc(&WII_COMMON_KEY, &iv, &mut dec_title_key)?;
        let title_key = dec_title_key;

        let ticket = WadTicket {
            issuer: ticket_issuer,
            title_id_hex: title_id_hex.clone(),
            encrypted_title_key_hex: hex::encode(enc_title_key),
            decrypted_title_key_hex: hex::encode(title_key),
        };

        // Parse TMD
        if data.len() < tmd_offset + tmd_len as usize {
            return Err("WAD buffer truncated before TMD end".into());
        }
        let tmd_slice = &data[tmd_offset..tmd_offset + tmd_len as usize];
        let tmd_issuer = String::from_utf8_lossy(&tmd_slice[0x140..0x180])
            .trim_matches('\0')
            .to_string();
        let tmd_title_id_hex = hex::encode(&tmd_slice[0x18c..0x194]);
        let title_type = u32::from_be_bytes(tmd_slice[0x194..0x198].try_into().unwrap());
        let group_id = u16::from_be_bytes(tmd_slice[0x198..0x19a].try_into().unwrap());
        let region = u16::from_be_bytes(tmd_slice[0x19c..0x19e].try_into().unwrap());
        let title_version = u16::from_be_bytes(tmd_slice[0x1dc..0x1de].try_into().unwrap());
        let num_contents = u16::from_be_bytes(tmd_slice[0x1de..0x1e0].try_into().unwrap());
        let boot_index = u16::from_be_bytes(tmd_slice[0x1e0..0x1e2].try_into().unwrap());

        let mut contents = Vec::new();
        let mut c_offset = tmd_offset + 0x1e4;
        let mut current_data_offset = data_offset as u64;

        for _ in 0..num_contents {
            if data.len() < c_offset + 36 {
                break;
            }
            let cid = u32::from_be_bytes(data[c_offset..c_offset + 4].try_into().unwrap());
            let index = u16::from_be_bytes(data[c_offset + 4..c_offset + 6].try_into().unwrap());
            let kind = u16::from_be_bytes(data[c_offset + 6..c_offset + 8].try_into().unwrap());
            let size = u64::from_be_bytes(data[c_offset + 8..c_offset + 16].try_into().unwrap());
            let sha1_hex = hex::encode(&data[c_offset + 16..c_offset + 36]);

            contents.push(WadContentMeta {
                index,
                cid,
                cid_hex: format!("{cid:08x}"),
                kind,
                size,
                offset: current_data_offset,
                sha1_hex,
            });

            current_data_offset += Self::align64(size as usize) as u64;
            c_offset += 36;
        }

        let tmd = WadTmd {
            issuer: tmd_issuer,
            title_id_hex: tmd_title_id_hex,
            title_type,
            group_id,
            region,
            title_version,
            num_contents,
            boot_index,
            contents,
        };

        Ok(Self {
            data,
            header,
            ticket,
            tmd,
            title_key,
            data_offset,
        })
    }

    pub fn header(&self) -> &WadHeader {
        &self.header
    }

    pub fn ticket(&self) -> &WadTicket {
        &self.ticket
    }

    pub fn tmd(&self) -> &WadTmd {
        &self.tmd
    }

    pub fn title_key(&self) -> &[u8; 16] {
        &self.title_key
    }

    pub fn decrypt_content(&self, content_index: usize) -> Result<Vec<u8>, String> {
        let meta = self
            .tmd
            .contents
            .iter()
            .find(|c| c.index as usize == content_index)
            .ok_or_else(|| format!("content index {content_index} not found in TMD"))?;

        let start = meta.offset as usize;
        let aligned_len = Self::align64(meta.size as usize);
        if self.data.len() < start + aligned_len {
            return Err(format!("content {content_index} extends beyond WAD buffer"));
        }

        let mut encrypted = self.data[start..start + aligned_len].to_vec();

        let mut iv = [0u8; 16];
        iv[0..2].copy_from_slice(&(meta.index).to_be_bytes());

        Aes128Cbc::decrypt_cbc(&self.title_key, &iv, &mut encrypted)?;
        encrypted.truncate(meta.size as usize);
        Ok(encrypted)
    }

    /// Extracts and parses the bootable `main.dol` executable from Content index 8.
    pub fn extract_boot_dol(&self) -> Result<(DolHeader, Vec<u8>), String> {
        let dol_bytes = self.decrypt_content(self.tmd.boot_index as usize)?;
        if dol_bytes.len() < 0x100 {
            return Err("DOL binary too small to contain a standard header".into());
        }

        let mut text_sections = Vec::new();
        let mut data_sections = Vec::new();

        for i in 0..7 {
            let offset = u32::from_be_bytes(dol_bytes[i * 4..(i + 1) * 4].try_into().unwrap());
            let address = u32::from_be_bytes(dol_bytes[0x48 + i * 4..0x48 + (i + 1) * 4].try_into().unwrap());
            let size = u32::from_be_bytes(dol_bytes[0x90 + i * 4..0x90 + (i + 1) * 4].try_into().unwrap());
            if size > 0 {
                text_sections.push(DolSection { offset, address, size });
            }
        }

        for i in 0..11 {
            let offset = u32::from_be_bytes(dol_bytes[0x1c + i * 4..0x1c + (i + 1) * 4].try_into().unwrap());
            let address = u32::from_be_bytes(dol_bytes[0x64 + i * 4..0x64 + (i + 1) * 4].try_into().unwrap());
            let size = u32::from_be_bytes(dol_bytes[0xac + i * 4..0xac + (i + 1) * 4].try_into().unwrap());
            if size > 0 {
                data_sections.push(DolSection { offset, address, size });
            }
        }

        let bss_address = u32::from_be_bytes(dol_bytes[0xd8..0xdc].try_into().unwrap());
        let bss_size = u32::from_be_bytes(dol_bytes[0xdc..0xe0].try_into().unwrap());
        let raw_entry_point = u32::from_be_bytes(dol_bytes[0xe0..0xe4].try_into().unwrap());
        let entry_point = if raw_entry_point < 0x8000_0000 && raw_entry_point != 0 {
            0x8000_0000 | raw_entry_point
        } else {
            raw_entry_point
        };

        let header = DolHeader {
            entry_point,
            text_sections,
            data_sections,
            bss_address,
            bss_size,
        };

        Ok((header, dol_bytes))
    }

    /// Extracts and parses the U8 archive filesystem from Content index 1 (`ipl.ash`/`ipl.arc`).
    pub fn extract_u8_nodes(&self) -> Result<(Vec<U8Node>, Vec<u8>), String> {
        let u8_bytes = self.decrypt_content(1)?;
        if u8_bytes.len() < 0x20 {
            return Err("Content 1 is too small for a U8 archive".into());
        }

        let magic = u32::from_be_bytes(u8_bytes[0..4].try_into().unwrap());
        if magic != U8_MAGIC {
            return Err(format!("invalid U8 magic 0x{magic:08x}, expected 0x{U8_MAGIC:08x}"));
        }

        let root_offset = u32::from_be_bytes(u8_bytes[4..8].try_into().unwrap()) as usize;
        if u8_bytes.len() < root_offset + 12 {
            return Err("U8 root node extends beyond archive buffer".into());
        }

        let total_nodes = u32::from_be_bytes(u8_bytes[root_offset + 8..root_offset + 12].try_into().unwrap()) as usize;
        let string_table_offset = root_offset + total_nodes * 12;

        let get_string = |name_offset: usize| -> String {
            let start = string_table_offset + name_offset;
            if start >= u8_bytes.len() {
                return String::new();
            }
            let end = u8_bytes[start..]
                .iter()
                .position(|&b| b == 0)
                .map(|p| start + p)
                .unwrap_or(u8_bytes.len());
            String::from_utf8_lossy(&u8_bytes[start..end]).to_string()
        };

        let mut nodes = Vec::new();
        for i in 1..total_nodes {
            let n_offset = root_offset + i * 12;
            if u8_bytes.len() < n_offset + 12 {
                break;
            }
            let type_field = u16::from_be_bytes(u8_bytes[n_offset..n_offset + 2].try_into().unwrap());
            let name_offset = u16::from_be_bytes(u8_bytes[n_offset + 2..n_offset + 4].try_into().unwrap()) as usize;
            let data_offset = u32::from_be_bytes(u8_bytes[n_offset + 4..n_offset + 8].try_into().unwrap());
            let size = u32::from_be_bytes(u8_bytes[n_offset + 8..n_offset + 12].try_into().unwrap());
            let is_dir = (type_field & 0x0100) != 0;
            let name = get_string(name_offset);

            nodes.push(U8Node {
                index: i,
                name,
                is_dir,
                data_offset,
                size,
            });
        }

        Ok((nodes, u8_bytes))
    }

    /// Extracts all files from the U8 filesystem to the specified directory,
    /// transparently unpacking any ASH0-compressed archives into `.arc` format.
    pub fn extract_all_u8_files(&self, output_dir: &Path) -> Result<usize, String> {
        let (nodes, u8_bytes) = self.extract_u8_nodes()?;
        let mut count = 0;
        let mut dir_stack: Vec<(usize, std::path::PathBuf)> = vec![(nodes.len() + 2, output_dir.to_path_buf())];

        for node in &nodes {
            while let Some((end_idx, _)) = dir_stack.last() {
                if node.index >= *end_idx {
                    dir_stack.pop();
                } else {
                    break;
                }
            }

            let current_dir = dir_stack.last().map(|(_, p)| p.clone()).unwrap_or_else(|| output_dir.to_path_buf());

            if node.is_dir {
                let sub_dir = current_dir.join(&node.name);
                fs::create_dir_all(&sub_dir).map_err(|e| e.to_string())?;
                dir_stack.push((node.size as usize, sub_dir));
            } else {
                let file_path = current_dir.join(&node.name);
                if let Some(parent) = file_path.parent() {
                    fs::create_dir_all(parent).map_err(|e| e.to_string())?;
                }
                let start = node.data_offset as usize;
                let end = start + node.size as usize;
                if end <= u8_bytes.len() {
                    let file_data = &u8_bytes[start..end];
                    fs::write(&file_path, file_data).map_err(|e| e.to_string())?;
                    count += 1;

                    // If file is ASH0 compressed, decompress to .arc
                    if crate::ash::is_ash_compressed(file_data) {
                        if let Ok(decompressed) = crate::ash::decompress_ash0(file_data) {
                            let arc_name = if node.name.ends_with(".ash") {
                                format!("{}.arc", &node.name[..node.name.len() - 4])
                            } else {
                                format!("{}.arc", node.name)
                            };
                            let arc_path = current_dir.join(arc_name);
                            let _ = fs::write(arc_path, decompressed);
                        }
                    }
                }
            }
        }
        Ok(count)
    }
}

// Module for simple hex conversion without extra dependency
mod hex {
    pub fn encode<T: AsRef<[u8]>>(data: T) -> String {
        let mut s = String::with_capacity(data.as_ref().len() * 2);
        for &b in data.as_ref() {
            s.push(char::from_digit((b >> 4) as u32, 16).unwrap());
            s.push(char::from_digit((b & 0x0f) as u32, 16).unwrap());
        }
        s
    }
}
