#[chapa::bitfield(u32, width = 32, order = msb0)]
#[derive(Clone, Copy, Default)]
pub struct Fpscr {
    #[bits(0)]
    pub fx: bool,

    #[bits(1)]
    pub fex: bool,

    #[bits(2)]
    pub vx: bool,

    #[bits(3)]
    pub ox: bool,

    #[bits(4)]
    pub ux: bool,

    #[bits(5)]
    pub zx: bool,

    #[bits(6)]
    pub xx: bool,

    #[bits(7)]
    pub vxsnan: bool,

    #[bits(8)]
    pub vxisi: bool,

    #[bits(9)]
    pub vxidi: bool,

    #[bits(10)]
    pub vxzdz: bool,

    #[bits(11)]
    pub vximz: bool,

    #[bits(12)]
    pub vxvc: bool,

    #[bits(13)]
    pub fr: bool,

    #[bits(14)]
    pub fi: bool,

    #[bits(15..=19)]
    pub fprf: u8,

    // bit 20 reserved
    #[bits(21)]
    pub vxsoft: bool,

    #[bits(22)]
    pub vxsqrt: bool,

    #[bits(23)]
    pub vxcvi: bool,

    #[bits(24)]
    pub ve: bool,

    #[bits(25)]
    pub oe: bool,

    #[bits(26)]
    pub ue: bool,

    #[bits(27)]
    pub ze: bool,

    #[bits(28)]
    pub xe: bool,

    #[bits(29)]
    pub ni: bool,

    #[bits(30..=31)]
    pub rn: u8,
}
