[CmdletBinding()]
param(
    [string] $Workspace = $PSScriptRoot,
    [string] $Catalog = (Join-Path $PSScriptRoot 'pipeline.catalog.json'),
    [string] $AdditionalRootsCatalog = (Join-Path $PSScriptRoot 'source-roots.json'),
    [string] $Output = (Join-Path $PSScriptRoot 'userland/build/reports')
)

$ErrorActionPreference = 'Stop'
$workspacePath = (Resolve-Path -LiteralPath $Workspace).Path
$catalogData = Get-Content -LiteralPath $Catalog -Raw | ConvertFrom-Json
$additionalCatalog = Get-Content -LiteralPath $AdditionalRootsCatalog -Raw | ConvertFrom-Json
$sourceRootPaths = @{}
foreach ($sourceRoot in $additionalCatalog.repositories) {
    $sourceRootPaths[$sourceRoot.name] = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot $sourceRoot.path))
}
$entries = [System.Collections.Generic.List[object]]::new()
$roots = [System.Collections.Generic.Dictionary[string, object]]::new([StringComparer]::OrdinalIgnoreCase)
$repoNames = [System.Collections.Generic.List[string]]::new()
$sourcePatterns = @('*.c', '*.h', '*.cc', '*.cpp', '*.cxx', '*.hh', '*.hpp', '*.hxx')

foreach ($repo in $catalogData.repositories) {
    if (-not $repoNames.Contains($repo.name)) { $repoNames.Add($repo.name) }
    $root = if ($repo.sourceRoot -and $sourceRootPaths.ContainsKey($repo.sourceRoot)) {
        $sourceRootPaths[$repo.sourceRoot]
    } else {
        $sourceName = if ($repo.sourceRepository) { $repo.sourceRepository } else { $repo.name }
        Join-Path $workspacePath $sourceName
    }
    if (-not (Test-Path -LiteralPath $root -PathType Container)) { continue }
    $resolvedRoot = (Resolve-Path -LiteralPath $root).Path
    if (-not $roots.ContainsKey($resolvedRoot)) {
        $roots[$resolvedRoot] = [pscustomobject]@{
            names = [System.Collections.Generic.List[string]]::new()
            adapters = [System.Collections.Generic.List[string]]::new()
        }
    }
    if (-not $roots[$resolvedRoot].names.Contains($repo.name)) { $roots[$resolvedRoot].names.Add($repo.name) }
    if ($repo.adapter -and -not $roots[$resolvedRoot].adapters.Contains($repo.adapter)) { $roots[$resolvedRoot].adapters.Add($repo.adapter) }
}

# Include explicitly related product source roots without widening the scan to
# unrelated directories on the machine.
if (Test-Path -LiteralPath $AdditionalRootsCatalog -PathType Leaf) {
    foreach ($repo in $additionalCatalog.repositories) {
        $candidate = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot $repo.path))
        if (-not (Test-Path -LiteralPath $candidate -PathType Container)) { continue }
        $resolvedRoot = (Resolve-Path -LiteralPath $candidate).Path
        if (-not $roots.ContainsKey($resolvedRoot)) {
            $roots[$resolvedRoot] = [pscustomobject]@{
                names = [System.Collections.Generic.List[string]]::new()
                adapters = [System.Collections.Generic.List[string]]::new()
            }
        }
        if (-not $roots[$resolvedRoot].names.Contains($repo.name)) { $roots[$resolvedRoot].names.Add($repo.name) }
        if ($repo.adapter -and -not $roots[$resolvedRoot].adapters.Contains($repo.adapter)) { $roots[$resolvedRoot].adapters.Add($repo.adapter) }
        if (-not $repoNames.Contains($repo.name)) { $repoNames.Add($repo.name) }
    }
}

# Git records a submodule as one gitlink, not as the source files inside it.
# Add initialized submodule checkouts as independent provenance roots; uninitialized
# submodules have no source files to index and remain visible in their parent repo.
foreach ($parentRoot in @($roots.Keys)) {
    if (-not (Test-Path -LiteralPath (Join-Path $parentRoot '.git'))) { continue }
    $parentName = [string]$roots[$parentRoot].names[0]
    $submoduleLines = @(& git -C $parentRoot submodule status --recursive 2>$null)
    foreach ($line in $submoduleLines) {
        if ($line -match '^([ +-])([0-9a-f]{40})\s+(\S+)') {
            if ($Matches[1] -eq '-') { continue }
            $submoduleRelative = $Matches[3]
            $candidate = Join-Path $parentRoot $submoduleRelative
            if (-not (Test-Path -LiteralPath (Join-Path $candidate '.git'))) { continue }
            $resolvedRoot = (Resolve-Path -LiteralPath $candidate).Path
            if ($roots.ContainsKey($resolvedRoot)) { continue }
            $submoduleLabel = "$parentName/$($submoduleRelative.Replace('\', '/'))"
            $roots[$resolvedRoot] = [pscustomobject]@{
                names = [System.Collections.Generic.List[string]]::new()
                adapters = [System.Collections.Generic.List[string]]::new()
            }
            $roots[$resolvedRoot].names.Add($submoduleLabel)
            foreach ($adapter in $roots[$parentRoot].adapters) { $roots[$resolvedRoot].adapters.Add($adapter) }
            $repoNames.Add($submoduleLabel)
        }
    }
}

foreach ($root in $roots.Keys) {
    $gitRoot = Test-Path -LiteralPath (Join-Path $root '.git')
    $head = $null
    $branch = $null
    $status = @()
    $trackedFiles = @()
    $trackedPaths = [System.Collections.Generic.HashSet[string]]::new([StringComparer]::OrdinalIgnoreCase)
    $snapshotMarker = Join-Path $root 'SOURCE-REVISION.txt'
    if ($gitRoot) {
        $head = (& git -C $root rev-parse HEAD 2>$null | Select-Object -First 1)
        $branch = (& git -C $root branch --show-current 2>$null | Select-Object -First 1)
        $status = @(& git -C $root status --porcelain --untracked-files=normal 2>$null)
        # Include tracked edits and non-ignored new C files so the hashes describe
        # the checked-out source being assembled, not only the last Git commit.
        $trackedFiles = @(& git -C $root ls-files --cached --others --exclude-standard -- @sourcePatterns 2>$null)
        foreach ($trackedPath in @(& git -C $root ls-files --cached -- @sourcePatterns 2>$null)) { $null = $trackedPaths.Add($trackedPath) }
    } elseif (Test-Path -LiteralPath $snapshotMarker -PathType Leaf) {
        $revisionText = Get-Content -LiteralPath $snapshotMarker -Raw
        if ($revisionText -match '(?m)^Commit:\s*([0-9a-f]{40})\s*$') { $head = $Matches[1] }
        $trackedFiles = @(Get-ChildItem -LiteralPath $root -Recurse -File -Include $sourcePatterns -ErrorAction SilentlyContinue |
            ForEach-Object { $_.FullName.Substring($root.Length).TrimStart('\', '/') })
    }
    $licenseFiles = @()
    if ($gitRoot) {
        $licensePaths = @(& git -C $root ls-files --cached --others --exclude-standard 2>$null | Where-Object { $_ -match '(^|/)(LICENSE|COPYING|COPYRIGHT|NOTICE)(\..*)?$' })
        foreach ($licensePath in $licensePaths) {
            $licenseFull = Join-Path $root $licensePath
            if (Test-Path -LiteralPath $licenseFull -PathType Leaf) {
                $licenseFiles += [pscustomobject]@{
                    path = $licensePath.Replace('\', '/')
                    sha256 = (Get-FileHash -LiteralPath $licenseFull -Algorithm SHA256).Hash.ToLowerInvariant()
                }
            }
        }
    } elseif (Test-Path -LiteralPath $snapshotMarker -PathType Leaf) {
        $licenseFiles = @(Get-ChildItem -LiteralPath $root -Recurse -File -Include LICENSE, COPYING, COPYRIGHT, NOTICE -ErrorAction SilentlyContinue |
            ForEach-Object {
                [pscustomobject]@{
                    path = $_.FullName.Substring($root.Length).TrimStart('\', '/').Replace('\', '/')
                    sha256 = (Get-FileHash -LiteralPath $_.FullName -Algorithm SHA256).Hash.ToLowerInvariant()
                }
            })
    }
    foreach ($relative in $trackedFiles) {
        $full = Join-Path $root $relative
        if (-not (Test-Path -LiteralPath $full -PathType Leaf)) { continue }
        $file = Get-Item -LiteralPath $full
        $entries.Add([pscustomobject]@{
            sourceRepositories = @($roots[$root].names.ToArray())
            sourceRepositoryRoot = [string]$roots[$root].names[0]
            buildAdapters = @($roots[$root].adapters.ToArray())
            sourceRevision = if ($head) { $head.Trim() } else { $null }
            sourceBranch = if ($branch) { $branch.Trim() } else { $null }
            worktreeDirty = ($status.Count -gt 0)
            fileState = if ($trackedPaths.Contains($relative)) { 'tracked' } elseif (-not $gitRoot -and (Test-Path -LiteralPath $snapshotMarker -PathType Leaf)) { 'snapshot' } else { 'untracked' }
            repositoryLicenseFiles = @($licenseFiles)
            kind = if ($file.Extension -in @('.c', '.cc', '.cpp', '.cxx')) { 'translation-unit' } else { 'header' }
            language = if ($file.Extension -eq '.c') { 'C' } elseif ($file.Extension -in @('.cc', '.cpp', '.cxx')) { 'C++' } else { 'ambiguous-header' }
            extension = $file.Extension.ToLowerInvariant()
            path = $relative.Replace('\', '/')
            bytes = $file.Length
            sha256 = (Get-FileHash -LiteralPath $full -Algorithm SHA256).Hash.ToLowerInvariant()
        })
    }
}

# Include every C/header file owned by this coordinator, including the shared C
# ABI and host example. Exclude generated output and Cargo build trees.
$localRoot = $PSScriptRoot
$localFiles = @(Get-ChildItem -LiteralPath $localRoot -Recurse -File -Include $sourcePatterns -ErrorAction SilentlyContinue |
    Where-Object { $_.FullName -notmatch '[\\/](out|target|build|node_modules|\.git|src[\\/](dolphin|gecko))[\\/]' })
foreach ($file in $localFiles) {
    $relative = $file.FullName.Substring($localRoot.Length).TrimStart('\', '/')
    $entries.Add([pscustomobject]@{
        sourceRepositories = @('wii-os')
        sourceRepositoryRoot = 'wii-os'
        buildAdapters = @('menu-core')
        sourceRevision = $null
        sourceBranch = $null
        worktreeDirty = $null
        fileState = 'local'
        repositoryLicenseFiles = @([pscustomobject]@{ path = 'crates/menu-core/Cargo.toml (license = MIT)'; sha256 = $null })
        kind = if ($file.Extension -in @('.c', '.cc', '.cpp', '.cxx')) { 'translation-unit' } else { 'header' }
        language = if ($file.Extension -eq '.c') { 'C' } elseif ($file.Extension -in @('.cc', '.cpp', '.cxx')) { 'C++' } else { 'ambiguous-header' }
        extension = $file.Extension.ToLowerInvariant()
        path = $relative.Replace('\', '/')
        bytes = $file.Length
        sha256 = (Get-FileHash -LiteralPath $file.FullName -Algorithm SHA256).Hash.ToLowerInvariant()
    })
}

$entries.Sort([Comparison[object]]{ param($a, $b) [StringComparer]::OrdinalIgnoreCase.Compare([string]$a.path, [string]$b.path) })
$translationUnits = @($entries | Where-Object kind -eq 'translation-unit')
$headers = @($entries | Where-Object kind -eq 'header')
$componentCatalogPath = Join-Path $PSScriptRoot 'c-projects.catalog.json'
$componentCatalog = Get-Content -LiteralPath $componentCatalogPath -Raw | ConvertFrom-Json
$componentInventory = foreach ($component in $componentCatalog.components) {
    $matchingEntries = @($entries | Where-Object {
        $component.sourceRoot -eq $_.sourceRepositoryRoot -or
        ($component.sourceRoot -eq 'wii-os/src/dolphin' -and $_.sourceRepositoryRoot -eq 'wii-os/src/dolphin') -or
        ($component.sourceRoot -eq 'wii-os/crates/gecko-core' -and $_.sourceRepositoryRoot -eq 'wii-os/crates/gecko-core')
    })
    [pscustomobject]@{
        id = $component.id
        sourceRoot = $component.sourceRoot
        sourceRepositoryRoots = @($matchingEntries | ForEach-Object sourceRepositoryRoot | Sort-Object -Unique)
        cTranslationUnits = @($matchingEntries | Where-Object { $_.kind -eq 'translation-unit' -and $_.language -eq 'C' }).Count
        cppTranslationUnits = @($matchingEntries | Where-Object { $_.kind -eq 'translation-unit' -and $_.language -eq 'C++' }).Count
        ambiguousHeaders = @($matchingEntries | Where-Object { $_.kind -eq 'header' -and $_.language -eq 'ambiguous-header' }).Count
        buildAdapter = $component.buildAdapter
        redistribution = $component.redistribution
    }
}
$result = [pscustomobject]@{
    schemaVersion = 4
    generatedAtUtc = (Get-Date).ToUniversalTime().ToString('o')
    workspace = $workspacePath
    policy = 'Provenance index of C and C++ translation units and headers in the Wii OS product. Hashes describe the product-owned source snapshots. Build adapters, license evidence and canonical product paths remain attached.'
    repositories = @($repoNames.ToArray()) + 'wii-os'
    totals = [pscustomobject]@{ translationUnits = $translationUnits.Count; headers = $headers.Count; indexedFiles = $entries.Count }
    cProjectCatalog = 'wii-os/c-projects.catalog.json'
    cProjects = @($componentInventory)
    files = @($entries)
}
$outputDir = Join-Path $Output 'c-source-inventory'
$null = New-Item -ItemType Directory -Path $outputDir -Force
$reportPath = Join-Path $outputDir 'inventory.json'
$result | ConvertTo-Json -Depth 10 | Set-Content -LiteralPath $reportPath -Encoding utf8
Write-Output "Native C/C++ source inventory: $reportPath ($($translationUnits.Count) translation units, $($headers.Count) headers from $($roots.Count) repository roots and the Wii OS coordinator)"
