param(
    [switch]$Trace,
    [switch]$BuildOnly,

    [Parameter(ValueFromRemainingArguments = $true)]
    [string[]]$EuroChefArgs
)

$ErrorActionPreference = 'Stop'
$root = $PSScriptRoot
$cargoEnvironmentScript = Join-Path $root 'tools\Initialize-CargoEnvironment.ps1'
if (-not (Test-Path -LiteralPath $cargoEnvironmentScript -PathType Leaf)) {
    throw "Cargo environment initializer is missing: $cargoEnvironmentScript"
}
$cargoContext = & $cargoEnvironmentScript -Root $root
$cargo = [string]$cargoContext.Cargo
$targetDirectory = [string]$cargoContext.TargetDirectory

function Set-RobotsManifestEnvironment {
    $projectRoot = [IO.Path]::GetFullPath((Join-Path $root '..\..'))
    $envFile = Join-Path $projectRoot '.env'
    $gameRoot = 'D:\Games\Robots'

    if (Test-Path -LiteralPath $envFile -PathType Leaf) {
        foreach ($line in [IO.File]::ReadAllLines($envFile)) {
            if ($line -match '^\s*GAME_ROOT=(.+?)\s*$') {
                $gameRoot = $Matches[1].Trim().Trim('"')
                break
            }
        }
    }

    $manifest = Join-Path $gameRoot '_eurotools_out\edb\manifest.tsv'
    if (Test-Path -LiteralPath $manifest -PathType Leaf) {
        $env:ROBOTS_EDB_MANIFEST = [IO.Path]::GetFullPath($manifest)
        Write-Host "Robots EDB manifest: $env:ROBOTS_EDB_MANIFEST" -ForegroundColor DarkGray
    }
    else {
        Remove-Item Env:ROBOTS_EDB_MANIFEST -ErrorAction SilentlyContinue
        Write-Warning "Robots EDB manifest not found: $manifest"
    }
}

Set-RobotsManifestEnvironment

$profile = if ($Trace) { 'debug' } else { 'release' }
$env:RUST_BACKTRACE = if ($Trace) { 'full' } else { '1' }
$env:RUST_LOG = if ($Trace) { 'debug' } else { 'info' }

Write-Host "EuroChef legacy root: $root" -ForegroundColor Cyan
Write-Host "Cargo: $cargo" -ForegroundColor DarkGray
Write-Host "Cargo target: $env:CARGO_TARGET_DIR" -ForegroundColor DarkGray
Write-Host "Mode: $profile" -ForegroundColor DarkGray

$buildArgs = @('build', '--locked', '-p', 'eurochef-gui', '--bin', 'eurochef')
if (-not $Trace) {
    $buildArgs += '--release'
}

Write-Host "Building EuroChef legacy $profile GUI..." -ForegroundColor Cyan
$buildProcess = Start-Process -FilePath $cargo -ArgumentList $buildArgs -WorkingDirectory $root -NoNewWindow -Wait -PassThru
if ($buildProcess.ExitCode -ne 0) {
    throw "EuroChef legacy $profile build failed with exit code $($buildProcess.ExitCode)"
}

$exe = Join-Path $targetDirectory "$profile\eurochef.exe"
if (-not (Test-Path -LiteralPath $exe -PathType Leaf)) {
    throw "EuroChef legacy executable not found: $exe"
}

if ($BuildOnly) {
    Write-Host "EuroChef legacy $profile build is ready: $exe" -ForegroundColor Green
    exit 0
}

function ConvertTo-WindowsCommandLineArgument([string]$Argument) {
    if ($null -eq $Argument -or $Argument.Length -eq 0) {
        return '""'
    }
    if ($Argument -notmatch '[\s"]') {
        return $Argument
    }

    $builder = New-Object System.Text.StringBuilder
    [void]$builder.Append('"')
    $backslashes = 0

    foreach ($character in $Argument.ToCharArray()) {
        if ($character -eq '\') {
            $backslashes++
            continue
        }

        if ($character -eq '"') {
            [void]$builder.Append(('\' * (($backslashes * 2) + 1)))
            [void]$builder.Append('"')
            $backslashes = 0
            continue
        }

        if ($backslashes -gt 0) {
            [void]$builder.Append(('\' * $backslashes))
            $backslashes = 0
        }
        [void]$builder.Append($character)
    }

    if ($backslashes -gt 0) {
        [void]$builder.Append(('\' * ($backslashes * 2)))
    }
    [void]$builder.Append('"')
    return $builder.ToString()
}

$processInfo = New-Object System.Diagnostics.ProcessStartInfo
$processInfo.FileName = $exe
$processInfo.WorkingDirectory = $root
$processInfo.UseShellExecute = $false
$processInfo.Arguments = (@($EuroChefArgs | ForEach-Object { ConvertTo-WindowsCommandLineArgument $_ }) -join ' ')

Write-Host "Launching EuroChef legacy: $exe" -ForegroundColor Green
$appProcess = [System.Diagnostics.Process]::Start($processInfo)
if ($null -eq $appProcess) {
    throw "Failed to start EuroChef legacy executable: $exe"
}
$appProcess.WaitForExit()
exit $appProcess.ExitCode
