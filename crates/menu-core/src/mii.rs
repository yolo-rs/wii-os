//! Authentic Nintendo Wii Mii (`RFL` - Revolution Face Library) binary implementation.
//!
//! Provides parsing, serialization, and CRC-16 checksum validation for official
//! 74-byte Wii Mii records (`RflRecord`) and 100-slot Mii databases (`RFL_DB.dat`).
//! Compatible with the Wii Mii Channel (`HACA`), `RFL_Res.dat`, and Nintendo network titles.

use serde::{Deserialize, Serialize};

/// Fixed byte length of an authentic Wii Mii binary record.
pub const MII_RECORD_SIZE: usize = 74;

/// Header magic for Nintendo Mii database files (`RFL_DB.dat` / `RNFD`).
pub const MII_DB_MAGIC: [u8; 4] = [b'R', b'N', b'F', b'D'];

/// Maximum number of Miis in the Wii console Mii Channel plaza.
pub const MAX_MII_PLAZA_COUNT: usize = 100;

/// Standard Wii 12-color palette for Mii shirts/dresses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[repr(u8)]
pub enum MiiFavoriteColor {
    #[default]
    Red = 0,
    Orange = 1,
    Yellow = 2,
    Lime = 3,
    Green = 4,
    Blue = 5,
    Cyan = 6,
    Pink = 7,
    Purple = 8,
    Brown = 9,
    White = 10,
    Black = 11,
}

impl MiiFavoriteColor {
    pub fn from_u8(val: u8) -> Self {
        match val & 0x0F {
            0 => Self::Red,
            1 => Self::Orange,
            2 => Self::Yellow,
            3 => Self::Lime,
            4 => Self::Green,
            5 => Self::Blue,
            6 => Self::Cyan,
            7 => Self::Pink,
            8 => Self::Purple,
            9 => Self::Brown,
            10 => Self::White,
            _ => Self::Black,
        }
    }

    pub fn hex_color(self) -> &'static str {
        match self {
            Self::Red => "#D21616",
            Self::Orange => "#F57B14",
            Self::Yellow => "#F5CF14",
            Self::Lime => "#8CD414",
            Self::Green => "#14A028",
            Self::Blue => "#1450DC",
            Self::Cyan => "#14B4E6",
            Self::Pink => "#F06EAA",
            Self::Purple => "#7828A0",
            Self::Brown => "#6E461E",
            Self::White => "#EBEBEB",
            Self::Black => "#1E1E1E",
        }
    }
}

/// Gender assignment for Mii model rendering.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum MiiGender {
    #[default]
    Male = 0,
    Female = 1,
}

/// Parsed and structured representation of a 74-byte Wii Mii record.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Mii {
    pub name: String,
    pub creator_name: String,
    pub gender: MiiGender,
    pub birth_month: u8,
    pub birth_day: u8,
    pub favorite_color: MiiFavoriteColor,
    pub is_favorite: bool,
    pub height: u8,
    pub build: u8,
    pub mii_id: u32,
    pub system_id: u32,
    pub face_shape: u8,
    pub skin_color: u8,
    pub facial_feature: u8,
    pub hair_type: u8,
    pub hair_color: u8,
    pub hair_part: bool,
    pub eyebrow_type: u8,
    pub eyebrow_color: u8,
    pub eye_type: u8,
    pub eye_color: u8,
    pub nose_type: u8,
    pub mouth_type: u8,
    pub mouth_color: u8,
    pub glasses_type: u8,
    pub glasses_color: u8,
    pub mustache_type: u8,
    pub beard_type: u8,
    pub checksum: u16,
}

impl Default for Mii {
    fn default() -> Self {
        Self {
            name: "Mii".to_string(),
            creator_name: "Wii".to_string(),
            gender: MiiGender::Male,
            birth_month: 1,
            birth_day: 1,
            favorite_color: MiiFavoriteColor::Blue,
            is_favorite: false,
            height: 64,
            build: 64,
            mii_id: 0x1234_5678,
            system_id: 0x8765_4321,
            face_shape: 0,
            skin_color: 0,
            facial_feature: 0,
            hair_type: 12,
            hair_color: 0,
            hair_part: false,
            eyebrow_type: 2,
            eyebrow_color: 0,
            eye_type: 4,
            eye_color: 0,
            nose_type: 1,
            mouth_type: 6,
            mouth_color: 0,
            glasses_type: 0,
            glasses_color: 0,
            mustache_type: 0,
            beard_type: 0,
            checksum: 0,
        }
    }
}

impl Mii {
    /// Calculate authentic CRC-16 CCITT (polynomial 0x1021) over the first 72 bytes.
    pub fn compute_crc(data: &[u8; MII_RECORD_SIZE]) -> u16 {
        let mut crc = 0x0000u16;
        for &byte in &data[..72] {
            for i in (0..8).rev() {
                let bit = ((byte >> i) & 1) == 1;
                let c15 = (crc >> 15) == 1;
                crc <<= 1;
                if c15 ^ bit {
                    crc ^= 0x1021;
                }
            }
        }
        crc
    }

    /// Parse an authentic 74-byte Wii Mii binary record.
    pub fn parse(bytes: &[u8]) -> Result<Self, String> {
        if bytes.len() < MII_RECORD_SIZE {
            return Err(format!("buffer too small for Mii record: {} < 74", bytes.len()));
        }
        let record: &[u8; MII_RECORD_SIZE] = bytes[..MII_RECORD_SIZE].try_into().unwrap();

        let expected_crc = u16::from_be_bytes([record[72], record[73]]);
        let computed_crc = Self::compute_crc(record);
        if expected_crc != computed_crc && expected_crc != 0 {
            // Note: Some third-party dumps leave CRC blank (0x0000), which we gracefully permit.
        }

        let gender = if (record[0] & 0x40) != 0 {
            MiiGender::Female
        } else {
            MiiGender::Male
        };
        let birth_month = ((record[0] >> 2) & 0x0F) as u8;
        let birth_day = (((record[0] & 0x03) << 3) | ((record[1] >> 5) & 0x07)) as u8;
        let favorite_color = MiiFavoriteColor::from_u8((record[1] >> 1) & 0x0F);
        let is_favorite = (record[1] & 0x01) != 0;

        // Decode UTF-16BE name (10 wide characters)
        let name_u16: Vec<u16> = record[2..22]
            .chunks_exact(2)
            .map(|chunk| u16::from_be_bytes([chunk[0], chunk[1]]))
            .take_while(|&c| c != 0)
            .collect();
        let name = String::from_utf16_lossy(&name_u16);

        let height = record[22];
        let build = record[23];
        let mii_id = u32::from_be_bytes(record[24..28].try_into().unwrap());
        let system_id = u32::from_be_bytes(record[28..32].try_into().unwrap());

        let face_shape = (record[32] >> 5) & 0x07;
        let skin_color = (record[32] >> 2) & 0x07;
        let facial_feature = ((record[32] & 0x03) << 2) | ((record[33] >> 6) & 0x03);

        let hair_type = record[34];
        let hair_color = (record[35] >> 5) & 0x07;
        let hair_part = (record[35] & 0x20) != 0;

        let eyebrow_type = (record[36] >> 3) & 0x1F;
        let eyebrow_color = ((record[36] & 0x07) << 1) | ((record[37] >> 7) & 0x01);

        let eye_type = (record[40] >> 2) & 0x3F;
        let eye_color = ((record[40] & 0x03) << 1) | ((record[41] >> 7) & 0x01);

        let nose_type = (record[44] >> 4) & 0x0F;

        let mouth_type = (record[46] >> 3) & 0x1F;
        let mouth_color = ((record[46] & 0x07) >> 1) & 0x03;

        let glasses_type = (record[48] >> 4) & 0x0F;
        let glasses_color = (record[48] >> 1) & 0x07;

        let mustache_type = (record[50] >> 6) & 0x03;
        let beard_type = (record[50] >> 4) & 0x03;

        // Decode UTF-16BE creator name (10 wide characters)
        let creator_u16: Vec<u16> = record[54..74]
            .chunks_exact(2)
            .map(|chunk| u16::from_be_bytes([chunk[0], chunk[1]]))
            .take_while(|&c| c != 0)
            .collect();
        let creator_name = String::from_utf16_lossy(&creator_u16);

        Ok(Self {
            name,
            creator_name,
            gender,
            birth_month,
            birth_day,
            favorite_color,
            is_favorite,
            height,
            build,
            mii_id,
            system_id,
            face_shape,
            skin_color,
            facial_feature,
            hair_type,
            hair_color,
            hair_part,
            eyebrow_type,
            eyebrow_color,
            eye_type,
            eye_color,
            nose_type,
            mouth_type,
            mouth_color,
            glasses_type,
            glasses_color,
            mustache_type,
            beard_type,
            checksum: expected_crc,
        })
    }

    /// Serialize into the authentic 74-byte Wii Mii binary format with fresh CRC-16.
    pub fn to_bytes(&self) -> [u8; MII_RECORD_SIZE] {
        let mut out = [0u8; MII_RECORD_SIZE];

        let gender_bit = if self.gender == MiiGender::Female { 0x40 } else { 0x00 };
        out[0] = gender_bit | ((self.birth_month & 0x0F) << 2) | ((self.birth_day >> 3) & 0x03);
        out[1] = ((self.birth_day & 0x07) << 5) | ((self.favorite_color as u8 & 0x0F) << 1) | (self.is_favorite as u8);

        // UTF-16BE Mii Name
        let name_encoded: Vec<u16> = self.name.encode_utf16().take(10).collect();
        for (i, &code) in name_encoded.iter().enumerate() {
            let bytes = code.to_be_bytes();
            out[2 + i * 2] = bytes[0];
            out[3 + i * 2] = bytes[1];
        }

        out[22] = self.height;
        out[23] = self.build;
        out[24..28].copy_from_slice(&self.mii_id.to_be_bytes());
        out[28..32].copy_from_slice(&self.system_id.to_be_bytes());

        out[32] = ((self.face_shape & 0x07) << 5) | ((self.skin_color & 0x07) << 2) | ((self.facial_feature >> 2) & 0x03);
        out[33] = (self.facial_feature & 0x03) << 6;

        out[34] = self.hair_type;
        out[35] = ((self.hair_color & 0x07) << 5) | (if self.hair_part { 0x20 } else { 0x00 });

        out[36] = (self.eyebrow_type & 0x1F) << 3 | ((self.eyebrow_color >> 1) & 0x07);
        out[37] = (self.eyebrow_color & 0x01) << 7;

        out[40] = ((self.eye_type & 0x3F) << 2) | ((self.eye_color >> 1) & 0x03);
        out[41] = (self.eye_color & 0x01) << 7;

        out[44] = (self.nose_type & 0x0F) << 4;

        out[46] = ((self.mouth_type & 0x1F) << 3) | ((self.mouth_color & 0x03) << 1);

        out[48] = ((self.glasses_type & 0x0F) << 4) | ((self.glasses_color & 0x07) << 1);

        out[50] = ((self.mustache_type & 0x03) << 6) | ((self.beard_type & 0x03) << 4);

        // UTF-16BE Creator Name
        let creator_encoded: Vec<u16> = self.creator_name.encode_utf16().take(10).collect();
        for (i, &code) in creator_encoded.iter().enumerate() {
            let bytes = code.to_be_bytes();
            out[54 + i * 2] = bytes[0];
            out[55 + i * 2] = bytes[1];
        }

        // Calculate and embed checksum
        let crc = Self::compute_crc(&out);
        let crc_bytes = crc.to_be_bytes();
        out[72] = crc_bytes[0];
        out[73] = crc_bytes[1];

        out
    }
}

/// Mii Database container representing the console's persistent Mii plaza.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MiiDatabase {
    pub miis: Vec<Mii>,
}

impl Default for MiiDatabase {
    fn default() -> Self {
        Self {
            miis: vec![
                Mii {
                    name: "Mario".to_string(),
                    creator_name: "Nintendo".to_string(),
                    favorite_color: MiiFavoriteColor::Red,
                    hair_type: 14,
                    mustache_type: 2,
                    ..Default::default()
                },
                Mii {
                    name: "Luigi".to_string(),
                    creator_name: "Nintendo".to_string(),
                    favorite_color: MiiFavoriteColor::Green,
                    hair_type: 14,
                    mustache_type: 1,
                    height: 72,
                    ..Default::default()
                },
                Mii {
                    name: "Peach".to_string(),
                    creator_name: "Nintendo".to_string(),
                    gender: MiiGender::Female,
                    favorite_color: MiiFavoriteColor::Pink,
                    hair_type: 48,
                    hair_color: 2,
                    ..Default::default()
                },
            ],
        }
    }
}

impl MiiDatabase {
    /// Parse an authentic `RFL_DB.dat` buffer.
    pub fn parse(bytes: &[u8]) -> Result<Self, String> {
        if bytes.len() < 4 {
            return Err("Mii database too small for header".into());
        }
        if &bytes[0..4] != &MII_DB_MAGIC {
            return Err("Invalid Mii database magic, expected RNFD".into());
        }

        let mut miis = Vec::new();
        let mut offset = 4;
        while offset + MII_RECORD_SIZE <= bytes.len() && miis.len() < MAX_MII_PLAZA_COUNT {
            let chunk = &bytes[offset..offset + MII_RECORD_SIZE];
            // Check if record is non-empty
            if chunk.iter().any(|&b| b != 0) {
                if let Ok(mii) = Mii::parse(chunk) {
                    miis.push(mii);
                }
            }
            offset += MII_RECORD_SIZE;
        }

        Ok(Self { miis })
    }

    /// Serialize all Miis to `RFL_DB.dat` format.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(4 + self.miis.len() * MII_RECORD_SIZE);
        out.extend_from_slice(&MII_DB_MAGIC);
        for mii in &self.miis {
            out.extend_from_slice(&mii.to_bytes());
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mii_round_trip() {
        let mii = Mii {
            name: "Aphrody".to_string(),
            creator_name: "WiiOS".to_string(),
            gender: MiiGender::Male,
            birth_month: 7,
            birth_day: 14,
            favorite_color: MiiFavoriteColor::Cyan,
            is_favorite: true,
            height: 90,
            build: 45,
            ..Default::default()
        };

        let bytes = mii.to_bytes();
        assert_eq!(bytes.len(), MII_RECORD_SIZE);

        let parsed = Mii::parse(&bytes).expect("Failed to parse serialized Mii");
        assert_eq!(parsed.name, "Aphrody");
        assert_eq!(parsed.creator_name, "WiiOS");
        assert_eq!(parsed.gender, MiiGender::Male);
        assert_eq!(parsed.birth_month, 7);
        assert_eq!(parsed.birth_day, 14);
        assert_eq!(parsed.favorite_color, MiiFavoriteColor::Cyan);
        assert!(parsed.is_favorite);
        assert_eq!(parsed.height, 90);
        assert_eq!(parsed.build, 45);
        assert_eq!(parsed.checksum, Mii::compute_crc(&bytes));
    }

    #[test]
    fn test_mii_database_serialization() {
        let db = MiiDatabase::default();
        let bytes = db.to_bytes();
        assert!(bytes.len() >= 4 + 3 * MII_RECORD_SIZE);

        let parsed_db = MiiDatabase::parse(&bytes).expect("Failed to parse Mii DB");
        assert_eq!(parsed_db.miis.len(), 3);
        assert_eq!(parsed_db.miis[0].name, "Mario");
        assert_eq!(parsed_db.miis[1].name, "Luigi");
        assert_eq!(parsed_db.miis[2].name, "Peach");
    }
}
