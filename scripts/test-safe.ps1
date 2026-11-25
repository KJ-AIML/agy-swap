<#
.SYNOPSIS
  AGY Swap - 100% Safe Verification & Sandbox E2E Test Script (Windows 10/11).

.DESCRIPTION
  Windows mirror of scripts/test-safe.sh. NEVER touches real Antigravity processes,
  %USERPROFILE%\.gemini, or %LOCALAPPDATA%\AgSwitch. All E2E checks run against simulated
  processes inside an isolated temp folder (whose path deliberately contains spaces so the
  NTFS-junction Shared Chats code path is exercised with spaced paths).

.EXAMPLE
  .\scripts\test-safe.ps1          # full verify + automated sandbox E2E suite
.EXAMPLE
  .\scripts\test-safe.ps1 -Ui      # also launch the interactive Sandbox GUI after tests pass
#>
[CmdletBinding()]
param(
    [switch]$Ui
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$RootDir = Split-Path -Parent $PSScriptRoot
Set-Location $RootDir

$SandboxDir = Join-Path ([System.IO.Path]::GetTempPath()) ("agyswap e2e " + [System.Guid]::NewGuid().ToString('N').Substring(0, 8))
New-Item -ItemType Directory -Force -Path $SandboxDir | Out-Null

function Write-Section([string]$Title) {
    Write-Host ''
    Write-Host '============================================================'
    Write-Host $Title
    Write-Host '============================================================'
}

function Invoke-Native {
    # Runs a native command, streaming output, and throws on a non-zero exit code.
    param([string]$Exe, [string[]]$Arguments)
    & $Exe @Arguments
    if ($LASTEXITCODE -ne 0) {
        throw "'$Exe $($Arguments -join ' ')' failed with exit code $LASTEXITCODE"
    }
}

function Invoke-Cli {
    # Runs the sandboxed CLI and returns @{ Code; Output } without throwing (Windows PowerShell 5.1
    # turns redirected native stderr into ErrorRecords, so relax the preference locally).
    param([string[]]$Arguments)
    $prev = $ErrorActionPreference
    $ErrorActionPreference = 'Continue'
    try {
        $out = & $script:Cli @script:Sb @Arguments 2>&1 | ForEach-Object { "$_" }
        $code = $LASTEXITCODE
    } finally {
        $ErrorActionPreference = $prev
    }
    [pscustomobject]@{ Code = $code; Output = ($out -join [Environment]::NewLine) }
}

function Invoke-CliOk {
    param([string[]]$Arguments)
    $r = Invoke-Cli $Arguments
    if ($r.Output) { Write-Host $r.Output }
    if ($r.Code -ne 0) {
        throw "FAIL: 'agswitch-cli $($Arguments -join ' ')' exited with $($r.Code)"
    }
    $r.Output
}

function Assert-Contains([string]$Haystack, [string]$Needle, [string]$What) {
    if (-not $Haystack.Contains($Needle)) {
        Write-Host $Haystack
        throw "FAIL: expected $What to contain '$Needle'"
    }
}

function Assert-StatusHas([string]$Expected) {
    $r = Invoke-Cli @('status')
    if ($r.Code -ne 0) { Write-Host $r.Output; throw "FAIL: status exited with $($r.Code)" }
    Assert-Contains $r.Output $Expected 'status'
}

try {
    Write-Section '1) Workspace Verify (layers + purity + clippy -D warnings + unit tests)'
    Invoke-Native 'cargo' @('run', '-p', 'xtask', '--', 'verify')

    Write-Section '2) Build CLI & Desktop binaries'
    Invoke-Native 'cargo' @('build', '-p', 'agswitch-cli', '-p', 'agswitch-desktop')

    $script:Cli = Join-Path $RootDir 'target\debug\agswitch-cli.exe'
    $script:Sb = @('--sandbox', '--sandbox-dir', $SandboxDir)

    Write-Section "3) Safe Sandbox E2E Flow (isolated dir: $SandboxDir)"

    Write-Host '-> [3.1] Initial status (5h + Weekly 7d + Gemini/Claude/GPT)'
    $status = Invoke-CliOk @('status')
    Assert-Contains $status 'Active: SC' 'status'
    Assert-Contains $status '[READY   ] Trade' 'status'
    Assert-Contains $status 'Claude' 'status'
    Assert-Contains $status 'GPT' 'status'

    Write-Host ''
    Write-Host '-> [3.2] Switch Active: SC -> trade (simulated, real Antigravity untouched!)'
    Invoke-CliOk @('switch', 'trade') | Out-Null
    Assert-StatusHas 'Active: Trade'

    Write-Host ''
    Write-Host '-> [3.3] Open Alongside: work-claude'
    Invoke-CliOk @('open', 'work-claude') | Out-Null
    Assert-StatusHas 'Active: Trade, Work Claude'

    Write-Host ''
    Write-Host '-> [3.4] Stop running window: work-claude'
    Invoke-CliOk @('stop', 'work-claude') | Out-Null
    Assert-StatusHas 'Active: Trade'

    Write-Host ''
    Write-Host '-> [3.5] 5h Cooldown + Smart Next Rotation (must skip cooldown account)'
    Invoke-CliOk @('cooldown', 'work-claude') | Out-Null
    Invoke-CliOk @('next') | Out-Null
    Assert-StatusHas 'Active: SC'

    Write-Host ''
    Write-Host '-> [3.6] Add temporary account + toggle Shared Chats (NTFS junctions)'
    Invoke-CliOk @('add', 'QA Bot', '--subtitle', 'qa.bot@gmail.com', '--tier', 'pro') | Out-Null
    $qaAg = Join-Path $SandboxDir 'data\Profiles\QA Bot\.gemini\antigravity'
    if (-not (Test-Path -LiteralPath $qaAg)) {
        $qaAg = Get-ChildItem -LiteralPath (Join-Path $SandboxDir 'data\Profiles') -Directory |
            Where-Object { $_.Name -like 'QA*' } |
            ForEach-Object { Join-Path $_.FullName '.gemini\antigravity' } |
            Select-Object -First 1
    }
    Invoke-CliOk @('share', 'qa-bot', '--enabled=false') | Out-Null
    if ($qaAg -and (Test-Path -LiteralPath $qaAg) -and ((Get-Item -LiteralPath $qaAg -Force).Attributes -band [IO.FileAttributes]::ReparsePoint)) {
        throw 'FAIL: Shared Chats disabled but .gemini\antigravity is still a junction'
    }
    Invoke-CliOk @('share', 'qa-bot', '--enabled=true') | Out-Null
    if (-not $qaAg -or -not ((Get-Item -LiteralPath $qaAg -Force).Attributes -band [IO.FileAttributes]::ReparsePoint)) {
        throw "FAIL: Shared Chats enabled but '$qaAg' is not an NTFS junction"
    }
    Write-Host "PASS: Shared Chats junction created at '$qaAg'"

    Write-Host ''
    Write-Host '-> [3.7] Rule R4 Guard: delete --delete-disk WITHOUT --yes must be BLOCKED'
    $r = Invoke-Cli @('delete', 'qa-bot', '--delete-disk')
    if ($r.Code -eq 0) { throw 'FAIL: Rule R4 should have blocked unconfirmed destructive delete!' }
    Write-Host 'PASS: Rule R4 blocked unconfirmed destructive delete'

    Write-Host '-> [3.8] Rule R4 Confirmed: delete --delete-disk --yes must SUCCEED'
    $fakeHomeAg = Join-Path $SandboxDir 'fake_home\.gemini\antigravity'
    Invoke-CliOk @('delete', 'qa-bot', '--delete-disk', '--yes') | Out-Null
    if (-not (Test-Path -LiteralPath $fakeHomeAg)) {
        throw 'FAIL: deleting the QA vault removed the shared primary .gemini\antigravity target!'
    }
    Write-Host 'PASS: vault deleted, shared primary target preserved'

    Write-Host '-> [3.9] Rule R1 Guard: deleting primary OS profile disk must ALWAYS be BLOCKED'
    $r = Invoke-Cli @('delete', 'default', '--delete-disk', '--yes')
    if ($r.Code -eq 0) { throw 'FAIL: Rule R1 should have blocked deleting primary OS profile!' }
    Write-Host 'PASS: Rule R1 protected primary OS profile'

    Write-Host ''
    Write-Host '-> [3.10] Centralized Audit Trail (latest 10)'
    Invoke-CliOk @('audit', '--limit', '10') | Out-Null

    Write-Section 'ALL SAFE E2E CHECKS PASSED!'
    Write-Host 'To test the interactive GUI safely without closing Antigravity:'
    Write-Host '  cargo run -p agswitch-desktop -- --sandbox'
}
finally {
    if (Test-Path -LiteralPath $SandboxDir) {
        # Every sandbox junction targets fake_home INSIDE $SandboxDir, so even if an older
        # PowerShell followed one during recursive removal, nothing outside the sandbox is touched.
        Remove-Item -LiteralPath $SandboxDir -Recurse -Force -ErrorAction SilentlyContinue
    }
}

if ($Ui) {
    Write-Host ''
    Write-Host 'Launching AGY Swap Desktop in SANDBOX mode...'
    & (Join-Path $RootDir 'target\debug\agswitch-desktop.exe') --sandbox
}
