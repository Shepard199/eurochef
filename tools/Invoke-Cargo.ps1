[CmdletBinding()]
param(
    [Parameter(ValueFromRemainingArguments = $true)]
    [string[]]$CargoArgs
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

function Split-NativeCommandLine {
    param([Parameter(Mandatory = $true)][string]$CommandLine)

    $arguments = [System.Collections.Generic.List[string]]::new()
    foreach ($match in [regex]::Matches($CommandLine, '(?:"([^"]*)"|(\S+))')) {
        if ($match.Groups[1].Success) {
            [void]$arguments.Add($match.Groups[1].Value)
        }
        else {
            [void]$arguments.Add($match.Groups[2].Value)
        }
    }
    return $arguments.ToArray()
}

$Root = Split-Path -Parent $PSScriptRoot
$Initializer = Join-Path $PSScriptRoot 'Initialize-CargoEnvironment.ps1'
if (-not (Test-Path -LiteralPath $Initializer -PathType Leaf)) {
    throw "Cargo environment initializer is missing: $Initializer"
}

$Context = & $Initializer -Root $Root -Quiet
[string[]]$Arguments = @()
if (-not [string]::IsNullOrWhiteSpace($env:EUROCHEF_CARGO_COMMAND_LINE)) {
    $Arguments = @(Split-NativeCommandLine -CommandLine $env:EUROCHEF_CARGO_COMMAND_LINE)
}
else {
    $Arguments = @($CargoArgs)
}

if ($Arguments.Count -eq 0) {
    $Arguments = @('--version')
}
elseif ($Arguments[0] -in @('build', 'check', 'test', 'clippy', 'run', 'bench') -and
        $Arguments -notcontains '--locked' -and
        $Arguments -notcontains '--frozen') {
    if ($Arguments.Count -eq 1) {
        $Arguments = @($Arguments[0], '--locked')
    }
    else {
        $Arguments = @($Arguments[0], '--locked') + $Arguments[1..($Arguments.Count - 1)]
    }
}

$ProcessInfo = [Diagnostics.ProcessStartInfo]::new()
$ProcessInfo.FileName = [string]$Context.Cargo
$ProcessInfo.WorkingDirectory = $Root
$ProcessInfo.UseShellExecute = $false
foreach ($Argument in $Arguments) {
    [void]$ProcessInfo.ArgumentList.Add([string]$Argument)
}

$Process = [Diagnostics.Process]::Start($ProcessInfo)
if ($null -eq $Process) {
    throw "Failed to start canonical Cargo: $($Context.Cargo)"
}
$Process.WaitForExit()
exit $Process.ExitCode
