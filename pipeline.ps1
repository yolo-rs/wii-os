[CmdletBinding()]
param(
    [ValidateSet('audit', 'build', 'inspect', 'validate', 'all', 'wasm', 'cli', 'server', 'menu-core', 'menu-native', 'c-inventory')]
    [string] $Profile = 'audit',
    [ValidateSet('host', 'windows-x64', 'linux-x64', 'web-wasm32-webgpu', 'wii-powerpc')]
    [string] $Target = 'host',
    [string] $Workspace = $PSScriptRoot,
    [string] $Manifest = (Join-Path $PSScriptRoot 'inputs.example.json'),
    [string] $Output = (Join-Path $PSScriptRoot 'userland/build/reports'),
    [string] $DtkPath,
    [string] $GeckoExecutable,
    [string] $DolphinPath,
    [string] $IplWadPath,
    [ValidateSet('43U', '43E', '43J', '43K')]
    [string] $IplVersion = '43U'
)

$ErrorActionPreference = 'Stop'
$Output = [IO.Path]::GetFullPath($Output)
$Workspace = [IO.Path]::GetFullPath($Workspace)
$catalogPath = Join-Path $PSScriptRoot 'pipeline.catalog.json'
$catalog = Get-Content $catalogPath -Raw | ConvertFrom-Json
    if ($catalog.schemaVersion -ne 2 -or $catalog.repositories.Count -ne 18 -or @($catalog.platforms).Count -lt 3) { throw "Invalid or incomplete pipeline catalog: $catalogPath" }
$dependencyPath = Join-Path $PSScriptRoot 'dependencies.json'
$dependencyCatalog = Get-Content $dependencyPath -Raw | ConvertFrom-Json
if ($dependencyCatalog.schemaVersion -ne 2 -or $dependencyCatalog.repositories.Count -ne 18 -or @($dependencyCatalog.platforms).Count -lt 3) { throw "Invalid or incomplete dependency inventory: $dependencyPath" }
$repoNames = @($catalog.repositories | ForEach-Object name)
$sourceRootByRepository = @{}
foreach ($repositoryEntry in $catalog.repositories) {
    if ($repositoryEntry.sourceRoot) { $sourceRootByRepository[$repositoryEntry.name] = $repositoryEntry.sourceRoot }
}
$sourceRootsCatalog = Get-Content -LiteralPath (Join-Path $PSScriptRoot 'source-roots.json') -Raw | ConvertFrom-Json
$canonicalSourceRoots = @{}
foreach ($sourceRoot in $sourceRootsCatalog.repositories) {
    $canonicalSourceRoots[$sourceRoot.name] = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot $sourceRoot.path))
}
$dependencyByName = @{}
foreach ($dependencyEntry in $dependencyCatalog.repositories) { $dependencyByName[$dependencyEntry.name] = $dependencyEntry }
if (@($repoNames | Where-Object { -not $dependencyByName.ContainsKey($_) }).Count -gt 0) { throw 'Dependency inventory does not cover every pipeline repository' }
$formatCatalogPath = Join-Path $PSScriptRoot 'formats.catalog.json'
$formatCatalog = Get-Content $formatCatalogPath -Raw | ConvertFrom-Json
if ($formatCatalog.schemaVersion -ne 1 -or $formatCatalog.formats.Count -lt 4) { throw "Invalid or incomplete console format catalog: $formatCatalogPath" }
$runId = (Get-Date).ToUniversalTime().ToString('yyyyMMddTHHmmssZ')
$startedAtUtc = (Get-Date).ToUniversalTime().ToString('o')
$runDir = Join-Path $Output $runId
$null = New-Item -ItemType Directory -Path $runDir -Force
$steps = [System.Collections.Generic.List[object]]::new()
$menuWebPackage = $null

function Add-Step([string] $Name, [string] $Status, [string] $Detail, [object[]] $Artifacts = @()) {
    $steps.Add([pscustomobject]@{
        name = $Name
        status = $Status
        detail = $Detail
        artifacts = @($Artifacts)
    })
}

function Invoke-Logged([string] $Name, [string] $File, [string[]] $Arguments, [string] $WorkingDirectory) {
    $safeName = $Name -replace '[^a-zA-Z0-9_-]', '_'
    $log = Join-Path $runDir ($safeName + '.log')
    Push-Location $WorkingDirectory
    try {
        & $File @Arguments *>&1 | Tee-Object -FilePath $log | Out-Host
        $exitCode = $LASTEXITCODE
    }
    finally {
        Pop-Location
    }
    if ($exitCode -ne 0) { throw "$Name exited with code $exitCode (see $log)" }
    return $log
}

function Find-CommandPath([string] $Name) {
    $command = Get-Command $Name -ErrorAction SilentlyContinue | Select-Object -First 1
    if ($command) { return $command.Source }
    return $null
}

function Get-RustupInstalledTargets([string] $RustupPath) {
    if (-not $RustupPath) { return @() }
    return @(& $RustupPath target list --installed 2>$null)
}

$repoRecords = foreach ($name in $repoNames) {
    $sourceRootName = $sourceRootByRepository[$name]
    $path = if ($sourceRootName -and $canonicalSourceRoots.ContainsKey($sourceRootName)) {
        $canonicalSourceRoots[$sourceRootName]
    } else { Join-Path $Workspace $name }
    $dependencyEntry = $dependencyByName[$name]
    $revisionFile = if ($sourceRootName -and $canonicalSourceRoots.ContainsKey($sourceRootName)) {
        $sourceRootEntry = @($sourceRootsCatalog.repositories | Where-Object name -eq $sourceRootName | Select-Object -First 1)
        if ($sourceRootEntry.Count -gt 0 -and $sourceRootEntry[0].revisionFile) { Join-Path $path $sourceRootEntry[0].revisionFile }
    }
    if (-not (Test-Path (Join-Path $path '.git'))) {
        if ($revisionFile -and (Test-Path -LiteralPath $revisionFile -PathType Leaf)) {
            $revisionText = Get-Content -LiteralPath $revisionFile -Raw
            $head = if ($revisionText -match '(?m)^Commit:\s*([0-9a-f]{40})\s*$') { $Matches[1] } else { $null }
            $state = if ($head) { 'snapshot' } else { 'invalid-snapshot' }
            [pscustomobject]@{ name = $name; path = $path; state = $state; branch = $null; head = $head; dirty = $null; manifests = @($dependencyEntry.manifests); hostDependencies = @($dependencyEntry.hostDependencies); nativeDependencies = @($dependencyEntry.nativeDependencies); setup = $dependencyEntry.setup; build = $dependencyEntry.build; artifacts = @($dependencyEntry.artifacts); scopes = @($dependencyEntry.scope); gate = $dependencyEntry.gate }
            continue
        }
        [pscustomobject]@{ name = $name; path = $path; state = 'missing'; branch = $null; head = $null; dirty = $null; manifests = @($dependencyEntry.manifests); hostDependencies = @($dependencyEntry.hostDependencies); nativeDependencies = @($dependencyEntry.nativeDependencies); setup = $dependencyEntry.setup; build = $dependencyEntry.build; artifacts = @($dependencyEntry.artifacts); scopes = @($dependencyEntry.scope); gate = $dependencyEntry.gate }
        continue
    }
    $branch = (& git -C $path branch --show-current 2>$null | Select-Object -First 1)
    $head = (& git -C $path rev-parse HEAD 2>$null | Select-Object -First 1)
    $status = @(& git -C $path status --porcelain 2>$null)
    [pscustomobject]@{ name = $name; path = $path; state = 'present'; branch = $branch; head = $head; dirty = ($status.Count -gt 0); manifests = @($dependencyEntry.manifests); hostDependencies = @($dependencyEntry.hostDependencies); nativeDependencies = @($dependencyEntry.nativeDependencies); setup = $dependencyEntry.setup; build = $dependencyEntry.build; artifacts = @($dependencyEntry.artifacts); scopes = @($dependencyEntry.scope); gate = $dependencyEntry.gate }
}
$presentCount = @($repoRecords | Where-Object { $_.state -in @('present', 'snapshot') }).Count
$dirtyCount = @($repoRecords | Where-Object { $_.state -eq 'present' -and $_.dirty }).Count
Add-Step 'repository-audit' 'passed' ("Found {0}/{1} source roots; {2} Git repositories have local changes" -f $presentCount, $repoNames.Count, $dirtyCount)

$dtk = if ($DtkPath) { $DtkPath } else { Find-CommandPath 'dtk' }
$dolphin = if ($DolphinPath) { $DolphinPath } else { Find-CommandPath 'Dolphin.exe' }
$cargo = Find-CommandPath 'cargo'
$rustup = Find-CommandPath 'rustup'
$wasmPack = Find-CommandPath 'wasm-pack'
$rustTargets = @(Get-RustupInstalledTargets $rustup)
$go = Find-CommandPath 'go'
$bun = Find-CommandPath 'bun'
$cmake = Find-CommandPath 'cmake'
$ninja = Find-CommandPath 'ninja'
$make = Find-CommandPath 'make'
$uv = Find-CommandPath 'uv'
$rustc = Find-CommandPath 'rustc'
$python = Find-CommandPath 'python'
$vulkanSdk = $env:VULKAN_SDK
$glslc = if ($vulkanSdk) { Join-Path $vulkanSdk 'Bin/glslc.exe' } else { $null }
$glslang = if ($vulkanSdk) { Join-Path $vulkanSdk 'Bin/glslangValidator.exe' } else { $null }
$msvcCompiler = $null
$vswhere = 'C:\Program Files (x86)\Microsoft Visual Studio\Installer\vswhere.exe'
if (Test-Path $vswhere) {
    $msvcCompiler = (& $vswhere -latest -products '*' -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -find 'VC\Tools\MSVC\**\bin\Hostx64\x64\cl.exe' 2>$null | Select-Object -First 1)
}

function Read-Version([string] $File, [string[]] $Arguments = @('--version')) {
    if (-not $File -or -not (Test-Path $File)) { return $null }
    try {
        $versionOutput = @(& $File @Arguments 2>&1 | Select-Object -First 2)
        return (($versionOutput | ForEach-Object { "$_" }) -join ' ').Trim()
    } catch { return "unavailable: $($_.Exception.Message)" }
}
$isWindowsHost = $env:OS -eq 'Windows_NT'
$hostTarget = if ($isWindowsHost) { 'windows-x64' } elseif ([System.Runtime.InteropServices.RuntimeInformation]::OSDescription -match 'Linux') { 'linux-x64' } else { 'other' }
$selectedTarget = if ($Target -eq 'host') { $hostTarget } else { $Target }
$targetCrossBuildSupported = $selectedTarget -eq $hostTarget -or $selectedTarget -in @('web-wasm32-webgpu', 'wii-powerpc')
$pythonPackageTool = if ($uv) { 'uv' } else { $null }

if ($Profile -eq 'c-inventory') {
    $inventoryScript = Join-Path $PSScriptRoot 'inventory-c-sources.ps1'
    try {
        & $inventoryScript -Workspace $Workspace -Catalog $catalogPath -Output $Output | Out-Host
        $inventoryPath = Join-Path $Output 'c-source-inventory/inventory.json'
        $inventory = Get-Content -LiteralPath $inventoryPath -Raw | ConvertFrom-Json
        Add-Step 'c-source-inventory' 'passed' ("Indexed {0} C/C++ translation units and {1} headers across repository checkouts, initialized submodules and the Wii OS coordinator; each record includes worktree hash, source root, adapter and license evidence" -f $inventory.totals.translationUnits, $inventory.totals.headers) @($inventoryPath)
    } catch { Add-Step 'c-source-inventory' 'failed' $_.Exception.Message }
}

if ($Profile -in @('build', 'all', 'wasm', 'cli', 'server', 'menu-core', 'menu-native')) {
    $menuCoreRoot = Join-Path $PSScriptRoot 'crates/menu-core'
    if (Test-Path (Join-Path $menuCoreRoot 'Cargo.toml')) {
        $previousCoreTargetDir = $env:CARGO_TARGET_DIR
        $env:CARGO_TARGET_DIR = Join-Path (Split-Path -Parent $Output) (Join-Path 'targets' 'wii-menu-core')
        if ($Profile -in @('menu-core', 'menu-native') -and $selectedTarget -eq 'linux-x64' -and $isWindowsHost) {
            Add-Step 'menu-core-linux-build' 'skipped' 'Linux builds require a Linux host; Windows does not provision or call a Linux distribution'
        } elseif (-not $cargo) {
            Add-Step 'menu-core-build' 'skipped' 'Cargo is unavailable'
        } elseif ($selectedTarget -eq 'wii-powerpc') {
            Add-Step 'menu-core-build' 'skipped' 'Shared UI model is a host/WASM component, not Wii PowerPC firmware'
        } else {
            try {
                $coreArgs = @('check', '--manifest-path', (Join-Path $menuCoreRoot 'Cargo.toml'), '--locked')
                if ($Profile -eq 'wasm' -or $selectedTarget -eq 'web-wasm32-webgpu') {
                    if ($rustTargets -notcontains 'wasm32-unknown-unknown') {
                        Add-Step 'menu-core-build' 'skipped' 'Install wasm32-unknown-unknown for the shared menu model'
                    } else {
                        $coreArgs += @('--target', 'wasm32-unknown-unknown')
                        $coreLog = Invoke-Logged 'menu-core-build' $cargo $coreArgs $menuCoreRoot
                        Add-Step 'menu-core-build' 'passed' 'Platform-neutral menu model checked for wasm32-unknown-unknown' @($coreLog)
                    }
                } else {
                    $coreArgs = @('build', '--manifest-path', (Join-Path $menuCoreRoot 'Cargo.toml'), '--locked', '--release')
                    $coreLog = Invoke-Logged 'menu-core-build' $cargo $coreArgs $menuCoreRoot
                    $coreRelease = Join-Path $env:CARGO_TARGET_DIR 'release'
                    $coreArtifacts = @(Get-ChildItem -LiteralPath $coreRelease -File | Where-Object { $_.Name -match 'wii_menu_core.*\.(dll|lib|a|so|dylib)$' } | ForEach-Object FullName)
                    $coreHeader = Join-Path $menuCoreRoot 'include/wii_menu_core.h'
                    if ($coreArtifacts.Count -eq 0) { throw "Cargo succeeded but emitted no C ABI library in $coreRelease" }
                    $coreBuildArtifacts = @($coreLog, $coreHeader) + $coreArtifacts
                    if (-not $cmake) { Add-Step 'menu-core-c-host' 'skipped' 'CMake is unavailable; Rust C ABI libraries were built' }
                    else {
                        if ($isWindowsHost) { $coreDll = Join-Path $coreRelease 'wii_menu_core.dll'; $coreImportLib = Join-Path $coreRelease 'wii_menu_core.dll.lib'; $cHostGenerator = @('-G', 'Visual Studio 18 2026', '-A', 'x64') }
                        elseif ($hostTarget -eq 'linux-x64') { $coreDll = Join-Path $coreRelease 'libwii_menu_core.so'; $coreImportLib = $coreDll; $cHostGenerator = @('-G', 'Ninja', '-DCMAKE_BUILD_TYPE=Release') }
                        else { $coreDll = $null; $coreImportLib = $null; $cHostGenerator = @() }
                        if (-not $coreDll -or -not (Test-Path $coreDll) -or -not (Test-Path $coreImportLib)) { Add-Step 'menu-core-c-host' 'skipped' "A C host linker/import library is unavailable for $hostTarget" }
                        else {
                            $cHostBuildDir = Join-Path $runDir 'menu-core-c-host-build'
                            $cHostExecutable = if ($isWindowsHost) { Join-Path $cHostBuildDir 'Release/wii-menu-c-host.exe' } else { Join-Path $cHostBuildDir 'wii-menu-c-host' }
                            $cHostSource = Join-Path $menuCoreRoot 'examples/c'
                            $cHostConfigureArgs = @('-S', $cHostSource, '-B', $cHostBuildDir) + $cHostGenerator + @("-DWII_MENU_CORE_INCLUDE_DIR=$(Join-Path $menuCoreRoot 'include')", "-DWII_MENU_CORE_LIBRARY=$coreImportLib", "-DWII_MENU_CORE_DLL=$coreDll")
                            $cHostConfigureLog = Invoke-Logged 'menu-core-c-host-configure' $cmake $cHostConfigureArgs $runDir
                            if ($isWindowsHost) { $cHostBuildLog = Invoke-Logged 'menu-core-c-host-build' $cmake @('--build', $cHostBuildDir, '--config', 'Release', '--parallel') $runDir }
                            else { $cHostBuildLog = Invoke-Logged 'menu-core-c-host-build' $cmake @('--build', $cHostBuildDir, '--parallel') $runDir }
                            $cHostCatalog = Join-Path $PSScriptRoot 'menu/catalog.example.json'
                            $cHostRunLog = Invoke-Logged 'menu-core-c-host-run' $cHostExecutable @($cHostCatalog) $runDir
                            $cHostSharedLibrary = Join-Path (Split-Path -Parent $cHostExecutable) (Split-Path -Leaf $coreDll)
                            Add-Step 'menu-core-c-host' 'passed' 'C example compiled, linked to the shared ABI and ran against the schema-v1 menu catalog' @($cHostConfigureLog, $cHostBuildLog, $cHostRunLog, $cHostExecutable, $cHostSharedLibrary)
                        }
                    }
                    Add-Step 'menu-core-build' 'passed' 'Shared Rust menu core and native C ABI libraries built for the active host' $coreBuildArtifacts
                }
            } catch { Add-Step 'menu-core-build' 'failed' $_.Exception.Message }
        }
        if ($null -eq $previousCoreTargetDir) { Remove-Item Env:CARGO_TARGET_DIR -ErrorAction SilentlyContinue }
        else { $env:CARGO_TARGET_DIR = $previousCoreTargetDir }
    }
    if ($Profile -eq 'menu-native') {
        if ($selectedTarget -notin @('windows-x64', 'linux-x64')) { Add-Step 'menu-native-build' 'skipped' "The native renderer targets Windows/Linux, not $selectedTarget" }
        elseif ($selectedTarget -eq 'linux-x64' -and $isWindowsHost) {
            Add-Step 'menu-native-build' 'skipped' 'Linux builds require a Linux host'
        } elseif (-not $cargo) { Add-Step 'menu-native-build' 'skipped' 'Cargo is unavailable' }
        else {
            $previousNativeTargetDir = $env:CARGO_TARGET_DIR
            $env:CARGO_TARGET_DIR = Join-Path (Split-Path -Parent $Output) (Join-Path 'targets' 'wii-menu-native')
            try {
                $nativeManifest = Join-Path $PSScriptRoot 'apps/menu-native/Cargo.toml'
                $nativeLog = Invoke-Logged 'menu-native-build' $cargo @('build', '--manifest-path', $nativeManifest, '--locked', '--release') (Join-Path $PSScriptRoot 'apps/menu-native')
                $nativeBinaryName = if ($isWindowsHost) { 'wii-menu-native.exe' } else { 'wii-menu-native' }
                $nativeBinary = Join-Path $env:CARGO_TARGET_DIR (Join-Path 'release' $nativeBinaryName)
                Add-Step 'menu-native-build' 'passed' "Shared native menu renderer built for $selectedTarget with explicit host GPU backend selection" @($nativeLog, $nativeBinary)
                if ($selectedTarget -eq 'linux-x64') {
                    $pkgConfig = Find-CommandPath 'pkg-config'
                    if (-not $pkgConfig) { Add-Step 'menu-gtk-linux-build' 'skipped' 'pkg-config is unavailable; GTK4 system dependencies cannot be resolved' }
                    else {
                        try {
                            $gtkProbeLog = Invoke-Logged 'menu-gtk-pkg-config' $pkgConfig @('--atleast-version=4.10', 'gtk4') $PSScriptRoot
                            $gtkManifest = Join-Path $PSScriptRoot 'Cargo.toml'
                            $gtkLog = Invoke-Logged 'menu-gtk-linux-build' $cargo @('build', '--manifest-path', $gtkManifest, '--package', 'wii-menu-gtk', '--features', 'gtk-host', '--locked', '--release') $PSScriptRoot
                            $gtkBinary = Join-Path $env:CARGO_TARGET_DIR 'release/wii-menu-gtk'
                            Add-Step 'menu-gtk-linux-build' 'passed' 'GTK4 Linux menu host built from the shared menu core and Dolphin launch runtime' @($gtkProbeLog, $gtkLog, $gtkBinary)
                        } catch { Add-Step 'menu-gtk-linux-build' 'failed' $_.Exception.Message }
                    }
                }
            } catch { Add-Step 'menu-native-build' 'failed' $_.Exception.Message }
            finally {
                if ($null -eq $previousNativeTargetDir) { Remove-Item Env:CARGO_TARGET_DIR -ErrorAction SilentlyContinue }
                else { $env:CARGO_TARGET_DIR = $previousNativeTargetDir }
            }
        }
    }
    if ($Profile -eq 'cli') {
        if (-not $cargo) { Add-Step 'wii-cli-build' 'skipped' 'Cargo is unavailable' }
        else {
            $cliManifest = Join-Path $PSScriptRoot 'apps/cli/Cargo.toml'
            $previousCargoTargetDir = $env:CARGO_TARGET_DIR
            # Keep self-hosted CLI rebuilds out of the executable path currently running on Windows.
            $env:CARGO_TARGET_DIR = Join-Path (Split-Path -Parent $Output) (Join-Path 'targets' 'wii-cli-bootstrap')
            try {
                $cliLog = Invoke-Logged 'wii-cli-build' $cargo @('build', '--manifest-path', $cliManifest, '--locked', '--release') (Join-Path $PSScriptRoot 'apps/cli')
                $cliExecutableName = if ($isWindowsHost) { 'wii.exe' } else { 'wii' }
                $cliBinary = Join-Path $env:CARGO_TARGET_DIR (Join-Path 'release' $cliExecutableName)
                Add-Step 'wii-cli-build' 'passed' 'Portable wii CLI built for the active host' @($cliLog, $cliBinary)
            } catch { Add-Step 'wii-cli-build' 'failed' $_.Exception.Message }
            finally {
                if ($null -eq $previousCargoTargetDir) { Remove-Item Env:CARGO_TARGET_DIR -ErrorAction SilentlyContinue }
                else { $env:CARGO_TARGET_DIR = $previousCargoTargetDir }
            }
        }
    }
    if ($Profile -eq 'server') {
        if ($selectedTarget -ne $hostTarget) { Add-Step 'wii-os-server-build' 'skipped' "Control-plane server needs a native host; requested $selectedTarget on $hostTarget" }
        elseif (-not $cargo) { Add-Step 'wii-os-server-build' 'skipped' 'Cargo is unavailable' }
        else {
            $serverManifest = Join-Path $PSScriptRoot 'apps/server/Cargo.toml'
            $previousServerTargetDir = $env:CARGO_TARGET_DIR
            $env:CARGO_TARGET_DIR = Join-Path (Split-Path -Parent $Output) (Join-Path 'targets' 'wii-os-server')
            try {
                $serverLog = Invoke-Logged 'wii-os-server-build' $cargo @('build', '--manifest-path', $serverManifest, '--locked', '--release') (Join-Path $PSScriptRoot 'apps/server')
                $serverExecutableName = if ($isWindowsHost) { 'wii-os-server.exe' } else { 'wii-os-server' }
                $serverBinary = Join-Path $env:CARGO_TARGET_DIR (Join-Path 'release' $serverExecutableName)
                Add-Step 'wii-os-server-build' 'passed' 'Wii OS HTTP/WebSocket app hub and lobby built for the active native host' @($serverLog, $serverBinary)
            } catch { Add-Step 'wii-os-server-build' 'failed' $_.Exception.Message }
            finally {
                if ($null -eq $previousServerTargetDir) { Remove-Item Env:CARGO_TARGET_DIR -ErrorAction SilentlyContinue }
                else { $env:CARGO_TARGET_DIR = $previousServerTargetDir }
            }
        }
    }
    if (($Profile -eq 'wasm' -or $selectedTarget -eq 'web-wasm32-webgpu') -and (Test-Path (Join-Path $PSScriptRoot 'crates/menu-web/Cargo.toml'))) {
        if (-not $cargo -or -not $wasmPack -or -not $rustup) { Add-Step 'menu-core-wasm-build' 'skipped' 'Rust, rustup, wasm-pack and wasm32-unknown-unknown target are required' }
        elseif ($rustTargets -notcontains 'wasm32-unknown-unknown') { Add-Step 'menu-core-wasm-build' 'skipped' 'Install wasm32-unknown-unknown for the active Rust toolchain' }
        else {
            $menuWebPackage = Join-Path $runDir 'menu-core-pkg'
            $null = New-Item -ItemType Directory -Path $menuWebPackage -Force
            $menuWebArgs = @('build', '--target', 'web', '--out-dir', $menuWebPackage, '--out-name', 'wii_menu', '--release')
            try {
                $menuWebLog = Invoke-Logged 'menu-core-wasm-build' $wasmPack $menuWebArgs (Join-Path $PSScriptRoot 'crates/menu-web')
                Add-Step 'menu-core-wasm-build' 'passed' 'Shared menu state core compiled to a browser WebAssembly module' @($menuWebLog, $menuWebPackage)
            } catch { Add-Step 'menu-core-wasm-build' 'failed' $_.Exception.Message; $menuWebPackage = $null }
        }
    }
    if ($Profile -in @('build', 'all', 'wasm')) {
    foreach ($entry in $catalog.repositories) {
        if (($Profile -eq 'wasm' -or $selectedTarget -eq 'web-wasm32-webgpu') -and $entry.adapter -ne 'gecko-wasm-pack') { continue }
        if ($Profile -ne 'wasm' -and $entry.adapter -eq 'gecko-wasm-pack' -and $selectedTarget -ne 'web-wasm32-webgpu') { Add-Step 'gecko-web-wasm-build' 'skipped' "Web output is a separate target; requested $selectedTarget"; continue }
        if ($selectedTarget -eq 'wii-powerpc' -and $entry.adapter -ne 'devkitpro-make') { continue }
        if (-not $targetCrossBuildSupported -and $entry.adapter -ne 'go-binary') { Add-Step "$($entry.name)-build" 'skipped' "Target $selectedTarget needs a matching native build host; this runner is $hostTarget"; continue }
        $sourceRootName = $sourceRootByRepository[$entry.name]
        $repoRoot = if ($entry.adapter -in @('gecko-matrix', 'gecko-wasm-pack')) {
            $PSScriptRoot
        } elseif ($sourceRootName -and $canonicalSourceRoots.ContainsKey($sourceRootName)) {
            $canonicalSourceRoots[$sourceRootName]
        } else { Join-Path $Workspace $entry.name }
        $manifestPath = Join-Path $repoRoot 'Cargo.toml'
        $previousCargoTargetDir = $env:CARGO_TARGET_DIR
        $env:CARGO_TARGET_DIR = Join-Path (Split-Path -Parent $Output) (Join-Path 'targets' $entry.name)
        try {
            switch ($entry.adapter) {
                'cargo-binary' {
                    if (-not $cargo) { Add-Step "$($entry.name)-build" 'skipped' 'Cargo is unavailable'; break }
                    $log = Invoke-Logged "$($entry.name)-build" $cargo @('build', '--manifest-path', $manifestPath, '--locked', '--release') $repoRoot
                    Add-Step "$($entry.name)-build" 'passed' 'Rust release build completed' @($log)
                }
                'cargo-workspace-targets' {
                    if (-not $cargo) { Add-Step "$($entry.name)-build" 'skipped' 'Cargo is unavailable'; break }
                    $args = @('build', '--manifest-path', $manifestPath, '--locked') + $entry.command
                    $log = Invoke-Logged "$($entry.name)-build" $cargo $args $repoRoot
                    Add-Step "$($entry.name)-build" 'passed' 'Gecko emulator and developer tools built' @($log)
                }
                'gecko-matrix' {
                    if (-not $cargo) { Add-Step 'gecko-build' 'skipped' 'Cargo is unavailable'; break }
                    $nativeArgs = @('build', '--manifest-path', $manifestPath, '--locked') + $entry.command
                    $nativeLog = Invoke-Logged 'gecko-native-build' $cargo $nativeArgs $repoRoot
                    Add-Step 'gecko-build' 'passed' 'Gecko native build completed for the active Windows or Linux host' @($nativeLog)
                }
                'gecko-wasm-pack' {
                    if (-not $cargo -or -not $wasmPack -or -not $rustup) { Add-Step 'gecko-web-wasm-build' 'skipped' 'Rust, rustup, wasm-pack and wasm32-unknown-unknown target are required'; break }
                    if ($rustTargets -notcontains 'wasm32-unknown-unknown') { Add-Step 'gecko-web-wasm-build' 'skipped' 'Install the wasm32-unknown-unknown Rust target for the active toolchain'; break }
                    $wasmRepo = $repoRoot
                    $wasmArgs = @('build', 'crates/gecko-web', '--target', 'web', '--out-dir', (Join-Path $runDir 'pkg'), '--out-name', 'gecko_web', '--release')
                    $previousRustcWrapper = $env:CARGO_BUILD_RUSTC_WRAPPER
                    $previousRustcWrapperLegacy = $env:RUSTC_WRAPPER
                    try {
                        # The user's globally configured sccache cannot spawn rustc for the very large web-sys feature set on Windows.
                        Remove-Item Env:CARGO_BUILD_RUSTC_WRAPPER -ErrorAction SilentlyContinue
                        Remove-Item Env:RUSTC_WRAPPER -ErrorAction SilentlyContinue
                        $wasmLog = Invoke-Logged 'gecko-web-wasm-build' $wasmPack $wasmArgs $wasmRepo
                    }
                    finally {
                        if ($null -eq $previousRustcWrapper) { Remove-Item Env:CARGO_BUILD_RUSTC_WRAPPER -ErrorAction SilentlyContinue }
                        else { $env:CARGO_BUILD_RUSTC_WRAPPER = $previousRustcWrapper }
                        if ($null -eq $previousRustcWrapperLegacy) { Remove-Item Env:RUSTC_WRAPPER -ErrorAction SilentlyContinue }
                        else { $env:RUSTC_WRAPPER = $previousRustcWrapperLegacy }
                    }
                    $siteDir = Join-Path $runDir 'site'
                    $emulatorSite = Join-Path $siteDir 'emulator'
                    $menuSite = Join-Path $siteDir 'menu'
                    $null = New-Item -ItemType Directory -Path $emulatorSite, $menuSite -Force
                    Copy-Item (Join-Path $wasmRepo 'crates/gecko-web/index.html') -Destination $emulatorSite
                    Copy-Item (Join-Path $runDir 'pkg') -Destination $emulatorSite -Recurse
                    # The maintained Wii.JS reproduction now lives with the
                    # coordinator so its shared catalog and WebGPU layer are
                    # staged from the same source revision as this pipeline.
                    $menuSource = Join-Path $PSScriptRoot 'frontend'
                    if (Test-Path (Join-Path $menuSource 'index.html')) {
                        Copy-Item -LiteralPath (Join-Path $menuSource 'index.html') -Destination $menuSite -Force
                        foreach ($frontendDir in @('assets', 'views')) {
                            $sourceDir = Join-Path $menuSource $frontendDir
                            if (Test-Path -LiteralPath $sourceDir -PathType Container) {
                                Copy-Item -LiteralPath $sourceDir -Destination $menuSite -Recurse -Force
                            }
                        }
                    }
                    $localWiiAssets = Join-Path $PSScriptRoot 'userland/data/wii/assets'
                    if (Test-Path -LiteralPath $localWiiAssets -PathType Container) {
                        $stagedWiiAssets = Join-Path $menuSite 'assets/user'
                        $null = New-Item -ItemType Directory -Path $stagedWiiAssets -Force
                        Get-ChildItem -LiteralPath $localWiiAssets -Force | Copy-Item -Destination $stagedWiiAssets -Recurse -Force
                    }
                    $localMenuCatalog = Join-Path $PSScriptRoot 'userland/data/wii/menu.catalog.json'
                    $menuCatalogSource = if (Test-Path -LiteralPath $localMenuCatalog -PathType Leaf) { $localMenuCatalog } else { Join-Path $PSScriptRoot 'menu/catalog.example.json' }
                    $webCatalog = Get-Content -LiteralPath $menuCatalogSource -Raw | ConvertFrom-Json
                    $wiiShareRoot = Join-Path $PSScriptRoot 'userland/data/wii'
                    $wiiFilesSite = Join-Path $siteDir 'wii-files'
                    $wiiTitlesSite = Join-Path $wiiFilesSite 'titles'
                    $wiiHomebrewSite = Join-Path $wiiFilesSite 'homebrew'
                    $null = New-Item -ItemType Directory -Path $wiiTitlesSite, $wiiHomebrewSite -Force
                    $stagedTitleFiles = [System.Collections.Generic.List[object]]::new()
                    $missingTitleFiles = [System.Collections.Generic.List[string]]::new()
                    $allowedWiiRoot = [IO.Path]::GetFullPath($wiiShareRoot).TrimEnd([IO.Path]::DirectorySeparatorChar) + [IO.Path]::DirectorySeparatorChar

                    function Stage-WiiContentFile([string] $ContentPath, [string] $DestinationPath, [string] $Role, [string] $ChannelId) {
                        if (-not $ContentPath -or $ContentPath -notmatch '^wii-os/userland/data/wii/') {
                            $missingTitleFiles.Add("$ChannelId ($Role): invalid or missing catalog path")
                            return $null
                        }
                        $sourcePath = Join-Path (Split-Path -Parent $PSScriptRoot) $ContentPath
                        if (-not (Test-Path -LiteralPath $sourcePath -PathType Leaf)) {
                            $missingTitleFiles.Add("$ChannelId ($Role): $ContentPath")
                            return $null
                        }
                        $resolvedSource = [IO.Path]::GetFullPath((Resolve-Path -LiteralPath $sourcePath).Path)
                        if (-not $resolvedSource.StartsWith($allowedWiiRoot, [StringComparison]::OrdinalIgnoreCase)) {
                            $missingTitleFiles.Add("$ChannelId ($Role): path is outside the Wii userland")
                            return $null
                        }
                        $destination = Join-Path $siteDir $DestinationPath
                        $null = New-Item -ItemType Directory -Path (Split-Path -Parent $destination) -Force
                        $null = Copy-Item -LiteralPath $resolvedSource -Destination $destination -Force
                        $relativePath = $DestinationPath.Replace('\\', '/')
                        $stagedTitleFiles.Add([pscustomobject]@{
                            channelId = $ChannelId
                            role = $Role
                            path = $relativePath
                            bytes = (Get-Item -LiteralPath $destination).Length
                            sha256 = (Get-FileHash -LiteralPath $destination -Algorithm SHA256).Hash.ToLowerInvariant()
                        })
                        return $relativePath
                    }

                    foreach ($channel in $webCatalog.channels) {
                        $channelId = [string]$channel.id
                        $channelSlug = $channelId -replace '[^a-zA-Z0-9_-]', '-'
                        if ([string]$channel.contentKind -eq 'wii-wad') {
                            $stagedPath = Stage-WiiContentFile ([string]$channel.contentPath) "wii-files/titles/$channelSlug.wad" 'title-wad' $channelId
                            if ($stagedPath) { $channel.contentPath = $stagedPath }
                        } elseif ($channel.contentKind -eq 'wii-disc') {
                            $discExtension = [IO.Path]::GetExtension([string]$channel.contentPath)
                            $stagedPath = Stage-WiiContentFile ([string]$channel.contentPath) "wii-files/games/$channelSlug$discExtension" 'wii-disc-image' $channelId
                            if ($stagedPath) { $channel.contentPath = $stagedPath }
                        } elseif ($channel.contentKind -eq 'homebrew') {
                            $relativeDirectory = Split-Path -Parent ([string]$channel.contentPath)
                            $sourceDirectory = Join-Path (Split-Path -Parent $PSScriptRoot) $relativeDirectory
                            if (Test-Path -LiteralPath $sourceDirectory -PathType Container) {
                                $resolvedDirectory = [IO.Path]::GetFullPath((Resolve-Path -LiteralPath $sourceDirectory).Path)
                                if ($resolvedDirectory.StartsWith($allowedWiiRoot, [StringComparison]::OrdinalIgnoreCase)) {
                                    $destinationDirectory = Join-Path $wiiHomebrewSite $channelSlug
                                    $null = New-Item -ItemType Directory -Path $destinationDirectory -Force
                                    Get-ChildItem -LiteralPath $resolvedDirectory -File | Copy-Item -Destination $destinationDirectory -Force | Out-Null
                                    $dolName = Split-Path -Leaf ([string]$channel.contentPath)
                                    $stagedPath = "wii-files/homebrew/$channelSlug/$dolName"
                                    $channel.contentPath = $stagedPath
                                    foreach ($homebrewFile in Get-ChildItem -LiteralPath $destinationDirectory -File) {
                                        $stagedTitleFiles.Add([pscustomobject]@{
                                            channelId = $channelId
                                            role = if ($homebrewFile.Extension -eq '.dol') { 'homebrew-executable' } else { 'homebrew-metadata' }
                                            path = "wii-files/homebrew/$channelSlug/$($homebrewFile.Name)"
                                            bytes = $homebrewFile.Length
                                            sha256 = (Get-FileHash -LiteralPath $homebrewFile.FullName -Algorithm SHA256).Hash.ToLowerInvariant()
                                        })
                                    }
                                } else {
                                    $missingTitleFiles.Add("$channelId (homebrew): source directory is outside the Wii userland")
                                }
                            } else {
                                $missingTitleFiles.Add("$channelId (homebrew): $relativeDirectory")
                            }
                        }

                        if ($null -ne $channel.requiredFiles) {
                            foreach ($requiredFile in $channel.requiredFiles) {
                                $requiredRole = [string]$requiredFile.role
                                $requiredSlug = $requiredRole -replace '[^a-zA-Z0-9_-]', '-'
                                $requiredExtension = [IO.Path]::GetExtension([string]$requiredFile.contentPath)
                                $requiredPath = Stage-WiiContentFile ([string]$requiredFile.contentPath) "wii-files/system/$channelSlug-$requiredSlug$requiredExtension" $requiredRole $channelId
                                if ($requiredPath) { $requiredFile.contentPath = $requiredPath }
                            }
                        }
                    }

                    $contentManifest = [pscustomobject]@{
                        schemaVersion = 1
                        source = 'userland/data/wii/menu.catalog.json'
                        stagedFiles = @($stagedTitleFiles)
                        missingFiles = @($missingTitleFiles)
                        runtimeBoundary = 'Catalogued RVZ, WADs, IOS and Homebrew are bundled as local data. Gecko streams Wii discs into its WebAssembly emulator but does not compile PowerPC game code into a standalone WASM or install WADs/boot a Wii NAND.'
                    }
                    $contentManifest | ConvertTo-Json -Depth 12 | Set-Content -LiteralPath (Join-Path $wiiFilesSite 'manifest.json') -Encoding utf8
                    $webCatalog | ConvertTo-Json -Depth 12 | Set-Content -LiteralPath (Join-Path $siteDir 'menu.catalog.json') -Encoding utf8
                    if ($missingTitleFiles.Count -eq 0) {
                        Add-Step 'wii-content-stage' 'passed' ("Staged {0} user-provided title, system, and Homebrew files in the local WASM site" -f $stagedTitleFiles.Count) @((Join-Path $wiiFilesSite 'manifest.json'))
                    } else {
                        Add-Step 'wii-content-stage' 'failed' ("Missing {0} referenced Wii content file(s); see the staged manifest" -f $missingTitleFiles.Count) @((Join-Path $wiiFilesSite 'manifest.json'))
                    }
                    if ($menuWebPackage -and (Test-Path $menuWebPackage)) {
                    $menuCoreAssets = Join-Path $menuSite 'assets/js/menu-core'
                        $null = New-Item -ItemType Directory -Path $menuCoreAssets -Force
                        Get-ChildItem $menuWebPackage -Force | Copy-Item -Destination $menuCoreAssets -Recurse -Force
                    }
                    $defaultDisc = $webCatalog.channels | Where-Object { $_.contentKind -eq 'wii-disc' -and $_.contentPath } | Select-Object -First 1
                    $defaultGameUrl = if ($defaultDisc) { "emulator/?game=../$($defaultDisc.contentPath)" } else { 'menu/' }
                    $defaultGameLabel = if ($defaultDisc) { "Jouer à $($defaultDisc.title)" } else { 'Ouvrir le menu Wii.JS' }
                    $landing = @"
<!doctype html>
<html lang="fr"><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1">
<title>Wii OS</title><style>body{margin:0;background:#111;color:#eee;font:16px system-ui;display:grid;min-height:100vh;place-items:center}main{max-width:48rem;padding:2rem}a{color:#8bd5ff}li{margin:1rem 0}</style>
<main><h1>Wii OS</h1><p>Le jeu Wii par défaut est empaqueté dans ce build et chargé par Gecko WebGPU/WASM.</p><ul><li><a href="$defaultGameUrl">$defaultGameLabel</a></li><li><a href="menu/">Ouvrir le menu Wii.JS</a></li><li><a href="wii-files/manifest.json">Voir les données Wii locales incluses</a></li></ul><p>Le RVZ reste un fichier de données local transmis à Gecko en flux par blocs; les WAD et IOS sont inclus sans installation NAND.</p></main>
</html>
"@
                    Set-Content -LiteralPath (Join-Path $siteDir 'index.html') -Value $landing -Encoding utf8
                    Add-Step 'gecko-web-wasm-build' 'passed' 'Unified static site staged: Wii.JS menu reproduction and Gecko WebGPU/WebAssembly emulator are separate entry points; actual System Menu WAD/NAND boot is not implemented in Gecko Web' @($wasmLog, $siteDir)
                }
                'cargo-workspace' {
                    if (-not $isWindowsHost) { Add-Step "$($entry.name)-build" 'skipped' 'Komorebi targets Windows only'; break }
                    if (-not $cargo) { Add-Step "$($entry.name)-build" 'skipped' 'Cargo is unavailable'; break }
                    $log = Invoke-Logged "$($entry.name)-build" $cargo (@('build', '--manifest-path', $manifestPath) + $entry.command) $repoRoot
                    Add-Step "$($entry.name)-build" 'passed' 'Windows shell workspace built' @($log)
                }
                'cargo-packages' {
                    if (-not $isWindowsHost) { Add-Step "$($entry.name)-build" 'skipped' 'Komorebi targets Windows only'; break }
                    if (-not $cargo) { Add-Step "$($entry.name)-build" 'skipped' 'Cargo is unavailable'; break }
                    $packageLogs = [System.Collections.Generic.List[string]]::new()
                    foreach ($package in $entry.packages) {
                        $packageLogs.Add((Invoke-Logged "$($entry.name)-$package" $cargo @('build', '--manifest-path', $manifestPath, '--locked', '--release', '--no-default-features', '-p', $package) $repoRoot))
                    }
                    Add-Step "$($entry.name)-build" 'passed' 'Built the Windows shell packages listed in its local justfile' @($packageLogs.ToArray())
                }
                'cargo-check' {
                    if (-not $cargo) { Add-Step "$($entry.name)-build" 'skipped' 'Cargo is unavailable'; break }
                    $log = Invoke-Logged "$($entry.name)-build" $cargo (@($entry.command) + @('--manifest-path', $manifestPath, '--locked')) $repoRoot
                    Add-Step "$($entry.name)-build" 'passed' 'Wayland Rust crates checked' @($log)
                }
                'cargo-check-linux' {
                    if ($env:OS -eq 'Windows_NT') { Add-Step "$($entry.name)-build" 'skipped' 'Smithay requires a Linux host and native Wayland/DRM development libraries'; break }
                    if (-not $cargo) { Add-Step "$($entry.name)-build" 'skipped' 'Cargo is unavailable'; break }
                    $log = Invoke-Logged "$($entry.name)-build" $cargo (@($entry.command) + @('--manifest-path', $manifestPath, '--locked')) $repoRoot
                    Add-Step "$($entry.name)-build" 'passed' 'Smithay all-feature library check completed' @($log)
                }
                'go-library' {
                    if (-not $go) { Add-Step "$($entry.name)-build" 'skipped' 'Go is unavailable'; break }
                    $log = Invoke-Logged "$($entry.name)-build" $go @($entry.command) $repoRoot
                    Add-Step "$($entry.name)-build" 'passed' 'Go library build completed' @($log)
                }
                'go-binary' {
                    if (-not $go) { Add-Step "$($entry.name)-build" 'skipped' 'Go is unavailable'; break }
                    if ($selectedTarget -eq 'wii-powerpc') { Add-Step "$($entry.name)-build" 'skipped' 'Go targets are not Wii PowerPC; use devkitPPC/libogc for Wii binaries'; break }
                    if ($entry.name -eq 'WiiUDownloader' -and -not (Test-Path (Join-Path $repoRoot 'db.go'))) { Add-Step "$($entry.name)-build" 'skipped' 'Checkout requires generated db.go; pipeline does not fetch or generate live title data'; break }
                    $goVersion = (& $go version 2>$null | Select-Object -First 1)
                    if ($entry.name -eq 'WiiUDownloader' -and $goVersion -notmatch 'go1\.(2[6-9]|[3-9]\d)') { Add-Step "$($entry.name)-build" 'skipped' "WiiUDownloader requires Go 1.26; found $goVersion"; break }
                    $binarySuffix = if ($selectedTarget -eq 'windows-x64') { '.exe' } else { '' }
                    $targetOutput = Join-Path $runDir $selectedTarget
                    $null = New-Item -ItemType Directory -Path $targetOutput -Force
                    $binaryPath = Join-Path $targetOutput ($entry.name + $binarySuffix)
                    $args = @('build', '-o', $binaryPath, '.')
                    $previousGoos = $env:GOOS; $previousGoarch = $env:GOARCH
                    try { $env:GOOS = if ($selectedTarget -eq 'windows-x64') { 'windows' } else { 'linux' }; $env:GOARCH = 'amd64'; $log = Invoke-Logged "$($entry.name)-build-$selectedTarget" $go $args $repoRoot }
                    finally { $env:GOOS = $previousGoos; $env:GOARCH = $previousGoarch }
                    Add-Step "$($entry.name)-build" 'passed' "Go executable built for $selectedTarget" @($log, $binaryPath)
                }
                'devkitpro-make' {
                    $ppcCompiler = Find-CommandPath 'powerpc-eabi-gcc'
                    if (-not $make -or -not $ppcCompiler -or -not $env:DEVKITPPC -or -not (Test-Path (Join-Path $env:DEVKITPPC 'wii_rules'))) { Add-Step "$($entry.name)-build" 'skipped' 'GNU make, powerpc-eabi-gcc, DEVKITPPC/wii_rules and libogc SDK are required'; break }
                    $stage = Join-Path $runDir 'wii-examples-source'
                    $null = New-Item -ItemType Directory -Path $stage -Force
                    Get-ChildItem $repoRoot -Force | Where-Object Name -ne '.git' | Copy-Item -Destination $stage -Recurse -Force
                    $log = Invoke-Logged "$($entry.name)-build" $make @('-C', $stage) $Workspace
                    $brewArtifacts = Join-Path $runDir 'brew'
                    $null = New-Item -ItemType Directory -Path $brewArtifacts -Force
                    $outputs = @(Get-ChildItem $stage -Recurse -File -Include '*.dol', '*.elf' | Copy-Item -Destination $brewArtifacts -PassThru)
                    Add-Step "$($entry.name)-build" 'passed' ("Homebrew examples built in isolated staging; collected {0} DOL/ELF files" -f $outputs.Count) @($log, $brewArtifacts)
                }
                'wii-ipl' {
                    if (-not $IplWadPath) { Add-Step "$($entry.name)-build" 'skipped' 'Pass -IplWadPath and -IplVersion to opt in with an operator-provided WAD'; break }
                    Add-Step "$($entry.name)-build" 'skipped' 'Wii Menu builds are isolated to prevent WAD copying or generated files in the source checkout'; break
                }
                'python-package' {
                    if (-not $uv) { Add-Step "$($entry.name)-build" 'skipped' 'uv is unavailable'; break }
                    if ($entry.name -eq 'yasb' -and -not $isWindowsHost) { Add-Step "$($entry.name)-build" 'skipped' 'YASB depends on Windows and WinRT packages'; break }
                    $pythonArgs = @('build', '--wheel', '--out-dir', $runDir)
                    $log = Invoke-Logged "$($entry.name)-build" $uv $pythonArgs $repoRoot
                    $pythonArtifacts = [System.Collections.Generic.List[string]]::new()
                    $pythonArtifacts.Add($log)
                    if ($entry.name -eq 'yasb') {
                        if (-not $cmake -or -not $msvcCompiler) { Add-Step 'yasb-build' 'skipped' 'Python wheel built; YASBTrayHook.dll requires CMake and MSVC'; break }
                        $hookSource = Join-Path $repoRoot 'src/core/widgets/services/systray/hook'
                        $hookStage = Join-Path $runDir 'staging/YASBTrayHook'
                        $hookBuild = Join-Path $runDir 'targets/YASBTrayHook'
                        $null = New-Item -ItemType Directory -Path $hookStage -Force
                        foreach ($sourceFile in @('CMakeLists.txt', 'trayhook.cpp', 'version.rc')) { Copy-Item -LiteralPath (Join-Path $hookSource $sourceFile) -Destination $hookStage }
                        $hookConfigure = Invoke-Logged 'yasb-trayhook-configure' $cmake @('-S', $hookStage, '-B', $hookBuild, '-G', 'Visual Studio 18 2026', '-A', 'x64') $runDir
                        $hookCompile = Invoke-Logged 'yasb-trayhook-build' $cmake @('--build', $hookBuild, '--config', 'Release', '--parallel') $runDir
                        $dllArtifacts = @(Get-ChildItem -LiteralPath $hookStage -Filter 'YASBTrayHook*.dll' -File)
                        if ($dllArtifacts.Count -eq 0) { throw 'CMake reported success but did not produce a YASBTrayHook DLL in isolated staging' }
                        $dllOutput = Join-Path $runDir 'windows-x64'
                        $null = New-Item -ItemType Directory -Path $dllOutput -Force
                        $dlls = @($dllArtifacts | Copy-Item -Destination $dllOutput -PassThru)
                        $pythonArtifacts.Add($hookConfigure); $pythonArtifacts.Add($hookCompile); $pythonArtifacts.AddRange([string[]]@($dlls | ForEach-Object FullName))
                    }
                    Add-Step "$($entry.name)-build" 'passed' 'Python package built; project-owned native library was built from staged source when supported' @($pythonArtifacts.ToArray())
                }
                'static-site' {
                    $dest = Join-Path $runDir 'Wii.JS'
                    $null = New-Item -ItemType Directory -Path $dest -Force
                    Get-ChildItem $repoRoot -Force | Where-Object Name -ne '.git' | Copy-Item -Destination $dest -Recurse -Force
                    Add-Step "$($entry.name)-build" 'passed' 'Static Wii.JS site staged in the run output directory' @($dest)
                }
                'archive-index' {
                    $items = @(Get-ChildItem $repoRoot -Recurse -File -ErrorAction SilentlyContinue | Where-Object { $_.FullName -notmatch '[\\/].git[\\/]' } | ForEach-Object { [pscustomobject]@{ path = $_.FullName.Substring($repoRoot.Length + 1); length = $_.Length } })
                    $indexPath = Join-Path $runDir 'wii-archive-index.json'
                    $items | ConvertTo-Json -Depth 4 | Set-Content -LiteralPath $indexPath -Encoding utf8
                    Add-Step "$($entry.name)-build" 'passed' ("Indexed {0} paths without copying archive content" -f $items.Count) @($indexPath)
                }
                'unix-make' { Add-Step "$($entry.name)-build" 'skipped' 'WIT requires a Unix toolchain, generated setup files and a separate GPL/source provenance review' }
                'container-build' { Add-Step "$($entry.name)-build" 'skipped' 'Room Server container build is isolated; its Python dependency layer and service configuration are not provisioned by this coordinator' }
                'cmake' {
                    if ($entry.sourceRoot -ne 'wii-os/src/dolphin' -and (Test-Path (Join-Path $repoRoot '.git'))) {
                        $submoduleStatus = @(& git -C $repoRoot submodule status 2>$null)
                        $missingRequiredSubmodules = @($submoduleStatus | Where-Object {
                            $_ -match '^-' -and $_ -notmatch ' Externals/(FFmpeg-bin|Qt|gtest|libadrenotools)$'
                        })
                        if ($missingRequiredSubmodules.Count -gt 0) { Add-Step "$($entry.name)-build" 'skipped' ('Required Dolphin externals are uninitialized; no recursive downloads are performed: ' + ($missingRequiredSubmodules -join '; ')); break }
                    }
                    if (-not $cmake) { Add-Step "$($entry.name)-build" 'skipped' 'CMake is unavailable'; break }
                    $buildDir = Join-Path $runDir 'dolphin-build'
                    if ($isWindowsHost) { $cmakeArgs = @('-S', $repoRoot, '-B', $buildDir, '-G', 'Visual Studio 18 2026', '-A', 'x64') }
                    else { $cmakeArgs = @('-S', $repoRoot, '-B', $buildDir, '-G', 'Ninja', '-DCMAKE_BUILD_TYPE=Release') }
                    $cmakeArgs += @('-DENABLE_QT=OFF', '-DENABLE_CLI_TOOL=ON', '-DENABLE_NOGUI=ON', '-DENABLE_HEADLESS=OFF', '-DUSE_SANITIZERS=OFF')
                    $dolphinTargets = @('dolphin-tool', 'dolphin-nogui')
                    if ($isWindowsHost) {
                        $cmakeArgs += '-DWII_OS_BUILD_RUST_FFI=ON'
                        $dolphinTargets += 'wii_dolphin_ffi'
                    }
                    if ($vulkanSdk -and (Test-Path (Join-Path $vulkanSdk 'Bin/glslc.exe'))) { $env:VULKAN_SDK = $vulkanSdk }
                    $configureLog = Invoke-Logged 'dolphin-cmake-configure' $cmake $cmakeArgs $repoRoot
                    $buildLog = Invoke-Logged 'dolphin-cmake-build' $cmake (@('--build', $buildDir, '--config', 'Release', '--parallel', '--target') + $dolphinTargets) $repoRoot
                    Add-Step "$($entry.name)-build" 'passed' "Dolphin Qt-free runtime targets built for $selectedTarget in the isolated run output directory" @($configureLog, $buildLog)
                }
                default { Add-Step "$($entry.name)-build" 'skipped' "No build adapter is defined for $($entry.adapter)" }
            }
        }
        catch { Add-Step "$($entry.name)-build" 'failed' $_.Exception.Message }
        finally {
            if ($null -eq $previousCargoTargetDir) { Remove-Item Env:CARGO_TARGET_DIR -ErrorAction SilentlyContinue }
            else { $env:CARGO_TARGET_DIR = $previousCargoTargetDir }
        }
    }
    }
}

if ($Profile -in @('inspect', 'validate', 'all')) {
    if (-not (Test-Path $Manifest)) {
        Add-Step 'input-manifest' 'failed' "Manifest not found: $Manifest"
    } else {
        try { $manifestData = Get-Content $Manifest -Raw | ConvertFrom-Json }
        catch { Add-Step 'input-manifest' 'failed' $_.Exception.Message }
        if ($manifestData -and ($manifestData.schemaVersion -ne 1 -or -not $manifestData.inputs)) {
            Add-Step 'input-manifest' 'failed' 'Manifest must contain schemaVersion: 1 and a non-empty inputs array'
            $manifestData = $null
        }
        if ($manifestData) {
            if (-not $dtk) { Add-Step 'dtk-inspection' 'skipped' 'dtk is not installed or -DtkPath was not provided' }
            foreach ($entry in $manifestData.inputs) {
                if (-not $entry.allowedUse) { Add-Step "input-$($entry.id)" 'skipped' 'Input is not marked allowedUse: true'; continue }
                $baseDir = Split-Path -Parent (Resolve-Path $Manifest).Path
                $inputPath = if ([IO.Path]::IsPathRooted($entry.path)) { $entry.path } else { Join-Path $baseDir $entry.path }
                if (-not (Test-Path -LiteralPath $inputPath -PathType Leaf)) { Add-Step "input-$($entry.id)" 'failed' "Input file not found: $inputPath"; continue }
                $resolved = (Resolve-Path -LiteralPath $inputPath).Path
                $hash = (Get-FileHash -LiteralPath $resolved -Algorithm SHA256).Hash.ToLowerInvariant()
                if ($entry.sha256 -and $entry.sha256.ToLowerInvariant() -ne $hash) { Add-Step "input-$($entry.id)" 'failed' 'SHA-256 does not match manifest'; continue }
                $artifacts = @([pscustomobject]@{ path = $resolved; sha256 = $hash; mediaType = 'application/octet-stream' })
                if (-not $dtk) { Add-Step "input-$($entry.id)" 'skipped' 'Hash recorded; dtk inspection unavailable' $artifacts; continue }
                try {
                    $infoLog = Invoke-Logged "dtk-info-$($entry.id)" $dtk @('disc', 'info', $resolved) $Workspace
                    $verifyLog = Invoke-Logged "dtk-verify-$($entry.id)" $dtk @('disc', 'verify', $resolved) $Workspace
                    Add-Step "input-$($entry.id)" 'passed' 'SHA-256, dtk disc info, and Redump verification succeeded' @($artifacts + $infoLog + $verifyLog)
                }
                catch { Add-Step "input-$($entry.id)" 'failed' $_.Exception.Message $artifacts }
            }
        }
    }

    if ($Profile -in @('validate', 'all')) {
        if (-not $GeckoExecutable) { Add-Step 'gecko-runtime' 'skipped' 'Provide -GeckoExecutable to run an explicit game validation' }
        elseif (-not (Test-Path -LiteralPath $GeckoExecutable -PathType Leaf)) { Add-Step 'gecko-runtime' 'failed' "Gecko executable not found: $GeckoExecutable" }
        else { Add-Step 'gecko-runtime' 'skipped' 'Gecko launch is interactive; automated pass/fail requires a project-owned deterministic fixture' }
        if (-not $dolphin) { Add-Step 'dolphin-runtime' 'skipped' 'Dolphin is not installed or -DolphinPath was not provided' }
        else { Add-Step 'dolphin-runtime' 'skipped' 'Dolphin is available; a deterministic fixture and expected-result contract are not configured' }
    }
}

# Keep concrete artifact integrity data in the report. Directory entries are
# summarized only at their top level; each explicitly listed output file gets a SHA-256.
$artifactManifest = [System.Collections.Generic.List[object]]::new()
$seenArtifactPaths = [System.Collections.Generic.HashSet[string]]::new([StringComparer]::OrdinalIgnoreCase)
foreach ($step in $steps) {
    foreach ($artifact in $step.artifacts) {
        if ($artifact -isnot [string] -or -not (Test-Path -LiteralPath $artifact)) { continue }
        $resolvedArtifact = (Resolve-Path -LiteralPath $artifact).Path
        if (-not $seenArtifactPaths.Add($resolvedArtifact)) { continue }
        $item = Get-Item -LiteralPath $resolvedArtifact
        if ($item.PSIsContainer) {
            $directFiles = @(Get-ChildItem -LiteralPath $resolvedArtifact -File)
            $directBytes = if ($directFiles.Count) { [long](($directFiles | Measure-Object -Property Length -Sum).Sum) } else { 0L }
            $artifactManifest.Add([pscustomobject]@{ path = $resolvedArtifact; kind = 'directory'; directFileCount = $directFiles.Count; directBytes = $directBytes })
        } else {
            $artifactHash = (Get-FileHash -LiteralPath $resolvedArtifact -Algorithm SHA256).Hash.ToLowerInvariant()
            $artifactManifest.Add([pscustomobject]@{ path = $resolvedArtifact; kind = 'file'; bytes = $item.Length; sha256 = $artifactHash })
        }
    }
}

$report = [pscustomobject]@{
    schemaVersion = 1
    runId = $runId
    profile = $Profile
    platformMatrix = [pscustomobject]@{ declared = @($formatCatalog.targets | ForEach-Object id); host = $hostTarget; requested = $selectedTarget; graphicsApi = switch ($selectedTarget) { 'windows-x64' { 'Dolphin D3D12; Gecko wgpu DX12 selectable with WGPU_BACKEND=dx12' } 'linux-x64' { 'Gecko wgpu native Vulkan/OpenGL as enabled by platform and drivers' } 'web-wasm32-webgpu' { 'Gecko wgpu BROWSER_WEBGPU; requires a WebGPU-capable browser' } 'wii-powerpc' { 'Wii GX hardware graphics' } default { 'See formats catalog' } }; browserExecution = 'web-wasm32-webgpu (run browser artifacts in a WebGPU-capable browser; this runner only builds)' }
    consoleFormats = @($formatCatalog.formats)
    startedAtUtc = $startedAtUtc
    workspace = (Resolve-Path $Workspace).Path
    repositories = @($repoRecords)
    artifactManifest = @($artifactManifest)
    tools = [pscustomobject]@{
        paths = [pscustomobject]@{ dtk = $dtk; cargo = $cargo; rustup = $rustup; rustTargets = $rustTargets; wasmPack = $wasmPack; rustc = $rustc; go = $go; bun = $bun; uv = $uv; python = $python; cmake = $cmake; ninja = $ninja; make = $make; dolphin = $dolphin; devkitPPC = $env:DEVKITPPC; msvcCompiler = $msvcCompiler; vulkanSdk = $vulkanSdk; glslc = $glslc; glslangValidator = $glslang }
        versions = [pscustomobject]@{ cargo = (Read-Version $cargo); rustc = (Read-Version $rustc); go = (Read-Version $go @('version')); bun = (Read-Version $bun); uv = (Read-Version $uv); python = (Read-Version $python); cmake = (Read-Version $cmake); ninja = (Read-Version $ninja); make = (Read-Version $make); glslc = (Read-Version $glslc); glslangValidator = (Read-Version $glslang); wasmPack = (Read-Version $wasmPack) }
        pythonPackageTool = $pythonPackageTool
    }
    steps = @($steps)
}
$reportPath = Join-Path $runDir 'report.json'
$report | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath $reportPath -Encoding utf8
Write-Output "Wii OS pipeline report: $reportPath"
if (@($steps | Where-Object status -eq 'failed').Count -gt 0) { exit 1 }
if (($Profile -eq 'wasm' -or $selectedTarget -eq 'web-wasm32-webgpu') -and @($steps | Where-Object { $_.name -eq 'gecko-web-wasm-build' -and $_.status -eq 'passed' }).Count -eq 0) { exit 2 }
