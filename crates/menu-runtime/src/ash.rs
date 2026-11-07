//! Pure-Rust bitwise ASH0 / LZ77 decompressor for Wii System Menu archives.
//!
//! Nintendo's System Menu 4.3 and internal Wii channels store UI layouts, icons,
//! and fonts packed in the proprietary ASH0 format (`.ash`). This module decodes
//! those archives into raw `.arc` U8 archives without requiring external binaries.

pub const ASH0_MAGIC: u32 = 0x4153_4830; // 'ASH0'

/// Check if a buffer begins with the 'ASH0' magic signature.
pub fn is_ash_compressed(data: &[u8]) -> bool {
    if data.len() < 8 {
        return false;
    }
    u32::from_be_bytes(data[0..4].try_into().unwrap()) == ASH0_MAGIC
}

/// Bit-level stream reader for big-endian bitstreams.
pub struct BitReader<'a> {
    data: &'a [u8],
    bit_pos: usize,
}

impl<'a> BitReader<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        Self { data, bit_pos: 0 }
    }

    #[inline]
    pub fn read_bit(&mut self) -> Option<u32> {
        let byte_idx = self.bit_pos >> 3;
        if byte_idx >= self.data.len() {
            return None;
        }
        let bit_idx = 7 - (self.bit_pos & 7);
        self.bit_pos += 1;
        Some(((self.data[byte_idx] >> bit_idx) & 1) as u32)
    }

    pub fn read_bits(&mut self, count: usize) -> Option<u32> {
        let mut value = 0u32;
        for _ in 0..count {
            value = (value << 1) | self.read_bit()?;
        }
        Some(value)
    }

    pub fn bits_remaining(&self) -> usize {
        (self.data.len() * 8).saturating_sub(self.bit_pos)
    }
}

/// Decompress an ASH0-compressed byte slice into its uncompressed `.arc` representation.
pub fn decompress_ash0(input: &[u8]) -> Result<Vec<u8>, String> {
    if input.len() < 8 {
        return Err("Input buffer too small for ASH0 header (minimum 8 bytes)".into());
    }

    let magic = u32::from_be_bytes(input[0..4].try_into().unwrap());
    if magic != ASH0_MAGIC {
        return Err(format!("Invalid ASH0 magic header: 0x{magic:08X}"));
    }

    let decompressed_size = u32::from_be_bytes(input[4..8].try_into().unwrap()) as usize;
    if decompressed_size == 0 {
        return Ok(Vec::new());
    }
    if decompressed_size > 64 * 1024 * 1024 {
        return Err(format!("Uncompressed size {decompressed_size} exceeds safety limit of 64 MiB"));
    }

    let mut out = Vec::with_capacity(decompressed_size);
    let mut reader = BitReader::new(&input[8..]);

    while out.len() < decompressed_size {
        match reader.read_bit() {
            Some(0) => {
                // Literal byte
                let literal = reader.read_bits(8).ok_or("Unexpected EOF reading literal byte")? as u8;
                out.push(literal);
            }
            Some(1) => {
                // LZ77 Back-reference token
                // Default 11-bit distance (up to 2048 lookback)
                let dist = reader.read_bits(11).ok_or("Unexpected EOF reading match distance")? as usize + 1;
                let len_code = reader.read_bits(4).ok_or("Unexpected EOF reading match length code")? as usize;
                let length = if len_code == 0 {
                    reader.read_bits(8).ok_or("Unexpected EOF reading extended length")? as usize + 18
                } else {
                    len_code + 2
                };

                if dist > out.len() {
                    return Err(format!(
                        "ASH0 back-reference distance {dist} exceeds current output length {}",
                        out.len()
                    ));
                }

                let start = out.len() - dist;
                for i in 0..length {
                    if out.len() >= decompressed_size {
                        break;
                    }
                    let byte = out[start + (i % dist)];
                    out.push(byte);
                }
            }
            None => break,
            _ => unreachable!(),
        }
    }

    if out.len() != decompressed_size {
        // If the stream ended before reaching the target size, provide clear diagnostic
        return Err(format!(
            "ASH0 decompression incomplete: expected {decompressed_size} bytes, got {}",
            out.len()
        ));
    }

    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_ash_compressed() {
        let invalid = [0u8; 16];
        assert!(!is_ash_compressed(&invalid));

        let valid = [0x41, 0x53, 0x48, 0x30, 0x00, 0x00, 0x00, 0x10];
        assert!(is_ash_compressed(&valid));
    }

    #[test]
    fn test_bit_reader() {
        let data = [0b10110010, 0b11000000];
        let mut reader = BitReader::new(&data);
        assert_eq!(reader.read_bit(), Some(1));
        assert_eq!(reader.read_bit(), Some(0));
        assert_eq!(reader.read_bit(), Some(1));
        assert_eq!(reader.read_bit(), Some(1));
        assert_eq!(reader.read_bits(4), Some(0b0010));
        assert_eq!(reader.read_bits(2), Some(0b11));
    }

    #[test]
    fn test_ash0_decompression_synthetic() {
        // Synthesize an ASH0 stream that outputs "HELLO HELLO!"
        // "HELLO " (literal) + back-ref dist=6, len=5 ("HELLO") + "!" (literal)
        let decompressed = b"HELLO HELLO!";
        let mut bits = Vec::new();

        // Helper macro/function to push bits
        fn push_bit(bits: &mut Vec<u8>, b: u8) {
            bits.push(b);
        }
        fn push_bits(bits: &mut Vec<u8>, val: u32, count: usize) {
            for i in (0..count).rev() {
                bits.push(((val >> i) & 1) as u8);
            }
        }

        // "H", "E", "L", "L", "O", " "
        for &byte in b"HELLO " {
            push_bit(&mut bits, 0); // literal flag
            push_bits(&mut bits, byte as u32, 8);
        }

        // Back-reference: dist=6 (dist-1 = 5), length=5 (len_code = 5 - 2 = 3)
        push_bit(&mut bits, 1); // back-reference flag
        push_bits(&mut bits, 5, 11); // 11-bit distance (5)
        push_bits(&mut bits, 3, 4);  // 4-bit length (3 -> 5)

        // "!"
        push_bit(&mut bits, 0);
        push_bits(&mut bits, b'!' as u32, 8);

        // Pack bits into bytes
        let mut packed = Vec::new();
        for chunk in bits.chunks(8) {
            let mut byte = 0u8;
            for (i, &bit) in chunk.iter().enumerate() {
                byte |= bit << (7 - i);
            }
            packed.push(byte);
        }

        let mut ash_data = Vec::new();
        ash_data.extend_from_slice(b"ASH0");
        ash_data.extend_from_slice(&(decompressed.len() as u32).to_be_bytes());
        ash_data.extend_from_slice(&packed);

        let result = decompress_ash0(&ash_data).expect("Decompression failed");
        assert_eq!(result, decompressed);
    }
}
