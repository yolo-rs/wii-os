pub const DPAD_LEFT: u16 = 0x0001;
pub const DPAD_RIGHT: u16 = 0x0002;
pub const DPAD_DOWN: u16 = 0x0004;
pub const DPAD_UP: u16 = 0x0008;
pub const Z: u16 = 0x0010;
pub const R: u16 = 0x0020;
pub const L: u16 = 0x0040;
pub const A: u16 = 0x0100;
pub const B: u16 = 0x0200;
pub const X: u16 = 0x0400;
pub const Y: u16 = 0x0800;
pub const START: u16 = 0x1000;

pub const USE_ORIGIN: u16 = 0x0080;
pub const STICK_MIN: u8 = 0;
pub const STICK_CENTER: u8 = 128;
pub const STICK_MAX: u8 = 255;
pub const TRIGGER_MIN: u8 = 0;
pub const TRIGGER_MAX: u8 = 255;
pub const GC_CONTROLLER_ID: u32 = 0x0900_0000;

#[derive(Clone, Copy, Debug)]
pub struct PadStatus {
    pub buttons: u16,
    pub stick_x: u8,
    pub stick_y: u8,
    pub substick_x: u8,
    pub substick_y: u8,
    pub trigger_left: u8,
    pub trigger_right: u8,
    pub connected: bool,
}

impl Default for PadStatus {
    fn default() -> Self {
        Self {
            buttons: 0,
            stick_x: STICK_CENTER,
            stick_y: STICK_CENTER,
            substick_x: STICK_CENTER,
            substick_y: STICK_CENTER,
            trigger_left: 0,
            trigger_right: 0,
            connected: false,
        }
    }
}

impl PadStatus {
    // [buttons|USE_ORIGIN (16)] [stick_x (8)] [stick_y (8)]
    pub fn encode_hi(&self) -> u32 {
        let btns = (self.buttons | USE_ORIGIN) as u32;
        (btns << 16) | ((self.stick_x as u32) << 8) | (self.stick_y as u32)
    }

    // [substick_x (8)] [substick_y (8)] [trigger_l (8)] [trigger_r (8)]
    pub fn encode_lo(&self) -> u32 {
        ((self.substick_x as u32) << 24)
            | ((self.substick_y as u32) << 16)
            | ((self.trigger_left as u32) << 8)
            | (self.trigger_right as u32)
    }

    // recalibration payload
    pub fn encode_origin() -> [u8; 10] {
        [
            0x00,
            0x00,
            STICK_CENTER,
            STICK_CENTER,
            STICK_CENTER,
            STICK_CENTER,
            0x00,
            0x00,
            0x00,
            0x00,
        ]
    }
}
