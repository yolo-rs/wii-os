#[inline(always)]
pub const fn read_be_subword(word: u32, sub_offset: u32, size: u32) -> u32 {
    let shift = (4 - sub_offset - size) * 8;
    let mask = if size >= 4 { !0u32 } else { (1u32 << (size * 8)) - 1 };
    (word >> shift) & mask
}

#[inline(always)]
pub const fn write_be_subword(current: u32, sub_offset: u32, size: u32, val: u32) -> u32 {
    let shift = (4 - sub_offset - size) * 8;
    let mask = if size >= 4 { !0u32 } else { (1u32 << (size * 8)) - 1 };
    (current & !(mask << shift)) | ((val & mask) << shift)
}

pub trait MmioRegister: Sized {
    const ADDR: u32;
    const SIZE: usize;

    fn from_raw(raw: u32) -> Self;
    fn to_raw(self) -> u32;

    fn contains(addr: u32) -> bool {
        addr >= Self::ADDR && addr < Self::ADDR + Self::SIZE as u32
    }

    fn fits(addr: u32, access_size: u32) -> bool {
        addr >= Self::ADDR && addr + access_size <= Self::ADDR + Self::SIZE as u32
    }

    fn read_sub(raw: u32, addr: u32, access_size: u32) -> u32 {
        let sub_offset = addr - Self::ADDR;
        let shift = (Self::SIZE as u32 - sub_offset - access_size) * 8;
        let mask = ((1u64 << (access_size * 8)) - 1) as u32;
        (raw >> shift) & mask
    }

    fn write_sub(full: u32, addr: u32, access_size: u32, val: u32) -> u32 {
        let sub_offset = addr - Self::ADDR;
        let shift = (Self::SIZE as u32 - sub_offset - access_size) * 8;
        let mask = ((1u64 << (access_size * 8)) - 1) as u32;
        (full & !(mask << shift)) | ((val & mask) << shift)
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct WriteMask(u8);

impl WriteMask {
    pub const FULL_U8: Self = Self(0b0001);
    pub const FULL_U16: Self = Self(0b0011);
    pub const FULL_U32: Self = Self(0b1111);

    /// Build a mask from the byte offset within the register and the access
    /// size in bytes.
    #[inline(always)]
    pub const fn from_offset_size(sub_offset: u32, access_size: u32) -> Self {
        Self((((1u32 << access_size) - 1) << sub_offset) as u8)
    }

    /// Was the byte at `idx` (offset from the register base) written?
    #[inline(always)]
    pub const fn byte(self, idx: u32) -> bool {
        (self.0 >> idx) & 1 != 0
    }

    /// Did this write touch any byte in `[start, end)`?
    #[inline(always)]
    pub const fn any(self, start: u32, end: u32) -> bool {
        let m = (((1u32 << (end - start)) - 1) << start) as u8;
        self.0 & m != 0
    }

    #[inline(always)]
    pub const fn raw(self) -> u8 {
        self.0
    }
}

pub trait MmioAccess<C>: MmioRegister {
    fn read(c: &mut C) -> Self;
    fn write(self, c: &mut C, mask: WriteMask);

    #[inline(always)]
    fn read_at(c: &mut C, addr: u32, access_size: u32) -> u32 {
        Self::read_sub(Self::read(c).to_raw(), addr, access_size)
    }

    #[inline(always)]
    fn write_at(c: &mut C, addr: u32, access_size: u32, val: u32) {
        let current = Self::read(c).to_raw();
        let merged = Self::write_sub(current, addr, access_size, val);
        let mask = WriteMask::from_offset_size(addr - Self::ADDR, access_size);
        Self::from_raw(merged).write(c, mask);
    }
}
