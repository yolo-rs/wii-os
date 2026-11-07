//! Port of core algorithms, camera/projection math, and state structures
//! from the authentic Wii System Menu 4.3 C decompilation (`src/scene/`, `src/system/`, `src/BS2/`).

use serde::{Deserialize, Serialize};

/// Aspect ratio modes from `SCGetAspectRatio()`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum IplAspectRatio {
    #[default]
    Ratio4x3 = 0,
    Ratio16x9 = 1,
}

impl IplAspectRatio {
    /// Compute horizontal coordinate adjustment factor (matching `mScaleAdjust` in `iplChannelSelect.cpp`).
    #[inline]
    pub fn scale_adjust(self) -> f32 {
        match self {
            Self::Ratio4x3 => 1.0,
            Self::Ratio16x9 => 1.333_333_4, // 16:9 widescreen projection compensation
        }
    }

    /// Offset values for channel thumbnail rendering matching `cfChanThumbOfss`.
    #[inline]
    pub fn thumbnail_offset(self) -> (f32, f32) {
        match self {
            Self::Ratio4x3 => (0.0, 0.0),
            Self::Ratio16x9 => (0.0, -2.0),
        }
    }
}

/// Authentic 4×3 Channel Grid Mathematics ported from `iplChannelSelect.cpp`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IplGridMath {
    pub columns: usize,
    pub rows: usize,
    pub max_pages: usize,
    pub slot_width: f32,
    pub slot_height: f32,
    pub horizontal_spacing: f32,
    pub vertical_spacing: f32,
}

impl Default for IplGridMath {
    fn default() -> Self {
        Self {
            columns: 4,
            rows: 3,
            max_pages: 4,
            slot_width: 172.0,
            slot_height: 120.0,
            horizontal_spacing: 210.0,
            vertical_spacing: 150.0,
        }
    }
}

impl IplGridMath {
    /// Calculate screen-space center `(x, y)` for a given slot index `(0..12)` on a 1280×720 canvas.
    pub fn slot_position(&self, slot: usize, aspect: IplAspectRatio) -> (f32, f32) {
        let col = (slot % self.columns) as f32;
        let row = (slot / self.columns) as f32;

        let center_x = 640.0;
        let center_y = 360.0;

        let col_offset = (col - 1.5) * self.horizontal_spacing * aspect.scale_adjust();
        let row_offset = (row - 1.0) * self.vertical_spacing;

        let (thumb_x, thumb_y) = aspect.thumbnail_offset();

        (center_x + col_offset + thumb_x, center_y + row_offset + thumb_y)
    }

    /// Returns arrow visibility tuple `(left_visible, right_visible)` matching:
    /// `if (mCurrentPage == 0) mbLeftArrowVisible = false;`
    /// `if (mCurrentPage == mMaxPages - 1) mbRightArrowVisible = false;`
    #[inline]
    pub fn arrow_visibility(&self, current_page: usize) -> (bool, bool) {
        let left = current_page > 0;
        let right = current_page + 1 < self.max_pages;
        (left, right)
    }
}

/// Wiimote Controller & Battery State ported from `src/system/iplHomeButtonMenu.cpp` and `src/homebutton/HBMGUIManager.cpp`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IplControllerSlot {
    pub channel_index: u8, // 0..=3 (1P..4P)
    pub connected: bool,
    /// 0..=4 (0 = empty, 1 = critical, 2 = low, 3 = medium, 4 = full)
    pub battery_level: u8,
}

impl Default for IplControllerSlot {
    fn default() -> Self {
        Self {
            channel_index: 0,
            connected: true,
            battery_level: 4,
        }
    }
}

/// Home Button Menu state machine matching `iplHomeButtonMenu.cpp`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IplHomeButtonState {
    pub is_open: bool,
    pub controllers: [IplControllerSlot; 4],
    pub selected_action: Option<IplHomeAction>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum IplHomeAction {
    WiiMenu,
    ResetGame,
    CloseMenu,
}

impl Default for IplHomeButtonState {
    fn default() -> Self {
        Self {
            is_open: false,
            controllers: [
                IplControllerSlot { channel_index: 0, connected: true, battery_level: 4 },
                IplControllerSlot { channel_index: 1, connected: false, battery_level: 0 },
                IplControllerSlot { channel_index: 2, connected: false, battery_level: 0 },
                IplControllerSlot { channel_index: 3, connected: false, battery_level: 0 },
            ],
            selected_action: None,
        }
    }
}

/// Play Time Log Record matching `/title/00000001/00000002/data/play_rec.dat` format
/// and `src/system/RIPL_BoardRecord.c`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IplPlayTimeLog {
    pub date_yyyymmdd: u32,
    pub title_id: String,
    pub title_name: String,
    pub duration_minutes: u32,
    pub start_timestamp_unix: u64,
}

impl IplPlayTimeLog {
    /// Formats record into a standard Wii Message Board memo body.
    pub fn to_message_board_memo(&self) -> String {
        let hours = self.duration_minutes / 60;
        let mins = self.duration_minutes % 60;
        if hours > 0 {
            format!("{}: {} h {} min de jeu aujourd'hui", self.title_name, hours, mins)
        } else {
            format!("{}: {} min de jeu aujourd'hui", self.title_name, mins)
        }
    }
}

/// NAND Block Calculator ported from `src/scene/memory/iplMemory.cpp`.
pub struct IplNandMath;

impl IplNandMath {
    pub const BLOCK_SIZE_BYTES: u64 = 131_072; // 128 KiB per block
    pub const TOTAL_BLOCKS: u32 = 2174;

    #[inline]
    pub fn bytes_to_blocks(bytes: u64) -> u32 {
        if bytes == 0 {
            return 1;
        }
        ((bytes + Self::BLOCK_SIZE_BYTES - 1) / Self::BLOCK_SIZE_BYTES) as u32
    }

    #[inline]
    pub fn free_blocks(used_blocks: u32) -> u32 {
        Self::TOTAL_BLOCKS.saturating_sub(used_blocks)
    }
}
