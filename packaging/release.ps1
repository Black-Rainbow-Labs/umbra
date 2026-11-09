<#
.SYNOPSIS
    Cuts an Umbra release: raises the version, tags, pushes, waits for the GitHub release, then
    writes the Homebrew formula and the ACP registry entry from the release's SHA256SUMS.

.DESCRIPTION
    Run it from a clean checkout of main:

        pwsh packaging/release.ps1 -Part patch      # 0.4.0 -> 0.4.1
        pwsh packaging/release.ps1 -Part minor      # 0.4.0 -> 0.5.0
        pwsh packaging/release.ps1 -NoBump          # release the version Cargo.toml already has
        pwsh packaging/release.ps1 -Part patch -WhatIf

    Steps:
      1. Refuses a checkout with uncommitted changes or one that is not on main.
      2. Writes the new version into Cargo.toml and Cargo.lock, commits, tags v<version>, pushes.
      3. Waits for .github/workflows/release.yml to publish the release with every archive.
      4. Downloads SHA256SUMS and writes Formula/umbra.rb in the tap checkout, commits and pushes it.
      5. Writes packaging/acp-registry/umbra/agent.json, commits and pushes it.
      6. Installs the release on this machine with install.ps1.

    `gh` is used with the token Git Credential Manager already holds for github.com when `gh` has
    no login of its own. The token is never printed.

.PARAMETER Part
    Which part of the version to raise: patch, minor or major.

.PARAMETER NoBump
    Release the version in Cargo.toml as it is.

.PARAMETER AfterTag
    The tag of the version in Cargo.toml is already pushed: start at the wait for the release.
    For a release whose workflow had to be run again.

.PARAMETER TapPath
    The checkout of Black-Rainbow-Labs/homebrew-umbra. Cloned there when missing. The default is
    homebrew-umbra beside this repository's main checkout.

.PARAMETER WhatIf
    Print what would happen and change nothing.
#>
param(
    [ValidateSet('patch', 'minor', 'major')][string]$Part = 'patch',
    [switch]$NoBump,
    [switch]$AfterTag,
    [string]$TapPath,
    [switch]$WhatIf
)

$ErrorActionPreference = 'Stop'
$repo = 'Black-Rainbow-Labs/umbra'
$tapRepo = 'Black-Rainbow-Labs/homebrew-umbra'
$root = (git -C $PSScriptRoot rev-parse --show-toplevel).Trim()
$targets = @('aarch64-apple-darwin', 'x86_64-apple-darwin', 'x86_64-unknown-linux-musl', 'aarch64-unknown-linux-musl', 'x86_64-pc-windows-msvc', 'aarch64-pc-windows-msvc')
$registryPlatforms = [ordered]@{
    'darwin-aarch64'  = 'aarch64-apple-darwin'
    'darwin-x86_64'   = 'x86_64-apple-darwin'
    'linux-aarch64'      = 'aarch64-unknown-linux-musl'
    'linux-x86_64'       = 'x86_64-unknown-linux-musl'
    'linux-aarch64-musl' = 'aarch64-unknown-linux-musl'
    'linux-x86_64-musl'  = 'x86_64-unknown-linux-musl'
    'windows-aarch64' = 'aarch64-pc-windows-msvc'
    'windows-x86_64'  = 'x86_64-pc-windows-msvc'
}

function Use-GitHubToken {
    <#
    .SYNOPSIS
        Puts a token gh can use in GH_TOKEN: an existing GH_TOKEN, gh's own login, or the
        credential git already has for github.com.
    #>
    if ($env:GH_TOKEN) { return }
    & gh auth status *> $null
    if ($LASTEXITCODE -eq 0) { return }
    $answer = ("protocol=https`nhost=github.com`n`n" | & git credential fill 2>$null)
    $line = $answer | Where-Object { $_ -like 'password=*' } | Select-Object -First 1
    if (-not $line) { throw 'No GitHub credential: run gh auth login, or set GH_TOKEN' }
    $env:GH_TOKEN = $line.Substring('password='.Length)
}

function Get-CargoVersion {
    <#
    .SYNOPSIS
        The package version in Cargo.toml.
    #>
    $line = Get-Content (Join-Path $root 'Cargo.toml') | Where-Object { $_ -match '^version = "(.+)"' } | Select-Object -First 1
    return ([regex]::Match($line, '^version = "(.+)"')).Groups[1].Value
}

function Get-NextVersion {
    <#
    .SYNOPSIS
        The version after `current`, raising `part`.
    .PARAMETER Current
        The version now, such as 0.4.0.
    .PARAMETER Which
        patch, minor or major.
    #>
    param([string]$Current, [string]$Which)
    $p = $Current.Split('.') | ForEach-Object { [int]$_ }
    switch ($Which) {
        'major' { return "$($p[0] + 1).0.0" }
        'minor' { return "$($p[0]).$($p[1] + 1).0" }
        default { return "$($p[0]).$($p[1]).$($p[2] + 1)" }
    }
}

function Set-Version {
    <#
    .SYNOPSIS
        Writes a version into Cargo.toml and into umbra's entry in Cargo.lock.
    .PARAMETER Version
        The new version.
    #>
    param([string]$Version)
    $toml = Join-Path $root 'Cargo.toml'
    $text = [IO.File]::ReadAllText($toml)
    $text = [regex]::Replace($text, '(?m)^version = ".+"', "version = `"$Version`"", 1)
    [IO.File]::WriteAllText($toml, $text)
    $lock = Join-Path $root 'Cargo.lock'
    $text = [IO.File]::ReadAllText($lock)
    $text = [regex]::Replace($text, '(name = "umbra"\r?\nversion = ")[^"]+(")', "`${1}$Version`${2}")
    [IO.File]::WriteAllText($lock, $text)
}

function Wait-Release {
    <#
    .SYNOPSIS
        Waits for the release of a tag to have every archive and SHA256SUMS, for at most an hour.
        Stops early when the release workflow failed.
    .PARAMETER Tag
        The tag, such as v0.4.1.
    #>
    param([string]$Tag)
    $deadline = (Get-Date).AddMinutes(60)
    while ((Get-Date) -lt $deadline) {
        $assets = & gh release view $Tag --repo $repo --json assets --jq '.assets[].name' 2>$null
        if ($LASTEXITCODE -eq 0 -and ($assets -contains 'SHA256SUMS')) { return }
        # The newest run of the workflow: the tag's own run, or a manual run of it after a failure.
        $failed = & gh run list --repo $repo --workflow release.yml --limit 1 --json conclusion --jq '.[0].conclusion' 2>$null
        if ($failed -eq 'failure') { throw "The release workflow for $Tag failed: gh run list --repo $repo --workflow release.yml" }
        Write-Host "  waiting for the $Tag release ($(Get-Date -Format HH:mm:ss))"
        Start-Sleep -Seconds 30
    }
    throw "The $Tag release did not appear within an hour"
}

function Get-Checksums {
    <#
    .SYNOPSIS
        Reads SHA256SUMS of a release into a table from archive name to checksum.
    .PARAMETER Tag
        The tag.
    #>
    param([string]$Tag)
    $file = Join-Path ([IO.Path]::GetTempPath()) "umbra-SHA256SUMS-$Tag"
    Invoke-WebRequest -UseBasicParsing -Uri "https://github.com/$repo/releases/download/$Tag/SHA256SUMS" -OutFile $file
    $sums = @{}
    foreach ($line in Get-Content $file) {
        $parts = $line -split '\s+'
        if ($parts.Count -ge 2) { $sums[$parts[1].TrimStart('*')] = $parts[0] }
    }
    Remove-Item $file
    return $sums
}

function Archive-Name {
    <#
    .SYNOPSIS
        The archive file name of a target.
    .PARAMETER Target
        The Rust target triple.
    #>
    param([string]$Target)
    if ($Target -like '*windows*') { return "umbra-$Target.zip" }
    return "umbra-$Target.tar.gz"
}

function Write-Formula {
    <#
    .SYNOPSIS
        Writes Formula/umbra.rb in the tap from the template, commits and pushes it.
    .PARAMETER Version
        The released version.
    .PARAMETER Sums
        Archive name to checksum.
    #>
    param([string]$Version, [hashtable]$Sums)
    if (-not (Test-Path $TapPath)) { git clone "https://github.com/$tapRepo.git" $TapPath }
    git -C $TapPath checkout -B main
    if (git -C $TapPath ls-remote --heads origin main) { git -C $TapPath pull --ff-only origin main }
    $text = [IO.File]::ReadAllText((Join-Path $root 'packaging/homebrew/umbra.rb.template')).Replace('@VERSION@', $Version)
    foreach ($target in $targets) {
        $text = $text.Replace("@SHA_$target@", $Sums[(Archive-Name $target)])
    }
    if ($text -match '@SHA_') { throw 'A target has no checksum in SHA256SUMS' }
    New-Item -ItemType Directory -Force -Path (Join-Path $TapPath 'Formula') | Out-Null
    [IO.File]::WriteAllText((Join-Path $TapPath 'Formula/umbra.rb'), $text)
    git -C $TapPath add Formula/umbra.rb
    git -C $TapPath commit -m "umbra $Version"
    git -C $TapPath push origin main
}

function Write-RegistryEntry {
    <#
    .SYNOPSIS
        Writes packaging/acp-registry/umbra/agent.json for this version.
    .PARAMETER Version
        The released version.
    .PARAMETER Sums
        Archive name to checksum.
    #>
    param([string]$Version, [hashtable]$Sums)
    $binary = [ordered]@{}
    foreach ($platform in $registryPlatforms.Keys) {
        $target = $registryPlatforms[$platform]
        $name = Archive-Name $target
        $cmd = if ($target -like '*windows*') { './umbra.exe' } else { './umbra' }
        $binary[$platform] = [ordered]@{ archive = "https://github.com/$repo/releases/download/v$Version/$name"; sha256 = $Sums[$name]; cmd = $cmd; args = @('acp') }
    }
    $entry = [ordered]@{
        id = 'umbra'
        name = 'Umbra'
        version = $Version
        description = 'A small coding agent modeled after mini-swe-agent. One bash tool, a fresh shell for every command, Claude, OpenAI compatible and local models, and the MCP servers, skills and instructions you already set up for other agents.'
        repository = "https://github.com/$repo"
        authors = @('Jason McAffee')
        license = 'MIT'
        license_url = "https://github.com/$repo/blob/main/LICENSE"
        distribution = [ordered]@{ binary = $binary }
    }
    $path = Join-Path $root 'packaging/acp-registry/umbra/agent.json'
    [IO.File]::WriteAllText($path, ($entry | ConvertTo-Json -Depth 6) + "`n")
}

$dirty = git -C $root status --porcelain
if ($dirty) { throw "The checkout has uncommitted changes:`n$dirty" }
$branch = (git -C $root rev-parse --abbrev-ref HEAD).Trim()
if ($branch -ne 'main') { throw "Release from main, not $branch" }
if (-not $TapPath) {
    $mainCheckout = Split-Path -Parent ((git -C $root rev-parse --path-format=absolute --git-common-dir).Trim())
    $TapPath = Join-Path (Split-Path -Parent $mainCheckout) 'homebrew-umbra'
}
$current = Get-CargoVersion
$version = if ($NoBump -or $AfterTag) { $current } else { Get-NextVersion $current $Part }
$tag = "v$version"
Write-Host "Release $tag from $root (Cargo.toml has $current). Tap: $TapPath"
if ($WhatIf) {
    Write-Host 'WhatIf: would write the version, commit, tag, push, wait for the release, update the tap and the registry entry, and install it here.'
    return
}

Use-GitHubToken
if (-not $AfterTag) {
    if (-not $NoBump) {
        Set-Version $version
        git -C $root add Cargo.toml Cargo.lock
        git -C $root commit -m "Release $version"
    }
    git -C $root tag $tag
    git -C $root push origin main
    git -C $root push origin $tag
}
Wait-Release $tag
$sums = Get-Checksums $tag
Write-Formula $version $sums
Write-RegistryEntry $version $sums
git -C $root add packaging/acp-registry/umbra/agent.json
git -C $root commit -m "ACP registry entry for $version"
git -C $root push origin main
$env:UMBRA_VERSION = $version
& (Join-Path $root 'install.ps1')
Write-Host "Released $tag."
