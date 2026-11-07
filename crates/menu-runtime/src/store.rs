//! Wii Channel Store and NAND Title Installer.
//!
//! Handles official title installation, WAD unpacking into the simulated NAND
//! filesystem (`/title/<group>/<title_id>/content/`), cryptographic verification,
//! NAND block allocation/accounting (2174 blocks total), and channel grid registration.

use std::{
    fs,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use wii_menu_core::{Channel, ContentKind, MenuCatalog, NAND_TOTAL_BLOCKS};

use crate::wad::WadArchive;

/// Size of one Wii NAND storage block in bytes (16 KiB = 16,384 bytes).
pub const NAND_BLOCK_SIZE: u64 = 16 * 1024;

/// Metadata of an installable channel in the Wii Store.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StoreItem {
    pub id: String,
    pub title_id: String,
    pub name: String,
    pub category: String,
    pub blocks: u32,
    pub version: u16,
    pub description: String,
    pub package_path: Option<String>,
    pub installed: bool,
}

/// Channel Store manager and NAND package installer.
pub struct ChannelStore {
    pub repo_root: PathBuf,
    pub nand_root: PathBuf,
    pub catalog_path: PathBuf,
}

impl ChannelStore {
    pub fn new(repo_root: impl Into<PathBuf>) -> Self {
        let mut root = repo_root.into();
        if !root.join("userland/data/wii/menu.catalog.json").is_file() {
            if Path::new("../../userland/data/wii/menu.catalog.json").is_file() {
                root = PathBuf::from("../..");
            } else if Path::new("../userland/data/wii/menu.catalog.json").is_file() {
                root = PathBuf::from("..");
            }
        }
        let nand_root = root.join("userland/data/wii/nand");
        let catalog_path = root.join("userland/data/wii/menu.catalog.json");
        Self {
            repo_root: root,
            nand_root,
            catalog_path,
        }
    }

    /// List all available titles in the Store, correlating with installed NAND titles.
    pub fn list_store_items(&self) -> Result<Vec<StoreItem>, String> {
        let catalog = self.load_catalog()?;
        let mut items = Vec::new();

        // 1. Mii Channel PAL v6
        let mii_installed = catalog.channels.iter().any(|c| c.id == "mii-channel-eu-v6" || c.title_id.as_deref() == Some("0001000248414341"));
        items.push(StoreItem {
            id: "mii-channel-eu-v6".to_string(),
            title_id: "0001000248414341".to_string(),
            name: "Chaîne Mii (Mii Channel)".to_string(),
            category: "Official Channel".to_string(),
            blocks: 48,
            version: 6,
            description: "Créez et personnalisez vos avatars Mii pour vos jeux Wii et le Bureau Wii.".to_string(),
            package_path: Some("userland/data/wii/wads/Official Channels/Mii Channel v6 HACA.wad".to_string()),
            installed: mii_installed,
        });

        // 2. Wii Shop Channel PAL v17
        let shop_installed = catalog.channels.iter().any(|c| c.id == "shop-channel-eu-v17" || c.title_id.as_deref() == Some("0001000248414241"));
        items.push(StoreItem {
            id: "shop-channel-eu-v17".to_string(),
            title_id: "0001000248414241".to_string(),
            name: "Chaîne Boutique Wii (Wii Shop Channel)".to_string(),
            category: "Official Channel".to_string(),
            blocks: 156,
            version: 17,
            description: "Téléchargez des logiciels WiiWare, Virtual Console et des chaînes supplémentaires.".to_string(),
            package_path: Some("userland/data/wii/wads/Official Channels/Shop Channel v17 HABA.wad".to_string()),
            installed: shop_installed,
        });

        // 3. Homebrew Browser
        let hb_installed = catalog.channels.iter().any(|c| c.id == "homebrew-browser-eu");
        items.push(StoreItem {
            id: "homebrew-browser-eu".to_string(),
            title_id: "0001000148425257".to_string(),
            name: "Homebrew Browser".to_string(),
            category: "Community Tool".to_string(),
            blocks: 128,
            version: 39,
            description: "Gestionnaire communautaire d'applications, émulateurs et utilitaires Wii.".to_string(),
            package_path: Some("userland/data/wii/brew/apps/homebrew-browser/boot.dol".to_string()),
            installed: hb_installed,
        });

        // 4. System Menu 4.3E
        let sys_installed = catalog.channels.iter().any(|c| c.id == "system-menu-eu-4-3" || c.id == "system-menu-installed-eu-4-3");
        items.push(StoreItem {
            id: "system-menu-eu-4-3".to_string(),
            title_id: "0000000100000002".to_string(),
            name: "Menu Wii Système 4.3E".to_string(),
            category: "System Firmware".to_string(),
            blocks: 486,
            version: 514,
            description: "Firmware officiel européen du Menu Wii avec interface Frutiger Aero.".to_string(),
            package_path: Some("userland/data/wii/wads/System Software/SystemMenu-v514.wad".to_string()),
            installed: sys_installed,
        });

        Ok(items)
    }

    /// Install a WAD archive into NAND and register it on the 48-slot channel grid.
    pub fn install_wad(&self, wad_path: &Path) -> Result<Channel, String> {
        let archive = WadArchive::open(wad_path)
            .map_err(|e| format!("cannot open WAD for installation: {e}"))?;

        let title_id = archive.tmd().title_id_hex.clone();
        let group_id = format!("{:04x}", archive.tmd().group_id);

        // Calculate required blocks
        let total_bytes: u64 = archive.tmd().contents.iter().map(|c| c.size).sum();
        let required_blocks = ((total_bytes + NAND_BLOCK_SIZE - 1) / NAND_BLOCK_SIZE) as u32;

        // Verify NAND capacity
        let current_free = self.calculate_free_blocks()?;
        if current_free < required_blocks {
            return Err(format!(
                "insufficient NAND space: required {required_blocks} blocks, but only {current_free} blocks available"
            ));
        }

        // Create target directory: /title/<group>/<title_id>/content/
        let title_dir = self.nand_root.join("title").join(&group_id).join(&title_id);
        let content_dir = title_dir.join("content");
        fs::create_dir_all(&content_dir).map_err(|e| e.to_string())?;

        // Write decrypted contents (.app)
        for meta in &archive.tmd().contents {
            let app_bytes = archive.decrypt_content(meta.index as usize)?;
            let app_filename = format!("{}.app", meta.cid_hex);
            fs::write(content_dir.join(app_filename), app_bytes).map_err(|e| e.to_string())?;
        }

        // Save metadata and ticket
        let tmd_json = serde_json::to_string_pretty(archive.tmd()).map_err(|e| e.to_string())?;
        fs::write(title_dir.join("title.tmd.json"), tmd_json).map_err(|e| e.to_string())?;
        let ticket_json = serde_json::to_string_pretty(archive.ticket()).map_err(|e| e.to_string())?;
        fs::write(title_dir.join("title.tik.json"), ticket_json).map_err(|e| e.to_string())?;

        // Derive channel name and ID
        let channel_id = format!("channel-{title_id}");
        let channel_title = format!("Title {}", &title_id[8..]);

        let new_channel = Channel {
            id: channel_id.clone(),
            title: channel_title,
            icon: Some("channel-blank".to_string()),
            content_kind: ContentKind::WiiNandTitle,
            content_path: Some(format!("title/{group_id}/{title_id}/content/00000000.app")),
            title_id: Some(title_id),
            app_id: None,
            blocks: Some(required_blocks),
            publisher: Some("Nintendo".to_string()),
            players: Some(1),
            preferred_page: None,
            preferred_slot: None,
            required_files: Vec::new(),
        };

        // Register in menu catalog
        let mut catalog = self.load_catalog()?;
        if let Some(pos) = catalog.channels.iter().position(|c| c.id == channel_id) {
            catalog.channels[pos] = new_channel.clone();
        } else {
            catalog.channels.push(new_channel.clone());
        }
        self.save_catalog(&catalog)?;

        Ok(new_channel)
    }

    /// Uninstall a title from NAND and remove it from the channel catalog.
    pub fn uninstall_title(&self, title_id_or_channel_id: &str) -> Result<u32, String> {
        let mut catalog = self.load_catalog()?;
        let pos = catalog
            .channels
            .iter()
            .position(|c| c.id == title_id_or_channel_id || c.title_id.as_deref() == Some(title_id_or_channel_id))
            .ok_or_else(|| format!("title {title_id_or_channel_id} not found in channel catalog"))?;

        let removed = catalog.channels.remove(pos);
        self.save_catalog(&catalog)?;

        // Delete from NAND directory if title_id is known
        if let Some(ref tid) = removed.title_id {
            let group_id = "0001"; // Default group
            let title_dir = self.nand_root.join("title").join(group_id).join(tid);
            if title_dir.is_dir() {
                let _ = fs::remove_dir_all(title_dir);
            }
        }

        let reclaimed_blocks = removed.blocks.unwrap_or(0);
        Ok(reclaimed_blocks)
    }

    /// Calculate currently free NAND blocks out of 2174 total blocks.
    pub fn calculate_free_blocks(&self) -> Result<u32, String> {
        let catalog = self.load_catalog()?;
        let used_blocks: u32 = catalog.channels.iter().filter_map(|c| c.blocks).sum();
        let free = NAND_TOTAL_BLOCKS.saturating_sub(used_blocks);
        Ok(free)
    }

    fn load_catalog(&self) -> Result<MenuCatalog, String> {
        if !self.catalog_path.is_file() {
            return Err(format!("menu catalog not found at {}", self.catalog_path.display()));
        }
        let text = fs::read_to_string(&self.catalog_path).map_err(|e| e.to_string())?;
        serde_json::from_str::<MenuCatalog>(&text).map_err(|e| e.to_string())
    }

    fn save_catalog(&self, catalog: &MenuCatalog) -> Result<(), String> {
        let text = serde_json::to_string_pretty(catalog).map_err(|e| e.to_string())?;
        fs::write(&self.catalog_path, text).map_err(|e| e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_store_items_listing() {
        let store = ChannelStore::new(".");
        let items = store.list_store_items().expect("failed to list store items");
        assert!(items.len() >= 4);
        assert!(items.iter().any(|i| i.id == "mii-channel-eu-v6"));
        assert!(items.iter().any(|i| i.id == "shop-channel-eu-v17"));
    }
}
