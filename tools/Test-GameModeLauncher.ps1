param()
# Static launcher contract checks. Remote automation may disallow spawning cmd.exe;
# these assertions do not pretend to exercise the interactive Windows batch UI.
$ErrorActionPreference = 'Stop'
$root = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..\..\..'))
$launcher = Join-Path $root 'RUN_GAME_MODE.cmd'
$quickLauncher = Join-Path $root 'RUN_GAME_MODE_FAST.cmd'
$guiCmd = Join-Path $root '_tools\eurochef-main_legacy\RUN_GUI.cmd'
$rustMain = Join-Path $root '_tools\eurochef-main_legacy\eurochef\gui\src\main.rs'
$gameLevel = Join-Path $root '_tools\eurochef-main_legacy\target\attachment_game_root\_eurotools_out\extracted_main\robots\binary\_bin_pc\m01_vill.edb'
foreach ($path in @($launcher, $quickLauncher, $guiCmd, $rustMain, $gameLevel)) {
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) { throw "Required launcher fixture not found: $path" }
}
$batch = [IO.File]::ReadAllText($launcher)
$gui = [IO.File]::ReadAllText($guiCmd)
$main = [IO.File]::ReadAllText($rustMain)
function Assert-Contains([string] $text, [string] $fragment) {
    if (-not $text.Contains($fragment)) { throw "Missing required launcher contract: $fragment" }
}
foreach ($fragment in @('/fast','/trace','/choose','/dry-run','/help','--play',
    'RUN_GUI.cmd','target\release\eurochef.exe','target\debug\eurochef.exe',
    'm01_vill.edb','ROBOTS_EDB_MANIFEST','if not exist "%MAP%"')) {
    Assert-Contains $batch $fragment
}
Assert-Contains $gui 'RUN_GUI.ps1'
Assert-Contains $main 'play: bool'
Assert-Contains $main 'args.play'
$quickBatch = [IO.File]::ReadAllText($quickLauncher)
Assert-Contains $quickBatch 'RUN_GAME_MODE.cmd'
Assert-Contains $quickBatch '/fast /trace'
Assert-Contains $batch 'if "%DRY_RUN%"=="1"'
Assert-Contains $batch 'if "%FAST%"=="1"'
Write-Host 'PASS: Game Mode launcher static contract, CLI --play, fixture EDB, build/fast modes.'
Write-Host 'NOTE: Actual cmd.exe /dry-run and GUI smoke test still require an interactive Windows shell.'
