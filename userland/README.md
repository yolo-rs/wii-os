# Unified userland

All Wii OS runtime data and build output now live inside this product tree:

```text
userland/data/wii/             Wii catalogs, artwork, homebrew, ROMs and WAD inputs
userland/data/dolphin-wii-eu/  isolated Dolphin NAND/configuration/save profile
userland/data/wii-os.sqlite3   persistent Wii OS service database
userland/build/                reports, compiler targets, packages and deployments
```

WAD/NAND/game data and mutable Dolphin profiles are local operator data. They
are ignored by Git and excluded from portable releases. Dolphin stores title
saves and NAND state in the selected Dolphin profile; SQLite stores service
room definitions and the selected video backend. Build outputs and reports
stay alongside the product.

The former WSL userland was unregistered. Its Wii content tree was moved into
`userland/data/wii`; Linux builds now run on a separate Linux host. An old
offline system image still remains outside this product tree because Windows
blocked its removal.
