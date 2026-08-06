[CmdletBinding()]
param(
    [string]$FbxSdkRoot = $env:FBX_SDK_ROOT,
    [ValidateSet('Release', 'Debug')]
    [string]$Configuration = 'Release'
)

$ErrorActionPreference = 'Stop'
$projectRoot = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path

if ([string]::IsNullOrWhiteSpace($FbxSdkRoot)) {
    $workspaceSdkRoot = Join-Path (Split-Path $projectRoot -Parent) 'FBX SDK'
    if (Test-Path -LiteralPath $workspaceSdkRoot) {
        $FbxSdkRoot = $workspaceSdkRoot
    } else {
        throw 'FBX_SDK_ROOT is not set and _tools\FBX SDK was not found.'
    }
}

$FbxSdkRoot = (Resolve-Path $FbxSdkRoot).Path
if (-not (Test-Path (Join-Path $FbxSdkRoot 'include\fbxsdk.h'))) {
    $versionRoots = Get-ChildItem -LiteralPath $FbxSdkRoot -Directory -ErrorAction SilentlyContinue |
        Where-Object { Test-Path (Join-Path $_.FullName 'include\fbxsdk.h') } |
        Sort-Object { try { [version]$_.Name } catch { [version]'0.0' } } -Descending
    if ($versionRoots.Count -eq 0) {
        throw "fbxsdk.h was not found below $FbxSdkRoot\include or a versioned child directory"
    }
    $FbxSdkRoot = $versionRoots[0].FullName
}

$buildRoot = Join-Path $projectRoot 'target\fbx-exporter-build'
$outputRoot = Join-Path $projectRoot ("target\{0}\tools\fbx" -f $Configuration.ToLowerInvariant())
New-Item -ItemType Directory -Force -Path $buildRoot, $outputRoot | Out-Null

& cmake.exe -S $PSScriptRoot -B $buildRoot -A x64 "-DFBX_SDK_ROOT=$FbxSdkRoot"
if ($LASTEXITCODE -ne 0) {
    throw "CMake configure failed with exit code $LASTEXITCODE"
}

& cmake.exe --build $buildRoot --config $Configuration --target fbx_export_helper
if ($LASTEXITCODE -ne 0) {
    throw "FBX helper build failed with exit code $LASTEXITCODE"
}

$builtExe = Get-ChildItem -Path $buildRoot -Filter 'fbx_export_helper.exe' -Recurse -File |
    Where-Object { $_.FullName -match [regex]::Escape("\$Configuration\") } |
    Select-Object -First 1
if (-not $builtExe) {
    $builtExe = Get-ChildItem -Path $buildRoot -Filter 'fbx_export_helper.exe' -Recurse -File | Select-Object -First 1
}
if (-not $builtExe) {
    throw 'CMake reported success, but fbx_export_helper.exe was not found.'
}

Copy-Item -Force $builtExe.FullName (Join-Path $outputRoot 'fbx_export_helper.exe')

$runtimeDlls = Get-ChildItem -Path $FbxSdkRoot -Filter '*.dll' -Recurse -File |
    Where-Object {
        $_.Name -match '^(libfbxsdk|libxml2|zlib).*\.dll$' -and
        $_.FullName -match '\\x64\\' -and
        $_.FullName -match '\\release\\'
    }
foreach ($dll in $runtimeDlls) {
    Copy-Item -Force $dll.FullName (Join-Path $outputRoot $dll.Name)
}

Write-Host "FBX exporter: $(Join-Path $outputRoot 'fbx_export_helper.exe')"
Write-Host "Autodesk FBX SDK: $FbxSdkRoot"
