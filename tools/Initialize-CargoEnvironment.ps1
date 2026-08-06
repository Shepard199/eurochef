[CmdletBinding()]
param(
    [string]$Root = (Split-Path -Parent $PSScriptRoot),
    [switch]$Quiet
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

$Root = (Resolve-Path -LiteralPath $Root -ErrorAction Stop).ProviderPath.TrimEnd('\')
$ProjectRoot = [IO.Path]::GetFullPath((Join-Path $Root '..\..'))
$ToolchainFile = Join-Path $Root 'rust-toolchain.toml'
$TargetDirectory = [IO.Path]::GetFullPath((Join-Path $Root 'target'))
$PortableProfile = Join-Path $ProjectRoot 'CodexPro-Up\state\profile'
# Keep the crate registry private to this workspace. Sharing CodexPro's registry
# made cleanup/copy operations able to damage EuroChef builds and also changed
# dependency source paths when the GUI was launched outside CodexPro.
$CanonicalCargoHome = Join-Path $Root '.cargo-local'
$CanonicalRustupHome = Join-Path $PortableProfile '.rustup'
$HostTriple = 'x86_64-pc-windows-msvc'

if (-not (Test-Path -LiteralPath $ToolchainFile -PathType Leaf)) {
    throw "Rust toolchain file is missing: $ToolchainFile"
}

$ToolchainText = [IO.File]::ReadAllText($ToolchainFile)
if ($ToolchainText -notmatch '(?m)^\s*channel\s*=\s*"([^"]+)"\s*$') {
    throw "Unable to resolve Rust channel from: $ToolchainFile"
}
$Channel = $Matches[1]
$ToolchainName = "$Channel-$HostTriple"
$ToolchainDirectory = Join-Path $CanonicalRustupHome ("toolchains\{0}" -f $ToolchainName)
$ToolchainBin = Join-Path $ToolchainDirectory 'bin'
$CargoExe = Join-Path $ToolchainBin 'cargo.exe'
$RustcExe = Join-Path $ToolchainBin 'rustc.exe'
$RustdocExe = Join-Path $ToolchainBin 'rustdoc.exe'
$RustfmtExe = Join-Path $ToolchainBin 'rustfmt.exe'
$CargoFmtExe = Join-Path $ToolchainBin 'cargo-fmt.exe'

New-Item -ItemType Directory -Force -Path $CanonicalCargoHome, $TargetDirectory | Out-Null
foreach ($required in @($CanonicalRustupHome, $CargoExe, $RustcExe, $RustdocExe)) {
    if (-not (Test-Path -LiteralPath $required)) {
        throw "Canonical portable Rust component is missing: $required"
    }
}

if (-not (Test-Path -LiteralPath $RustfmtExe -PathType Leaf) -or
    -not (Test-Path -LiteralPath $CargoFmtExe -PathType Leaf)) {
    $RustupCommand = Get-Command rustup.exe -ErrorAction SilentlyContinue
    $RustupExe = if ($null -ne $RustupCommand) {
        $RustupCommand.Source
    }
    elseif (-not [string]::IsNullOrWhiteSpace($env:USERPROFILE)) {
        Join-Path $env:USERPROFILE '.cargo\bin\rustup.exe'
    }
    else {
        $null
    }

    if ([string]::IsNullOrWhiteSpace($RustupExe) -or
        -not (Test-Path -LiteralPath $RustupExe -PathType Leaf)) {
        throw "rustfmt is required by $ToolchainFile, but rustup.exe was not found to repair $ToolchainName"
    }

    $PreviousCargoHome = $env:CARGO_HOME
    $PreviousRustupHome = $env:RUSTUP_HOME
    try {
        $env:CARGO_HOME = $CanonicalCargoHome
        $env:RUSTUP_HOME = $CanonicalRustupHome
        if (-not $Quiet) {
            Write-Host "Installing missing rustfmt component for $ToolchainName..." -ForegroundColor DarkGray
        }
        $RustupProcess = Start-Process -FilePath $RustupExe `
            -ArgumentList @('component', 'add', '--toolchain', $ToolchainName, 'rustfmt') `
            -WorkingDirectory $Root -NoNewWindow -Wait -PassThru
        if ($RustupProcess.ExitCode -ne 0) {
            throw "rustup failed to install rustfmt for $ToolchainName (exit $($RustupProcess.ExitCode))"
        }
    }
    finally {
        if ($null -eq $PreviousCargoHome) {
            Remove-Item Env:CARGO_HOME -ErrorAction SilentlyContinue
        }
        else {
            $env:CARGO_HOME = $PreviousCargoHome
        }
        if ($null -eq $PreviousRustupHome) {
            Remove-Item Env:RUSTUP_HOME -ErrorAction SilentlyContinue
        }
        else {
            $env:RUSTUP_HOME = $PreviousRustupHome
        }
    }

    foreach ($required in @($RustfmtExe, $CargoFmtExe)) {
        if (-not (Test-Path -LiteralPath $required -PathType Leaf)) {
            throw "rustup completed, but the canonical rustfmt component is still missing: $required"
        }
    }
}

# Cargo fingerprints include registry source paths, compiler paths, wrappers and
# rustflags. Inheriting any of these from CodexPro, a terminal, or a system Cargo
# installation makes the same dependency look like a different crate and causes
# a full rebuild. The legacy workspace therefore owns one deterministic context.
$VariablesToClear = @(
    'RUSTFLAGS',
    'RUSTDOCFLAGS',
    'CARGO_ENCODED_RUSTFLAGS',
    'CARGO_BUILD_RUSTFLAGS',
    'CARGO_INCREMENTAL',
    'RUSTC_WRAPPER',
    'RUSTC_WORKSPACE_WRAPPER',
    'CARGO_BUILD_TARGET'
)
foreach ($name in $VariablesToClear) {
    Remove-Item "Env:$name" -ErrorAction SilentlyContinue
}

foreach ($entry in @(Get-ChildItem Env:)) {
    if ($entry.Name -like 'CARGO_PROFILE_*' -or
        $entry.Name -like 'CARGO_TARGET_*_RUSTFLAGS' -or
        $entry.Name -like 'CARGO_TARGET_*_LINKER') {
        Remove-Item ("Env:{0}" -f $entry.Name) -ErrorAction SilentlyContinue
    }
}

$env:CARGO_HOME = $CanonicalCargoHome
$env:RUSTUP_HOME = $CanonicalRustupHome
$env:RUSTUP_TOOLCHAIN = $ToolchainName
$env:CARGO_TARGET_DIR = $TargetDirectory
$env:RUSTC = $RustcExe
$env:RUSTDOC = $RustdocExe

# Release builds deliberately do not use incremental compilation. Cargo never
# garbage-collects old incremental generations, so keeping it enabled for this
# large GUI made target\release\incremental grow without a useful bound.
$LegacyReleaseIncremental = Join-Path $TargetDirectory 'release\incremental'
if (Test-Path -LiteralPath $LegacyReleaseIncremental -PathType Container) {
    try {
        Remove-Item -LiteralPath $LegacyReleaseIncremental -Recurse -Force -ErrorAction Stop
        if (-not $Quiet) {
            Write-Host "Removed obsolete release incremental cache: $LegacyReleaseIncremental" -ForegroundColor DarkGray
        }
    }
    catch {
        Write-Warning "Could not remove obsolete release incremental cache: $LegacyReleaseIncremental. $($_.Exception.Message)"
    }
}

$PathParts = New-Object 'System.Collections.Generic.List[string]'
foreach ($candidate in @($ToolchainBin, (Join-Path $CanonicalCargoHome 'bin'))) {
    if ((Test-Path -LiteralPath $candidate -PathType Container) -and -not $PathParts.Contains($candidate)) {
        $PathParts.Add($candidate)
    }
}
foreach ($part in @($env:PATH -split ';')) {
    if ([string]::IsNullOrWhiteSpace($part)) { continue }
    if (-not $PathParts.Contains($part)) { $PathParts.Add($part) }
}
$env:PATH = $PathParts -join ';'

$RegistryRepairScript = Join-Path $PSScriptRoot 'Repair-CargoRegistry.ps1'
if (-not (Test-Path -LiteralPath $RegistryRepairScript -PathType Leaf)) {
    throw "Cargo registry repair helper is missing: $RegistryRepairScript"
}
$RegistryHealth = & $RegistryRepairScript -Root $Root -Quiet
if (-not $RegistryHealth.Healthy -and -not $Quiet) {
    Write-Warning "Cargo registry repair removed $(@($RegistryHealth.RemovedPackages).Count) incomplete package(s); cached archives will be re-extracted."
}

$RustcVersionOut = Join-Path $TargetDirectory '.rustc-version.stdout.tmp'
$RustcVersionErr = Join-Path $TargetDirectory '.rustc-version.stderr.tmp'
Remove-Item -LiteralPath $RustcVersionOut, $RustcVersionErr -Force -ErrorAction SilentlyContinue
try {
    $RustcVersionProcess = Start-Process -FilePath $RustcExe -ArgumentList @('-vV') `
        -WorkingDirectory $Root -RedirectStandardOutput $RustcVersionOut `
        -RedirectStandardError $RustcVersionErr -NoNewWindow -Wait -PassThru
    if ($RustcVersionProcess.ExitCode -ne 0) {
        $RustcErrorText = if (Test-Path -LiteralPath $RustcVersionErr) {
            [IO.File]::ReadAllText($RustcVersionErr).Trim()
        }
        else { '' }
        throw "Unable to query canonical rustc (exit $($RustcVersionProcess.ExitCode)): $RustcExe $RustcErrorText"
    }
    $RustcVersionLines = @([IO.File]::ReadAllLines($RustcVersionOut))
}
finally {
    Remove-Item -LiteralPath $RustcVersionOut, $RustcVersionErr -Force -ErrorAction SilentlyContinue
}
if ($RustcVersionLines.Count -eq 0 -or [string]::IsNullOrWhiteSpace($RustcVersionLines[0])) {
    throw "Canonical rustc returned an empty version: $RustcExe"
}
$RustcVersion = [string]$RustcVersionLines[0]

$Context = [ordered]@{
    schema = 2
    projectRoot = $Root
    cargoHome = $CanonicalCargoHome
    rustupHome = $CanonicalRustupHome
    toolchain = $ToolchainName
    cargo = $CargoExe
    rustc = $RustcExe
    rustcVersion = [string]$RustcVersion
    rustfmt = $RustfmtExe
    targetDirectory = $TargetDirectory
    incremental = [ordered]@{
        dev = $true
        release = $false
    }
    lockedDependencies = $true
}
$ContextJson = $Context | ConvertTo-Json -Depth 5
$ContextPath = Join-Path $TargetDirectory '.eurochef-cargo-context.json'
$PreviousContext = if (Test-Path -LiteralPath $ContextPath -PathType Leaf) {
    [IO.File]::ReadAllText($ContextPath)
}
else {
    ''
}
if ($PreviousContext -ne $ContextJson) {
    if ($PreviousContext -and -not $Quiet) {
        Write-Warning 'EuroChef Cargo build context changed. One dependency rebuild may be required; subsequent builds must reuse this context.'
    }
    $IncomingPath = "$ContextPath.incoming"
    [IO.File]::WriteAllText($IncomingPath, $ContextJson, [Text.UTF8Encoding]::new($false))
    Move-Item -LiteralPath $IncomingPath -Destination $ContextPath -Force
}

if (-not $Quiet) {
    Write-Host "Cargo context: canonical workspace" -ForegroundColor DarkGray
    Write-Host "Cargo home: $CanonicalCargoHome" -ForegroundColor DarkGray
    Write-Host "Rustup home: $CanonicalRustupHome" -ForegroundColor DarkGray
    Write-Host "Rust toolchain: $ToolchainName" -ForegroundColor DarkGray
}

return [PSCustomObject]@{
    Root = $Root
    ProjectRoot = $ProjectRoot
    Cargo = $CargoExe
    Rustc = $RustcExe
    Rustdoc = $RustdocExe
    Rustfmt = $RustfmtExe
    CargoFmt = $CargoFmtExe
    CargoHome = $CanonicalCargoHome
    RustupHome = $CanonicalRustupHome
    Toolchain = $ToolchainName
    TargetDirectory = $TargetDirectory
    ContextPath = $ContextPath
}
