//! Embedded static assets for the single-binary self-hostable Wii OS BIOS 4.3E server.
//! Every image, sound effect, stylesheet, and script from `frontend/assets/` is compiled
//! directly into the Rust binary via `include_bytes!` so `wii` / `wii-os-server` runs
//! as a true zero-dependency single binary anywhere.

pub struct EmbeddedAsset {
    pub bytes: &'static [u8],
    pub content_type: &'static str,
}

pub fn lookup_embedded_asset(rel_path: &str) -> Option<EmbeddedAsset> {
    let normalized = rel_path.trim_start_matches('/').replace('\\', "/");
    let (bytes, content_type): (&'static [u8], &'static str) = match normalized.as_str() {
        // Audio cues (Wii System Menu 4.3E)
        "audio/back.mp3" => (
            include_bytes!("../../../frontend/assets/audio/back.mp3"),
            "audio/mpeg",
        ),
        "audio/bg-music.mp3" => (
            include_bytes!("../../../frontend/assets/audio/bg-music.mp3"),
            "audio/mpeg",
        ),
        "audio/button-hover.mp3" => (
            include_bytes!("../../../frontend/assets/audio/button-hover.mp3"),
            "audio/mpeg",
        ),
        "audio/button-select.mp3" => (
            include_bytes!("../../../frontend/assets/audio/button-select.mp3"),
            "audio/mpeg",
        ),
        "audio/startup.mp3" => (
            include_bytes!("../../../frontend/assets/audio/startup.mp3"),
            "audio/mpeg",
        ),
        "audio/zip.mp3" => (
            include_bytes!("../../../frontend/assets/audio/zip.mp3"),
            "audio/mpeg",
        ),

        // Authentic Wii CSS
        "css/menu.css" => (
            include_bytes!("../../../frontend/assets/css/menu.css"),
            "text/css; charset=utf-8",
        ),
        "css/settings.css" => (
            include_bytes!("../../../frontend/assets/css/settings.css"),
            "text/css; charset=utf-8",
        ),
        "css/style.css" => (
            include_bytes!("../../../frontend/assets/css/style.css"),
            "text/css; charset=utf-8",
        ),
        "css/tokens.css" => (
            include_bytes!("../../../frontend/assets/css/tokens.css"),
            "text/css; charset=utf-8",
        ),
        "css/universal.css" => (
            include_bytes!("../../../frontend/assets/css/universal.css"),
            "text/css; charset=utf-8",
        ),

        // Authentic Wii Menu 4.3E Images & Spritesheets
        "images/bg-pattern.png" => (
            include_bytes!("../../../frontend/assets/images/bg-pattern.png"),
            "image/png",
        ),
        "images/border-bottomleft.png" => (
            include_bytes!("../../../frontend/assets/images/border-bottomleft.png"),
            "image/png",
        ),
        "images/border-bottomright.png" => (
            include_bytes!("../../../frontend/assets/images/border-bottomright.png"),
            "image/png",
        ),
        "images/border-topleft.png" => (
            include_bytes!("../../../frontend/assets/images/border-topleft.png"),
            "image/png",
        ),
        "images/border-topright.png" => (
            include_bytes!("../../../frontend/assets/images/border-topright.png"),
            "image/png",
        ),
        "images/bottom-bg.png" => (
            include_bytes!("../../../frontend/assets/images/bottom-bg.png"),
            "image/png",
        ),
        "images/bottom-title.png" => (
            include_bytes!("../../../frontend/assets/images/bottom-title.png"),
            "image/png",
        ),
        "images/btn-left.png" => (
            include_bytes!("../../../frontend/assets/images/btn-left.png"),
            "image/png",
        ),
        "images/btn-right.png" => (
            include_bytes!("../../../frontend/assets/images/btn-right.png"),
            "image/png",
        ),
        "images/btn-texture.png" => (
            include_bytes!("../../../frontend/assets/images/btn-texture.png"),
            "image/png",
        ),
        "images/channel-border.png" => (
            include_bytes!("../../../frontend/assets/images/channel-border.png"),
            "image/png",
        ),
        "images/channel-hover.png" => (
            include_bytes!("../../../frontend/assets/images/channel-hover.png"),
            "image/png",
        ),
        "images/channel-icon.png" => (
            include_bytes!("../../../frontend/assets/images/channel-icon.png"),
            "image/png",
        ),
        "images/channel-spritesheet.png" => (
            include_bytes!("../../../frontend/assets/images/channel-spritesheet.png"),
            "image/png",
        ),
        "images/channel-wiilogo.png" => (
            include_bytes!("../../../frontend/assets/images/channel-wiilogo.png"),
            "image/png",
        ),
        "images/cursor.png" => (
            include_bytes!("../../../frontend/assets/images/cursor.png"),
            "image/png",
        ),
        "images/info-icon.png" => (
            include_bytes!("../../../frontend/assets/images/info-icon.png"),
            "image/png",
        ),
        "images/left-button.png" => (
            include_bytes!("../../../frontend/assets/images/left-button.png"),
            "image/png",
        ),
        "images/logo-white.svg" => (
            include_bytes!("../../../frontend/assets/images/logo-white.svg"),
            "image/svg+xml",
        ),
        "images/mail-button.png" => (
            include_bytes!("../../../frontend/assets/images/mail-button.png"),
            "image/png",
        ),
        "images/miichannel.jpg" => (
            include_bytes!("../../../frontend/assets/images/miichannel.jpg"),
            "image/jpeg",
        ),
        "images/mini-bottom-bg.png" => (
            include_bytes!("../../../frontend/assets/images/mini-bottom-bg.png"),
            "image/png",
        ),
        "images/right-button.png" => (
            include_bytes!("../../../frontend/assets/images/right-button.png"),
            "image/png",
        ),
        "images/settings-back.png" => (
            include_bytes!("../../../frontend/assets/images/settings-back.png"),
            "image/png",
        ),
        "images/settings-bottombar.png" => (
            include_bytes!("../../../frontend/assets/images/settings-bottombar.png"),
            "image/png",
        ),
        "images/settings-icon.png" => (
            include_bytes!("../../../frontend/assets/images/settings-icon.png"),
            "image/png",
        ),
        "images/settings-main-bg.png" => (
            include_bytes!("../../../frontend/assets/images/settings-main-bg.png"),
            "image/png",
        ),
        "images/settings-main-nav.png" => (
            include_bytes!("../../../frontend/assets/images/settings-main-nav.png"),
            "image/png",
        ),
        "images/settings-topbar.png" => (
            include_bytes!("../../../frontend/assets/images/settings-topbar.png"),
            "image/png",
        ),
        "images/splash-bar.png" => (
            include_bytes!("../../../frontend/assets/images/splash-bar.png"),
            "image/png",
        ),
        "images/wii-button.png" => (
            include_bytes!("../../../frontend/assets/images/wii-button.png"),
            "image/png",
        ),
        _ => return None,
    };

    Some(EmbeddedAsset {
        bytes,
        content_type,
    })
}
