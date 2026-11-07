use crate::hollywood::ipc::{IPC_EINVAL, IosDevice};

pub const IOCTL_GET_DEVICE_STATUS: u32 = 11;
pub const IOCTL_SEND_CMD: u32 = 7;

#[chapa::bitfield(u32, order = lsb0)]
#[derive(Clone, Copy)]
#[rustfmt::skip]
struct DeviceStatus {
    #[bits(0)] sd_card_inserted: bool,
    #[bits(1)] sd_card_not_inserted: bool,
    #[bits(2)] sd_card_write_unprotected: bool,
    #[bits(16)] sd_card_initialized: bool,
    #[bits(20)] sd_card_is_sdhc: bool,
}

pub struct SdCard;

impl IosDevice for SdCard {
    fn ioctl(
        &mut self,
        ctx: &mut super::DeviceContext<'_>,
        cmd: u32,
        _in_ptr: u32,
        _in_len: u32,
        out_ptr: u32,
        out_len: u32,
    ) -> i32 {
        match cmd {
            IOCTL_GET_DEVICE_STATUS => {
                let status = DeviceStatus::new()
                    .with_sd_card_inserted(false)
                    .with_sd_card_not_inserted(true)
                    .with_sd_card_write_unprotected(false)
                    .with_sd_card_initialized(false)
                    .with_sd_card_is_sdhc(false);
                ctx.mmio.phys_write_u32(out_ptr, status.raw());
                0
            }
            IOCTL_SEND_CMD => {
                ctx.mmio.phys_slice_mut(out_ptr, out_len as usize).fill(0);
                0
            }
            _ => {
                tracing::error!(
                    device = &ctx.device_path,
                    cmd = format!("{cmd:#010X}"),
                    "SDIO: unimplemented ioctl"
                );
                IPC_EINVAL
            }
        }
    }
}
