use colored::Colorize;
use disasm::tokenizer::{self, AsmToken};

pub fn colorize(tok: &AsmToken<'_>) -> String {
    match tok {
        AsmToken::Mnemonic(s) => s.bold().cyan().to_string(),
        AsmToken::Gpr(n) => format!("r{n}").yellow().to_string(),
        AsmToken::Fpr(n) => format!("f{n}").magenta().to_string(),
        AsmToken::CrField(n) => format!("cr{n}").green().to_string(),
        AsmToken::Spr(s) => s.green().bold().to_string(),
        AsmToken::ImmSigned(v) => format!("{v}").blue().to_string(),
        AsmToken::ImmUnsigned(v) => format!("{v}").blue().to_string(),
        AsmToken::ImmHex(v) if *v < 0 => format!("-0x{:X}", -v).blue().to_string(),
        AsmToken::ImmHex(v) => format!("0x{v:X}").blue().to_string(),
        AsmToken::Displacement(v) => format!("{v}").blue().to_string(),
        AsmToken::BranchTarget(s) => s.bright_red().to_string(),
        AsmToken::AddrPrefix | AsmToken::ImmPrefix => tok.to_string().blue().to_string(),
        AsmToken::Punct(_) | AsmToken::Text(_) => tok.to_string(),
    }
}

pub fn colorize_instr(instr: &disasm::gekko::GekkoInstruction) -> String {
    let text = format!("{}", instr);
    colorize_asm(&text)
}

pub fn colorize_dsp_instr(instr: &disasm::dsp::GcDspInstruction) -> String {
    let text = format!("{}", instr);
    colorize_asm(&text)
}

fn colorize_asm(text: &str) -> String {
    let tokens = tokenizer::tokenize(text);
    tokens.into_iter().map(|t| colorize(&t)).collect::<Vec<_>>().join("")
}

pub fn gpr_refs(instr: &disasm::gekko::GekkoInstruction) -> Vec<u8> {
    let text = format!("{}", instr);
    let tokens = tokenizer::tokenize(&text);
    let mut seen = [false; 32];
    let mut refs = Vec::new();
    for tok in tokens {
        if let AsmToken::Gpr(n) = tok {
            let n = n as usize;
            if !seen[n] {
                seen[n] = true;
                refs.push(n as u8);
            }
        }
    }
    refs
}

pub fn fpr_refs(instr: &disasm::gekko::GekkoInstruction) -> Vec<u8> {
    let text = format!("{}", instr);
    let tokens = tokenizer::tokenize(&text);
    let mut seen = [false; 32];
    let mut refs = Vec::new();
    for tok in tokens {
        if let AsmToken::Fpr(n) = tok {
            let n = n as usize;
            if !seen[n] {
                seen[n] = true;
                refs.push(n as u8);
            }
        }
    }
    refs
}

pub fn reg_comment(gprs: &[u32; 32], gpr_refs: &[u8], fprs: &[f64; 32], fpr_refs: &[u8]) -> String {
    let mut parts: Vec<String> = gpr_refs
        .iter()
        .map(|&n| format!("r{}={:08X}", n, gprs[n as usize]))
        .collect();
    for &n in fpr_refs {
        parts.push(format!("f{}={:.6e}", n, fprs[n as usize]));
    }
    if parts.is_empty() {
        return String::new();
    }
    format!("; {}", parts.join(", ")).dimmed().to_string()
}

/// Extract DSP register references from a formatted instruction string and
/// build an inline comment showing their current values.
pub fn dsp_reg_comment(text: &str, regs: &gecko::flipper::dsp::core::Registers) -> String {
    let mut parts: Vec<String> = Vec::new();
    let mut seen = [false; 40]; // indices: 0-31 = reg5, 32 = ac0, 33 = ac1, 34 = ax0, 35 = ax1, 36 = prod

    let bytes = text.as_bytes();
    let len = bytes.len();
    let mut i = 0;
    while i < len {
        if bytes[i] == b'$' {
            let start = i + 1;
            let mut end = start;
            while end < len && (bytes[end].is_ascii_alphanumeric() || bytes[end] == b'.') {
                end += 1;
            }
            let name = &text[start..end];
            match name {
                // Full 40-bit accumulators
                "ac0" if !seen[32] => {
                    seen[32] = true;
                    let v = regs.ac(0) as u64 & 0xFF_FFFF_FFFF;
                    parts.push(format!(
                        "ac0={:02X}_{:04X}_{:04X}",
                        (v >> 32) as u16,
                        (v >> 16) as u16,
                        v as u16
                    ));
                }
                "ac1" if !seen[33] => {
                    seen[33] = true;
                    let v = regs.ac(1) as u64 & 0xFF_FFFF_FFFF;
                    parts.push(format!(
                        "ac1={:02X}_{:04X}_{:04X}",
                        (v >> 32) as u16,
                        (v >> 16) as u16,
                        v as u16
                    ));
                }
                // Full 32-bit AX registers
                "ax0" if !seen[34] => {
                    seen[34] = true;
                    parts.push(format!("ax0={:04X}_{:04X}", regs.axh[0], regs.ax[0]));
                }
                "ax1" if !seen[35] => {
                    seen[35] = true;
                    parts.push(format!("ax1={:04X}_{:04X}", regs.axh[1], regs.ax[1]));
                }
                // Product register
                "prod" if !seen[36] => {
                    seen[36] = true;
                    parts.push(format!(
                        "prod={:04X}_{:04X}_{:04X}_{:04X}",
                        regs.product_mid2, regs.product_high, regs.product_mid1, regs.product_low
                    ));
                }
                _ => {
                    // Individual 16-bit registers (reg5 index 0-31)
                    if let Some(idx) = dsp_reg_index(name)
                        && !seen[idx as usize]
                    {
                        seen[idx as usize] = true;
                        let val = dsp_reg_read(regs, idx);
                        parts.push(format!("{}={:04X}", name, val));
                    }
                }
            }
            i = end;
        } else {
            i += 1;
        }
    }

    if parts.is_empty() {
        return String::new();
    }
    format!("; {}", parts.join(", ")).dimmed().to_string()
}

fn dsp_reg_index(name: &str) -> Option<u8> {
    match name {
        "ar0" => Some(0),
        "ar1" => Some(1),
        "ar2" => Some(2),
        "ar3" => Some(3),
        "ix0" => Some(4),
        "ix1" => Some(5),
        "ix2" => Some(6),
        "ix3" => Some(7),
        "wr0" => Some(8),
        "wr1" => Some(9),
        "wr2" => Some(10),
        "wr3" => Some(11),
        "st0" => Some(12),
        "st1" => Some(13),
        "st2" => Some(14),
        "st3" => Some(15),
        "ac0.h" => Some(16),
        "ac1.h" => Some(17),
        "cr" => Some(18),
        "sr" => Some(19),
        "prod.l" => Some(20),
        "prod.m1" => Some(21),
        "prod.h" => Some(22),
        "prod.m2" => Some(23),
        "ax0.l" => Some(24),
        "ax1.l" => Some(25),
        "ax0.h" => Some(26),
        "ax1.h" => Some(27),
        "ac0.l" => Some(28),
        "ac1.l" => Some(29),
        "ac0.m" => Some(30),
        "ac1.m" => Some(31),
        _ => None,
    }
}

/// Read a DSP register by index without side effects (no stack pops).
fn dsp_reg_read(regs: &gecko::flipper::dsp::core::Registers, idx: u8) -> u16 {
    match idx {
        0..=3 => regs.ar[idx as usize],
        4..=7 => regs.ix[(idx - 4) as usize],
        8..=11 => regs.wr[(idx - 8) as usize],
        12 => regs.call_stack.top(),
        13 => regs.data_stack.top(),
        14 => regs.loop_addr.top(),
        15 => regs.loop_counter.top(),
        16 => regs.ac0_high,
        17 => regs.ac1_high,
        18 => regs.config,
        19 => regs.status.into(),
        20 => regs.product_low,
        21 => regs.product_mid1,
        22 => regs.product_high,
        23 => regs.product_mid2,
        24..=25 => regs.ax[(idx - 24) as usize],
        26..=27 => regs.axh[(idx - 26) as usize],
        28 => regs.ac0_low,
        29 => regs.ac1_low,
        30 => regs.ac0_mid,
        31 => regs.ac1_mid,
        _ => 0,
    }
}

pub fn visible_len(s: &str) -> usize {
    let mut len = 0;
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c == '\x1b' {
            for c2 in chars.by_ref() {
                if c2 == 'm' {
                    break;
                }
            }
        } else {
            len += 1;
        }
    }
    len
}
