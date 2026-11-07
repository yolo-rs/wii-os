//! Nintendo Wii Network Protocols Implementation:
//! 1. WiiConnect24 (WC24) Mail & Standby Message Service.
//! 2. Nintendo Wi-Fi Connection (DWC / NAS) Authentication & Friend Code derivation.
//! 3. Nintendo Update Server (NUS) Content & Metadata Retrieval Protocol.

use serde::{Deserialize, Serialize};

/// Epoch difference between Unix timestamp (1970) and Nintendo Wii timestamp (2000-01-01 00:00:00 UTC).
pub const WII_TIME_EPOCH_DIFF_SECS: u64 = 946_684_800;

// -----------------------------------------------------------------------------
// 1. WiiConnect24 (WC24)
// -----------------------------------------------------------------------------

/// Structure representing an authentic WiiConnect24 Message Board record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Wc24Message {
    pub message_id: u64,
    pub timestamp_wii_secs: u32,
    pub sender_friend_code: String,
    pub recipient_friend_code: String,
    pub subject: String,
    pub body: String,
    pub attached_mii_id: Option<u32>,
    pub read: bool,
}

impl Wc24Message {
    /// Create a new message with current timestamp.
    pub fn new(sender: impl Into<String>, recipient: impl Into<String>, subject: impl Into<String>, body: impl Into<String>) -> Self {
        let now_unix = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(WII_TIME_EPOCH_DIFF_SECS);
        let timestamp_wii_secs = now_unix.saturating_sub(WII_TIME_EPOCH_DIFF_SECS) as u32;

        Self {
            message_id: rand_or_timestamp_id(timestamp_wii_secs),
            timestamp_wii_secs,
            sender_friend_code: sender.into(),
            recipient_friend_code: recipient.into(),
            subject: subject.into(),
            body: body.into(),
            attached_mii_id: None,
            read: false,
        }
    }
}

/// Simulated WiiConnect24 configuration matching `nwc24msg.cfg`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Wc24Config {
    pub friend_code: String,
    pub email: String,
    pub registered: bool,
    pub standby_secret: [u8; 16],
    pub check_interval_minutes: u32,
}

impl Default for Wc24Config {
    fn default() -> Self {
        Self {
            friend_code: "4821-9034-7712-6508".to_string(),
            email: "w4821903477126508@wii.com".to_string(),
            registered: true,
            standby_secret: [0x57, 0x43, 0x32, 0x34, 0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0A, 0x0B],
            check_interval_minutes: 15,
        }
    }
}

// -----------------------------------------------------------------------------
// 2. Nintendo Wi-Fi Connection (DWC / NAS)
// -----------------------------------------------------------------------------

/// DWC Matchmaking Game Service Descriptor.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DwcGameProfile {
    pub game_code: String,
    pub game_title: String,
    pub nas_url: String,
    pub auth_token: Option<String>,
}

impl DwcGameProfile {
    pub fn for_game(game_code: &str) -> Self {
        let (title, nas) = match game_code {
            "RDSPAF" | "RDSEAF" => (
                "Dragon Ball Z : Budokai Tenkaichi 3",
                "https://nas.nintendowifi.net/ac",
            ),
            "SMNP01" => (
                "New Super Mario Bros. Wii",
                "https://nas.nintendowifi.net/ac",
            ),
            "RMCP01" => (
                "Mario Kart Wii",
                "https://nas.nintendowifi.net/ac",
            ),
            _ => (
                "Wii Title",
                "https://nas.nintendowifi.net/ac",
            ),
        };

        Self {
            game_code: game_code.to_string(),
            game_title: title.to_string(),
            nas_url: nas.to_string(),
            auth_token: None,
        }
    }

    /// Derive 16-digit Wii Friend Code from Console ID and Game ID using Nintendo CRC algorithm.
    pub fn derive_friend_code(console_id: u32, game_code: &str) -> String {
        let crc_input = format!("{console_id}:{game_code}");
        let crc = simple_crc32(crc_input.as_bytes());
        let fc_raw = ((console_id as u64) & 0xFFFF_FFFF) | (((crc as u64) & 0x7F) << 32);
        let digits = format!("{:016}", fc_raw % 10_000_000_000_000_000);
        format!(
            "{}-{}-{}-{}",
            &digits[0..4],
            &digits[4..8],
            &digits[8..12],
            &digits[12..16]
        )
    }
}

// -----------------------------------------------------------------------------
// 3. Nintendo Update Server (NUS)
// -----------------------------------------------------------------------------

/// Nintendo Update Server URL builder and title locator.
pub struct NusClient {
    pub base_url: String,
}

impl Default for NusClient {
    fn default() -> Self {
        Self {
            base_url: "http://nus.cdn.shop.wii.com/ccs/download".to_string(),
        }
    }
}

impl NusClient {
    pub fn new(base_url: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into(),
        }
    }

    pub fn tmd_url(&self, title_id: &str, version: Option<u16>) -> String {
        if let Some(v) = version {
            format!("{}/{}/tmd.{}", self.base_url, title_id, v)
        } else {
            format!("{}/{}/tmd", self.base_url, title_id)
        }
    }

    pub fn ticket_url(&self, title_id: &str) -> String {
        format!("{}/{}/cetk", self.base_url, title_id)
    }

    pub fn content_url(&self, title_id: &str, content_id_hex: &str) -> String {
        format!("{}/{}/{}", self.base_url, title_id, content_id_hex)
    }

    /// Classify a Wii Title ID by category.
    pub fn classify_title(title_id: &str) -> &'static str {
        if title_id.len() < 8 {
            return "Unknown";
        }
        match &title_id[0..8] {
            "00000001" => "System Firmware (IOS / System Menu / BC / MIOS)",
            "00010000" => "Disc Game Save Data",
            "00010001" => "Downloaded Channel / WiiWare / Virtual Console",
            "00010002" => "Official System Channel (Mii, Shop, Photo, Forecast)",
            "00010004" => "Disc Channel Launcher",
            "00010005" => "Downloadable Content (DLC)",
            "00010008" => "Hidden Channel",
            _ => "User Title",
        }
    }
}

// -----------------------------------------------------------------------------
// 4. Nintendo Wi-Fi Connection Services (NAS & GameSpy / NATNEG)
// -----------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NasAuthToken {
    pub token: String,
    pub challenge: String,
    pub locator_url: String,
    pub expires_in_seconds: u32,
}

impl NasAuthToken {
    pub fn new(game_code: &str, friend_code: &str) -> Self {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        std::hash::Hash::hash(game_code, &mut hasher);
        std::hash::Hash::hash(friend_code, &mut hasher);
        let h = std::hash::Hasher::finish(&hasher);
        Self {
            token: format!("NDS{:016X}", h),
            challenge: format!("CHAL_{:08X}", (h & 0xFFFF_FFFF) as u32),
            locator_url: "https://nas.nintendowifi.net/locator".to_string(),
            expires_in_seconds: 86400,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum NatType {
    Open,
    Moderate,
    Strict,
    Symmetric,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NatNegotiationPacket {
    pub cookie: u32,
    pub client_index: u8,
    pub nat_type: NatType,
    pub remote_ip: String,
    pub remote_port: u16,
}

impl NatNegotiationPacket {
    pub fn new(cookie: u32, client_index: u8, nat_type: NatType, ip: &str, port: u16) -> Self {
        Self {
            cookie,
            client_index,
            nat_type,
            remote_ip: ip.to_string(),
            remote_port: port,
        }
    }
}

// -----------------------------------------------------------------------------
// Helper math
// -----------------------------------------------------------------------------

fn rand_or_timestamp_id(ts: u32) -> u64 {
    ((ts as u64) << 32) | 0x0000_A5A5
}

fn simple_crc32(data: &[u8]) -> u32 {
    let mut crc = 0xFFFF_FFFFu32;
    for &b in data {
        crc ^= b as u32;
        for _ in 0..8 {
            let mask = if (crc & 1) != 0 { 0xEDB8_8320 } else { 0 };
            crc = (crc >> 1) ^ mask;
        }
    }
    !crc
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wc24_message_creation() {
        let msg = Wc24Message::new(
            "1234-5678-9012-3456",
            "4821-9034-7712-6508",
            "Salut depuis Wii OS!",
            "Rejoins le salon Budokai Tenkaichi 3 ce soir!",
        );
        assert_eq!(msg.sender_friend_code, "1234-5678-9012-3456");
        assert_eq!(msg.subject, "Salut depuis Wii OS!");
        assert!(msg.timestamp_wii_secs > 0);
    }

    #[test]
    fn test_dwc_friend_code_derivation() {
        let fc = DwcGameProfile::derive_friend_code(0x12345678, "RDSPAF");
        assert_eq!(fc.len(), 19); // XXXX-XXXX-XXXX-XXXX
        assert_eq!(&fc[4..5], "-");
        assert_eq!(&fc[9..10], "-");
        assert_eq!(&fc[14..15], "-");
    }

    #[test]
    fn test_nus_client_url_generation() {
        let nus = NusClient::default();
        let tmd = nus.tmd_url("0000000100000002", Some(514));
        assert_eq!(tmd, "http://nus.cdn.shop.wii.com/ccs/download/0000000100000002/tmd.514");

        let cetk = nus.ticket_url("0000000100000002");
        assert_eq!(cetk, "http://nus.cdn.shop.wii.com/ccs/download/0000000100000002/cetk");

        let class = NusClient::classify_title("0001000248414341");
        assert_eq!(class, "Official System Channel (Mii, Shop, Photo, Forecast)");
    }
}
