use crate::gekko::lut::*;

pub const DEFAULT_CYCLES: i64 = 1;

#[inline]
pub const fn cycles_for_op(op: u32) -> i64 {
    match op {
        OP_MULLI => 3,
        OP_MULLWX | OP_MULHWX | OP_MULHWUX => 5,
        OP_DIVWX | OP_DIVWUX => 40,
        OP_FDIVSX => 17,
        OP_FDIVX => 31,
        OP_PS_DIV => 17,
        OP_PS_RSQRTE => 2,
        OP_LMW | OP_STMW => 11,
        OP_ICBI => 4,
        OP_DCBF | OP_DCBI | OP_DCBST | OP_DCBZ | OP_DCBZ_L => 5,
        OP_DCBT | OP_DCBTST => 2,
        OP_SYNC => 3,
        OP_MTSPR => 2,
        OP_MFSR | OP_MFSRIN => 3,
        OP_MTFSB0X | OP_MTFSB1X | OP_MTFSFIX | OP_MTFSFX => 3,
        OP_TW => 2,
        OP_RFI | OP_SC => 2,
        _ => DEFAULT_CYCLES,
    }
}
