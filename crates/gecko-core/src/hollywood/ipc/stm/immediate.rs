use crate::hollywood::ipc::{DeviceContext, IPC_EINVAL, IosDevice};

const IOCTL_GET_IDLEMODE: u32 = 0x2001;
const IOCTL_RELEASE_EH: u32 = 0x3001;
const IOCTL_HOTRESET: u32 = 0x3002;
const IOCTL_HOTRESET_FOR_PD: u32 = 0x3003;
const IOCTL_SHUTDOWN: u32 = 0x3004;
const IOCTL_IDLE: u32 = 0x4001;
const IOCTL_WAKEUP: u32 = 0x4002;
const IOCTL_VIDIMMING: u32 = 0x5001;
const IOCTL_LEDFLASH: u32 = 0x6001;
const IOCTL_LEDMODE: u32 = 0x6002;
const IOCTL_READVER: u32 = 0x7001;
const IOCTL_READDDRREG: u32 = 0x8001;
const IOCTL_READDDRREG2: u32 = 0x8002;

pub struct Immediate;

impl IosDevice for Immediate {
    fn ioctl(
        &mut self,
        ctx: &mut DeviceContext<'_>,
        cmd: u32,
        in_ptr: u32,
        in_len: u32,
        out_ptr: u32,
        out_len: u32,
    ) -> i32 {
        match cmd {
            IOCTL_GET_IDLEMODE
            | IOCTL_RELEASE_EH
            | IOCTL_HOTRESET
            | IOCTL_HOTRESET_FOR_PD
            | IOCTL_SHUTDOWN
            | IOCTL_IDLE
            | IOCTL_WAKEUP
            | IOCTL_VIDIMMING
            | IOCTL_LEDFLASH
            | IOCTL_LEDMODE
            | IOCTL_READVER
            | IOCTL_READDDRREG
            | IOCTL_READDDRREG2 => 0,
            _ => {
                tracing::warn!(
                    device = &ctx.device_path,
                    cmd = format!("{cmd:#010X}"),
                    in_buf = format!("{:02X?}", ctx.mmio.phys_slice(in_ptr, in_len as usize)),
                    in_len,
                    out_buf = format!("{:02X?}", ctx.mmio.phys_slice(out_ptr, out_len as usize)),
                    out_len,
                    "STM_Immediate: unimplemented ioctl"
                );
                IPC_EINVAL
            }
        }
    }
}
