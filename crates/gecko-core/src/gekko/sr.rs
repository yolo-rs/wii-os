#[chapa::bitfield(u32, width = 32, order = msb0)]
#[derive(Clone, Copy, Default)]
pub struct Sr {
    #[bits(0, alias = "t")] // decides overlay, gekko only uses T=0 AFAICT
    pub format: bool,
    #[bits(1, alias = "ks")]
    pub supervisor_protection_key: bool,
    #[bits(2, alias = "kp")]
    pub user_protection_key: bool,

    #[bits(3, alias = "n", overlay = "t0")]
    pub no_execute_protection: bool,
    #[bits(8..=31, alias = "vsid", overlay = "t0")]
    pub virtual_segment_id: u32,

    #[bits(3..=11, alias = "buid", overlay = "t1")]
    pub bus_unit_id: u16,
    #[bits(12..=31, alias = "cntrl_spec", overlay = "t1")]
    pub device_specific_data: u32,
}
