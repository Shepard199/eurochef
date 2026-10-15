[CmdletBinding()]
param(
    [int]$Seconds = 4,
    [string]$LevelPath
)

$ErrorActionPreference = 'Stop'
$root = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$exe = Join-Path $root 'target\debug\eurochef.exe'
if (-not (Test-Path -LiteralPath $exe -PathType Leaf)) {
    throw "Debug EuroChef EXE missing: $exe"
}
if ([string]::IsNullOrWhiteSpace($LevelPath)) {
    $LevelPath = 'D:\Games\Robots\_eurotools_out\extracted_main\robots\binary\_bin_pc\m01_vill.edb'
    if (-not (Test-Path -LiteralPath $LevelPath -PathType Leaf)) {
        $LevelPath = Join-Path $root 'target\attachment_game_root\_eurotools_out\extracted_main\robots\binary\_bin_pc\m01_vill.edb'
    }
}
if (-not (Test-Path -LiteralPath $LevelPath -PathType Leaf)) {
    throw "Robots level EDB missing: $LevelPath"
}
if ($Seconds -lt 1 -or $Seconds -gt 20) { throw 'Seconds must be between 1 and 20' }

$psi = [Diagnostics.ProcessStartInfo]::new()
$psi.FileName = $exe
$psi.WorkingDirectory = $root
$psi.UseShellExecute = $false
[void]$psi.ArgumentList.Add([IO.Path]::GetFullPath($LevelPath))
[void]$psi.ArgumentList.Add('--play')
$psi.Environment['RUST_BACKTRACE'] = '1'
$psi.Environment['RUST_LOG'] = 'info'
$manifest = 'D:\Games\Robots\_eurotools_out\edb\manifest.tsv'
if (Test-Path -LiteralPath $manifest -PathType Leaf) {
    $psi.Environment['ROBOTS_EDB_MANIFEST'] = $manifest
}
Write-Host "Smoke test: ${exe} --play (checking for $Seconds seconds)"
$game = [Diagnostics.Process]::Start($psi)
if ($null -eq $game) { throw 'Game process did not start' }
try {
    Start-Sleep -Seconds $Seconds
    if ($game.HasExited) {
        throw "EuroChef exited during Game Mode startup with code $($game.ExitCode)"
    }
    Write-Host "PASS: Game Mode process alive after $Seconds seconds (PID $($game.Id))"
}
finally {
    if (-not $game.HasExited) {
        $game.Kill($true)
        $game.WaitForExit(5000) | Out-Null
    }
    $game.Dispose()
}
