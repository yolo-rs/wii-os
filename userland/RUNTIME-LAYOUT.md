# Wii runtime data layout

The userland is a normal directory under the Wii OS product and does not depend
on WSL or a Linux distribution.

| Path | Purpose | Handling |
|---|---|---|
| `data/wii/roms/` | Operator-provided Wii/GameCube disc images | Never copy into releases |
| `data/wii/wads/` | Operator-provided WAD files | Never fetch or redistribute |
| `data/wii/brew/` | Homebrew `.dol` / `.elf` inputs | User-managed |
| `data/wii/assets/` | User-provided channel art and optional startup media | Excluded from releases |
| `data/dolphin-wii-eu/` | Isolated Dolphin NAND, configuration and game saves | Persistent mutable profile |
| `data/wii-os.sqlite3` | Persistent service room definitions | WAL mode; versioned schema |
| `build/` | Compiler output, reports, portable packages and deployment staging | Generated product output |

Do not store ROMs, NAND dumps, BIOS files, WADs or user artwork in tracked
source. Dolphin title saves remain in the configured Dolphin profile; service
room records persist in SQLite.
