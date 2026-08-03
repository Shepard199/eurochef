[CmdletBinding()]
param(
    [string]$Package = 'eurochef-filelist',
    [string]$SourceFile = 'eurochef-filelist\src\lib.rs'
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

$Root = Split-Path -Parent $PSScriptRoot
$Initializer = Join-Path $PSScriptRoot 'Initialize-CargoEnvironment.ps1'
$Context = & $Initializer -Root $Root -Quiet
$Cargo = [string]$Context.Cargo
$SourcePath = Join-Path $Root $SourceFile
if (-not (Test-Path -LiteralPath $SourcePath -PathType Leaf)) {
    throw "Cargo cache probe source is missing: $SourcePath"
}

function Invoke-ProbeBuild {
    param([Parameter(Mandatory = $true)][string]$Label)

    $ProcessInfo = [Diagnostics.ProcessStartInfo]::new()
    $ProcessInfo.FileName = $Cargo
    $ProcessInfo.WorkingDirectory = $Root
    $ProcessInfo.UseShellExecute = $false
    $ProcessInfo.RedirectStandardOutput = $true
    $ProcessInfo.RedirectStandardError = $true
    foreach ($argument in @('build', '--locked', '-p', $Package, '--release', '-vv')) {
        [void]$ProcessInfo.ArgumentList.Add($argument)
    }

    $Watch = [Diagnostics.Stopwatch]::StartNew()
    $Process = [Diagnostics.Process]::new()
    $Process.StartInfo = $ProcessInfo
    if (-not $Process.Start()) { throw "Failed to start Cargo cache probe: $Label" }
    $StdoutTask = $Process.StandardOutput.ReadToEndAsync()
    $StderrTask = $Process.StandardError.ReadToEndAsync()
    $Process.WaitForExit()
    $Stdout = $StdoutTask.GetAwaiter().GetResult()
    $Stderr = $StderrTask.GetAwaiter().GetResult()
    $Watch.Stop()

    $Combined = $Stdout + [Environment]::NewLine + $Stderr
    $DirtySourcePath = @([regex]::Matches($Combined, '(?m)^\s*Dirty\s+.+?:\s+the path to the source changed\s*$')).Count
    $CompileMatches = @([regex]::Matches($Combined, '(?m)^\s*Compiling\s+([^\s]+)\s+v?[^\r\n]*$'))
    $CompiledCrates = @($CompileMatches | ForEach-Object { $_.Groups[1].Value } | Sort-Object -Unique)
    $ExternalCrates = @($CompiledCrates | Where-Object { $_ -notlike 'eurochef-*' })

    [PSCustomObject]@{
        Label = $Label
        ExitCode = $Process.ExitCode
        Seconds = [Math]::Round($Watch.Elapsed.TotalSeconds, 2)
        DirtySourcePath = $DirtySourcePath
        CompiledCrates = $CompiledCrates
        ExternalCrates = $ExternalCrates
        Output = $Combined
    }
}

# Deliberately poison the inherited environment first. The initializer must
# replace it with the canonical portable context before Cargo starts.
$env:CARGO_HOME = 'C:\Users\Developer\.cargo'
$env:RUSTUP_HOME = 'C:\Users\Developer\.rustup'
$env:RUSTFLAGS = '-C debuginfo=1'
$Context = & $Initializer -Root $Root -Quiet
$Cargo = [string]$Context.Cargo
if ($env:CARGO_HOME -ne [string]$Context.CargoHome) { throw 'CARGO_HOME was not canonicalized.' }
if ($env:RUSTUP_HOME -ne [string]$Context.RustupHome) { throw 'RUSTUP_HOME was not canonicalized.' }
if (Test-Path Env:RUSTFLAGS) { throw 'RUSTFLAGS was not cleared.' }

$Warm = Invoke-ProbeBuild -Label 'warm'
if ($Warm.ExitCode -ne 0) {
    throw "Canonical warm build failed with exit code $($Warm.ExitCode).`n$($Warm.Output)"
}

$Repeat = Invoke-ProbeBuild -Label 'repeat'
if ($Repeat.ExitCode -ne 0) {
    throw "Canonical repeat build failed with exit code $($Repeat.ExitCode).`n$($Repeat.Output)"
}
if ($Repeat.DirtySourcePath -ne 0) {
    throw "Repeat build still invalidated $($Repeat.DirtySourcePath) dependencies because registry source paths changed."
}
if ($Repeat.ExternalCrates.Count -ne 0) {
    throw "Repeat build unexpectedly recompiled external crates: $($Repeat.ExternalCrates -join ', ')"
}

$OriginalBytes = [IO.File]::ReadAllBytes($SourcePath)
$OriginalTime = (Get-Item -LiteralPath $SourcePath).LastWriteTimeUtc
try {
    [IO.File]::AppendAllText($SourcePath, [Environment]::NewLine + '// cargo-cache-context-probe', [Text.UTF8Encoding]::new($false))
    $LocalEdit = Invoke-ProbeBuild -Label 'local-edit'
}
finally {
    [IO.File]::WriteAllBytes($SourcePath, $OriginalBytes)
    [IO.File]::SetLastWriteTimeUtc($SourcePath, $OriginalTime)
}

if ($LocalEdit.ExitCode -ne 0) {
    throw "Local source edit build failed with exit code $($LocalEdit.ExitCode).`n$($LocalEdit.Output)"
}
if ($LocalEdit.DirtySourcePath -ne 0) {
    throw "Local source edit invalidated registry source paths: $($LocalEdit.DirtySourcePath)"
}
if ($LocalEdit.ExternalCrates.Count -ne 0) {
    throw "Local source edit recompiled external crates: $($LocalEdit.ExternalCrates -join ', ')"
}

# Restore the build artifact to the actual source bytes. This should still touch
# only the edited workspace crate and never third-party dependencies.
$Restore = Invoke-ProbeBuild -Label 'restore'
if ($Restore.ExitCode -ne 0) {
    throw "Source restore build failed with exit code $($Restore.ExitCode).`n$($Restore.Output)"
}
if ($Restore.DirtySourcePath -ne 0 -or $Restore.ExternalCrates.Count -ne 0) {
    throw 'Restoring the source unexpectedly invalidated external dependencies.'
}

$Summary = [ordered]@{
    cargo = $Cargo
    cargoHome = $env:CARGO_HOME
    rustupHome = $env:RUSTUP_HOME
    target = $env:CARGO_TARGET_DIR
    package = $Package
    warmSeconds = $Warm.Seconds
    repeatSeconds = $Repeat.Seconds
    localEditSeconds = $LocalEdit.Seconds
    restoreSeconds = $Restore.Seconds
    repeatCompiled = @($Repeat.CompiledCrates)
    localEditCompiled = @($LocalEdit.CompiledCrates)
    externalRecompiled = 0
    sourcePathInvalidations = 0
}
$Summary | ConvertTo-Json -Depth 5
Write-Host '[OK] Cargo cache context is stable across inherited environments and local source edits.'
