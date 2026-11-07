use clap::{Parser, Subcommand, ValueEnum};

pub fn parse_offset(s: &str) -> Result<usize, String> {
    if let Some(hex) = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
        usize::from_str_radix(hex, 16).map_err(|e| e.to_string())
    } else {
        s.parse().map_err(|e: std::num::ParseIntError| e.to_string())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub enum DisasmArch {
    Ppc,
    Dsp,
}

/// Decode/encode direction shared by the file-transform subcommands.
#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub enum Action {
    Decode,
    Encode,
}

#[derive(Subcommand)]
pub enum Command {
    /// Show file header information
    Info {
        /// Input DOL file
        file: String,
    },
    /// Disassemble binary code
    Disasm {
        /// Target architecture
        #[arg(long, value_enum, default_value_t = DisasmArch::Ppc)]
        arch: DisasmArch,
        /// Start offset (hex or decimal)
        #[arg(long, value_parser = parse_offset)]
        offset: Option<usize>,
        /// Input file
        file: String,
    },
    /// Decode or encode a GameCube IPL
    Ipl {
        /// IPL transformation to apply
        #[arg(long, value_enum, default_value_t = Action::Decode)]
        action: Action,
        /// Input IPL file
        file: String,
        /// Output file path (defaults to <name>.encoded.bin or <name>.decoded.bin)
        output: Option<String>,
    },
    /// Decode a Wii SYSCONF to editable text, or encode text back to binary
    Sysconf {
        /// SYSCONF transformation to apply
        #[arg(long, value_enum, default_value_t = Action::Decode)]
        action: Action,
        /// Input file (binary SYSCONF for decode, text for encode)
        file: String,
        /// Output path (defaults to <file>.txt for decode, <file> minus .txt for encode)
        output: Option<String>,
    },
    /// Decode an encrypted Wii setting.txt to editable text, or encode it back
    Setting {
        /// setting.txt transformation to apply
        #[arg(long, value_enum, default_value_t = Action::Decode)]
        action: Action,
        /// Input file (encrypted setting.txt for decode, text for encode)
        file: String,
        /// Output path (defaults to <file>.decoded for decode, <file> minus .decoded for encode)
        output: Option<String>,
    },
    /// Dump GameCube/Wii disc image (ISO or RVZ) information
    Dvd {
        /// Input disc image (auto-detected: ISO, RVZ, or ZIP-wrapped)
        file: String,
        /// Extract files from the disc image to an "output" directory
        #[arg(long)]
        extract: bool,
        /// Extract only the main DOL executable to this path
        #[arg(long, conflicts_with = "extract")]
        main_dol: Option<String>,
    },
    /// Deep RE & Native Porting Extractor: extracts all disc assets (FST, AFS, PAK, TPL, THP, BNR)
    /// and recovers all PowerPC functions, C++ classes/vtables, strings/xrefs, and native Rust SDK stubs
    ReExtract {
        /// Input disc image (ISO or RVZ)
        file: String,
        /// Output directory for extracted assets, code catalog, and native Rust SDK
        #[arg(long, default_value = "userland/build/re/budokai-tenkaichi-3-eu")]
        out_dir: String,
    },
}

#[derive(Parser)]
#[command(
    about = "GameCube/Wii multitool",
    long_about = None,
    after_help = "Repository: https://github.com/ioncodes/gecko",
)]
pub struct Args {
    #[command(subcommand)]
    pub command: Command,
}
