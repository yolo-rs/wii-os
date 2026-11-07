use std::path::{Path, PathBuf};

use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};
use serde::{Deserialize, Serialize};
use wii_menu_core::WiiSysconf;

use crate::Room;

const SCHEMA_VERSION: i64 = 2;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BoardMessage {
    pub id: i64,
    pub date_key: String,
    /// "memo", "play-history", "system", "netplay"
    pub kind: String,
    pub title: String,
    pub author: String,
    pub body: String,
    pub title_id: Option<String>,
    pub duration_minutes: i64,
    pub read: bool,
    pub created_at: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct NewBoardMessage {
    pub date_key: Option<String>,
    pub kind: Option<String>,
    pub title: String,
    pub author: Option<String>,
    pub body: String,
    pub title_id: Option<String>,
    pub duration_minutes: Option<i64>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RoomChatMessage {
    pub id: i64,
    pub room_id: String,
    pub sender: String,
    pub message: String,
    pub created_at: String,
}

#[derive(Clone)]
pub(super) struct RoomDatabase {
    path: PathBuf,
}

impl RoomDatabase {
    pub(super) async fn open(
        path: PathBuf,
        legacy_json: PathBuf,
    ) -> Result<(Self, Vec<Room>), String> {
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent).await.map_err(|error| {
                format!("create SQLite directory {}: {error}", parent.display())
            })?;
        }

        let database_path = path.clone();
        let rooms = tokio::task::spawn_blocking(move || initialize(&database_path, &legacy_json))
            .await
            .map_err(|error| format!("SQLite initialization task failed: {error}"))??;

        Ok((Self { path }, rooms))
    }

    pub(super) async fn insert_room(&self, room: Room) -> Result<(), String> {
        let path = self.path.clone();
        tokio::task::spawn_blocking(move || {
            let mut connection = connect(&path)?;
            let transaction = connection
                .transaction_with_behavior(TransactionBehavior::Immediate)
                .map_err(db_error)?;
            transaction
                .execute(
                    "INSERT INTO rooms (id, title_id, max_players, transport, channel_id, host_nickname, host_address, port, pad_buffer, game_digest, status)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
                     ON CONFLICT(id) DO UPDATE SET
                        title_id = excluded.title_id,
                        max_players = excluded.max_players,
                        transport = excluded.transport,
                        channel_id = excluded.channel_id,
                        host_nickname = excluded.host_nickname,
                        host_address = excluded.host_address,
                        port = excluded.port,
                        pad_buffer = excluded.pad_buffer,
                        game_digest = excluded.game_digest,
                        status = excluded.status",
                    params![
                        room.id,
                        room.title_id,
                        room.max_players,
                        room.transport,
                        room.channel_id,
                        room.host_nickname.unwrap_or_else(|| "Hôte Wii".to_string()),
                        room.host_address.unwrap_or_else(|| "127.0.0.1".to_string()),
                        room.port.unwrap_or(2626),
                        room.pad_buffer.unwrap_or(6),
                        room.game_digest,
                        room.status.unwrap_or_else(|| "lobby".to_string()),
                    ],
                )
                .map_err(db_error)?;
            transaction.commit().map_err(db_error)
        })
        .await
        .map_err(|error| format!("SQLite room write task failed: {error}"))?
    }

    pub(super) async fn setting(&self, key: &str) -> Result<Option<String>, String> {
        let path = self.path.clone();
        let key = key.to_owned();
        tokio::task::spawn_blocking(move || {
            let connection = connect(&path)?;
            connection
                .query_row("SELECT value FROM settings WHERE key = ?1", [key], |row| {
                    row.get(0)
                })
                .optional()
                .map_err(db_error)
        })
        .await
        .map_err(|error| format!("SQLite setting read task failed: {error}"))?
    }

    pub(super) async fn set_setting(&self, key: &str, value: &str) -> Result<(), String> {
        let path = self.path.clone();
        let key = key.to_owned();
        let value = value.to_owned();
        tokio::task::spawn_blocking(move || {
            let mut connection = connect(&path)?;
            let transaction = connection
                .transaction_with_behavior(TransactionBehavior::Immediate)
                .map_err(db_error)?;
            transaction
                .execute(
                    "INSERT INTO settings (key, value, updated_at) VALUES (?1, ?2, CURRENT_TIMESTAMP)
                     ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = CURRENT_TIMESTAMP",
                    params![key, value],
                )
                .map_err(db_error)?;
            transaction.commit().map_err(db_error)
        })
        .await
        .map_err(|error| format!("SQLite setting write task failed: {error}"))?
    }

    pub(super) async fn get_sysconf(&self) -> Result<WiiSysconf, String> {
        let mut sysconf = match self.setting("bios.sysconf").await? {
            Some(json) => serde_json::from_str::<WiiSysconf>(&json).unwrap_or_default(),
            None => WiiSysconf::default(),
        };
        if let Some(backend) = self.setting("dolphin.videoBackend").await? {
            sysconf.video_backend = backend;
        }
        Ok(sysconf)
    }

    pub(super) async fn save_sysconf(&self, sysconf: &WiiSysconf) -> Result<(), String> {
        let json = serde_json::to_string(sysconf).map_err(|e| e.to_string())?;
        self.set_setting("bios.sysconf", &json).await?;
        self.set_setting("dolphin.videoBackend", &sysconf.video_backend)
            .await?;
        Ok(())
    }

    pub(super) async fn list_board_messages(&self) -> Result<Vec<BoardMessage>, String> {
        let path = self.path.clone();
        tokio::task::spawn_blocking(move || {
            let connection = connect(&path)?;
            let mut stmt = connection
                .prepare(
                    "SELECT id, date_key, kind, title, author, body, title_id, duration_minutes, read, created_at
                     FROM board_messages
                     ORDER BY id DESC LIMIT 100",
                )
                .map_err(db_error)?;
            let rows = stmt
                .query_map([], |row| {
                    let read_int: i64 = row.get(8)?;
                    Ok(BoardMessage {
                        id: row.get(0)?,
                        date_key: row.get(1)?,
                        kind: row.get(2)?,
                        title: row.get(3)?,
                        author: row.get(4)?,
                        body: row.get(5)?,
                        title_id: row.get(6)?,
                        duration_minutes: row.get(7)?,
                        read: read_int != 0,
                        created_at: row.get(9)?,
                    })
                })
                .map_err(db_error)?;
            rows.collect::<rusqlite::Result<Vec<_>>>().map_err(db_error)
        })
        .await
        .map_err(|error| format!("SQLite board read task failed: {error}"))?
    }

    pub(super) async fn insert_board_message(
        &self,
        input: NewBoardMessage,
    ) -> Result<BoardMessage, String> {
        let path = self.path.clone();
        tokio::task::spawn_blocking(move || {
            let mut connection = connect(&path)?;
            let tx = connection
                .transaction_with_behavior(TransactionBehavior::Immediate)
                .map_err(db_error)?;
            let date_key = input
                .date_key
                .unwrap_or_else(|| "Aujourd'hui".to_string());
            let kind = input.kind.unwrap_or_else(|| "memo".to_string());
            let author = input.author.unwrap_or_else(|| "Console Wii".to_string());
            let duration = input.duration_minutes.unwrap_or(0);
            tx.execute(
                "INSERT INTO board_messages (date_key, kind, title, author, body, title_id, duration_minutes, read)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 0)",
                params![
                    date_key,
                    kind,
                    input.title,
                    author,
                    input.body,
                    input.title_id,
                    duration
                ],
            )
            .map_err(db_error)?;
            let id = tx.last_insert_rowid();
            let msg = tx
                .query_row(
                    "SELECT id, date_key, kind, title, author, body, title_id, duration_minutes, read, created_at
                     FROM board_messages WHERE id = ?1",
                    [id],
                    |row| {
                        let read_int: i64 = row.get(8)?;
                        Ok(BoardMessage {
                            id: row.get(0)?,
                            date_key: row.get(1)?,
                            kind: row.get(2)?,
                            title: row.get(3)?,
                            author: row.get(4)?,
                            body: row.get(5)?,
                            title_id: row.get(6)?,
                            duration_minutes: row.get(7)?,
                            read: read_int != 0,
                            created_at: row.get(9)?,
                        })
                    },
                )
                .map_err(db_error)?;
            tx.commit().map_err(db_error)?;
            Ok(msg)
        })
        .await
        .map_err(|error| format!("SQLite board write task failed: {error}"))?
    }

    pub(super) async fn mark_board_message_read(&self, id: i64) -> Result<(), String> {
        let path = self.path.clone();
        tokio::task::spawn_blocking(move || {
            let connection = connect(&path)?;
            connection
                .execute("UPDATE board_messages SET read = 1 WHERE id = ?1", [id])
                .map_err(db_error)?;
            Ok(())
        })
        .await
        .map_err(|error| format!("SQLite board update task failed: {error}"))?
    }

    pub(super) async fn delete_board_message(&self, id: i64) -> Result<(), String> {
        let path = self.path.clone();
        tokio::task::spawn_blocking(move || {
            let connection = connect(&path)?;
            connection
                .execute("DELETE FROM board_messages WHERE id = ?1", [id])
                .map_err(db_error)?;
            Ok(())
        })
        .await
        .map_err(|error| format!("SQLite board delete task failed: {error}"))?
    }

    pub(super) async fn list_room_messages(
        &self,
        room_id: &str,
    ) -> Result<Vec<RoomChatMessage>, String> {
        let path = self.path.clone();
        let room_id = room_id.to_owned();
        tokio::task::spawn_blocking(move || {
            let connection = connect(&path)?;
            let mut stmt = connection
                .prepare(
                    "SELECT id, room_id, sender, message, created_at
                     FROM room_messages
                     WHERE room_id = ?1
                     ORDER BY id ASC LIMIT 200",
                )
                .map_err(db_error)?;
            let rows = stmt
                .query_map([room_id], |row| {
                    Ok(RoomChatMessage {
                        id: row.get(0)?,
                        room_id: row.get(1)?,
                        sender: row.get(2)?,
                        message: row.get(3)?,
                        created_at: row.get(4)?,
                    })
                })
                .map_err(db_error)?;
            rows.collect::<rusqlite::Result<Vec<_>>>().map_err(db_error)
        })
        .await
        .map_err(|error| format!("SQLite room chat read failed: {error}"))?
    }

    pub(super) async fn insert_room_message(
        &self,
        room_id: &str,
        sender: &str,
        message: &str,
    ) -> Result<RoomChatMessage, String> {
        let path = self.path.clone();
        let room_id = room_id.to_owned();
        let sender = sender.to_owned();
        let message = message.to_owned();
        tokio::task::spawn_blocking(move || {
            let mut connection = connect(&path)?;
            let tx = connection
                .transaction_with_behavior(TransactionBehavior::Immediate)
                .map_err(db_error)?;
            tx.execute(
                "INSERT INTO room_messages (room_id, sender, message) VALUES (?1, ?2, ?3)",
                params![room_id, sender, message],
            )
            .map_err(db_error)?;
            let id = tx.last_insert_rowid();
            let row = tx
                .query_row(
                    "SELECT id, room_id, sender, message, created_at FROM room_messages WHERE id = ?1",
                    [id],
                    |r| {
                        Ok(RoomChatMessage {
                            id: r.get(0)?,
                            room_id: r.get(1)?,
                            sender: r.get(2)?,
                            message: r.get(3)?,
                            created_at: r.get(4)?,
                        })
                    },
                )
                .map_err(db_error)?;
            tx.commit().map_err(db_error)?;
            Ok(row)
        })
        .await
        .map_err(|error| format!("SQLite room chat write failed: {error}"))?
    }
}

fn connect(path: &Path) -> Result<Connection, String> {
    let connection = Connection::open(path).map_err(db_error)?;
    connection
        .busy_timeout(std::time::Duration::from_secs(5))
        .map_err(db_error)?;
    connection
        .pragma_update(None, "foreign_keys", "ON")
        .map_err(db_error)?;
    connection
        .pragma_update(None, "synchronous", "FULL")
        .map_err(db_error)?;
    Ok(connection)
}

fn initialize(path: &Path, legacy_json: &Path) -> Result<Vec<Room>, String> {
    let mut connection = connect(path)?;
    connection
        .pragma_update(None, "journal_mode", "WAL")
        .map_err(db_error)?;

    let current_version: i64 = connection
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .map_err(db_error)?;
    if current_version > SCHEMA_VERSION {
        return Err(format!(
            "database schema {current_version} is newer than this server supports ({SCHEMA_VERSION})"
        ));
    }

    if current_version < 1 {
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db_error)?;
        transaction
            .execute_batch(
                "CREATE TABLE IF NOT EXISTS rooms (
                    id TEXT PRIMARY KEY NOT NULL CHECK(length(id) BETWEEN 1 AND 64),
                    title_id TEXT,
                    max_players INTEGER NOT NULL CHECK(max_players BETWEEN 1 AND 16),
                    transport TEXT NOT NULL,
                    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
                );
                CREATE INDEX IF NOT EXISTS rooms_created_at_idx ON rooms(created_at);
                CREATE TABLE IF NOT EXISTS settings (
                    key TEXT PRIMARY KEY NOT NULL CHECK(length(key) BETWEEN 1 AND 128),
                    value TEXT NOT NULL,
                    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
                );
                PRAGMA user_version = 1;",
            )
            .map_err(db_error)?;
        transaction.commit().map_err(db_error)?;
    }

    if current_version < 2 {
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db_error)?;
        for alter in [
            "ALTER TABLE rooms ADD COLUMN channel_id TEXT",
            "ALTER TABLE rooms ADD COLUMN host_nickname TEXT NOT NULL DEFAULT 'Hôte Wii'",
            "ALTER TABLE rooms ADD COLUMN host_address TEXT NOT NULL DEFAULT '127.0.0.1'",
            "ALTER TABLE rooms ADD COLUMN port INTEGER NOT NULL DEFAULT 2626",
            "ALTER TABLE rooms ADD COLUMN pad_buffer INTEGER NOT NULL DEFAULT 6",
            "ALTER TABLE rooms ADD COLUMN game_digest TEXT",
            "ALTER TABLE rooms ADD COLUMN status TEXT NOT NULL DEFAULT 'lobby'",
        ] {
            let _ = transaction.execute(alter, []);
        }

        transaction
            .execute_batch(
                "CREATE TABLE IF NOT EXISTS room_messages (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    room_id TEXT NOT NULL,
                    sender TEXT NOT NULL,
                    message TEXT NOT NULL,
                    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
                );
                CREATE INDEX IF NOT EXISTS room_messages_room_idx ON room_messages(room_id, id);

                CREATE TABLE IF NOT EXISTS board_messages (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    date_key TEXT NOT NULL,
                    kind TEXT NOT NULL,
                    title TEXT NOT NULL,
                    author TEXT NOT NULL,
                    body TEXT NOT NULL,
                    title_id TEXT,
                    duration_minutes INTEGER NOT NULL DEFAULT 0,
                    read INTEGER NOT NULL DEFAULT 0,
                    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
                );
                PRAGMA user_version = 2;",
            )
            .map_err(db_error)?;

        let board_count: i64 = transaction
            .query_row("SELECT COUNT(*) FROM board_messages", [], |row| row.get(0))
            .unwrap_or(0);
        if board_count == 0 {
            let _ = transaction.execute(
                "INSERT INTO board_messages (date_key, kind, title, author, body, title_id, duration_minutes, read)
                 VALUES
                 ('Aujourd''hui', 'system', 'Menu Système Wii 4.3E — BIOS & NetPlay', 'Nintendo / Wii OS', 'Bienvenue sur l''interface BIOS Wii System Menu 4.3E pixel-perfect.\n• Maintenez + et - sur l''écran Santé et Sécurité pour le Mode Maintenance.\n• Glissez-déposez vos chaînes (A+B) sur les 4 pages.\n• Gérez vos sauvegardes NAND/SD et les 3 pages de Paramètres Wii.\n• Hébergez vos parties NetPlay et packagez vos jeux en WASM ou EXE.', '0000000100000002', 0, 0),
                 ('Aujourd''hui', 'play-history', 'Historique du jour', 'Bureau Wii', 'Dragon Ball Z : Budokai Tenkaichi 3 — 0 h 48 min\nChaîne Mii PAL v6 — 0 h 14 min\nHomebrew Browser 0.3.9e — 0 h 09 min', 'RDSPAF', 71, 0),
                 ('Aujourd''hui', 'netplay', 'Arène NetPlay Budokai Tenkaichi 3 prête', 'WiiConnect24', 'Le pont C++ DolphinNetPlaySession (ENet :2626) et la signalisation WebRTC sont actifs. Ouvrez la Chaîne NetPlay ou cliquez sur Démarrer en NetPlay depuis l''aperçu d''un jeu.', 'RDSPAF', 0, 0)",
                [],
            );
        }
        transaction.commit().map_err(db_error)?;
    }

    import_legacy_rooms(&mut connection, legacy_json)?;
    read_rooms(&connection)
}

fn import_legacy_rooms(connection: &mut Connection, path: &Path) -> Result<(), String> {
    if !path.is_file() {
        return Ok(());
    }
    let bytes = std::fs::read(path)
        .map_err(|error| format!("read legacy room catalog {}: {error}", path.display()))?;
    let rooms: std::collections::BTreeMap<String, Room> = serde_json::from_slice(&bytes)
        .map_err(|error| format!("parse legacy room catalog {}: {error}", path.display()))?;

    let existing: i64 = connection
        .query_row("SELECT COUNT(*) FROM rooms", [], |row| row.get(0))
        .map_err(db_error)?;
    if existing != 0 || rooms.is_empty() {
        return Ok(());
    }

    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(db_error)?;
    for (id, room) in rooms {
        if id != room.id || id.trim().is_empty() || id.len() > 64 {
            continue;
        }
        transaction
            .execute(
                "INSERT INTO rooms (id, title_id, max_players, transport) VALUES (?1, ?2, ?3, ?4)",
                params![room.id, room.title_id, room.max_players, room.transport],
            )
            .map_err(db_error)?;
    }
    transaction.commit().map_err(db_error)
}

fn read_rooms(connection: &Connection) -> Result<Vec<Room>, String> {
    let mut statement = connection
        .prepare(
            "SELECT id, title_id, max_players, transport, channel_id, host_nickname, host_address, port, pad_buffer, game_digest, status
             FROM rooms ORDER BY created_at, id",
        )
        .map_err(db_error)?;
    let rows = statement
        .query_map([], |row| {
            let id: String = row.get(0)?;
            let invite_code = crate::generate_room_invite_code(&id);
            let invite_url = format!("http://127.0.0.1:8780/?join={invite_code}");
            Ok(Room {
                id,
                title_id: row.get(1)?,
                players: 0,
                max_players: row.get(2)?,
                transport: row.get(3)?,
                channel_id: row.get(4).ok().flatten(),
                host_nickname: row.get(5).ok(),
                host_address: row.get(6).ok(),
                port: row.get::<_, Option<u16>>(7).ok().flatten(),
                pad_buffer: row.get::<_, Option<u16>>(8).ok().flatten(),
                game_digest: row.get(9).ok().flatten(),
                status: row.get(10).ok(),
                invite_code: Some(invite_code),
                invite_url: Some(invite_url),
            })
        })
        .map_err(db_error)?;
    rows.collect::<rusqlite::Result<Vec<_>>>().map_err(db_error)
}

fn db_error(error: rusqlite::Error) -> String {
    format!("SQLite: {error}")
}
