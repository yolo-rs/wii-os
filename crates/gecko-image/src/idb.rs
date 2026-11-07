use crate::symbols::{Symbol, SymbolKind, SymbolTable};
use idb_rs::addr_info::AddressInfo;
use idb_rs::id0::function::IDBFunctionType;
use idb_rs::{Address, IDAKind, IDAVariants, IDBFormat, IDBFormats, identify_idb_file};
use std::io::{BufRead, BufReader, Cursor, Seek};

#[derive(Debug)]
pub enum IdbError {
    Parse(String),
}

impl std::fmt::Display for IdbError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            IdbError::Parse(msg) => write!(f, "IDB parse error: {msg}"),
        }
    }
}

impl std::error::Error for IdbError {}

pub fn parse_idb_symbols(data: &[u8]) -> Result<SymbolTable, IdbError> {
    let mut cursor = BufReader::new(Cursor::new(data));
    let format = identify_idb_file(&mut cursor).map_err(|e| IdbError::Parse(e.to_string()))?;

    match format {
        IDBFormats::Separated(IDAVariants::IDA32(sections)) => extract_symbols(sections, &mut cursor),
        IDBFormats::Separated(IDAVariants::IDA64(sections)) => extract_symbols(sections, &mut cursor),
        IDBFormats::InlineUncompressed(sections) => extract_symbols(sections, &mut cursor),
        IDBFormats::InlineCompressed(compressed) => {
            let mut decompressed = Vec::new();
            let sections = compressed
                .decompress_into_memory(&mut cursor, &mut decompressed)
                .map_err(|e| IdbError::Parse(e.to_string()))?;
            extract_symbols(sections, Cursor::new(&decompressed[..]))
        }
    }
}

fn extract_symbols<K, F, I>(idb: F, mut input: I) -> Result<SymbolTable, IdbError>
where
    K: IDAKind,
    F: IDBFormat<K>,
    I: BufRead + Seek,
    u64: From<K::Usize>,
{
    let id0_loc = idb
        .id0_location()
        .ok_or_else(|| IdbError::Parse("IDB has no ID0 section".into()))?;
    let id1_loc = idb
        .id1_location()
        .ok_or_else(|| IdbError::Parse("IDB has no ID1 section".into()))?;
    let id2_loc = idb.id2_location();

    let id0 = idb
        .read_id0(&mut input, id0_loc)
        .map_err(|e| IdbError::Parse(e.to_string()))?;
    let id1 = idb
        .read_id1(&mut input, id1_loc)
        .map_err(|e| IdbError::Parse(e.to_string()))?;
    let id2 = id2_loc
        .map(|loc| idb.read_id2(&mut input, loc))
        .transpose()
        .map_err(|e| IdbError::Parse(e.to_string()))?;

    let root_info_idx = id0.root_node().map_err(|e| IdbError::Parse(e.to_string()))?;
    let root_info = id0
        .ida_info(root_info_idx)
        .map_err(|e| IdbError::Parse(e.to_string()))?;
    let netdelta = root_info.netdelta();

    let mut symbols = Vec::new();

    // extract function chunks with address ranges
    if let Some(funcs_idx) = id0.funcs_idx().map_err(|e| IdbError::Parse(e.to_string()))? {
        for func_result in id0.fchunks(funcs_idx) {
            let func = match func_result {
                Ok(f) => f,
                Err(_) => continue,
            };

            // skip tail chunks, only take real function entries
            if matches!(func.extra, IDBFunctionType::Tail(_)) {
                continue;
            }

            let start: u64 = func.address.start.into_raw().into();
            let end: u64 = func.address.end.into_raw().into();
            let addr = start as u32;
            let size = (end - start) as u32;

            // resolve name via AddressInfo label
            let name = AddressInfo::new(&id0, &id1, id2.as_ref(), netdelta, func.address.start)
                .and_then(|info| info.label().ok().flatten())
                .map(|s| s.as_utf8_lossy().into_owned());

            // fall back to forced lookup (for labels outside mapped regions)
            let name = name.or_else(|| {
                AddressInfo::new_forced(&id0, netdelta, func.address.start)
                    .and_then(|info| info.label().ok().flatten())
                    .map(|s| s.as_utf8_lossy().into_owned())
            });

            if let Some(name) = name {
                if !name.is_empty() {
                    symbols.push(Symbol {
                        name,
                        addr,
                        size,
                        kind: SymbolKind::Func,
                    });
                }
            }
        }
    }

    // also pick up named addresses from the dirtree that aren't already covered
    if let Ok(Some(dirtree)) = id0.dirtree_function_address() {
        let mut entries = dirtree.entries;
        while let Some(entry) = entries.pop() {
            match entry {
                idb_rs::id0::DirTreeEntry::Leaf(raw_addr) => {
                    let addr32: u64 = raw_addr.into();
                    let addr32 = addr32 as u32;

                    // skip if we already have a symbol at this address
                    if symbols.iter().any(|s| s.addr == addr32) {
                        continue;
                    }

                    let address = Address::<K>::from_raw(raw_addr);
                    let name = AddressInfo::new(&id0, &id1, id2.as_ref(), netdelta, address)
                        .and_then(|info| info.label().ok().flatten())
                        .map(|s| s.as_utf8_lossy().into_owned());

                    if let Some(name) = name {
                        if !name.is_empty() {
                            symbols.push(Symbol {
                                name,
                                addr: addr32,
                                size: 0,
                                kind: SymbolKind::Func,
                            });
                        }
                    }
                }
                idb_rs::id0::DirTreeEntry::Directory { entries: sub, .. } => {
                    entries.extend(sub);
                }
            }
        }
    }

    Ok(SymbolTable::new(symbols))
}
