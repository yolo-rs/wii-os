[CmdletBinding()]
param(
    [ValidateSet('windows-x64')]
    [string] $Target = 'windows-x64',
    [string] $OutputDirectory = (Join-Path $PSScriptRoot 'userland/build/packages'),
    [string] $SevenZip = 'C:\Program Files\7-Zip\7z.exe'
)

$ErrorActionPreference = 'Stop'
$repoRoot = $PSScriptRoot
$productRoot = $repoRoot
$manifest = Join-Path $repoRoot 'apps/server/Cargo.toml'
$targetDirectory = Join-Path $productRoot 'userland/build/targets/wii-os-portable'
$sourceRoots = Get-Content -LiteralPath (Join-Path $repoRoot 'source-roots.json') -Raw | ConvertFrom-Json
$dolphinSourceRecord = @($sourceRoots.repositories | Where-Object { $_.name -eq 'wii-os/src/dolphin' }) | Select-Object -First 1
if ($null -eq $dolphinSourceRecord) { throw 'source-roots.json must identify the local Wii OS Dolphin source snapshot' }
$dolphinSource = [IO.Path]::GetFullPath((Join-Path $repoRoot $dolphinSourceRecord.path))
$dolphinBuild = Join-Path $productRoot 'userland/build/targets/dolphin-wii-os-src-windows-msvc-x64'
$dolphinExe = Join-Path $dolphinBuild 'Binaries/DolphinNoGUI.exe'
$dolphinFfiDll = Join-Path $dolphinBuild 'Binaries/wii_dolphin_ffi.dll'
$gameFileName = 'Dragon-Ball-Z-Budokai-Tenkaichi-3-Europe-En-Fr-De-Es-It.rvz'
$gameSource = Join-Path $productRoot "userland/data/wii/roms/$gameFileName"
$wiiDataRoot = Join-Path $productRoot 'userland/data/wii'
$profileSource = Join-Path $productRoot 'userland/data/dolphin-wii-eu'
$selectedWads = @{
    'SystemMenu-v514.wad' = 'System Software/SystemMenu-v514.wad'
    'IOS80-64-v6944.wad' = 'System Software/IOS80-64-v6944.wad'
    'Mii Channel v6 HACA.wad' = 'Official Channels/Mii Channel v6 HACA.wad'
    'Shop Channel v17 HABA.wad' = 'Official Channels/Shop Channel v17 HABA.wad'
}
$nativeMenuAssets = @(
    'bottom-bg.png', 'bottom-title.png', 'channel-border.png', 'channel-hover.png',
    'channel-spritesheet.png', 'channel-wiilogo.png', 'left-button.png',
    'right-button.png', 'wii-button.png', 'mail-button.png', 'channel-icon.png',
    'miichannel.jpg'
)
$server = Join-Path $targetDirectory 'release/wii-os-server.exe'
$menu = Join-Path $targetDirectory 'release/wii-menu-native.exe'
$outputDirectory = [IO.Path]::GetFullPath($OutputDirectory)
$stagingRoot = Join-Path ([IO.Path]::GetTempPath()) ('wii-os-package-' + [guid]::NewGuid().ToString('N'))
$stage = Join-Path $stagingRoot 'wii-os-windows-x64'
$archive = Join-Path $outputDirectory 'wii-os-windows-x64.7z'

if (-not (Test-Path $SevenZip -PathType Leaf)) {
    throw "7-Zip executable not found: $SevenZip"
}
$cargo = Get-Command cargo -ErrorAction Stop
$cmake = Get-Command cmake -ErrorAction Stop
$null = New-Item -ItemType Directory -Path $outputDirectory -Force

if (-not (Test-Path (Join-Path $dolphinSource 'CMakeLists.txt') -PathType Leaf)) {
    throw "Dolphin source checkout is missing: $dolphinSource"
}
$revisionFile = Join-Path $dolphinSource $dolphinSourceRecord.revisionFile
if (-not (Test-Path -LiteralPath $revisionFile -PathType Leaf)) { throw "Dolphin source revision marker is missing: $revisionFile" }
$revisionMatch = [regex]::Match((Get-Content -LiteralPath $revisionFile -Raw), '(?m)^Commit:\s*([0-9a-f]{40})\s*$')
$dolphinRevision = if ($revisionMatch.Success) { $revisionMatch.Groups[1].Value } else { $null }
if (-not $dolphinRevision) { throw "Could not identify the pinned Dolphin source revision: $dolphinSource" }
if (-not (Test-Path $gameSource -PathType Leaf)) {
    throw "The configured European Tenkaichi 3 RVZ is missing: $gameSource"
}
& $cmake.Source -S $dolphinSource -B $dolphinBuild -G 'Visual Studio 18 2026' -A x64 -T v145 `
    -DCMAKE_BUILD_TYPE=Release `
    -DENABLE_QT=OFF -DENABLE_NOGUI=ON -DENABLE_HEADLESS=OFF -DWII_OS_BUILD_RUST_FFI=ON `
    -DENABLE_TESTS=OFF -DENABLE_ANALYTICS=OFF -DENABLE_AUTOUPDATE=OFF `
    -DENABLE_VULKAN=OFF -DENABLE_LLVM=OFF -DENABLE_CLI_TOOL=OFF `
    -DUSE_UPNP=OFF -DUSE_DISCORD_PRESENCE=OFF -DUSE_MGBA=OFF `
    -DUSE_RETRO_ACHIEVEMENTS=OFF -DUSE_SYSTEM_LIBS=OFF -DUSE_SANITIZERS=OFF
if ($LASTEXITCODE -ne 0) { throw "Dolphin CMake configure failed with exit code $LASTEXITCODE" }
& $cmake.Source --build $dolphinBuild --target dolphin-nogui wii_dolphin_ffi --config Release --parallel 12
if ($LASTEXITCODE -ne 0) { throw "DolphinNoGUI / Wii OS FFI build failed with exit code $LASTEXITCODE" }
if (-not (Test-Path $dolphinExe -PathType Leaf)) { throw "DolphinNoGUI was not produced: $dolphinExe" }
if (-not (Test-Path $dolphinFfiDll -PathType Leaf)) { throw "The in-process Dolphin FFI DLL was not produced: $dolphinFfiDll" }

$previousTargetDirectory = $env:CARGO_TARGET_DIR
$previousFfiLibraryDirectory = $env:WII_OS_DOLPHIN_FFI_LIB_DIR
$env:CARGO_TARGET_DIR = $targetDirectory
$env:WII_OS_DOLPHIN_FFI_LIB_DIR = Join-Path $dolphinBuild 'Binaries'
try {
    & $cargo.Source build --manifest-path (Join-Path $repoRoot 'apps/menu-native/Cargo.toml') --locked --release --features embedded-dolphin
    if ($LASTEXITCODE -ne 0) { throw "Native menu build failed with exit code $LASTEXITCODE" }
    & $cargo.Source build --manifest-path $manifest --locked --release
    if ($LASTEXITCODE -ne 0) { throw "Rust server build failed with exit code $LASTEXITCODE" }
}
finally {
    if ($null -eq $previousFfiLibraryDirectory) { Remove-Item Env:WII_OS_DOLPHIN_FFI_LIB_DIR -ErrorAction SilentlyContinue }
    else { $env:WII_OS_DOLPHIN_FFI_LIB_DIR = $previousFfiLibraryDirectory }
    if ($null -eq $previousTargetDirectory) { Remove-Item Env:CARGO_TARGET_DIR -ErrorAction SilentlyContinue }
    else { $env:CARGO_TARGET_DIR = $previousTargetDirectory }
}

if (-not (Test-Path $server -PathType Leaf)) { throw "Server binary was not produced: $server" }
if (-not (Test-Path $menu -PathType Leaf)) { throw "Native menu binary was not produced: $menu" }
$null = New-Item -ItemType Directory -Path (Join-Path $stage 'wii-os/menu') -Force
$null = New-Item -ItemType Directory -Path (Join-Path $stage 'wii-os/frontend/assets/images') -Force
$null = New-Item -ItemType Directory -Path (Join-Path $stage 'dolphin/Sys') -Force
$null = New-Item -ItemType Directory -Path (Join-Path $stage 'dolphin/licenses') -Force
$null = New-Item -ItemType Directory -Path (Join-Path $stage 'data/games') -Force
$null = New-Item -ItemType Directory -Path (Join-Path $stage 'data/wads') -Force
$null = New-Item -ItemType Directory -Path (Join-Path $stage 'data/brew/homebrew-browser') -Force
$null = New-Item -ItemType Directory -Path (Join-Path $stage 'data/dolphin-profile') -Force
$null = New-Item -ItemType Directory -Path (Join-Path $stage 'data/dolphin-profile/Config') -Force
$null = New-Item -ItemType Directory -Path (Join-Path $stage 'wii-os/frontend/assets/images') -Force
$null = New-Item -ItemType Directory -Path (Join-Path $stage 'wii-os/userland/data/wii/assets/channels/homebrew-browser') -Force
foreach ($assetName in $nativeMenuAssets) {
    $assetPath = Join-Path $repoRoot "frontend/assets/images/$assetName"
    if (-not (Test-Path -LiteralPath $assetPath -PathType Leaf)) {
        throw "Required native menu asset is missing: $assetPath"
    }
}
Copy-Item -LiteralPath $server -Destination (Join-Path $stage 'wii-os-server.exe') -Force
Copy-Item -LiteralPath $menu -Destination (Join-Path $stage 'wii-menu-native.exe') -Force
$null = Copy-Item -LiteralPath $dolphinExe -Destination (Join-Path $stage 'dolphin/DolphinNoGUI.exe') -PassThru
Copy-Item -LiteralPath $dolphinFfiDll -Destination (Join-Path $stage 'wii_dolphin_ffi.dll') -Force
Copy-Item -Path (Join-Path $dolphinSource 'Data/Sys/*') -Destination (Join-Path $stage 'dolphin/Sys') -Recurse -Force
Copy-Item -LiteralPath (Join-Path $dolphinSource 'COPYING') -Destination (Join-Path $stage 'dolphin/COPYING') -Force
Copy-Item -LiteralPath (Join-Path $dolphinSource 'LICENSES') -Destination (Join-Path $stage 'dolphin/licenses/LICENSES') -Recurse -Force
Set-Content -LiteralPath (Join-Path $stage 'dolphin/portable.txt') -Value '' -Encoding ascii
Set-Content -LiteralPath (Join-Path $stage 'dolphin/SOURCE-REVISION.txt') -Value "Source: $($dolphinSourceRecord.name)`nCommit: $dolphinRevision`nBuild: Release, Windows x64, DolphinNoGUI, Qt disabled" -Encoding utf8
Copy-Item -Path (Join-Path $repoRoot 'frontend/assets/images/*') -Destination (Join-Path $stage 'wii-os/frontend/assets/images') -Recurse -Force
Copy-Item -LiteralPath $gameSource -Destination (Join-Path $stage "data/games/$gameFileName") -Force
foreach ($wadName in $selectedWads.Keys) {
    $wadSource = Join-Path (Join-Path $wiiDataRoot 'wads') $selectedWads[$wadName]
    if (-not (Test-Path -LiteralPath $wadSource -PathType Leaf)) {
        throw "Required European Wii WAD is missing: $wadSource"
    }
    Copy-Item -LiteralPath $wadSource -Destination (Join-Path $stage "data/wads/$wadName") -Force
}
foreach ($assetName in @('channel-wiilogo.png', 'miichannel.jpg')) {
    $assetSource = Join-Path $repoRoot "frontend/assets/images/$assetName"
    if (-not (Test-Path -LiteralPath $assetSource -PathType Leaf)) { throw "Channel artwork is missing: $assetSource" }
    Copy-Item -LiteralPath $assetSource -Destination (Join-Path $stage "wii-os/frontend/assets/images/$assetName") -Force
}
Copy-Item -LiteralPath (Join-Path $wiiDataRoot 'brew/apps/homebrew-browser/boot.dol') -Destination (Join-Path $stage 'data/brew/homebrew-browser/boot.dol') -Force
Copy-Item -LiteralPath (Join-Path $wiiDataRoot 'assets/channels/homebrew-browser/icon.png') -Destination (Join-Path $stage 'wii-os/userland/data/wii/assets/channels/homebrew-browser/icon.png') -Force
if (-not (Test-Path -LiteralPath (Join-Path $profileSource 'Wii/title/00000001/00000002/content/title.tmd') -PathType Leaf)) {
    throw 'The source NAND does not contain the European Wii System Menu title 0000000100000002'
}
Copy-Item -LiteralPath (Join-Path $profileSource 'Wii') -Destination (Join-Path $stage 'data/dolphin-profile/Wii') -Recurse -Force
$catalog = Get-Content -LiteralPath (Join-Path $productRoot 'userland/data/wii/menu.catalog.json') -Raw | ConvertFrom-Json
foreach ($channel in $catalog.channels) {
    switch ($channel.appId) {
        'system-menu' {
            $channel.contentPath = 'data/wads/SystemMenu-v514.wad'
            $channel.icon = 'assets/images/channel-wiilogo.png'
            foreach ($requiredFile in $channel.requiredFiles) {
                if ($requiredFile.role -eq 'system-ios') { $requiredFile.contentPath = 'data/wads/IOS80-64-v6944.wad' }
            }
        }
        'system-menu-nand' { $channel.icon = 'assets/images/channel-wiilogo.png' }
        'mii-channel' { $channel.contentPath = 'data/wads/Mii Channel v6 HACA.wad'; $channel.icon = 'assets/images/miichannel.jpg' }
        'shop-channel' { $channel.contentPath = 'data/wads/Shop Channel v17 HABA.wad'; $channel.icon = $null }
        'homebrew-browser' { $channel.contentPath = 'data/brew/homebrew-browser/boot.dol'; $channel.icon = 'assets/user/channels/homebrew-browser/icon.png' }
        'budokai-tenkaichi-3' { $channel.contentPath = "data/games/$gameFileName"; $channel.icon = 'assets/images/channel-icon.png' }
    }
}
$catalog | ConvertTo-Json -Depth 64 | Set-Content -LiteralPath (Join-Path $stage 'wii-os/menu/catalog.json') -Encoding utf8

$launcher = @'
$ErrorActionPreference = 'Stop'
$packageRoot = $PSScriptRoot
$env:WII_OS_BIND = if ($env:WII_OS_BIND) { $env:WII_OS_BIND } else { '127.0.0.1:8780' }
$env:WII_OS_WORKSPACE = if ($env:WII_OS_WORKSPACE) { $env:WII_OS_WORKSPACE } else { $packageRoot }
$env:WII_OS_DOLPHIN_VIDEO_BACKEND = if ($env:WII_OS_DOLPHIN_VIDEO_BACKEND) { $env:WII_OS_DOLPHIN_VIDEO_BACKEND } else { 'D3D12' }
$env:WII_OS_DATABASE = if ($env:WII_OS_DATABASE) { $env:WII_OS_DATABASE } else { Join-Path $packageRoot 'data/wii-os.sqlite3' }
$env:WII_OS_DOLPHIN_USER_DIR = if ($env:WII_OS_DOLPHIN_USER_DIR) { $env:WII_OS_DOLPHIN_USER_DIR } else { Join-Path $packageRoot 'data/dolphin-profile' }
$env:WII_OS_MENU_CATALOG = if ($env:WII_OS_MENU_CATALOG) { $env:WII_OS_MENU_CATALOG } else { Join-Path $packageRoot 'wii-os/menu/catalog.json' }
$bundledDolphin = Join-Path $packageRoot 'dolphin/DolphinNoGUI.exe'
$env:WII_OS_DOLPHIN_EXE = if ($env:WII_OS_DOLPHIN_EXE) { $env:WII_OS_DOLPHIN_EXE } elseif (Test-Path $bundledDolphin) { $bundledDolphin } else { $null }
$noQtDolphin = Get-Command DolphinNoGUI.exe -ErrorAction SilentlyContinue | Select-Object -First 1
if (-not $env:WII_OS_DOLPHIN_EXE -and $noQtDolphin) { $env:WII_OS_DOLPHIN_EXE = $noQtDolphin.Source }
Push-Location $packageRoot
try {
    & (Join-Path $packageRoot 'wii-os-server.exe')
    exit $LASTEXITCODE
}
finally {
    Pop-Location
}
'@
Set-Content -LiteralPath (Join-Path $stage 'start-wii-os.ps1') -Value $launcher -Encoding utf8

$menuLauncher = @'
$ErrorActionPreference = 'Stop'
$packageRoot = $PSScriptRoot
$env:WII_OS_WORKSPACE = $packageRoot
$env:WII_OS_DOLPHIN_VIDEO_BACKEND = if ($env:WII_OS_DOLPHIN_VIDEO_BACKEND) { $env:WII_OS_DOLPHIN_VIDEO_BACKEND } else { 'D3D12' }
$env:WII_OS_MENU_CATALOG = if ($env:WII_OS_MENU_CATALOG) { $env:WII_OS_MENU_CATALOG } else { Join-Path $packageRoot 'wii-os/menu/catalog.json' }
$env:WII_OS_DOLPHIN_USER_DIR = if ($env:WII_OS_DOLPHIN_USER_DIR) { $env:WII_OS_DOLPHIN_USER_DIR } else { Join-Path $packageRoot 'data/dolphin-profile' }
$bundledDolphin = Join-Path $packageRoot 'dolphin/DolphinNoGUI.exe'
$env:WII_OS_DOLPHIN_EXE = if ($env:WII_OS_DOLPHIN_EXE) { $env:WII_OS_DOLPHIN_EXE } elseif (Test-Path $bundledDolphin) { $bundledDolphin } else { $null }
$noQtDolphin = Get-Command DolphinNoGUI.exe -ErrorAction SilentlyContinue | Select-Object -First 1
if (-not $env:WII_OS_DOLPHIN_EXE -and $noQtDolphin) { $env:WII_OS_DOLPHIN_EXE = $noQtDolphin.Source }
Push-Location $packageRoot
try {
    & (Join-Path $packageRoot 'wii-menu-native.exe')
    exit $LASTEXITCODE
}
finally {
    Pop-Location
}
'@
Set-Content -LiteralPath (Join-Path $stage 'start-wii-menu.ps1') -Value $menuLauncher -Encoding utf8

$readme = @'
# Wii OS portable menu and control plane (Windows x64)

Run `start-wii-menu.ps1` to open the native menu or `start-wii-os.ps1` to start
the HTTP, WebSocket, and MCP service on `127.0.0.1:8780`. MCP Streamable HTTP
is available at `POST /mcp` in the same process and uses the same API token.
Open `http://127.0.0.1:8780/` for the embedded Wii OS Web interface. It is
served by the same executable, works without a separate web folder, and can be
installed as a PWA. The offline cache contains only the interface shell; games,
settings, room state, and Dolphin still require this server and local files.
The service creates `data/wii-os.sqlite3` next
to this package. Room definitions and the Dolphin video-backend preference are
stored durably in SQLite; `GET`/`PUT /v1/settings` reads and writes the latter.
The web app exposes app listing and launch, video-backend settings, and
persistent multiplayer room listing and creation. Set `WII_OS_API_TOKEN` to a
random secret of at least 32 URL-safe characters before binding beyond
loopback, and set `WII_OS_COOKIE_SECURE=1` when serving through HTTPS. The app
uses an HTTP-only cookie and does not store the secret in browser storage.
Live WebSocket presence resets when the process exits. The isolated Dolphin profile and its
NAND/save data use `data/dolphin-profile` unless `WII_OS_DOLPHIN_USER_DIR`
points elsewhere.

The native menu starts at 90% of the primary monitor's pixel dimensions,
keeps its scene in 16:9, and detects controllers as they connect. Use the
controller D-pad, South, East, Start, Select, and Mode buttons for navigation,
back, start, settings, and fullscreen. `WII_OS_FULLSCREEN=1` starts borderless
fullscreen; F11 toggles it at runtime.

The package bundles a source-built Qt-free Dolphin runtime. Wii OS starts the emulator through
the in-process `wii_dolphin_ffi.dll` C ABI and creates its frameless D3D12 render child directly
under the Wii OS window. The standalone `DolphinNoGUI.exe` remains available to the local
HTTP/MCP service. The package also includes Dolphin `Sys` data and license notices, plus the
local European Tenkaichi 3 RVZ configured in
the menu. The RVZ is stored without recompression in the archive, and the app
warms the full image into the operating-system file cache on startup in the
background. Four catalogued European WADs (System Menu 4.3E, IOS80 v6944,
Mii v6, and Shop v17), the matching installed Wii NAND subtree, and Homebrew
Browser boot file are included. The portable profile starts with a fresh
Dolphin Config folder while retaining the packaged NAND titles and saves.
Dolphin runs the Wii game through its Direct3D 12 backend with 3x internal
resolution and 16x anisotropic filtering. HD texture replacement loading is
enabled for user-supplied packs under `data/dolphin-profile/Load/Textures`; no game
textures are bundled or generated. This is an upscaled Wii emulation session,
not a recompiled PC port of the Wii game.

The archive is one Wii OS product containing the native menu, MCP control
service, the in-process Dolphin FFI runtime, the standalone no-Qt Dolphin host
for service launches, and the configured local Wii data. The Windows menu owns
the visible application window and embeds Dolphin's frameless render child.
This package is compressed with 7-Zip LZMA2. Linux and Wii PowerPC require
separate target builds; a Windows executable cannot run on those ABIs.
'@
Set-Content -LiteralPath (Join-Path $stage 'README.md') -Value $readme -Encoding utf8

if (Test-Path $archive) { Remove-Item -LiteralPath $archive -Force }
Push-Location $outputDirectory
try {
    Push-Location $stagingRoot
    try {
        & $SevenZip a -t7z -m0=lzma2 -mx=9 -mmt=on -ms=on -md=128m $archive 'wii-os-windows-x64\*' '-x!wii-os-windows-x64\data\games\*.rvz' | Out-Host
        if ($LASTEXITCODE -ne 0) { throw "7-Zip executable compression failed with exit code $LASTEXITCODE" }
        & $SevenZip a -t7z -m0=Copy -ms=off $archive "wii-os-windows-x64\data\games\$gameFileName" | Out-Host
    }
    finally {
        Pop-Location
    }
    if ($LASTEXITCODE -ne 0) { throw "7-Zip archive creation failed with exit code $LASTEXITCODE" }
    & $SevenZip t $archive | Out-Host
    if ($LASTEXITCODE -ne 0) { throw "7-Zip archive verification failed with exit code $LASTEXITCODE" }
}
finally {
    Pop-Location
    if (Test-Path $stagingRoot) { Remove-Item -LiteralPath $stagingRoot -Recurse -Force }
}

$hash = (Get-FileHash -LiteralPath $archive -Algorithm SHA256).Hash.ToLowerInvariant()
Set-Content -LiteralPath ($archive + '.sha256') -Value "$hash  $(Split-Path -Leaf $archive)" -Encoding ascii
Write-Output "Package: $archive"
Write-Output "SHA256:  $hash"
