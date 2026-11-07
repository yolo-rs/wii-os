use crate::elf::ElfError;
use crate::idb::IdbError;
use crate::symbols::SymbolTable;
use std::path::Path;

#[derive(Debug)]
pub enum SymbolLoadError {
    Io(std::io::Error),
    UnknownFormat(String),
    Elf(ElfError),
    Idb(IdbError),
}

impl std::fmt::Display for SymbolLoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SymbolLoadError::Io(e) => write!(f, "I/O error: {e}"),
            SymbolLoadError::UnknownFormat(ext) => {
                write!(f, "unknown symbol file format: {ext}")
            }
            SymbolLoadError::Elf(e) => write!(f, "{e}"),
            SymbolLoadError::Idb(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for SymbolLoadError {}

impl From<std::io::Error> for SymbolLoadError {
    fn from(e: std::io::Error) -> Self {
        SymbolLoadError::Io(e)
    }
}

impl From<ElfError> for SymbolLoadError {
    fn from(e: ElfError) -> Self {
        SymbolLoadError::Elf(e)
    }
}

impl From<IdbError> for SymbolLoadError {
    fn from(e: IdbError) -> Self {
        SymbolLoadError::Idb(e)
    }
}

pub fn load_symbols(path: &Path) -> Result<SymbolTable, SymbolLoadError> {
    let data = std::fs::read(path)?;

    match path.extension().and_then(|e| e.to_str()) {
        Some("elf") => Ok(crate::elf::parse_elf_symbols(&data)?),
        Some("idb") | Some("i64") => Ok(crate::idb::parse_idb_symbols(&data)?),
        Some(ext) => Err(SymbolLoadError::UnknownFormat(ext.to_string())),
        None => Err(SymbolLoadError::UnknownFormat("(no extension)".to_string())),
    }
}
