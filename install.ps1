# Installs Umbra on Windows from the latest GitHub release.
#
#   powershell -ExecutionPolicy ByPass -c "irm https://raw.githubusercontent.com/Black-Rainbow-Labs/umbra/main/install.ps1 | iex"
#
# It downloads the archive for this machine and SHA256SUMS, checks the archive against
# SHA256SUMS, puts umbra.exe in %LOCALAPPDATA%\Programs\umbra and adds that folder to the user's
# PATH. It needs no administrator rights.
#
#   $env:UMBRA_VERSION = '0.4.0'     install that version instead of the latest
#   $env:UMBRA_INSTALL_DIR = 'dir'   install into dir instead

$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'

$repo = 'Black-Rainbow-Labs/umbra'
$installDir = if ($env:UMBRA_INSTALL_DIR) { $env:UMBRA_INSTALL_DIR } else { Join-Path $env:LOCALAPPDATA 'Programs\umbra' }
$arch = if ($env:PROCESSOR_ARCHITECTURE -eq 'ARM64' -or $env:PROCESSOR_ARCHITEW6432 -eq 'ARM64') { 'aarch64' } else { 'x86_64' }
$name = "umbra-$arch-pc-windows-msvc.zip"
$base = if ($env:UMBRA_VERSION) { "https://github.com/$repo/releases/download/v$($env:UMBRA_VERSION.TrimStart('v'))" } else { "https://github.com/$repo/releases/latest/download" }

$work = Join-Path ([System.IO.Path]::GetTempPath()) ("umbra-install-" + [System.Guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $work | Out-Null
try {
    Write-Host "Downloading $base/$name"
    Invoke-WebRequest -UseBasicParsing -Uri "$base/$name" -OutFile (Join-Path $work $name)
    Invoke-WebRequest -UseBasicParsing -Uri "$base/SHA256SUMS" -OutFile (Join-Path $work 'SHA256SUMS')

    $line = Get-Content (Join-Path $work 'SHA256SUMS') | Where-Object { $_ -match " $([regex]::Escape($name))$" } | Select-Object -First 1
    if (-not $line) { throw "SHA256SUMS has no line for $name" }
    $expected = ($line -split ' ')[0].ToLower()
    $actual = (Get-FileHash -Algorithm SHA256 (Join-Path $work $name)).Hash.ToLower()
    if ($expected -ne $actual) { throw "$name does not match SHA256SUMS (expected $expected, got $actual)" }

    Expand-Archive -Path (Join-Path $work $name) -DestinationPath (Join-Path $work 'unpacked') -Force
    New-Item -ItemType Directory -Force -Path $installDir | Out-Null
    Copy-Item -Force (Join-Path $work 'unpacked\umbra.exe') (Join-Path $installDir 'umbra.exe')
} finally {
    Remove-Item -Recurse -Force -Path $work -ErrorAction SilentlyContinue
}

$userPath = [Environment]::GetEnvironmentVariable('Path', 'User')
if (-not (($userPath -split ';') -contains $installDir)) {
    $newPath = if ($userPath) { "$userPath;$installDir" } else { $installDir }
    [Environment]::SetEnvironmentVariable('Path', $newPath, 'User')
    Write-Host "Added $installDir to your PATH. Open a new terminal to use it."
}
$env:Path = "$env:Path;$installDir"

$version = & (Join-Path $installDir 'umbra.exe') --version
Write-Host "Installed $version to $installDir\umbra.exe"
if (-not (Test-Path 'C:\Program Files\Git\bin\bash.exe') -and -not (Get-Command bash.exe -ErrorAction SilentlyContinue)) {
    Write-Host 'Umbra runs commands with Git Bash. Install Git for Windows: https://gitforwindows.org'
}
Write-Host 'Next: run `umbra setup` to choose a model.'
