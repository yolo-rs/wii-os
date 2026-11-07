[CmdletBinding()]
param(
    [switch] $LaunchDolphin,
    [switch] $BootSystemMenu,
    [switch] $LaunchHomebrewBrowser,
    [switch] $StartWiiOsService
)

$ErrorActionPreference = 'Stop'
$workspace = [IO.Path]::GetFullPath($PSScriptRoot)
$dataRoot = Join-Path $workspace 'userland/data/wii'
$profile = Join-Path $workspace 'userland/data/dolphin-wii-eu'
$catalog = Join-Path $dataRoot 'menu.catalog.json'
$dolphin = if ($env:WII_OS_DOLPHIN_EXE -and (Test-Path -LiteralPath $env:WII_OS_DOLPHIN_EXE -PathType Leaf)) {
    $env:WII_OS_DOLPHIN_EXE
} elseif (Test-Path -LiteralPath 'C:\Program Files\Dolphin\Dolphin.exe') {
    'C:\Program Files\Dolphin\Dolphin.exe'
} else {
    (Get-Command Dolphin.exe -ErrorAction SilentlyContinue | Select-Object -First 1).Source
}

if (-not $dolphin) { throw 'Dolphin.exe is unavailable; install Dolphin or set WII_OS_DOLPHIN_EXE.' }
if (-not (Test-Path -LiteralPath $catalog -PathType Leaf)) { throw "European Wii catalog not found: $catalog" }
$requiredFiles = @(
    'wads/System Software/SystemMenu-v514.wad',
    'wads/System Software/IOS80-64-v6944.wad',
    'wads/System Software/IOS58-64-v6176.wad',
    'wads/Official Channels/Mii Channel v6 HACA.wad',
    'wads/Official Channels/Shop Channel v17 HABA.wad',
    'brew/apps/homebrew-browser/boot.dol'
)
foreach ($relative in $requiredFiles) {
    $path = Join-Path $dataRoot $relative
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) { throw "Required local Wii content is missing: $path" }
}

$env:WII_OS_MENU_CATALOG = $catalog
$env:WII_OS_DOLPHIN_EXE = $dolphin
$env:WII_OS_DOLPHIN_USER_DIR = $profile
$env:WII_OS_SYSTEM_MENU_WAD = Join-Path $dataRoot 'wads/System Software/SystemMenu-v514.wad'
$env:WII_OS_MII_CHANNEL_WAD = Join-Path $dataRoot 'wads/Official Channels/Mii Channel v6 HACA.wad'
$env:WII_OS_SHOP_CHANNEL_WAD = Join-Path $dataRoot 'wads/Official Channels/Shop Channel v17 HABA.wad'
$cleanGraphicsArguments = @(
    '--config', 'GFX.Settings.DumpTextures=False',
    '--config', 'GFX.Settings.EnableMods=False',
    '--config', 'GFX.Settings.EnableValidationLayer=False',
    '--config', 'GFX.Settings.WireFrame=False',
    '--config', 'GFX.Settings.ShowFPS=False',
    '--config', 'GFX.Settings.ShowFTimes=False',
    '--config', 'GFX.Settings.ShowVPS=False',
    '--config', 'GFX.Settings.ShowVTimes=False',
    '--config', 'GFX.Settings.ShowGraphs=False',
    '--config', 'GFX.Settings.ShowSpeed=False',
    '--config', 'GFX.Settings.LogRenderTimeToFile=False',
    '--config', 'GFX.Settings.OverlayStats=False',
    '--config', 'GFX.Settings.OverlayProjStats=False',
    '--config', 'GFX.Settings.OverlayScissorStats=False',
    '--config', 'GFX.Settings.ShowInternalResolution=False',
    '--config', 'GFX.Settings.TexFmtOverlayEnable=False',
    '--config', 'GFX.Settings.DumpEFBTarget=False',
    '--config', 'GFX.Settings.DumpXFBTarget=False',
    '--config', 'GFX.Settings.DumpFramesAsImages=False',
    '--config', 'GFX.Enhancements.PostProcessingShader='
)

Write-Output "Dolphin: $dolphin"
Write-Output "Region set: PAL / Europe, Wii Menu 4.3E v514"
Write-Output "IOS80: $($requiredFiles[1])"
Write-Output "Catalog: $catalog"
Write-Output "Isolated Dolphin NAND/profile: $profile"
Write-Output 'Local European WADs remain in the ignored userland data directory; no WAD/NAND data is copied into releases.'

if ($BootSystemMenu -and $LaunchHomebrewBrowser) {
    throw 'Choose either -BootSystemMenu or -LaunchHomebrewBrowser, not both.'
}
$launchModes = @($BootSystemMenu.IsPresent, $LaunchHomebrewBrowser.IsPresent, $LaunchDolphin.IsPresent, $StartWiiOsService.IsPresent)
if (@($launchModes | Where-Object { $_ }).Count -gt 1) {
    throw 'Choose only one launch mode.'
}

if ($BootSystemMenu -or $StartWiiOsService) {
    foreach ($relative in @(
        'Wii/title/00000001/00000002/content/title.tmd',
        'Wii/title/00000001/00000050/content/title.tmd'
    )) {
        $installedTitle = Join-Path $profile $relative
        if (-not (Test-Path -LiteralPath $installedTitle -PathType Leaf)) {
            throw "The European NAND profile is missing a required installed title: $installedTitle"
        }
    }
}

if ($BootSystemMenu) {
    New-Item -ItemType Directory -Path $profile -Force | Out-Null
    Start-Process -FilePath $dolphin -ArgumentList (@('--user', $profile, '--nand_title', '0000000100000002', '--video_backend', 'D3D12') + $cleanGraphicsArguments) -WorkingDirectory (Split-Path -Parent $dolphin)
} elseif ($LaunchHomebrewBrowser) {
    New-Item -ItemType Directory -Path $profile -Force | Out-Null
    Start-Process -FilePath $dolphin -ArgumentList (@('--user', $profile, '--exec', (Join-Path $dataRoot 'brew/apps/homebrew-browser/boot.dol'), '--video_backend', 'D3D12') + $cleanGraphicsArguments) -WorkingDirectory (Split-Path -Parent $dolphin)
} elseif ($LaunchDolphin) {
    New-Item -ItemType Directory -Path $profile -Force | Out-Null
    Start-Process -FilePath $dolphin -ArgumentList @('--user', $profile) -WorkingDirectory (Split-Path -Parent $dolphin)
} elseif ($StartWiiOsService) {
    $server = Join-Path $workspace 'userland/build/targets/wii-os-server/release/wii-os-server.exe'
    if (-not (Test-Path -LiteralPath $server -PathType Leaf)) {
        throw "Wii OS server binary is missing: $server. Build it with .\wii-os\pipeline.ps1 -Profile server."
    }
    if (-not $env:WII_OS_BIND) { $env:WII_OS_BIND = '127.0.0.1:8780' }
    if ($env:WII_OS_BIND -notmatch '^(127\.0\.0\.1|\[::1\]):') {
        if (-not $env:WII_OS_API_TOKEN -or $env:WII_OS_API_TOKEN.Length -lt 32) {
            throw 'A non-loopback WII_OS_BIND requires WII_OS_API_TOKEN with at least 32 characters.'
        }
    }
    $env:WII_OS_ROOM_STORE = Join-Path $workspace 'userland/data/rooms.json'
    Write-Output 'Starting local Wii OS API with private European WADs and the isolated user NAND profile.'
    Write-Output 'The WAD/NAND files remain in userland and are not copied into the executable or web site.'
    Push-Location $workspace
    try { & $server } finally { Pop-Location }
}
