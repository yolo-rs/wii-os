# Wii PAL / Europe runtime set

The local Wii catalog selects these operator-provided files for Windows:

| Role | Local file | Region / relationship |
|---|---|---|
| System Menu | `System Software/SystemMenu-v514.wad` | Wii System Menu 4.3 PAL/EUR, v514 |
| System Menu IOS | `System Software/IOS80-64-v6944.wad` | IOS80 v6944; IOS80 is required by System Menu 4.3 |
| USB 2.0 IOS | `System Software/IOS58-64-v6176.wad` | IOS58 v6176, retained for compatible homebrew |
| Mii Channel | `Official Channels/Mii Channel v6 HACA.wad` | HACA is the PAL/EUR title variant |
| Shop Channel | `Official Channels/Shop Channel v17 HABA.wad` | HABA is the PAL/EUR title variant |
| Homebrew | `brew/apps/homebrew-browser/boot.dol` | Extracted from the local `homebrew_browser_v0.3.9e.zip`; Wii `.dol` homebrew is region-neutral |

`userland/data/wii/menu.catalog.json` points the application tiles
at these files. WADs, the extracted DOL and its icon stay in the ignored Wii
data tree and are never copied into the native portable package. A local web
build stages only the title files referenced by its catalog into that local
site output; building the site does not publish those files.

The Windows setup uses an isolated Dolphin user folder at
`userland/data/dolphin-wii-eu/`. Install IOS80 first, then System Menu v514,
then the Mii and Shop channels using Dolphin's WAD installer in that profile.
The runtime launcher now has explicit modes:

```powershell
./setup-windows-wii-europe.ps1 -BootSystemMenu
./setup-windows-wii-europe.ps1 -LaunchHomebrewBrowser
```

The first mode checks that the System Menu is installed, then starts NAND title
`0000000100000002` with Dolphin's D3D12 backend and diagnostics disabled. The second asks
Dolphin to boot the extracted Homebrew Browser DOL. No Homebrew Channel WAD or
full NAND dump was present in the local collection, so this setup does not claim
those contents or the original console's account, Mii database, saves or
certificates. The Shop Channel's network/account features require genuine
console credentials and are not simulated here.

## Host verification (2026-10-30)

On this Windows host, Dolphin confirmed successful installation of IOS80
v6944, IOS58 v6176, System Menu v514, Mii Channel HACA and Shop Channel HABA in
the isolated profile. The System Menu was then booted as NAND title
`0000000100000002`; Dolphin reported `Direct3D 12` and `Menu Wii` in the
emulation window title. Xbox XInput hardware is present, but its Wii Remote
mapping still needs to be configured and exercised in Dolphin's controller UI.

The version mapping is checked against [WiiBrew's System Menu table](https://www.wiibrew.org/wiki/System_Menu)
and [Dolphin's NAND guide](https://dolphin-emu.org/docs/guides/nand-usage-guide/).
