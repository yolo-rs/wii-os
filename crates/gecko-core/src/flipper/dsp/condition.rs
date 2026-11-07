use crate::flipper::dsp;

pub enum BranchControl {
    GreaterThanOrEqual = 0b0000,
    LessThan = 0b0001,
    GreaterThan = 0b0010,
    LessThanOrEqual = 0b0011,
    NotZero = 0b0100,
    Zero = 0b0101,
    NotCarry = 0b0110,
    Carry = 0b0111,
    BelowS32 = 0b1000,
    AboveS32 = 0b1001,
    AccumulatorNotZeroExtended = 0b1010,
    AccumulatorNotZero = 0b1011,
    NotLogicalZero = 0b1100,
    LogicalZero = 0b1101,
    Overflow = 0b1110,
    Always = 0b1111,
}

impl BranchControl {
    pub fn from(raw: u8) -> Self {
        assert!(raw <= 0b1111);
        unsafe { std::mem::transmute(raw) }
    }

    pub fn evaluate(&self, dsp: &dsp::Dsp) -> bool {
        match self {
            Self::GreaterThanOrEqual => dsp.registers.status.overflow() == dsp.registers.status.sign(),
            Self::LessThan => dsp.registers.status.overflow() != dsp.registers.status.sign(),
            Self::GreaterThan => {
                (dsp.registers.status.overflow() == dsp.registers.status.sign())
                    && !dsp.registers.status.arithmetic_zero()
            }
            Self::LessThanOrEqual => {
                (dsp.registers.status.overflow() != dsp.registers.status.sign())
                    || dsp.registers.status.arithmetic_zero()
            }
            Self::NotZero => !dsp.registers.status.arithmetic_zero(),
            Self::Zero => dsp.registers.status.arithmetic_zero(),
            Self::NotCarry => !dsp.registers.status.carry(),
            Self::Carry => dsp.registers.status.carry(),
            Self::BelowS32 => !dsp.registers.status.above_s32(),
            Self::AboveS32 => dsp.registers.status.above_s32(),
            Self::AccumulatorNotZeroExtended => {
                (dsp.registers.status.above_s32() || dsp.registers.status.top_two_bits_equal())
                    && !dsp.registers.status.arithmetic_zero()
            }
            Self::AccumulatorNotZero => {
                (!dsp.registers.status.above_s32() && !dsp.registers.status.top_two_bits_equal())
                    || dsp.registers.status.arithmetic_zero()
            }
            Self::NotLogicalZero => !dsp.registers.status.logical_zero(),
            Self::LogicalZero => dsp.registers.status.logical_zero(),
            Self::Overflow => dsp.registers.status.overflow(),
            Self::Always => true,
        }
    }
}
