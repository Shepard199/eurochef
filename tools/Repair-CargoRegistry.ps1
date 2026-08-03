[CmdletBinding()]
param(
    [string]$Root = (Split-Path -Parent $PSScriptRoot),
    [switch]$Quiet,
    [switch]$ForceFullScan
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

$Root = (Resolve-Path -LiteralPath $Root -ErrorAction Stop).ProviderPath.TrimEnd('\')
$CargoHome = Join-Path $Root '.cargo-local'
$RegistryRoot = Join-Path $CargoHome 'registry'
$SourceRoot = Join-Path $RegistryRoot 'src'
$CacheRoot = Join-Path $RegistryRoot 'cache'
$HealthPath = Join-Path $RegistryRoot '.eurochef-registry-health.json'
$LockPath = Join-Path $Root 'Cargo.lock'
$ProjectRoot = [IO.Path]::GetFullPath((Join-Path $Root '..\..'))
$FallbackCacheRoot = Join-Path $ProjectRoot 'CodexPro-Up\state\profile\.cargo\registry\cache'

function Read-StreamExactly {
    param(
        [Parameter(Mandatory = $true)][IO.Stream]$Stream,
        [Parameter(Mandatory = $true)][int]$Length
    )
    $Buffer = [byte[]]::new($Length)
    $Offset = 0
    while ($Offset -lt $Length) {
        $Read = $Stream.Read($Buffer, $Offset, $Length - $Offset)
        if ($Read -le 0) { break }
        $Offset += $Read
    }
    if ($Offset -eq 0) { return $null }
    if ($Offset -ne $Length) { throw "Unexpected end of .crate archive: wanted $Length bytes, got $Offset." }
    return $Buffer
}

function Skip-StreamExactly {
    param(
        [Parameter(Mandatory = $true)][IO.Stream]$Stream,
        [Parameter(Mandatory = $true)][long]$Length
    )
    $Buffer = [byte[]]::new(65536)
    $Remaining = $Length
    while ($Remaining -gt 0) {
        $Wanted = [int][Math]::Min([long]$Buffer.Length, $Remaining)
        $Read = $Stream.Read($Buffer, 0, $Wanted)
        if ($Read -le 0) { throw "Unexpected end of .crate archive while skipping $Length bytes." }
        $Remaining -= $Read
    }
}

function Get-TarString {
    param([byte[]]$Header, [int]$Offset, [int]$Length)
    return [Text.Encoding]::UTF8.GetString($Header, $Offset, $Length).Trim([char]0).Trim()
}

function Get-TarOctal {
    param([byte[]]$Header, [int]$Offset, [int]$Length)
    $Text = [Text.Encoding]::ASCII.GetString($Header, $Offset, $Length).Trim([char]0).Trim()
    if ([string]::IsNullOrWhiteSpace($Text)) { return [long]0 }
    return [Convert]::ToInt64($Text, 8)
}

function Get-CrateArchiveFiles {
    param([Parameter(Mandatory = $true)][string]$ArchivePath)

    $Files = New-Object 'System.Collections.Generic.List[string]'
    $FileStream = [IO.File]::Open($ArchivePath, [IO.FileMode]::Open, [IO.FileAccess]::Read, [IO.FileShare]::Read)
    try {
        $Gzip = [IO.Compression.GZipStream]::new($FileStream, [IO.Compression.CompressionMode]::Decompress, $false)
        try {
            $PendingPath = $null
            while ($true) {
                $Header = Read-StreamExactly -Stream $Gzip -Length 512
                if ($null -eq $Header) { break }
                $NonZero = $false
                foreach ($Byte in $Header) {
                    if ($Byte -ne 0) { $NonZero = $true; break }
                }
                if (-not $NonZero) { break }

                $Name = Get-TarString -Header $Header -Offset 0 -Length 100
                $Prefix = Get-TarString -Header $Header -Offset 345 -Length 155
                if ($Prefix) { $Name = "$Prefix/$Name" }
                $Size = Get-TarOctal -Header $Header -Offset 124 -Length 12
                $TypeFlag = [char]$Header[156]
                $PaddedSize = [long]([Math]::Ceiling($Size / 512.0) * 512)

                if ($TypeFlag -eq 'L' -or $TypeFlag -eq 'x') {
                    $Payload = if ($Size -gt 0) { Read-StreamExactly -Stream $Gzip -Length ([int]$Size) } else { [byte[]]::new(0) }
                    if ($PaddedSize -gt $Size) { Skip-StreamExactly -Stream $Gzip -Length ($PaddedSize - $Size) }
                    $PayloadText = [Text.Encoding]::UTF8.GetString($Payload).Trim([char]0)
                    if ($TypeFlag -eq 'L') {
                        $PendingPath = $PayloadText.Trim()
                    }
                    else {
                        $PathMatch = [regex]::Match($PayloadText, '(?m)(?:^|\n)\d+ path=([^\n]+)')
                        if ($PathMatch.Success) { $PendingPath = $PathMatch.Groups[1].Value.Trim() }
                    }
                    continue
                }

                $EffectiveName = if ($PendingPath) { $PendingPath } else { $Name }
                $PendingPath = $null
                if ($TypeFlag -eq [char]0 -or $TypeFlag -eq '0') {
                    $Normalized = $EffectiveName.Replace('\', '/')
                    $Slash = $Normalized.IndexOf('/')
                    if ($Slash -ge 0 -and $Slash + 1 -lt $Normalized.Length) {
                        $Normalized = $Normalized.Substring($Slash + 1)
                    }
                    if ($Normalized) { $Files.Add($Normalized) }
                }
                if ($PaddedSize -gt 0) { Skip-StreamExactly -Stream $Gzip -Length $PaddedSize }
            }
        }
        finally {
            $Gzip.Dispose()
        }
    }
    finally {
        $FileStream.Dispose()
    }
    return @($Files)
}

New-Item -ItemType Directory -Force -Path $CargoHome, $RegistryRoot, $CacheRoot | Out-Null
$SyncedArchives = 0
if (Test-Path -LiteralPath $FallbackCacheRoot -PathType Container) {
    foreach ($FallbackIndex in @(Get-ChildItem -LiteralPath $FallbackCacheRoot -Directory -Force -ErrorAction SilentlyContinue)) {
        $LocalIndex = Join-Path $CacheRoot $FallbackIndex.Name
        New-Item -ItemType Directory -Force -Path $LocalIndex | Out-Null
        foreach ($Archive in @(Get-ChildItem -LiteralPath $FallbackIndex.FullName -File -Filter '*.crate' -Force -ErrorAction SilentlyContinue)) {
            $Destination = Join-Path $LocalIndex $Archive.Name
            if (-not (Test-Path -LiteralPath $Destination -PathType Leaf)) {
                Copy-Item -LiteralPath $Archive.FullName -Destination $Destination
                $SyncedArchives++
            }
        }
    }
    if ($SyncedArchives -gt 0 -and -not $Quiet) {
        Write-Host "Cargo registry cache: synchronized $SyncedArchives missing archive(s) from portable cache" -ForegroundColor DarkGray
    }
}

if (-not (Test-Path -LiteralPath $SourceRoot -PathType Container)) {
    return [PSCustomObject]@{
        Scanned = $false
        Healthy = $true
        RemovedPackages = @()
        MissingFiles = 0
        Reason = 'registry source directory is not populated yet'
    }
}

$LockHash = if (Test-Path -LiteralPath $LockPath -PathType Leaf) {
    (Get-FileHash -LiteralPath $LockPath -Algorithm SHA256).Hash.ToLowerInvariant()
}
else { '' }
$CurrentFileCount = @(Get-ChildItem -LiteralPath $SourceRoot -Recurse -File -Force -ErrorAction Stop).Count

if (-not $ForceFullScan -and (Test-Path -LiteralPath $HealthPath -PathType Leaf)) {
    try {
        $Health = Get-Content -LiteralPath $HealthPath -Raw | ConvertFrom-Json
        if ([int]$Health.schema -eq 2 -and
            [string]$Health.lockSha256 -eq $LockHash -and
            [int]$Health.sourceFileCount -eq $CurrentFileCount) {
            return [PSCustomObject]@{
                Scanned = $false
                Healthy = $true
                RemovedPackages = @()
                MissingFiles = 0
                Reason = 'health marker and registry file count match'
            }
        }
    }
    catch {
        if (-not $Quiet) { Write-Warning "Ignoring invalid Cargo registry health marker: $HealthPath" }
    }
}

$RemovedPackages = New-Object 'System.Collections.Generic.List[string]'
$MissingFileCount = 0
$PackageCount = 0
$IndexDirectories = @(Get-ChildItem -LiteralPath $SourceRoot -Directory -Force -ErrorAction Stop)
foreach ($IndexDirectory in $IndexDirectories) {
    $CacheDirectory = Join-Path $CacheRoot $IndexDirectory.Name
    foreach ($PackageDirectory in @(Get-ChildItem -LiteralPath $IndexDirectory.FullName -Directory -Force -ErrorAction Stop)) {
        $PackageCount++
        $ArchivePath = Join-Path $CacheDirectory ($PackageDirectory.Name + '.crate')
        if (-not (Test-Path -LiteralPath $ArchivePath -PathType Leaf)) {
            throw "Cargo source package has no cached archive: $($PackageDirectory.FullName)"
        }

        $Missing = New-Object 'System.Collections.Generic.List[string]'
        foreach ($RelativePath in @(Get-CrateArchiveFiles -ArchivePath $ArchivePath)) {
            $ExpectedPath = Join-Path $PackageDirectory.FullName $RelativePath
            if (-not (Test-Path -LiteralPath $ExpectedPath -PathType Leaf)) {
                $Missing.Add($RelativePath)
            }
        }
        if ($Missing.Count -eq 0) { continue }

        $MissingFileCount += $Missing.Count
        if (-not $Quiet) {
            $Preview = @($Missing | Select-Object -First 4) -join ', '
            if ($Missing.Count -gt 4) { $Preview += ', ...' }
            Write-Warning "Removing incomplete Cargo package [$($PackageDirectory.Name)]: missing $($Missing.Count) file(s): $Preview"
        }
        Remove-Item -LiteralPath $PackageDirectory.FullName -Recurse -Force
        $RemovedPackages.Add($PackageDirectory.Name)
    }
}

if ($RemovedPackages.Count -gt 0) {
    Remove-Item -LiteralPath $HealthPath -Force -ErrorAction SilentlyContinue
    return [PSCustomObject]@{
        Scanned = $true
        Healthy = $false
        RemovedPackages = @($RemovedPackages)
        MissingFiles = $MissingFileCount
        Reason = 'incomplete package directories removed; Cargo will re-extract their cached archives'
    }
}

$FinalFileCount = @(Get-ChildItem -LiteralPath $SourceRoot -Recurse -File -Force -ErrorAction Stop).Count
$HealthRecord = [ordered]@{
    schema = 2
    lockSha256 = $LockHash
    packageCount = $PackageCount
    sourceFileCount = $FinalFileCount
    checkedAtUtc = [DateTime]::UtcNow.ToString('o')
}
$IncomingPath = "$HealthPath.incoming"
$HealthRecord | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath $IncomingPath -Encoding UTF8
Move-Item -LiteralPath $IncomingPath -Destination $HealthPath -Force

if (-not $Quiet) {
    Write-Host "Cargo registry: healthy ($PackageCount packages, $FinalFileCount files)" -ForegroundColor DarkGray
}
return [PSCustomObject]@{
    Scanned = $true
    Healthy = $true
    RemovedPackages = @()
    MissingFiles = 0
    Reason = 'full .crate archive file-presence scan passed'
}
