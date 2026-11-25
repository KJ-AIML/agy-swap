<#
.SYNOPSIS
  Packages AGY Swap for Windows 10/11 (x86_64) as a portable .zip + SHA-256 checksum.

.DESCRIPTION
  Windows counterpart of scripts/package-macos.sh. Builds release binaries and produces:
    dist\AGY-Swap-v<version>-windows-x86_64.zip
        AGY Swap\agswitch-desktop.exe   (tray app; icon + version info embedded by build.rs)
        AGY Swap\agswitch.exe           (CLI)
        AGY Swap\README.md, AGY Swap\LICENSE
    dist\SHA256SUMS-windows-x86_64.txt   (lowercase, `sha256sum -c` compatible)

  -Install copies the app to %LOCALAPPDATA%\Programs\AGY Swap\ and creates a Start Menu
  shortcut. No Administrator rights are required.

.EXAMPLE
  .\scripts\package-windows.ps1
.EXAMPLE
  .\scripts\package-windows.ps1 -Install
.EXAMPLE
  .\scripts\package-windows.ps1 -RegenerateIcon   # rebuild apps\agswitch-desktop\assets\agy-swap.ico
#>
[CmdletBinding()]
param(
    [switch]$Install,
    [switch]$SkipBuild,
    [switch]$RegenerateIcon
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$Root = Split-Path -Parent $PSScriptRoot
Set-Location $Root

$AppName = 'AGY Swap'
$Arch = 'x86_64'
$DistDir = Join-Path $Root 'dist'
$IconPath = Join-Path $Root 'apps\agswitch-desktop\assets\agy-swap.ico'
$IconSource = Join-Path $Root 'docs\media\agy-swap-icon.png'

$versionLine = Select-String -Path (Join-Path $Root 'Cargo.toml') -Pattern '^\s*version\s*=\s*"([^"]+)"' | Select-Object -First 1
if (-not $versionLine) { throw 'could not read workspace version from Cargo.toml' }
$Version = $versionLine.Matches[0].Groups[1].Value

$ZipName = "AGY-Swap-v$Version-windows-$Arch.zip"
$ZipPath = Join-Path $DistDir $ZipName
$SumsPath = Join-Path $DistDir "SHA256SUMS-windows-$Arch.txt"
$Stage = Join-Path $DistDir 'windows-stage'
$StageApp = Join-Path $Stage $AppName

function New-AgyIcon {
    # Multi-resolution .ico (PNG-compressed entries, Vista+) generated from the 256px app icon.
    param([string]$Source, [string]$Destination)
    Add-Type -AssemblyName System.Drawing
    $sizes = @(16, 20, 24, 32, 40, 48, 64, 128, 256)
    $src = [System.Drawing.Image]::FromFile($Source)
    $pngs = @()
    try {
        foreach ($s in $sizes) {
            $bmp = New-Object System.Drawing.Bitmap $s, $s, ([System.Drawing.Imaging.PixelFormat]::Format32bppArgb)
            $g = [System.Drawing.Graphics]::FromImage($bmp)
            $g.InterpolationMode = [System.Drawing.Drawing2D.InterpolationMode]::HighQualityBicubic
            $g.PixelOffsetMode = [System.Drawing.Drawing2D.PixelOffsetMode]::HighQuality
            $g.SmoothingMode = [System.Drawing.Drawing2D.SmoothingMode]::HighQuality
            $g.CompositingQuality = [System.Drawing.Drawing2D.CompositingQuality]::HighQuality
            $g.Clear([System.Drawing.Color]::Transparent)
            $g.DrawImage($src, 0, 0, $s, $s)
            $g.Dispose()
            $ms = New-Object System.IO.MemoryStream
            $bmp.Save($ms, [System.Drawing.Imaging.ImageFormat]::Png)
            $bmp.Dispose()
            $pngs += , $ms.ToArray()
        }
    } finally {
        $src.Dispose()
    }

    New-Item -ItemType Directory -Force -Path (Split-Path $Destination) | Out-Null
    $fs = [System.IO.File]::Create($Destination)
    $w = New-Object System.IO.BinaryWriter $fs
    try {
        $w.Write([UInt16]0); $w.Write([UInt16]1); $w.Write([UInt16]$sizes.Count)
        $offset = 6 + 16 * $sizes.Count
        for ($i = 0; $i -lt $sizes.Count; $i++) {
            $dim = if ($sizes[$i] -ge 256) { 0 } else { $sizes[$i] }
            $w.Write([byte]$dim); $w.Write([byte]$dim); $w.Write([byte]0); $w.Write([byte]0)
            $w.Write([UInt16]1); $w.Write([UInt16]32)
            $w.Write([UInt32]$pngs[$i].Length); $w.Write([UInt32]$offset)
            $offset += $pngs[$i].Length
        }
        foreach ($p in $pngs) { $w.Write($p) }
    } finally {
        $w.Dispose(); $fs.Dispose()
    }
}

function New-PortableZip {
    # Writes the archive with forward-slash entry names (Windows PowerShell 5.1's Compress-Archive
    # emits backslashes, which break extraction on macOS/Linux tools).
    param([string]$SourceDir, [string]$Destination)
    Add-Type -AssemblyName System.IO.Compression, System.IO.Compression.FileSystem
    if (Test-Path -LiteralPath $Destination) { Remove-Item -LiteralPath $Destination -Force }
    $base = (Resolve-Path -LiteralPath $SourceDir).Path.TrimEnd('\') + '\'
    $zip = [System.IO.Compression.ZipFile]::Open($Destination, [System.IO.Compression.ZipArchiveMode]::Create)
    try {
        Get-ChildItem -LiteralPath $SourceDir -Recurse -File | Sort-Object FullName | ForEach-Object {
            $entry = $_.FullName.Substring($base.Length).Replace('\', '/')
            [System.IO.Compression.ZipFileExtensions]::CreateEntryFromFile(
                $zip, $_.FullName, $entry, [System.IO.Compression.CompressionLevel]::Optimal) | Out-Null
        }
    } finally {
        $zip.Dispose()
    }
}

if ($RegenerateIcon -or -not (Test-Path -LiteralPath $IconPath)) {
    Write-Host "==> Generating Windows icon $IconPath ..."
    New-AgyIcon -Source $IconSource -Destination $IconPath
}

if (-not $SkipBuild) {
    Write-Host "==> Building release binaries (v$Version, windows-$Arch)..."
    & cargo build --release -p agswitch-desktop -p agswitch-cli
    if ($LASTEXITCODE -ne 0) { throw "cargo build failed with exit code $LASTEXITCODE" }
}

$DesktopExe = Join-Path $Root 'target\release\agswitch-desktop.exe'
$CliExe = Join-Path $Root 'target\release\agswitch-cli.exe'
foreach ($f in @($DesktopExe, $CliExe)) {
    if (-not (Test-Path -LiteralPath $f)) { throw "missing release binary: $f" }
}

Write-Host '==> Staging package...'
New-Item -ItemType Directory -Force -Path $DistDir | Out-Null
if (Test-Path -LiteralPath $Stage) {
    # Stage holds only files this script copied (no links), so a non-recursive sweep suffices.
    Get-ChildItem -LiteralPath $StageApp -File -ErrorAction SilentlyContinue | Remove-Item -Force
}
New-Item -ItemType Directory -Force -Path $StageApp | Out-Null
Copy-Item -LiteralPath $DesktopExe -Destination (Join-Path $StageApp 'agswitch-desktop.exe') -Force
Copy-Item -LiteralPath $CliExe -Destination (Join-Path $StageApp 'agswitch.exe') -Force
Copy-Item -LiteralPath (Join-Path $Root 'README.md') -Destination $StageApp -Force
Copy-Item -LiteralPath (Join-Path $Root 'LICENSE') -Destination $StageApp -Force

Write-Host "==> Creating $ZipName ..."
New-PortableZip -SourceDir $Stage -Destination $ZipPath

$hash = (Get-FileHash -LiteralPath $ZipPath -Algorithm SHA256).Hash.ToLowerInvariant()
[System.IO.File]::WriteAllText($SumsPath, "$hash  $ZipName`n", (New-Object System.Text.UTF8Encoding $false))

Write-Host '==> Packaged artifacts:'
Get-Item -LiteralPath $ZipPath, $SumsPath | ForEach-Object {
    Write-Host ("  {0}  ({1:N0} KB)" -f $_.FullName, ($_.Length / 1KB))
}
Write-Host "  sha256: $hash"

if ($Install) {
    $InstallDir = Join-Path $env:LOCALAPPDATA "Programs\$AppName"
    Write-Host "==> Installing '$AppName' into $InstallDir ..."
    Get-Process -Name 'agswitch-desktop' -ErrorAction SilentlyContinue | Where-Object {
        $_.Path -and $_.Path.StartsWith($InstallDir, [System.StringComparison]::OrdinalIgnoreCase)
    } | Stop-Process -Force
    Start-Sleep -Milliseconds 300
    New-Item -ItemType Directory -Force -Path $InstallDir | Out-Null
    Get-ChildItem -LiteralPath $StageApp -File | Copy-Item -Destination $InstallDir -Force

    $StartMenu = Join-Path $env:APPDATA 'Microsoft\Windows\Start Menu\Programs'
    $Lnk = Join-Path $StartMenu "$AppName.lnk"
    $shell = New-Object -ComObject WScript.Shell
    $sc = $shell.CreateShortcut($Lnk)
    $sc.TargetPath = Join-Path $InstallDir 'agswitch-desktop.exe'
    $sc.WorkingDirectory = $InstallDir
    $sc.IconLocation = "$(Join-Path $InstallDir 'agswitch-desktop.exe'),0"
    $sc.Description = 'AGY Swap - Antigravity account switcher'
    $sc.Save()

    Write-Host 'Installed:'
    Write-Host "  App:        $(Join-Path $InstallDir 'agswitch-desktop.exe')"
    Write-Host "  CLI:        $(Join-Path $InstallDir 'agswitch.exe')"
    Write-Host "  Start Menu: $Lnk"
    Write-Host "Tip: add '$InstallDir' to your user PATH to run 'agswitch' from any terminal."
}
