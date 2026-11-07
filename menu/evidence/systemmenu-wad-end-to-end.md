# Wii System Menu 4.3E WAD — End-to-End Reverse-Engineering Ledger

Detailed technical ledger for the verification and bit-level dissection of `SystemMenu-v514.wad`.

- **File**: `userland/data/wii/wads/System Software/SystemMenu-v514.wad`
- **Size**: 23,801,856 bytes
- **Title ID**: `0000000100000002` (System Menu)
- **Version**: 514 (Wii Menu 4.3E, PAL Europe)
- **Required OS**: IOS80 (`0000000100000050`)
- **Build Tag**: `systemmenu.rvl.1004200553` (April 20, 2010 05:53 UTC by `irduser@IPLBUIL`)
- **Common Key**: Index 0 (`ebe42a225e8593e448d9c5457381aaf7`)
- **Decrypted Title Key**: `33c5c117ae51c17682ff9f1c85f8d20a`
- **Total Contents**: 9 (all verified with 100% SHA-1 matches against TMD)

## Contents Summary

| Index | CID | Type | Size | Nature |
| :--- | :--- | :--- | :--- | :--- |
| 0 | `00000099` | Private | 64 B | Build string (`systemmenu.rvl.1004200553`) |
| 1 | `0000009a` | Private | 7.68 MB | Main UI U8 archive: 63 files (`chanSel.ash`, `IplSound.brsar`, `ipl_common_noe.bmg`, etc.) |
| 2 | `00000009` | Shared | 4.55 MB | `wwwlib-rvl.lz7`: Embedded Opera HTML/JS rendering engine |
| 3 | `0000000a` | Shared | 2.66 MB | `WiiNTLG-Regular.ttc`: Official New Rodin / NTLG system TrueType font |
| 4 | `0000000b` | Shared | 686 KB | `RFL_Res.dat`: Revolution Face Library (Mii database) |
| 5 | `0000000c` | Shared | 3.64 MB | `wbf1.brfna`, `wbf2.brfna`: GX hardware texture bitmap fonts |
| 6 | `0000001b` | Shared | 606 KB | `eZTSystemEU.arc`: Electronic manual engine |
| 7 | `00000033` | Shared | 70 KB | `eZTNintendoEU.arc`: Safety and regulatory warnings |
| 8 | `0000009b` | Boot | 3.86 MB | `main.dol`: PowerPC 750CL executable (Entry: `0x80003400`) |
