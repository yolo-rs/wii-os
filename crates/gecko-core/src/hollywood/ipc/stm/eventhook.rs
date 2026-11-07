use crate::hollywood::ipc::{DeviceContext, IPC_HUNG, IosDevice};

pub struct EventHook;

impl IosDevice for EventHook {
    fn ioctl(
        &mut self,
        _ctx: &mut DeviceContext<'_>,
        _cmd: u32,
        _in_buf: u32,
        _in_len: u32,
        _out_buf: u32,
        _out_len: u32,
    ) -> i32 {
        tracing::warn!("STM_EventHook fucky fucky");
        IPC_HUNG
    }
}
