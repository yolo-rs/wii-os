-- ==============================================================================
-- Wii OS Architecture Schema (Production SQL DDL)
-- Compatible with SQLite 3.35+ (RETURNING) and PostgreSQL 14+
-- ==============================================================================

-- 1. Sysconf & Settings (Wii 4.3E System Configuration)
CREATE TABLE IF NOT EXISTS sysconf_settings (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL,
    category TEXT NOT NULL DEFAULT 'system' CHECK (category IN ('system', 'video', 'audio', 'network', 'parental')),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_sysconf_category ON sysconf_settings (category);

-- 2. Installed Channels & Disc Titles
CREATE TABLE IF NOT EXISTS channels (
    id TEXT PRIMARY KEY,
    app_id TEXT NOT NULL,
    title TEXT NOT NULL,
    title_id TEXT UNIQUE,
    content_kind TEXT NOT NULL CHECK (content_kind IN ('wii-disc', 'wii-wad', 'homebrew', 'wii-nand-title', 'system-utility')),
    icon_path TEXT NOT NULL,
    content_path TEXT,
    blocks INTEGER NOT NULL DEFAULT 1 CHECK (blocks >= 0),
    publisher TEXT NOT NULL DEFAULT 'Nintendo / Wii OS',
    players INTEGER NOT NULL DEFAULT 1 CHECK (players BETWEEN 1 AND 4),
    preferred_page INTEGER NOT NULL DEFAULT 0 CHECK (preferred_page BETWEEN 0 AND 3),
    preferred_slot INTEGER NOT NULL DEFAULT 0 CHECK (preferred_slot BETWEEN 0 AND 11),
    content_ready BOOLEAN NOT NULL DEFAULT 1,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_channels_content_kind ON channels (content_kind);
CREATE INDEX IF NOT EXISTS idx_channels_grid ON channels (preferred_page, preferred_slot);

-- 3. NetPlay Rooms & Arenas
CREATE TABLE IF NOT EXISTS rooms (
    id TEXT PRIMARY KEY,
    title_id TEXT REFERENCES channels(title_id) ON DELETE SET NULL,
    channel_id TEXT REFERENCES channels(id) ON DELETE CASCADE,
    room_name TEXT NOT NULL,
    host_nickname TEXT NOT NULL,
    host_address TEXT NOT NULL,
    port INTEGER NOT NULL DEFAULT 2626 CHECK (port BETWEEN 1024 AND 65535),
    pad_buffer INTEGER NOT NULL DEFAULT 6 CHECK (pad_buffer BETWEEN 1 AND 30),
    max_players INTEGER NOT NULL DEFAULT 4 CHECK (max_players BETWEEN 2 AND 16),
    status TEXT NOT NULL DEFAULT 'lobby' CHECK (status IN ('lobby', 'in_game', 'closed')),
    transport TEXT NOT NULL DEFAULT 'dolphin-netplay-ffi' CHECK (transport IN ('dolphin-netplay-ffi', 'webrtc-peer', 'websocket-relay')),
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_rooms_status ON rooms (status);
CREATE INDEX IF NOT EXISTS idx_rooms_channel ON rooms (channel_id);

-- 4. NetPlay Room Messages & Chat
CREATE TABLE IF NOT EXISTS room_messages (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    room_id TEXT NOT NULL REFERENCES rooms(id) ON DELETE CASCADE,
    sender TEXT NOT NULL,
    message TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_room_messages_room_created ON room_messages (room_id, created_at);

-- 5. Wii Message Board (Bureau Wii / Journal quotidien)
CREATE TABLE IF NOT EXISTS board_messages (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    date_key TEXT NOT NULL,
    kind TEXT NOT NULL DEFAULT 'memo' CHECK (kind IN ('memo', 'system', 'play_log', 'letter', 'wii_mail')),
    title TEXT NOT NULL,
    author TEXT NOT NULL,
    body TEXT NOT NULL,
    title_id TEXT REFERENCES channels(title_id) ON DELETE SET NULL,
    duration_minutes INTEGER NOT NULL DEFAULT 0 CHECK (duration_minutes >= 0),
    read BOOLEAN NOT NULL DEFAULT 0,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_board_messages_date ON board_messages (date_key, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_board_messages_unread ON board_messages (read) WHERE read = 0;

-- 6. NAND Titles & Storage Accounting
CREATE TABLE IF NOT EXISTS nand_titles (
    id TEXT PRIMARY KEY,
    title_id TEXT NOT NULL UNIQUE,
    title TEXT NOT NULL,
    subtitle TEXT NOT NULL,
    category TEXT NOT NULL CHECK (category IN ('save-wii', 'channel-wii', 'sysmenu-wii', 'ios-system')),
    blocks INTEGER NOT NULL CHECK (blocks >= 0),
    bytes INTEGER NOT NULL CHECK (bytes >= 0),
    can_copy BOOLEAN NOT NULL DEFAULT 0,
    can_delete BOOLEAN NOT NULL DEFAULT 0,
    payload_path TEXT,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_nand_titles_category ON nand_titles (category);

-- 7. Audit & Launch Logs
CREATE TABLE IF NOT EXISTS launch_audit_logs (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    channel_id TEXT NOT NULL REFERENCES channels(id) ON DELETE CASCADE,
    title_id TEXT,
    launcher_mode TEXT NOT NULL CHECK (launcher_mode IN ('webgpu-wasm', 'dolphin-native', 'browser-preview')),
    client_ip TEXT NOT NULL,
    user_agent TEXT,
    duration_seconds INTEGER,
    exit_status TEXT CHECK (exit_status IN ('success', 'user_exit', 'oom_killed', 'file_not_found', 'unsupported_gpu')),
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_launch_audit_channel ON launch_audit_logs (channel_id, created_at DESC);
