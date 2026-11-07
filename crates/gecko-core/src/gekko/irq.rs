use crate::gekko::spr::Srr0;
use crate::system::{System, SystemId};

// Exception vectors
#[rustfmt::skip] pub const IRQ_SYSTEM_RESET: u32         = crate::mmio::virt_to_phys(0x8000_0100);
#[rustfmt::skip] pub const IRQ_MACHINE_CHECK: u32        = crate::mmio::virt_to_phys(0x8000_0200);
#[rustfmt::skip] pub const IRQ_DSI: u32                  = crate::mmio::virt_to_phys(0x8000_0300);
#[rustfmt::skip] pub const IRQ_ISI: u32                  = crate::mmio::virt_to_phys(0x8000_0400);
#[rustfmt::skip] pub const IRQ_EXTERNAL: u32             = crate::mmio::virt_to_phys(0x8000_0500); // traditional IRQ
#[rustfmt::skip] pub const IRQ_ALIGNMENT: u32            = crate::mmio::virt_to_phys(0x8000_0600);
#[rustfmt::skip] pub const IRQ_PROGRAM: u32              = crate::mmio::virt_to_phys(0x8000_0700);
#[rustfmt::skip] pub const IRQ_FP_UNAVAILABLE: u32       = crate::mmio::virt_to_phys(0x8000_0800);
#[rustfmt::skip] pub const IRQ_DECREMENTER: u32          = crate::mmio::virt_to_phys(0x8000_0900);
#[rustfmt::skip] pub const IRQ_SYSTEM_CALL: u32          = crate::mmio::virt_to_phys(0x8000_0C00);
#[rustfmt::skip] pub const IRQ_TRACE: u32                = crate::mmio::virt_to_phys(0x8000_0D00);
#[rustfmt::skip] pub const IRQ_PERFORMANCE_MONITOR: u32  = crate::mmio::virt_to_phys(0x8000_0F00);
#[rustfmt::skip] pub const IRQ_IABR: u32                 = crate::mmio::virt_to_phys(0x8000_1300);
#[rustfmt::skip] pub const IRQ_THERMAL: u32              = crate::mmio::virt_to_phys(0x8000_1400);

impl<const SYSTEM: SystemId> System<SYSTEM> {
    pub fn cause_external_interrupt(&mut self) {
        let base: u32 = if self.gekko.msr.exception_prefix() {
            0xFFF0_0000
        } else {
            0
        };

        // Table 4-7. System Reset Exception—Register Settings
        self.gekko.spr.srr0 = Srr0::from(self.gekko.pc);
        self.gekko.spr.srr1 = chapa::extract_bits!(self.gekko.msr; 0, 5..=9, 16..=31).raw();

        self.gekko.msr = self
            .gekko
            .msr
            .with_pow(false)
            .with_fp(false)
            .with_be(false)
            .with_dr(false)
            .with_fe1(false)
            .with_pm(false)
            .with_ee(false)
            .with_fe0(false)
            .with_ri(false)
            .with_pr(false)
            .with_se(false)
            .with_ir(false)
            .with_le(self.gekko.msr.ile());

        self.gekko.pc = base | IRQ_EXTERNAL;

        tracing::debug!(addr = format!("{:08X}", self.gekko.pc), "IRQ triggered");
    }

    pub fn cause_decrementer_interrupt(&mut self) {
        let base: u32 = if self.gekko.msr.exception_prefix() {
            0xFFF0_0000
        } else {
            0
        };

        self.gekko.dec.clear_interrupt();
        self.gekko.spr.srr0 = Srr0::from(self.gekko.pc);
        self.gekko.spr.srr1 = chapa::extract_bits!(self.gekko.msr; 0, 5..=9, 16..=31).raw();

        self.gekko.msr = self
            .gekko
            .msr
            .with_pow(false)
            .with_fp(false)
            .with_be(false)
            .with_dr(false)
            .with_fe1(false)
            .with_ee(false)
            .with_fe0(false)
            .with_ri(false)
            .with_pr(false)
            .with_se(false)
            .with_ir(false)
            .with_le(self.gekko.msr.ile());

        self.gekko.pc = base | IRQ_DECREMENTER;

        tracing::debug!(addr = format!("{:08X}", self.gekko.pc), "decrementer IRQ triggered");
    }

    pub fn cause_trap_exception(&mut self) {
        let base: u32 = if self.gekko.msr.exception_prefix() {
            0xFFF0_0000
        } else {
            0
        };

        self.gekko.spr.srr0 = Srr0::from(self.gekko.cia);
        // SRR1: MSR bits 0, 5-9, 16-31 preserved; bit 14 (TRAP) set
        self.gekko.spr.srr1 = chapa::extract_bits!(self.gekko.msr; 0, 5..=9, 16..=31).raw() | (1 << (31 - 14));

        self.gekko.msr = self
            .gekko
            .msr
            .with_pow(false)
            .with_fp(false)
            .with_be(false)
            .with_dr(false)
            .with_fe1(false)
            .with_pm(false)
            .with_ee(false)
            .with_fe0(false)
            .with_ri(false)
            .with_pr(false)
            .with_se(false)
            .with_ir(false)
            .with_le(self.gekko.msr.ile());

        self.gekko.nia = base | IRQ_PROGRAM;

        tracing::debug!(addr = format!("{:08X}", self.gekko.nia), "trap exception triggered");
    }

    /// Raise a Floating-Point Unavailable exception (0x00800).
    ///
    /// Called when an FP/PS/quantized load-store/move/compute instruction is
    /// dispatched with MSR[FP] = 0. SRR0 holds the address of the offending
    /// instruction; SRR1 is a clean copy of MSR (no status bits set).
    #[inline(always)]
    pub fn cause_fp_unavailable(&mut self) {
        let base: u32 = if self.gekko.msr.exception_prefix() {
            0xFFF0_0000
        } else {
            0
        };

        self.gekko.spr.srr0 = Srr0::from(self.gekko.cia);
        self.gekko.spr.srr1 = chapa::extract_bits!(self.gekko.msr; 0, 5..=9, 16..=31).raw();

        self.gekko.msr = self
            .gekko
            .msr
            .with_pow(false)
            .with_fp(false)
            .with_be(false)
            .with_dr(false)
            .with_fe1(false)
            .with_pm(false)
            .with_ee(false)
            .with_fe0(false)
            .with_ri(false)
            .with_pr(false)
            .with_se(false)
            .with_ir(false)
            .with_le(self.gekko.msr.ile());

        self.gekko.nia = base | IRQ_FP_UNAVAILABLE;

        tracing::debug!(
            addr = format!("{:08X}", self.gekko.nia),
            "FP unavailable exception triggered"
        );
    }

    /// Raise a floating-point-enabled Program exception (0x00700, SRR1[11]=1).
    ///
    /// Triggered when MSR[FE0]|MSR[FE1] is set and FPSCR[FEX] is set after
    /// an instruction that updates FPSCR.
    #[inline(always)]
    pub fn cause_fp_program_exception(&mut self) {
        let base: u32 = if self.gekko.msr.exception_prefix() {
            0xFFF0_0000
        } else {
            0
        };

        self.gekko.spr.srr0 = Srr0::from(self.gekko.cia);
        // SRR1[11] = floating-point enabled exception indicator
        self.gekko.spr.srr1 = chapa::extract_bits!(self.gekko.msr; 0, 5..=9, 16..=31).raw() | (1 << (31 - 11));

        self.gekko.msr = self
            .gekko
            .msr
            .with_pow(false)
            .with_fp(false)
            .with_be(false)
            .with_dr(false)
            .with_fe1(false)
            .with_pm(false)
            .with_ee(false)
            .with_fe0(false)
            .with_ri(false)
            .with_pr(false)
            .with_se(false)
            .with_ir(false)
            .with_le(self.gekko.msr.ile());

        self.gekko.nia = base | IRQ_PROGRAM;

        tracing::debug!(
            addr = format!("{:08X}", self.gekko.nia),
            "FP program exception triggered"
        );
    }

    /// Guard for FP instruction dispatch. Returns true if the instruction may
    /// proceed; false if MSR[FP]=0 and an FP-Unavailable exception was raised
    /// (the caller must return without executing the body or advancing CIA).
    /// Fuck you, motherfucker
    #[inline(always)]
    pub fn check_fp_available(&mut self) -> bool {
        if self.gekko.msr.floating_point_available() {
            true
        } else {
            self.cause_fp_unavailable();
            false
        }
    }

    /// Tail-check after FPSCR may have been updated. If FE0|FE1 is set and
    /// FPSCR[FEX] is set, raise a Program exception (0x00700).
    #[inline(always)]
    pub fn check_fp_program_exception(&mut self) {
        if (self.gekko.msr.fe0() || self.gekko.msr.fe1()) && self.gekko.fpscr.fex() {
            self.cause_fp_program_exception();
        }
    }

    pub fn cause_syscall_interrupt(&mut self) {
        let base: u32 = if self.gekko.msr.exception_prefix() {
            0xFFF0_0000
        } else {
            0
        };

        self.gekko.spr.srr0 = Srr0::from(self.gekko.cia.wrapping_add(4));
        self.gekko.spr.srr1 = chapa::extract_bits!(self.gekko.msr; 0, 5..=9, 16..=31).raw();

        self.gekko.msr = self
            .gekko
            .msr
            .with_pow(false)
            .with_fp(false)
            .with_be(false)
            .with_dr(false)
            .with_fe1(false)
            .with_pm(false)
            .with_ee(false)
            .with_fe0(false)
            .with_ri(false)
            .with_pr(false)
            .with_se(false)
            .with_ir(false)
            .with_le(self.gekko.msr.ile());

        self.gekko.nia = base | IRQ_SYSTEM_CALL;

        tracing::debug!(addr = format!("{:08X}", self.gekko.nia), "system call IRQ triggered");
    }
}
