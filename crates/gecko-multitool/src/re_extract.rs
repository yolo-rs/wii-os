use disasm::gekko::GekkoInstruction;
use image::dol::Dol;
use image::dvd::{DVD_APPLOADER_OFFSET, FstNode};
use image::{Dvd, Executable};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fs;
use std::io::BufWriter;
use std::path::Path;

#[derive(Serialize, Clone, Debug)]
pub struct DiscFileEntry {
    pub path: String,
    pub name: String,
    pub partition_offset: u64,
    pub partition_offset_hex: String,
    pub end_offset_hex: String,
    pub wii_word_offset_hex: String,
    pub size: u32,
    pub category: String,
    pub extracted_to: Option<String>,
}

#[derive(Serialize, Clone, Debug)]
pub struct AfsEntryInfo {
    pub index: usize,
    pub name: String,
    pub offset: u32,
    pub size: u32,
    pub magic_hex: String,
    pub kind: String,
    pub pak_subfile_count: Option<usize>,
    pub extracted_path: Option<String>,
}

#[derive(Serialize, Clone, Debug)]
pub struct AfsArchiveCatalog {
    pub archive_name: String,
    pub partition_offset_hex: String,
    pub archive_size: u32,
    pub entry_count: usize,
    pub pak_count: usize,
    pub adx_count: usize,
    pub entries_sample: Vec<AfsEntryInfo>,
}

#[derive(Serialize, Clone, Debug)]
pub struct ThpVideoInfo {
    pub name: String,
    pub partition_offset_hex: String,
    pub size: u32,
    pub version_hex: String,
    pub fps: f32,
    pub num_frames: u32,
    pub width: u32,
    pub height: u32,
    pub audio_channels: u32,
    pub audio_sample_rate: u32,
    pub first_frame_offset: u32,
    pub first_frame_size: u32,
    pub extracted_keyframes: Vec<String>,
}

#[derive(Serialize, Clone, Debug)]
pub struct DecodedTextureInfo {
    pub source_file: String,
    pub texture_index: usize,
    pub width: u16,
    pub height: u16,
    pub format_id: u32,
    pub format_name: String,
    pub png_path: String,
}

#[derive(Serialize, Clone, Debug)]
pub struct StringXref {
    pub vaddr_hex: String,
    pub section: String,
    pub text: String,
    pub referenced_by_functions: Vec<String>,
}

#[derive(Serialize, Clone, Debug)]
pub struct VTableMethodInfo {
    pub slot: usize,
    pub vaddr_hex: String,
    pub inferred_name: String,
    pub size_bytes: u32,
}

#[derive(Serialize, Clone, Debug)]
pub struct ClassVTableInfo {
    pub class_id: usize,
    pub inferred_class_name: String,
    pub subsystem: String,
    pub vtable_vaddr_hex: String,
    pub method_count: usize,
    pub constructor_vaddrs: Vec<String>,
    pub associated_strings: Vec<String>,
    pub methods: Vec<VTableMethodInfo>,
}

#[derive(Serialize, Clone, Debug)]
pub struct FunctionInfo {
    pub vaddr: u32,
    pub vaddr_hex: String,
    pub end_vaddr_hex: String,
    pub size_bytes: u32,
    pub instruction_count: u32,
    pub stack_frame_bytes: i32,
    pub is_leaf: bool,
    pub inferred_name: String,
    pub subsystem: String,
    pub caller_count: usize,
    pub sample_callers: Vec<String>,
    pub callees: Vec<String>,
    pub string_refs: Vec<String>,
    pub vtable_refs: Vec<String>,
}

#[derive(Serialize, Clone, Debug)]
pub struct ReMasterManifest {
    pub game_title: String,
    pub game_id: String,
    pub maker_code: String,
    pub apploader_date: String,
    pub dol_entrypoint_hex: String,
    pub dol_size_bytes: usize,
    pub total_disc_files: usize,
    pub extracted_disc_files: usize,
    pub total_afs_archives: usize,
    pub total_afs_subfiles: usize,
    pub decoded_textures_count: usize,
    pub thp_videos: Vec<ThpVideoInfo>,
    pub recovered_functions_count: usize,
    pub recovered_classes_vtables_count: usize,
    pub recovered_strings_count: usize,
    pub subsystem_breakdown: BTreeMap<String, usize>,
    pub cvs_repository_metadata: BTreeMap<String, String>,
    pub spike_dir_descriptors: BTreeMap<String, String>,
}

pub fn run(data: Vec<u8>, out_dir: &Path) {
    println!("=== [BT3 Native Port & RE Extractor] Starting full extraction ===");
    fs::create_dir_all(out_dir).expect("create out_dir");
    let assets_dir = out_dir.join("assets");
    let fst_out_dir = assets_dir.join("fst");
    let afs_out_dir = assets_dir.join("afs");
    let tex_out_dir = assets_dir.join("textures_png");
    let video_out_dir = assets_dir.join("video");
    let code_out_dir = out_dir.join("code");
    let sdk_out_dir = out_dir.join("native_port_sdk");

    for d in [
        &assets_dir,
        &fst_out_dir,
        &afs_out_dir,
        &tex_out_dir,
        &video_out_dir,
        &code_out_dir,
        &sdk_out_dir,
    ] {
        fs::create_dir_all(d).expect("create subdir");
    }

    let dvd = image::load_dvd(data);
    let header = dvd.header();
    let game_title = String::from_utf8_lossy(&header.game_name)
        .trim_end_matches('\0')
        .trim()
        .to_string();
    let game_id = header.game_id();
    let maker_code = String::from_utf8_lossy(&header.maker_code).to_string();
    let apploader_date = String::from_utf8_lossy(&dvd.apploader().timestamp)
        .trim_end_matches('\0')
        .to_string();

    println!("1. Extracting Apploader & main.dol for {} ({})...", game_title, game_id);
    let apploader_size = dvd.apploader().size.get() as usize + dvd.apploader().trailer_size.get() as usize + 0x20;
    let mut apploader_bytes = vec![0u8; apploader_size];
    dvd.read_disc_into(DVD_APPLOADER_OFFSET, &mut apploader_bytes);
    fs::write(out_dir.join("apploader.bin"), &apploader_bytes).expect("write apploader.bin");

    let dol_offset = header.offset_main_executable.get() as usize;
    let mut dol_hdr = vec![0u8; 0x100];
    dvd.read_disc_into(dol_offset, &mut dol_hdr);
    let dol_size = Dol::parse(dol_hdr).size();
    let mut dol_bytes = vec![0u8; dol_size];
    dvd.read_disc_into(dol_offset, &mut dol_bytes);
    fs::write(out_dir.join("main.dol"), &dol_bytes).expect("write main.dol");

    println!("2. Walking FST & extracting disc files...");
    let fst_offset = header.offset_filesystem.get() as usize;
    let fst_size = header.filesystem_size.get() as usize;
    let mut fst_buf = vec![0u8; fst_size];
    dvd.read_disc_into(fst_offset, &mut fst_buf);
    let shift = if header.is_wii() { 2 } else { 0 };
    let fst_root = FstNode::parse(&fst_buf, shift);

    let mut disc_files = Vec::new();
    let mut cvs_metadata = BTreeMap::new();
    let mut spike_descriptors = BTreeMap::new();
    let mut decoded_textures = Vec::new();
    let mut afs_catalogs = Vec::new();
    let mut thp_videos = Vec::new();

    extract_fst_recursive(
        dvd.as_ref(),
        &fst_root,
        "",
        &fst_out_dir,
        &afs_out_dir,
        &tex_out_dir,
        &video_out_dir,
        &mut disc_files,
        &mut cvs_metadata,
        &mut spike_descriptors,
        &mut decoded_textures,
        &mut afs_catalogs,
        &mut thp_videos,
    );

    // Sort disc files by partition offset so offset lookups are instant
    disc_files.sort_by_key(|f| f.partition_offset);
    fs::write(
        assets_dir.join("disc_files.json"),
        serde_json::to_string_pretty(&disc_files).unwrap(),
    )
    .unwrap();
    fs::write(
        assets_dir.join("afs_catalog.json"),
        serde_json::to_string_pretty(&afs_catalogs).unwrap(),
    )
    .unwrap();
    fs::write(
        assets_dir.join("textures_decoded.json"),
        serde_json::to_string_pretty(&decoded_textures).unwrap(),
    )
    .unwrap();
    fs::write(
        assets_dir.join("thp_videos.json"),
        serde_json::to_string_pretty(&thp_videos).unwrap(),
    )
    .unwrap();

    println!(
        "   -> Disc files indexed: {} (AFS archives: {}, decoded TPL textures: {}, THP videos: {})",
        disc_files.len(),
        afs_catalogs.len(),
        decoded_textures.len(),
        thp_videos.len()
    );

    println!("3. Analyzing main.dol (PowerPC functions, C++ classes/vtables, strings & xrefs)...");
    let dol = Dol::parse(dol_bytes.clone());
    let code_analysis = analyze_dol(&dol, &dol_bytes, &code_out_dir);

    println!(
        "   -> Recovered {} PowerPC functions, {} C++ classes/vtables, {} strings with xrefs!",
        code_analysis.functions.len(),
        code_analysis.classes.len(),
        code_analysis.strings.len()
    );

    println!("4. Generating Native Rust Porting SDK in {}...", sdk_out_dir.display());
    generate_native_rust_sdk(
        &sdk_out_dir,
        &game_title,
        &game_id,
        &disc_files,
        &afs_catalogs,
        &code_analysis,
    );

    let total_afs_subfiles: usize = afs_catalogs.iter().map(|a| a.entry_count).sum();
    let extracted_disc_files = disc_files.iter().filter(|f| f.extracted_to.is_some()).count();

    let manifest = ReMasterManifest {
        game_title,
        game_id,
        maker_code,
        apploader_date,
        dol_entrypoint_hex: format!("0x{:08X}", dol.entry_point()),
        dol_size_bytes: dol_size,
        total_disc_files: disc_files.len(),
        extracted_disc_files,
        total_afs_archives: afs_catalogs.len(),
        total_afs_subfiles,
        decoded_textures_count: decoded_textures.len(),
        thp_videos,
        recovered_functions_count: code_analysis.functions.len(),
        recovered_classes_vtables_count: code_analysis.classes.len(),
        recovered_strings_count: code_analysis.strings.len(),
        subsystem_breakdown: code_analysis.subsystem_counts,
        cvs_repository_metadata: cvs_metadata,
        spike_dir_descriptors: spike_descriptors,
    };

    fs::write(
        out_dir.join("manifest.json"),
        serde_json::to_string_pretty(&manifest).unwrap(),
    )
    .unwrap();

    println!("=== [BT3 Native Port & RE Extractor] Complete! Output at {} ===", out_dir.display());
}

#[allow(clippy::too_many_arguments)]
fn extract_fst_recursive(
    dvd: &dyn Dvd,
    node: &FstNode,
    parent_rel: &str,
    fst_out_dir: &Path,
    afs_out_dir: &Path,
    tex_out_dir: &Path,
    video_out_dir: &Path,
    disc_files: &mut Vec<DiscFileEntry>,
    cvs_metadata: &mut BTreeMap<String, String>,
    spike_descriptors: &mut BTreeMap<String, String>,
    decoded_textures: &mut Vec<DecodedTextureInfo>,
    afs_catalogs: &mut Vec<AfsArchiveCatalog>,
    thp_videos: &mut Vec<ThpVideoInfo>,
) {
    match node {
        FstNode::Directory { name, children } => {
            let rel = if parent_rel.is_empty() {
                name.clone()
            } else if name.is_empty() {
                parent_rel.to_string()
            } else {
                format!("{}/{}", parent_rel, name)
            };
            let target_dir = if rel.is_empty() {
                fst_out_dir.to_path_buf()
            } else {
                fst_out_dir.join(&rel)
            };
            let _ = fs::create_dir_all(&target_dir);
            for child in children {
                extract_fst_recursive(
                    dvd,
                    child,
                    &rel,
                    fst_out_dir,
                    afs_out_dir,
                    tex_out_dir,
                    video_out_dir,
                    disc_files,
                    cvs_metadata,
                    spike_descriptors,
                    decoded_textures,
                    afs_catalogs,
                    thp_videos,
                );
            }
        }
        FstNode::File { name, offset, size } => {
            let rel_path = if parent_rel.is_empty() {
                name.clone()
            } else {
                format!("{}/{}", parent_rel, name)
            };
            let category = classify_file(&rel_path);
            let mut extracted_to = None;

            // Extract all files <= 32 MB directly into assets/fst/
            if *size <= 32 * 1024 * 1024 {
                let mut buf = vec![0u8; *size as usize];
                dvd.read_disc_into(*offset as usize, &mut buf);
                let out_path = fst_out_dir.join(&rel_path);
                if let Some(p) = out_path.parent() {
                    let _ = fs::create_dir_all(p);
                }
                fs::write(&out_path, &buf).expect("write FST file");
                extracted_to = Some(format!("assets/fst/{}", rel_path));

                if rel_path.contains("CVS/") || rel_path == ".cvsignore" {
                    let txt = String::from_utf8_lossy(&buf).trim().to_string();
                    cvs_metadata.insert(rel_path.clone(), txt);
                }
                if name.ends_with(".dir") || name.ends_with(".alg") || name.ends_with(".csv") || name.ends_with(".txt") {
                    let txt = if buf.iter().all(|&b| b.is_ascii_graphic() || b.is_ascii_whitespace()) {
                        String::from_utf8_lossy(&buf).trim().to_string()
                    } else {
                        format!("HEX({} bytes): {:02X?}", buf.len(), &buf[..buf.len().min(64)])
                    };
                    spike_descriptors.insert(rel_path.clone(), txt);
                }
                if name.ends_with(".tpl") {
                    decode_tpl_bytes(&buf, &rel_path, tex_out_dir, decoded_textures);
                }
                if name == "opening.bnr" {
                    extract_opening_bnr(&buf, tex_out_dir, decoded_textures);
                }
                if name.ends_with(".arc") {
                    scan_and_decode_embedded_tpls(&buf, &rel_path, tex_out_dir, decoded_textures);
                }
            }

            if name.ends_with(".afs") {
                let cat = process_afs_archive(dvd, name, *offset, *size, afs_out_dir, tex_out_dir, decoded_textures);
                afs_catalogs.push(cat);
            }

            if name.ends_with(".thp") {
                if let Some(thp) = process_thp_video(dvd, name, *offset, *size, video_out_dir) {
                    thp_videos.push(thp);
                }
            }

            disc_files.push(DiscFileEntry {
                path: rel_path,
                name: name.clone(),
                partition_offset: *offset,
                partition_offset_hex: format!("0x{:09X}", offset),
                end_offset_hex: format!("0x{:09X}", offset + (*size as u64)),
                wii_word_offset_hex: format!("0x{:08X}", offset >> 2),
                size: *size,
                category,
                extracted_to,
            });
        }
    }
}

fn classify_file(rel_path: &str) -> String {
    if rel_path.contains("CVS") || rel_path.ends_with(".cvsignore") {
        "cvs_scm_metadata".to_string()
    } else if rel_path.ends_with(".afs") {
        "criware_afs_archive".to_string()
    } else if rel_path.ends_with(".alg") || rel_path.ends_with(".dir") {
        "spike_archive_descriptor".to_string()
    } else if rel_path.ends_with(".thp") {
        "wii_thp_video_stream".to_string()
    } else if rel_path.ends_with(".brsar") || rel_path.ends_with(".pcm") || rel_path.ends_with(".wt") || rel_path.ends_with(".DLS") || rel_path.ends_with(".mid") {
        "audio_soundbank_midi".to_string()
    } else if rel_path.starts_with("HomeButton2") {
        "wii_home_button_overlay".to_string()
    } else if rel_path.ends_with(".tpl") || rel_path.ends_with(".dbt") {
        "wii_texture_font".to_string()
    } else if rel_path == "opening.bnr" {
        "wii_disc_banner_imet".to_string()
    } else {
        "game_data".to_string()
    }
}

fn process_afs_archive(
    dvd: &dyn Dvd,
    name: &str,
    offset: u64,
    size: u32,
    afs_out_dir: &Path,
    tex_out_dir: &Path,
    decoded_textures: &mut Vec<DecodedTextureInfo>,
) -> AfsArchiveCatalog {
    // Read first 64KB of AFS archive (contains header + offset/size table)
    let hdr_len = (size as usize).min(256 * 1024);
    let mut hdr = vec![0u8; hdr_len];
    dvd.read_disc_into(offset as usize, &mut hdr);

    if hdr.len() < 8 || &hdr[0..4] != b"AFS\0" {
        return AfsArchiveCatalog {
            archive_name: name.to_string(),
            partition_offset_hex: format!("0x{:09X}", offset),
            archive_size: size,
            entry_count: 0,
            pak_count: 0,
            adx_count: 0,
            entries_sample: Vec::new(),
        };
    }

    let count = u32::from_le_bytes(hdr[4..8].try_into().unwrap()) as usize;
    let table_end = 8 + count * 8;
    let mut raw_entries = Vec::with_capacity(count);
    for i in 0..count {
        let base = 8 + i * 8;
        if base + 8 > hdr.len() {
            break;
        }
        let e_off = u32::from_le_bytes(hdr[base..base + 4].try_into().unwrap());
        let e_sz = u32::from_le_bytes(hdr[base + 4..base + 8].try_into().unwrap());
        raw_entries.push((e_off, e_sz));
    }

    // Optional filename directory table pointer sits right after the entries table
    let mut names = vec![String::new(); raw_entries.len()];
    if table_end + 8 <= hdr.len() {
        let meta_off = u32::from_le_bytes(hdr[table_end..table_end + 4].try_into().unwrap()) as usize;
        let meta_sz = u32::from_le_bytes(hdr[table_end + 4..table_end + 8].try_into().unwrap()) as usize;
        if meta_off > 0 && meta_sz >= raw_entries.len() * 48 && (meta_off as u64) + (meta_sz as u64) <= size as u64 {
            let mut meta_buf = vec![0u8; meta_sz];
            dvd.read_disc_into((offset as usize) + meta_off, &mut meta_buf);
            for (i, slot) in names.iter_mut().enumerate() {
                let mbase = i * 48;
                if mbase + 32 <= meta_buf.len() {
                    let raw_name = &meta_buf[mbase..mbase + 32];
                    let end = raw_name.iter().position(|&b| b == 0).unwrap_or(32);
                    let s = String::from_utf8_lossy(&raw_name[..end]).trim().to_string();
                    if !s.is_empty() && s.chars().all(|c| c.is_ascii_graphic() || c == ' ') {
                        *slot = s;
                    }
                }
            }
        }
    }

    let archive_stem = name.trim_end_matches(".afs");
    let archive_extract_dir = afs_out_dir.join(archive_stem);
    let _ = fs::create_dir_all(&archive_extract_dir);

    let is_small_afs = size <= 4 * 1024 * 1024;
    let max_inspect = if is_small_afs { raw_entries.len() } else { raw_entries.len().min(96) };
    let max_extract = if is_small_afs { raw_entries.len() } else { raw_entries.len().min(20) };

    let mut pak_count = 0usize;
    let mut adx_count = 0usize;
    let mut entries_sample = Vec::new();

    for (i, &(e_off, e_sz)) in raw_entries.iter().enumerate() {
        if e_sz == 0 {
            continue;
        }
        if i >= max_inspect {
            break;
        }
        let probe_len = (e_sz as usize).min(64);
        let mut probe = vec![0u8; probe_len];
        dvd.read_disc_into((offset as usize) + (e_off as usize), &mut probe);

        let magic_hex = probe
            .iter()
            .take(8)
            .map(|b| format!("{:02X}", b))
            .collect::<Vec<_>>()
            .join("");

        let (kind, pak_subfiles) = classify_afs_subfile(&probe, e_sz);
        if kind.starts_with("spike_pak") {
            pak_count += 1;
        } else if kind.starts_with("cri_adx") {
            adx_count += 1;
        }

        let ext = if kind.starts_with("spike_pak") {
            "pak"
        } else if kind.starts_with("cri_adx") {
            "adx"
        } else {
            "bin"
        };

        let entry_name = if !names[i].is_empty() {
            names[i].clone()
        } else {
            format!("{}_{:04}.{}", archive_stem, i, ext)
        };

        let mut extracted_path = None;
        if i < max_extract && e_sz <= 16 * 1024 * 1024 {
            let mut full_entry = vec![0u8; e_sz as usize];
            dvd.read_disc_into((offset as usize) + (e_off as usize), &mut full_entry);
            let sanitized = entry_name.replace(['/', '\\', ':'], "_");
            let file_out = archive_extract_dir.join(&sanitized);
            fs::write(&file_out, &full_entry).expect("write AFS entry");
            extracted_path = Some(format!("assets/afs/{}/{}", archive_stem, sanitized));

            // If this is a Spike PAK file in wzs3eu0 or first 6 entries, unpack its sub-chunks!
            if (is_small_afs || i < 6) && kind.starts_with("spike_pak") {
                unpack_spike_pak(&full_entry, &archive_extract_dir.join(format!("{}_unpacked", sanitized)), &sanitized, tex_out_dir, decoded_textures);
            }
        }

        entries_sample.push(AfsEntryInfo {
            index: i,
            name: entry_name,
            offset: e_off,
            size: e_sz,
            magic_hex,
            kind,
            pak_subfile_count: pak_subfiles,
            extracted_path,
        });
    }

    AfsArchiveCatalog {
        archive_name: name.to_string(),
        partition_offset_hex: format!("0x{:09X}", offset),
        archive_size: size,
        entry_count: raw_entries.len(),
        pak_count,
        adx_count,
        entries_sample,
    }
}

fn classify_afs_subfile(probe: &[u8], total_size: u32) -> (String, Option<usize>) {
    if probe.len() >= 4 && probe[0] == 0x80 && probe[1] == 0x00 {
        return ("cri_adx_audio".to_string(), None);
    }
    if probe.len() >= 12 {
        // Check big-endian or little-endian Spike PAK header:
        // count (1..512), followed by first offset == (count + 1 or count + 2) * 4 aligned
        let count_be = u32::from_be_bytes(probe[0..4].try_into().unwrap()) as usize;
        let off0_be = u32::from_be_bytes(probe[4..8].try_into().unwrap());
        if (1..=1024).contains(&count_be) && off0_be >= ((count_be as u32 + 1) * 4) && off0_be < total_size {
            return ("spike_pak_be".to_string(), Some(count_be));
        }
        let count_le = u32::from_le_bytes(probe[0..4].try_into().unwrap()) as usize;
        let off0_le = u32::from_le_bytes(probe[4..8].try_into().unwrap());
        if (1..=1024).contains(&count_le) && off0_le >= ((count_le as u32 + 1) * 4) && off0_le < total_size {
            return ("spike_pak_le".to_string(), Some(count_le));
        }
    }
    ("raw_asset_chunk".to_string(), None)
}

fn unpack_spike_pak(
    pak: &[u8],
    out_dir: &Path,
    pak_label: &str,
    tex_out_dir: &Path,
    decoded_textures: &mut Vec<DecodedTextureInfo>,
) {
    if pak.len() < 12 {
        return;
    }
    let count_be = u32::from_be_bytes(pak[0..4].try_into().unwrap()) as usize;
    let off0_be = u32::from_be_bytes(pak[4..8].try_into().unwrap()) as usize;
    let is_be = (1..=1024).contains(&count_be) && off0_be >= (count_be + 1) * 4 && off0_be <= pak.len();
    let count = if is_be {
        count_be
    } else {
        u32::from_le_bytes(pak[0..4].try_into().unwrap()) as usize
    };
    if !(1..=1024).contains(&count) || (count + 2) * 4 > pak.len() {
        return;
    }

    let read_u32 = |idx: usize| -> usize {
        let base = idx * 4;
        if is_be {
            u32::from_be_bytes(pak[base..base + 4].try_into().unwrap()) as usize
        } else {
            u32::from_le_bytes(pak[base..base + 4].try_into().unwrap()) as usize
        }
    };

    let _ = fs::create_dir_all(out_dir);
    for i in 0..count {
        let start = read_u32(1 + i);
        let end = read_u32(2 + i).min(pak.len());
        if start < end && end <= pak.len() {
            let sub = &pak[start..end];
            let sub_name = format!("sub_{:03}.bin", i);
            let _ = fs::write(out_dir.join(&sub_name), sub);
            scan_and_decode_embedded_tpls(sub, &format!("{}/{}", pak_label, sub_name), tex_out_dir, decoded_textures);
        }
    }
}

fn scan_and_decode_embedded_tpls(
    buf: &[u8],
    source_label: &str,
    tex_out_dir: &Path,
    decoded_textures: &mut Vec<DecodedTextureInfo>,
) {
    // Wii TPL magic is 0x0020AF30
    let mut pos = 0;
    while pos + 16 <= buf.len() {
        if buf[pos..pos + 4] == [0x00, 0x20, 0xAF, 0x30] {
            let count = u32::from_be_bytes(buf[pos + 4..pos + 8].try_into().unwrap());
            let hdr_sz = u32::from_be_bytes(buf[pos + 8..pos + 12].try_into().unwrap());
            if (1..=256).contains(&count) && hdr_sz == 0x0C {
                decode_tpl_bytes(&buf[pos..], &format!("{}@0x{:X}", source_label, pos), tex_out_dir, decoded_textures);
            }
        }
        pos += 4;
    }
}

fn extract_opening_bnr(
    buf: &[u8],
    tex_out_dir: &Path,
    decoded_textures: &mut Vec<DecodedTextureInfo>,
) {
    scan_and_decode_embedded_tpls(buf, "opening.bnr", tex_out_dir, decoded_textures);
}

fn decode_tpl_bytes(
    tpl: &[u8],
    source_file: &str,
    tex_out_dir: &Path,
    decoded_textures: &mut Vec<DecodedTextureInfo>,
) {
    if tpl.len() < 12 || tpl[0..4] != [0x00, 0x20, 0xAF, 0x30] {
        return;
    }
    let num_textures = u32::from_be_bytes(tpl[4..8].try_into().unwrap()) as usize;
    let table_offset = u32::from_be_bytes(tpl[8..12].try_into().unwrap()) as usize;
    if num_textures > 256 || table_offset + num_textures * 8 > tpl.len() {
        return;
    }

    for idx in 0..num_textures {
        let entry_off = table_offset + idx * 8;
        let tex_hdr_off = u32::from_be_bytes(tpl[entry_off..entry_off + 4].try_into().unwrap()) as usize;
        let _pal_hdr_off = u32::from_be_bytes(tpl[entry_off + 4..entry_off + 8].try_into().unwrap()) as usize;
        if tex_hdr_off == 0 || tex_hdr_off + 36 > tpl.len() {
            continue;
        }
        let height = u16::from_be_bytes(tpl[tex_hdr_off..tex_hdr_off + 2].try_into().unwrap());
        let width = u16::from_be_bytes(tpl[tex_hdr_off + 2..tex_hdr_off + 4].try_into().unwrap());
        let format_id = u32::from_be_bytes(tpl[tex_hdr_off + 4..tex_hdr_off + 8].try_into().unwrap());
        let data_off = u32::from_be_bytes(tpl[tex_hdr_off + 8..tex_hdr_off + 12].try_into().unwrap()) as usize;

        if width == 0 || height == 0 || width > 2048 || height > 2048 || data_off >= tpl.len() {
            continue;
        }

        let (format_name, rgba) = match decode_gx_texture(&tpl[data_off..], width as usize, height as usize, format_id) {
            Some(res) => res,
            None => continue,
        };

        let safe_name = source_file.replace(['/', '\\', ':', '@', '.'], "_");
        let png_filename = format!("{}_tex{:02}_{}x{}_{}.png", safe_name, idx, width, height, format_name);
        let png_path = tex_out_dir.join(&png_filename);
        if write_rgba_png(&png_path, width as u32, height as u32, &rgba).is_ok() {
            decoded_textures.push(DecodedTextureInfo {
                source_file: source_file.to_string(),
                texture_index: idx,
                width,
                height,
                format_id,
                format_name,
                png_path: format!("assets/textures_png/{}", png_filename),
            });
        }
    }
}

fn decode_gx_texture(data: &[u8], width: usize, height: usize, format_id: u32) -> Option<(String, Vec<u8>)> {
    let mut rgba = vec![0u8; width * height * 4];
    match format_id {
        0x00 => {
            // I4 (8x8 tiles, 4bpp)
            let mut src = 0;
            for ty in (0..height).step_by(8) {
                for tx in (0..width).step_by(8) {
                    for y in 0..8 {
                        for x in (0..8).step_by(2) {
                            let b = *data.get(src)?;
                            src += 1;
                            let i0 = (b >> 4) * 17;
                            let i1 = (b & 0x0F) * 17;
                            set_px(&mut rgba, width, height, tx + x, ty + y, [i0, i0, i0, 255]);
                            set_px(&mut rgba, width, height, tx + x + 1, ty + y, [i1, i1, i1, 255]);
                        }
                    }
                }
            }
            Some(("I4".to_string(), rgba))
        }
        0x01 => {
            // I8 (8x4 tiles, 8bpp)
            let mut src = 0;
            for ty in (0..height).step_by(4) {
                for tx in (0..width).step_by(8) {
                    for y in 0..4 {
                        for x in 0..8 {
                            let v = *data.get(src)?;
                            src += 1;
                            set_px(&mut rgba, width, height, tx + x, ty + y, [v, v, v, 255]);
                        }
                    }
                }
            }
            Some(("I8".to_string(), rgba))
        }
        0x02 => {
            // IA4 (8x4 tiles, 8bpp)
            let mut src = 0;
            for ty in (0..height).step_by(4) {
                for tx in (0..width).step_by(8) {
                    for y in 0..4 {
                        for x in 0..8 {
                            let b = *data.get(src)?;
                            src += 1;
                            let a = (b >> 4) * 17;
                            let i = (b & 0x0F) * 17;
                            set_px(&mut rgba, width, height, tx + x, ty + y, [i, i, i, a]);
                        }
                    }
                }
            }
            Some(("IA4".to_string(), rgba))
        }
        0x03 => {
            // IA8 (4x4 tiles, 16bpp)
            let mut src = 0;
            for ty in (0..height).step_by(4) {
                for tx in (0..width).step_by(4) {
                    for y in 0..4 {
                        for x in 0..4 {
                            let a = *data.get(src)?;
                            let i = *data.get(src + 1)?;
                            src += 2;
                            set_px(&mut rgba, width, height, tx + x, ty + y, [i, i, i, a]);
                        }
                    }
                }
            }
            Some(("IA8".to_string(), rgba))
        }
        0x04 => {
            // RGB565 (4x4 tiles, 16bpp)
            let mut src = 0;
            for ty in (0..height).step_by(4) {
                for tx in (0..width).step_by(4) {
                    for y in 0..4 {
                        for x in 0..4 {
                            let hi = *data.get(src)? as u16;
                            let lo = *data.get(src + 1)? as u16;
                            src += 2;
                            let p = (hi << 8) | lo;
                            let r = (((p >> 11) & 0x1F) * 255 / 31) as u8;
                            let g = (((p >> 5) & 0x3F) * 255 / 63) as u8;
                            let b = ((p & 0x1F) * 255 / 31) as u8;
                            set_px(&mut rgba, width, height, tx + x, ty + y, [r, g, b, 255]);
                        }
                    }
                }
            }
            Some(("RGB565".to_string(), rgba))
        }
        0x05 => {
            // RGB5A3 (4x4 tiles, 16bpp)
            let mut src = 0;
            for ty in (0..height).step_by(4) {
                for tx in (0..width).step_by(4) {
                    for y in 0..4 {
                        for x in 0..4 {
                            let hi = *data.get(src)? as u16;
                            let lo = *data.get(src + 1)? as u16;
                            src += 2;
                            let p = (hi << 8) | lo;
                            let px = if (p & 0x8000) != 0 {
                                let r = (((p >> 10) & 0x1F) * 255 / 31) as u8;
                                let g = (((p >> 5) & 0x1F) * 255 / 31) as u8;
                                let b = ((p & 0x1F) * 255 / 31) as u8;
                                [r, g, b, 255]
                            } else {
                                let a = (((p >> 12) & 0x07) * 255 / 7) as u8;
                                let r = (((p >> 8) & 0x0F) * 17) as u8;
                                let g = (((p >> 4) & 0x0F) * 17) as u8;
                                let b = ((p & 0x0F) * 17) as u8;
                                [r, g, b, a]
                            };
                            set_px(&mut rgba, width, height, tx + x, ty + y, px);
                        }
                    }
                }
            }
            Some(("RGB5A3".to_string(), rgba))
        }
        0x06 => {
            // RGBA8 (4x4 tiles, 64 bytes per tile: 32 bytes AR, 32 bytes GB)
            let mut src = 0;
            for ty in (0..height).step_by(4) {
                for tx in (0..width).step_by(4) {
                    if src + 64 > data.len() {
                        return None;
                    }
                    for y in 0..4 {
                        for x in 0..4 {
                            let idx = y * 4 + x;
                            let a = data[src + idx * 2];
                            let r = data[src + idx * 2 + 1];
                            let g = data[src + 32 + idx * 2];
                            let b = data[src + 32 + idx * 2 + 1];
                            set_px(&mut rgba, width, height, tx + x, ty + y, [r, g, b, a]);
                        }
                    }
                    src += 64;
                }
            }
            Some(("RGBA8".to_string(), rgba))
        }
        _ => None,
    }
}

#[inline]
fn set_px(rgba: &mut [u8], w: usize, h: usize, x: usize, y: usize, c: [u8; 4]) {
    if x < w && y < h {
        let idx = (y * w + x) * 4;
        rgba[idx..idx + 4].copy_from_slice(&c);
    }
}

fn write_rgba_png(path: &Path, width: u32, height: u32, rgba: &[u8]) -> std::io::Result<()> {
    let file = fs::File::create(path)?;
    let w = BufWriter::new(file);
    let mut encoder = png::Encoder::new(w, width, height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().map_err(std::io::Error::other)?;
    writer.write_image_data(rgba).map_err(std::io::Error::other)?;
    Ok(())
}

fn process_thp_video(
    dvd: &dyn Dvd,
    name: &str,
    offset: u64,
    size: u32,
    video_out_dir: &Path,
) -> Option<ThpVideoInfo> {
    let mut hdr = [0u8; 0x100];
    dvd.read_disc_into(offset as usize, &mut hdr);
    if &hdr[0..4] != b"THP\0" {
        return None;
    }
    let version = u32::from_be_bytes(hdr[4..8].try_into().unwrap());
    let fps = f32::from_be_bytes(hdr[16..20].try_into().unwrap());
    let num_frames = u32::from_be_bytes(hdr[20..24].try_into().unwrap());
    let first_frame_size = u32::from_be_bytes(hdr[24..28].try_into().unwrap());
    let comp_info_off = u32::from_be_bytes(hdr[32..36].try_into().unwrap()) as usize;
    let first_frame_off = u32::from_be_bytes(hdr[40..44].try_into().unwrap());

    let mut width = 640;
    let mut height = 480;
    let mut audio_channels = 0;
    let mut audio_sample_rate = 0;

    if comp_info_off + 32 <= hdr.len() {
        let num_components = u32::from_be_bytes(hdr[comp_info_off..comp_info_off + 4].try_into().unwrap()) as usize;
        let mut info_ptr = comp_info_off + 20;
        for i in 0..num_components.min(4) {
            let comp_type = hdr[comp_info_off + 4 + i];
            if comp_type == 0 && info_ptr + 8 <= hdr.len() {
                width = u32::from_be_bytes(hdr[info_ptr..info_ptr + 4].try_into().unwrap());
                height = u32::from_be_bytes(hdr[info_ptr + 4..info_ptr + 8].try_into().unwrap());
                info_ptr += if version >= 0x00011000 { 12 } else { 8 };
            } else if comp_type == 1 && info_ptr + 12 <= hdr.len() {
                audio_channels = u32::from_be_bytes(hdr[info_ptr..info_ptr + 4].try_into().unwrap());
                audio_sample_rate = u32::from_be_bytes(hdr[info_ptr + 4..info_ptr + 8].try_into().unwrap());
                info_ptr += if version >= 0x00011000 { 16 } else { 12 };
            }
        }
    }

    let mut extracted_keyframes = Vec::new();
    let stem = name.trim_end_matches(".thp");
    let mut cur_off = first_frame_off as usize;
    let mut cur_sz = first_frame_size as usize;

    for frame_idx in 0..3 {
        if cur_sz == 0 || cur_sz > 2 * 1024 * 1024 || (cur_off as u64) + (cur_sz as u64) > size as u64 {
            break;
        }
        let mut frame_buf = vec![0u8; cur_sz];
        dvd.read_disc_into((offset as usize) + cur_off, &mut frame_buf);
        if frame_buf.len() >= 16 {
            let next_sz = u32::from_be_bytes(frame_buf[0..4].try_into().unwrap()) as usize;
            let jpeg_sz = u32::from_be_bytes(frame_buf[8..12].try_into().unwrap()) as usize;
            let hdr_skip = if audio_channels > 0 { 16 } else { 12 };
            if hdr_skip + jpeg_sz <= frame_buf.len() {
                let raw_thp_jpeg = &frame_buf[hdr_skip..hdr_skip + jpeg_sz];
                let std_jpeg = convert_thp_jpeg_to_standard(raw_thp_jpeg);
                let jpg_name = format!("{}_frame_{:03}.jpg", stem, frame_idx);
                if fs::write(video_out_dir.join(&jpg_name), &std_jpeg).is_ok() {
                    extracted_keyframes.push(format!("assets/video/{}", jpg_name));
                }
            }
            cur_off += cur_sz;
            cur_sz = next_sz;
        } else {
            break;
        }
    }

    Some(ThpVideoInfo {
        name: name.to_string(),
        partition_offset_hex: format!("0x{:09X}", offset),
        size,
        version_hex: format!("0x{:08X}", version),
        fps,
        num_frames,
        width,
        height,
        audio_channels,
        audio_sample_rate,
        first_frame_offset: first_frame_off,
        first_frame_size,
        extracted_keyframes,
    })
}

fn convert_thp_jpeg_to_standard(thp_jpeg: &[u8]) -> Vec<u8> {
    // Wii THP JPEGs strip 0x00 stuffing after 0xFF inside entropy-coded scan data (between SOS 0xFFDA and EOI 0xFFD9)
    let mut out = Vec::with_capacity(thp_jpeg.len() + 1024);
    let mut in_scan = false;
    let mut i = 0;
    while i < thp_jpeg.len() {
        let b = thp_jpeg[i];
        if !in_scan {
            out.push(b);
            if b == 0xFF && i + 1 < thp_jpeg.len() && thp_jpeg[i + 1] == 0xDA {
                out.push(0xDA);
                i += 2;
                if i + 2 <= thp_jpeg.len() {
                    let sos_len = u16::from_be_bytes([thp_jpeg[i], thp_jpeg[i + 1]]) as usize;
                    for _ in 0..sos_len {
                        if i < thp_jpeg.len() {
                            out.push(thp_jpeg[i]);
                            i += 1;
                        }
                    }
                    in_scan = true;
                }
                continue;
            }
            i += 1;
        } else if b == 0xFF {
            if i + 1 < thp_jpeg.len() && thp_jpeg[i + 1] == 0xD9 {
                out.push(0xFF);
                out.push(0xD9);
                break;
            } else {
                out.push(0xFF);
                out.push(0x00);
                i += 1;
            }
        } else {
            out.push(b);
            i += 1;
        }
    }
    out
}

pub struct DolCodeAnalysis {
    pub functions: Vec<FunctionInfo>,
    pub classes: Vec<ClassVTableInfo>,
    pub strings: Vec<StringXref>,
    pub subsystem_counts: BTreeMap<String, usize>,
}

fn analyze_dol(dol: &Dol, full_dol: &[u8], code_out_dir: &Path) -> DolCodeAnalysis {
    // 1. Collect all strings in data/rodata sections
    let mut string_map: BTreeMap<u32, (String, String)> = BTreeMap::new();
    for (idx, sec) in dol.data_sections().iter().enumerate() {
        let sec_name = match idx {
            0 => ".rodata",
            1 => ".data",
            2 => ".sdata",
            3 => ".sdata2",
            _ => ".data_ext",
        };
        let bytes = &full_dol[sec.offset as usize..(sec.offset + sec.size) as usize];
        let mut i = 0;
        while i < bytes.len() {
            if is_printable_ascii(bytes[i]) {
                let start = i;
                while i < bytes.len() && is_printable_ascii(bytes[i]) {
                    i += 1;
                }
                let len = i - start;
                if len >= 4 && i < bytes.len() && bytes[i] == 0 {
                    let s = String::from_utf8_lossy(&bytes[start..i]).to_string();
                    let vaddr = sec.vaddr + (start as u32);
                    string_map.insert(vaddr, (sec_name.to_string(), s));
                }
            }
            i += 1;
        }
    }

    // Helper to check if an address is inside executable text sections
    let text_ranges: Vec<(u32, u32, u32)> = dol
        .text_sections()
        .iter()
        .map(|s| (s.vaddr, s.vaddr + s.size, s.offset))
        .collect();

    let is_code_addr = |addr: u32| -> bool {
        (addr & 3) == 0 && text_ranges.iter().any(|&(s, e, _)| addr >= s && addr < e)
    };

    // 2. Discover candidate VTables in data sections (runs of >= 2 code pointers)
    let mut vtables_raw: BTreeMap<u32, Vec<u32>> = BTreeMap::new();
    let mut vtable_targets: BTreeSet<u32> = BTreeSet::new();

    for sec in dol.data_sections() {
        let bytes = &full_dol[sec.offset as usize..(sec.offset + sec.size) as usize];
        let mut off = 0;
        while off + 8 <= bytes.len() {
            let w0 = u32::from_be_bytes(bytes[off..off + 4].try_into().unwrap());
            let w1 = u32::from_be_bytes(bytes[off + 4..off + 8].try_into().unwrap());
            if is_code_addr(w0) && is_code_addr(w1) {
                let vt_vaddr = sec.vaddr + (off as u32);
                let mut methods = Vec::new();
                while off + 4 <= bytes.len() {
                    let w = u32::from_be_bytes(bytes[off..off + 4].try_into().unwrap());
                    if is_code_addr(w) {
                        methods.push(w);
                        vtable_targets.insert(w);
                        off += 4;
                    } else {
                        break;
                    }
                }
                if methods.len() >= 2 && methods.len() <= 128 {
                    vtables_raw.insert(vt_vaddr, methods);
                }
            } else {
                off += 4;
            }
        }
    }

    // 3. Discover all function entrypoints in .init & .text
    let mut func_starts: BTreeSet<u32> = BTreeSet::new();
    func_starts.insert(dol.entry_point());
    for &(s, _, _) in &text_ranges {
        func_starts.insert(s);
    }
    for &vt_fn in &vtable_targets {
        func_starts.insert(vt_fn);
    }

    for &(sec_start, sec_end, sec_off) in &text_ranges {
        let len = (sec_end - sec_start) as usize;
        let code = &full_dol[sec_off as usize..(sec_off as usize) + len];
        let mut pc = sec_start;
        let mut prev_was_terminator = true;

        for chunk in code.chunks_exact(4) {
            let insn = u32::from_be_bytes(chunk.try_into().unwrap());
            let op = insn >> 26;

            // stwu r1, -N(r1) prologue
            if (insn & 0xFFFF_0000) == 0x9421_0000 && (insn as i16) < 0 {
                func_starts.insert(pc);
            } else if prev_was_terminator && insn != 0 && insn != 0x6000_0000 {
                func_starts.insert(pc);
            }

            // bl / bla call target (op == 18, LK == 1)
            if op == 18 && (insn & 1) == 1 {
                let mut li = (insn & 0x03FF_FFFC) as i32;
                if (li & 0x0200_0000) != 0 {
                    li |= !0x03FF_FFFF;
                }
                let target = if (insn & 2) != 0 {
                    li as u32
                } else {
                    pc.wrapping_add(li as u32)
                };
                if is_code_addr(target) {
                    func_starts.insert(target);
                }
            }

            // Check if instruction is blr (0x4E800020), rfi (0x4C000064), or unconditional branch b (op==18, LK==0)
            prev_was_terminator = insn == 0x4E80_0020 || insn == 0x4C00_0064 || (op == 18 && (insn & 1) == 0);
            pc += 4;
        }
    }

    let sorted_starts: Vec<u32> = func_starts.into_iter().collect();

    // 4. Analyze each function: boundaries, callees, callers, lis+addi xrefs to strings and vtables
    let mut callers_map: HashMap<u32, Vec<u32>> = HashMap::new();
    let mut string_To_funcs: BTreeMap<u32, BTreeSet<u32>> = BTreeMap::new();
    let mut vtable_to_funcs: BTreeMap<u32, BTreeSet<u32>> = BTreeMap::new();

    struct RawFuncData {
        vaddr: u32,
        end_vaddr: u32,
        stack_frame: i32,
        is_leaf: bool,
        callees: BTreeSet<u32>,
        string_addrs: BTreeSet<u32>,
        vtable_addrs: BTreeSet<u32>,
    }

    let mut raw_funcs = Vec::with_capacity(sorted_starts.len());

    for (idx, &f_start) in sorted_starts.iter().enumerate() {
        let Some(&(sec_start, sec_end, sec_off)) = text_ranges.iter().find(|&&(s, e, _)| f_start >= s && f_start < e) else {
            continue;
        };
        let next_start = sorted_starts
            .get(idx + 1)
            .copied()
            .unwrap_or(sec_end)
            .min(sec_end);
        if next_start <= f_start {
            continue;
        }

        let rel_start = (f_start - sec_start) as usize + (sec_off as usize);
        let rel_end = (next_start - sec_start) as usize + (sec_off as usize);
        let code = &full_dol[rel_start..rel_end];

        let mut stack_frame = 0i32;
        let mut is_leaf = true;
        let mut callees = BTreeSet::new();
        let mut string_addrs = BTreeSet::new();
        let mut vtable_addrs = BTreeSet::new();

        // Track recent `lis rD, imm` values per GPR
        let mut reg_hi: [Option<u32>; 32] = [None; 32];

        for (step, chunk) in code.chunks_exact(4).enumerate() {
            let pc = f_start + (step as u32) * 4;
            let insn = u32::from_be_bytes(chunk.try_into().unwrap());
            let op = insn >> 26;
            let rd = ((insn >> 21) & 0x1F) as usize;
            let ra = ((insn >> 16) & 0x1F) as usize;
            let imm_u16 = (insn & 0xFFFF) as u16;
            let imm_i16 = imm_u16 as i16 as i32;

            if step <= 2 && (insn & 0xFFFF_0000) == 0x9421_0000 && imm_i16 < 0 {
                stack_frame = -imm_i16;
            }
            if insn == 0x7C08_02A6 {
                // mflr r0
                is_leaf = false;
            }

            // bl / bla
            if op == 18 && (insn & 1) == 1 {
                is_leaf = false;
                let mut li = (insn & 0x03FF_FFFC) as i32;
                if (li & 0x0200_0000) != 0 {
                    li |= !0x03FF_FFFF;
                }
                let target = if (insn & 2) != 0 {
                    li as u32
                } else {
                    pc.wrapping_add(li as u32)
                };
                if is_code_addr(target) {
                    callees.insert(target);
                    callers_map.entry(target).or_default().push(f_start);
                }
                // Clear volatile r3..r12 across call
                for r in 3..=12 {
                    reg_hi[r] = None;
                }
            } else if op == 15 && ra == 0 {
                // lis rD, imm
                reg_hi[rd] = Some((imm_u16 as u32) << 16);
            } else if op == 14 && ra != 0 {
                // addi rD, rA, simm
                if let Some(hi) = reg_hi[ra] {
                    let full_addr = hi.wrapping_add(imm_i16 as u32);
                    if string_map.contains_key(&full_addr) {
                        string_addrs.insert(full_addr);
                        string_To_funcs.entry(full_addr).or_default().insert(f_start);
                    }
                    if vtables_raw.contains_key(&full_addr) || vtables_raw.contains_key(&(full_addr.wrapping_add(8))) {
                        let vt_key = if vtables_raw.contains_key(&full_addr) {
                            full_addr
                        } else {
                            full_addr.wrapping_add(8)
                        };
                        vtable_addrs.insert(vt_key);
                        vtable_to_funcs.entry(vt_key).or_default().insert(f_start);
                    }
                }
                reg_hi[rd] = None;
            } else if op == 24 {
                // ori rA, rS, uimm (note: in ori, rd is rS and ra is rA)
                let rs = rd;
                let ra_dst = ra;
                if let Some(hi) = reg_hi[rs] {
                    let full_addr = hi | (imm_u16 as u32);
                    if string_map.contains_key(&full_addr) {
                        string_addrs.insert(full_addr);
                        string_To_funcs.entry(full_addr).or_default().insert(f_start);
                    }
                    if vtables_raw.contains_key(&full_addr) {
                        vtable_addrs.insert(full_addr);
                        vtable_to_funcs.entry(full_addr).or_default().insert(f_start);
                    }
                }
                reg_hi[ra_dst] = None;
            }
        }

        raw_funcs.push(RawFuncData {
            vaddr: f_start,
            end_vaddr: next_start,
            stack_frame,
            is_leaf,
            callees,
            string_addrs,
            vtable_addrs,
        });
    }

    // 5. Name functions and classify into subsystems
    let mut vaddr_to_name: HashMap<u32, (String, String)> = HashMap::new();
    seed_known_bt3_symbols(&mut vaddr_to_name, dol.entry_point());

    for rf in &raw_funcs {
        if vaddr_to_name.contains_key(&rf.vaddr) {
            continue;
        }
        let strs: Vec<&str> = rf
            .string_addrs
            .iter()
            .filter_map(|a| string_map.get(a).map(|(_, s)| s.as_str()))
            .collect();
        let (name, subsys) = infer_function_name_and_subsystem(rf.vaddr, &strs, &rf.vtable_addrs);
        vaddr_to_name.insert(rf.vaddr, (name, subsys));
    }

    let mut subsystem_counts: BTreeMap<String, usize> = BTreeMap::new();
    let mut functions = Vec::with_capacity(raw_funcs.len());
    let mut vaddr_to_size: HashMap<u32, u32> = HashMap::new();

    for rf in &raw_funcs {
        let size = rf.end_vaddr - rf.vaddr;
        vaddr_to_size.insert(rf.vaddr, size);
        let (inferred_name, subsystem) = vaddr_to_name
            .get(&rf.vaddr)
            .cloned()
            .unwrap_or_else(|| (format!("sub_{:08X}", rf.vaddr), "bt3_game_engine".to_string()));
        *subsystem_counts.entry(subsystem.clone()).or_insert(0) += 1;

        let callers = callers_map.get(&rf.vaddr);
        let caller_count = callers.map(|c| c.len()).unwrap_or(0);
        let sample_callers = callers
            .map(|c| c.iter().take(8).map(|a| format!("0x{:08X}", a)).collect())
            .unwrap_or_default();

        let callees = rf
            .callees
            .iter()
            .map(|a| {
                if let Some((n, _)) = vaddr_to_name.get(a) {
                    format!("0x{:08X} ({})", a, n)
                } else {
                    format!("0x{:08X}", a)
                }
            })
            .collect();

        let string_refs = rf
            .string_addrs
            .iter()
            .filter_map(|a| string_map.get(a).map(|(_, s)| s.clone()))
            .take(8)
            .collect();

        let vtable_refs = rf
            .vtable_addrs
            .iter()
            .map(|a| format!("0x{:08X}", a))
            .collect();

        functions.push(FunctionInfo {
            vaddr: rf.vaddr,
            vaddr_hex: format!("0x{:08X}", rf.vaddr),
            end_vaddr_hex: format!("0x{:08X}", rf.end_vaddr),
            size_bytes: size,
            instruction_count: size / 4,
            stack_frame_bytes: rf.stack_frame,
            is_leaf: rf.is_leaf,
            inferred_name,
            subsystem,
            caller_count,
            sample_callers,
            callees,
            string_refs,
            vtable_refs,
        });
    }

    // 6. Build C++ Classes & VTable catalog
    let mut classes = Vec::new();
    for (class_id, (&vt_vaddr, methods_raw)) in vtables_raw.iter().enumerate() {
        let ctors: Vec<u32> = vtable_to_funcs
            .get(&vt_vaddr)
            .map(|s| s.iter().copied().collect())
            .unwrap_or_default();

        let mut assoc_strings = BTreeSet::new();
        for &ctor in &ctors {
            if let Some(rf) = raw_funcs.iter().find(|f| f.vaddr == ctor) {
                for &sa in &rf.string_addrs {
                    if let Some((_, s)) = string_map.get(&sa) {
                        assoc_strings.insert(s.clone());
                    }
                }
            }
        }
        for &m in methods_raw.iter().take(12) {
            if let Some(rf) = raw_funcs.iter().find(|f| f.vaddr == m) {
                for &sa in &rf.string_addrs {
                    if let Some((_, s)) = string_map.get(&sa) {
                        assoc_strings.insert(s.clone());
                    }
                }
            }
        }

        let assoc_vec: Vec<String> = assoc_strings.into_iter().take(8).collect();
        let (inferred_class_name, subsystem) = infer_class_name(class_id, vt_vaddr, &assoc_vec);

        let methods = methods_raw
            .iter()
            .enumerate()
            .map(|(slot, &m_vaddr)| {
                let m_name = vaddr_to_name
                    .get(&m_vaddr)
                    .map(|(n, _)| n.clone())
                    .unwrap_or_else(|| format!("{}::vfunc_{:02}_{:08X}", inferred_class_name, slot, m_vaddr));
                let sz = vaddr_to_size.get(&m_vaddr).copied().unwrap_or(4);
                VTableMethodInfo {
                    slot,
                    vaddr_hex: format!("0x{:08X}", m_vaddr),
                    inferred_name: m_name,
                    size_bytes: sz,
                }
            })
            .collect();

        classes.push(ClassVTableInfo {
            class_id,
            inferred_class_name,
            subsystem,
            vtable_vaddr_hex: format!("0x{:08X}", vt_vaddr),
            method_count: methods_raw.len(),
            constructor_vaddrs: ctors.iter().map(|a| format!("0x{:08X}", a)).collect(),
            associated_strings: assoc_vec,
            methods,
        });
    }

    // 7. Build Strings Xref table
    let mut strings = Vec::with_capacity(string_map.len());
    for (&s_vaddr, (sec_name, text)) in &string_map {
        let refs = string_To_funcs
            .get(&s_vaddr)
            .map(|set| {
                set.iter()
                    .map(|f| {
                        if let Some((n, _)) = vaddr_to_name.get(f) {
                            format!("0x{:08X} ({})", f, n)
                        } else {
                            format!("0x{:08X}", f)
                        }
                    })
                    .collect()
            })
            .unwrap_or_default();
        strings.push(StringXref {
            vaddr_hex: format!("0x{:08X}", s_vaddr),
            section: sec_name.clone(),
            text: text.clone(),
            referenced_by_functions: refs,
        });
    }

    // Write JSON files & Dolphin/Ghidra .map file
    fs::write(
        code_out_dir.join("functions.json"),
        serde_json::to_string_pretty(&functions).unwrap(),
    )
    .unwrap();
    fs::write(
        code_out_dir.join("classes_vtables.json"),
        serde_json::to_string_pretty(&classes).unwrap(),
    )
    .unwrap();
    fs::write(
        code_out_dir.join("strings_xrefs.json"),
        serde_json::to_string_pretty(&strings).unwrap(),
    )
    .unwrap();

    let mut map_lines = Vec::with_capacity(functions.len() + 4);
    map_lines.push(".text section layout".to_string());
    for f in &functions {
        map_lines.push(format!(
            "{:08x} {:08x} {:08x} 0 {}",
            f.vaddr, f.size_bytes, f.vaddr, f.inferred_name
        ));
    }
    fs::write(code_out_dir.join("RDSPAF.map"), map_lines.join("\n")).unwrap();

    // Also dump annotated disassembly of key functions
    dump_annotated_disassembly(dol, full_dol, &functions, &vaddr_to_name, code_out_dir);

    DolCodeAnalysis {
        functions,
        classes,
        strings,
        subsystem_counts,
    }
}

fn seed_known_bt3_symbols(vaddr_to_name: &mut HashMap<u32, (String, String)>, entrypoint: u32) {
    vaddr_to_name.insert(entrypoint, ("__start".to_string(), "rvl_sdk_os".to_string()));
    vaddr_to_name.insert(0x80004000, ("__init_registers".to_string(), "rvl_sdk_os".to_string()));
    vaddr_to_name.insert(0x8000EE38, ("BT3_VideoAndRenderInit".to_string(), "bt3_core_init".to_string()));
    vaddr_to_name.insert(0x80210990, ("__OSReschedule".to_string(), "rvl_sdk_os".to_string()));
    vaddr_to_name.insert(0x80210A1C, ("OSSelectThread".to_string(), "rvl_sdk_os".to_string()));
    vaddr_to_name.insert(0x80210F78, ("OSSleepThread".to_string(), "rvl_sdk_os".to_string()));
    vaddr_to_name.insert(0x80211058, ("OSWakeupThread".to_string(), "rvl_sdk_os".to_string()));
    vaddr_to_name.insert(0x802095E8, ("OSDisableInterrupts".to_string(), "rvl_sdk_os".to_string()));
    vaddr_to_name.insert(0x8020960C, ("OSRestoreInterrupts".to_string(), "rvl_sdk_os".to_string()));
    vaddr_to_name.insert(0x8021BBA8, ("__VIRetraceHandler".to_string(), "rvl_sdk_vi".to_string()));
    vaddr_to_name.insert(0x8021C548, ("__VIInit".to_string(), "rvl_sdk_vi".to_string()));
    vaddr_to_name.insert(0x8021C758, ("VIInit".to_string(), "rvl_sdk_vi".to_string()));
    vaddr_to_name.insert(0x8021CCA0, ("VIWaitForRetrace".to_string(), "rvl_sdk_vi".to_string()));
    vaddr_to_name.insert(0x8021CF10, ("VIConfigure".to_string(), "rvl_sdk_vi".to_string()));
    vaddr_to_name.insert(0x8021DC24, ("VIFlush".to_string(), "rvl_sdk_vi".to_string()));
    vaddr_to_name.insert(0x8021DD64, ("VISetNextFrameBuffer".to_string(), "rvl_sdk_vi".to_string()));
    vaddr_to_name.insert(0x8021DE20, ("VISetBlack".to_string(), "rvl_sdk_vi".to_string()));
}

fn infer_function_name_and_subsystem(
    vaddr: u32,
    strs: &[&str],
    vtables: &BTreeSet<u32>,
) -> (String, String) {
    let joined = strs.join(" ");
    let lower = joined.to_ascii_lowercase();

    let subsystem = if lower.contains("cri") || lower.contains("adx") || lower.contains("afs") || lower.contains("ahx") || lower.contains(".afs") {
        "criware_middleware"
    } else if lower.contains("homebutton") || lower.contains("homebtn") || lower.contains("speakerse") {
        "wii_homebutton_lib"
    } else if lower.contains("nw4r") || lower.contains("lyt") || lower.contains("brlyt") || lower.contains("brlan") {
        "nw4r_layout_engine"
    } else if lower.contains("thp") || lower.contains(".thp") {
        "wii_thp_video"
    } else if lower.contains("/dev/") || lower.contains("ios") || lower.contains("ipc") || lower.contains("nand") || lower.contains("sysconf") || lower.contains("es_") {
        "rvl_sdk_ios_ipc"
    } else if lower.contains("wpad") || lower.contains("kpad") || lower.contains("pad") || lower.contains("wiimote") || lower.contains("nunchuk") {
        "rvl_sdk_input_wpad"
    } else if lower.contains("gx") || lower.contains("vi") || lower.contains("xfb") || lower.contains("efb") || lower.contains("mtx") {
        "rvl_sdk_gx_vi"
    } else if lower.contains("dvd") {
        "rvl_sdk_dvd"
    } else if lower.contains("ax") || lower.contains("dsp") || lower.contains("ai") || lower.contains("sound") || lower.contains("roland") || lower.contains("gm16") {
        "rvl_sdk_audio_synth"
    } else if lower.contains("dbz") || lower.contains("zs3") || lower.contains("pak") || lower.contains("chr") || lower.contains("btl") || lower.contains("stg") || lower.contains("kamehame") {
        "bt3_spike_combat_engine"
    } else if (0x8020_0000..0x8029_0000).contains(&vaddr) {
        "rvl_sdk_core"
    } else {
        "bt3_spike_engine"
    };

    // Try to build a clean identifier from the most descriptive string reference
    for s in strs {
        let clean: String = s
            .chars()
            .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
            .collect();
        let trimmed = clean.trim_matches('_');
        if trimmed.len() >= 4 {
            let short = &trimmed[..trimmed.len().min(28)];
            return (format!("fn_{}_{:08X}", short, vaddr), subsystem.to_string());
        }
    }

    if let Some(&vt) = vtables.iter().next() {
        return (
            format!("ctor_vt{:08X}_{:08X}", vt, vaddr),
            subsystem.to_string(),
        );
    }

    (format!("sub_{:08X}", vaddr), subsystem.to_string())
}

fn infer_class_name(class_id: usize, vt_vaddr: u32, strs: &[String]) -> (String, String) {
    let joined = strs.join(" ").to_ascii_lowercase();
    let subsys = if joined.contains("nw4r") || joined.contains("lyt") {
        "nw4r_ui"
    } else if joined.contains("home") {
        "wii_homebutton"
    } else if joined.contains("cri") || joined.contains("adx") || joined.contains("afs") {
        "criware"
    } else if joined.contains("snd") || joined.contains("sound") || joined.contains("seq") {
        "audio_engine"
    } else {
        "bt3_spike_engine"
    };

    for s in strs {
        let token: String = s
            .chars()
            .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
            .collect();
        let trimmed = token.trim_matches('_');
        if trimmed.len() >= 4 {
            let short = &trimmed[..trimmed.len().min(24)];
            return (format!("C_{}_{:03}", short, class_id), subsys.to_string());
        }
    }

    (
        format!("CBT3Class_{:03}_vt{:08X}", class_id, vt_vaddr),
        subsys.to_string(),
    )
}

fn dump_annotated_disassembly(
    dol: &Dol,
    full_dol: &[u8],
    functions: &[FunctionInfo],
    vaddr_to_name: &HashMap<u32, (String, String)>,
    code_out_dir: &Path,
) {
    let mut out = String::with_capacity(256 * 1024);
    out.push_str("# ============================================================================\n");
    out.push_str("# Dragon Ball Z: Budokai Tenkaichi 3 (RDSPAF) - Annotated PowerPC Disassembly\n");
    out.push_str("# Key Entrypoint, Video/GX, AFS/PAK, Input & Battle Engine Functions\n");
    out.push_str("# ============================================================================\n\n");

    let text_ranges: Vec<(u32, u32, u32)> = dol
        .text_sections()
        .iter()
        .map(|s| (s.vaddr, s.vaddr + s.size, s.offset))
        .collect();

    for f in functions.iter().take(120) {
        let Some(&(sec_start, _, sec_off)) = text_ranges
            .iter()
            .find(|&&(s, e, _)| f.vaddr >= s && f.vaddr < e)
        else {
            continue;
        };
        out.push_str(&format!(
            "\n# ----------------------------------------------------------------------------\n# Function: {} ({}) | Size: {} bytes | Stack: {} bytes\n",
            f.inferred_name, f.vaddr_hex, f.size_bytes, f.stack_frame_bytes
        ));
        if !f.string_refs.is_empty() {
            out.push_str(&format!("# Strings: {:?}\n", f.string_refs));
        }
        out.push_str("# ----------------------------------------------------------------------------\n");

        let rel_start = (f.vaddr - sec_start) as usize + (sec_off as usize);
        let max_bytes = (f.size_bytes as usize).min(256);
        let slice = &full_dol[rel_start..rel_start + max_bytes];
        for (i, chunk) in slice.chunks_exact(4).enumerate() {
            let pc = f.vaddr + (i as u32) * 4;
            let word = u32::from_be_bytes(chunk.try_into().unwrap());
            let decoded = match GekkoInstruction::decode(&slice[i * 4..]) {
                Some((insn, _)) => format!("{}", insn),
                None => format!(".long 0x{:08X}", word),
            };
            // Annotate `bl` targets with symbol names
            let mut comment = String::new();
            if (word >> 26) == 18 && (word & 1) == 1 {
                let mut li = (word & 0x03FF_FFFC) as i32;
                if (li & 0x0200_0000) != 0 {
                    li |= !0x03FF_FFFF;
                }
                let target = pc.wrapping_add(li as u32);
                if let Some((sym, _)) = vaddr_to_name.get(&target) {
                    comment = format!("  # -> {}", sym);
                }
            }
            out.push_str(&format!("  {:08X}:  {:08X}    {}{}\n", pc, word, decoded, comment));
        }
    }

    fs::write(code_out_dir.join("key_subsystems_disasm.s"), out).unwrap();
}

fn generate_native_rust_sdk(
    sdk_dir: &Path,
    game_title: &str,
    game_id: &str,
    disc_files: &[DiscFileEntry],
    afs_catalogs: &[AfsArchiveCatalog],
    code: &DolCodeAnalysis,
) {
    let src_dir = sdk_dir.join("src");
    fs::create_dir_all(&src_dir).unwrap();

    let cargo_toml = r#"[package]
name = "bt3-native-sdk"
version = "0.1.0"
edition = "2024"
description = "Auto-generated Native Rust Porting SDK & Reverse Engineering Bindings for Dragon Ball Z: Budokai Tenkaichi 3 (RDSPAF)"

[dependencies]
serde = { version = "1", features = ["derive"] }
"#;
    fs::write(sdk_dir.join("Cargo.toml"), cargo_toml).unwrap();

    // 1. src/disc_layout.rs
    let mut disc_rs = String::from("//! Auto-generated Disc FST Partition Layout for Budokai Tenkaichi 3 (RDSPAF)\n\n");
    disc_rs.push_str("#[derive(Debug, Clone, Copy)]\npub struct DiscAssetSpec {\n    pub path: &'static str,\n    pub partition_offset: u64,\n    pub size: u32,\n    pub category: &'static str,\n}\n\n");
    disc_rs.push_str(&format!("pub const DISC_ASSETS: &[DiscAssetSpec] = &[\n"));
    for f in disc_files {
        disc_rs.push_str(&format!(
            "    DiscAssetSpec {{ path: {:?}, partition_offset: 0x{:09X}, size: {}, category: {:?} }},\n",
            f.path, f.partition_offset, f.size, f.category
        ));
    }
    disc_rs.push_str("];\n");
    fs::write(src_dir.join("disc_layout.rs"), disc_rs).unwrap();

    // 2. src/afs_pak.rs
    let mut afs_rs = String::from("//! Auto-generated CriWare AFS & Spike PAK Archive Catalog for BT3\n\n");
    afs_rs.push_str("#[derive(Debug, Clone, Copy)]\npub struct AfsArchiveSpec {\n    pub name: &'static str,\n    pub partition_offset: u64,\n    pub size: u32,\n    pub entry_count: usize,\n}\n\n");
    afs_rs.push_str("pub const BT3_AFS_ARCHIVES: &[AfsArchiveSpec] = &[\n");
    for a in afs_catalogs {
        afs_rs.push_str(&format!(
            "    AfsArchiveSpec {{ name: {:?}, partition_offset: {}, size: {}, entry_count: {} }},\n",
            a.archive_name, a.partition_offset_hex, a.archive_size, a.entry_count
        ));
    }
    afs_rs.push_str("];\n");
    fs::write(src_dir.join("afs_pak.rs"), afs_rs).unwrap();

    // 3. src/classes.rs
    let mut cls_rs = String::from("//! Auto-generated C++ Classes & Virtual Method Table (VTable) Traits for BT3 Native Port\n\n");
    for c in code.classes.iter().take(64) {
        let trait_name: String = c
            .inferred_class_name
            .chars()
            .map(|ch| if ch.is_ascii_alphanumeric() { ch } else { '_' })
            .collect();
        cls_rs.push_str(&format!(
            "/// Recovered C++ Class `{}` (VTable @ {}, {} virtual methods, subsystem: `{}`)\n",
            c.inferred_class_name, c.vtable_vaddr_hex, c.method_count, c.subsystem
        ));
        cls_rs.push_str(&format!("pub trait {} {{\n", trait_name));
        cls_rs.push_str(&format!("    const VTABLE_ADDR: u32 = {};\n", c.vtable_vaddr_hex));
        for m in c.methods.iter().take(16) {
            cls_rs.push_str(&format!(
                "    /// Slot {} -> {} ({}, {} bytes)\n    fn vfunc_slot_{:02}(&mut self);\n",
                m.slot, m.inferred_name, m.vaddr_hex, m.size_bytes, m.slot
            ));
        }
        cls_rs.push_str("}\n\n");
    }
    fs::write(src_dir.join("classes.rs"), cls_rs).unwrap();

    // 4. src/functions.rs
    let mut fn_rs = String::from("//! Auto-generated PowerPC Function Symbol Table for BT3 Native Port & HLE Hooks\n\n");
    fn_rs.push_str("#[derive(Debug, Clone, Copy)]\npub struct PpcFunctionSymbol {\n    pub vaddr: u32,\n    pub size: u32,\n    pub stack_frame: i32,\n    pub name: &'static str,\n    pub subsystem: &'static str,\n}\n\n");
    fn_rs.push_str("pub const BT3_KEY_FUNCTIONS: &[PpcFunctionSymbol] = &[\n");
    for f in code.functions.iter().filter(|f| !f.string_refs.is_empty() || f.vaddr < 0x8001_0000 || f.inferred_name.starts_with("VI") || f.inferred_name.starts_with("OS")).take(250) {
        fn_rs.push_str(&format!(
            "    PpcFunctionSymbol {{ vaddr: 0x{:08X}, size: {}, stack_frame: {}, name: {:?}, subsystem: {:?} }},\n",
            f.vaddr, f.size_bytes, f.stack_frame_bytes, f.inferred_name, f.subsystem
        ));
    }
    fn_rs.push_str("];\n");
    fs::write(src_dir.join("functions.rs"), fn_rs).unwrap();

    // 5. src/lib.rs
    let lib_rs = format!(
        "//! Native Porting SDK for {} ({})\n//! Recovered {} functions, {} C++ classes/vtables, and {} disc files.\n\npub mod afs_pak;\npub mod classes;\npub mod disc_layout;\npub mod functions;\n",
        game_title,
        game_id,
        code.functions.len(),
        code.classes.len(),
        disc_files.len()
    );
    fs::write(src_dir.join("lib.rs"), lib_rs).unwrap();
}

#[inline]
fn is_printable_ascii(b: u8) -> bool {
    (0x20..=0x7E).contains(&b) || b == b'\t'
}
