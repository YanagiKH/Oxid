[CmdletBinding()]
param(
    [string]$Version = $env:OXID_VERSION,
    [string]$InstallDir = $env:OXID_INSTALL_DIR,
    [string]$Archive = $env:OXID_ARCHIVE
)

$ErrorActionPreference = "Stop"

function Test-OxidDestination {
    param([string]$Path)
    try {
        $attributes = [System.IO.File]::GetAttributes($Path)
    } catch [System.IO.FileNotFoundException] {
        return $false
    } catch [System.IO.DirectoryNotFoundException] {
        return $false
    }
    $unsupported = [System.IO.FileAttributes]::Directory -bor
        [System.IO.FileAttributes]::ReparsePoint -bor [System.IO.FileAttributes]::Device
    if ($attributes -band $unsupported) {
        throw "The install destination must be a regular file: $Path"
    }
    return $true
}

function Copy-OxidStream {
    param([System.IO.Stream]$Source, [System.IO.Stream]$Destination)
    $Source.CopyTo($Destination)
}

function Move-OxidExecutable {
    param([string]$Source, [string]$Destination, [string]$Backup, [bool]$Replace)
    if ($Replace -and $Backup) {
        [System.IO.File]::Replace($Source, $Destination, $Backup)
    } elseif ($Replace) {
        [System.IO.File]::Replace($Source, $Destination, [System.Management.Automation.Language.NullString]::Value)
    } else {
        [System.IO.File]::Move($Source, $Destination)
    }
}

function Install-OxidRelease {
    param([string]$Version, [string]$InstallDir, [string]$Archive)
    if (-not $Version) { $Version = "latest" }
    if (-not $InstallDir) { $InstallDir = Join-Path $env:LOCALAPPDATA "Oxid\bin" }
    $InstallDir = $ExecutionContext.SessionState.Path.GetUnresolvedProviderPathFromPSPath($InstallDir)
    $destination = Join-Path $InstallDir "oxid.exe"
    $null = Test-OxidDestination $destination

    $architecture = [System.Runtime.InteropServices.RuntimeInformation]::OSArchitecture.ToString().ToLowerInvariant()
    if ($architecture -eq "x64") { $architecture = "x86_64" }
    if ($architecture -ne "x86_64") { throw "Windows release binaries currently support x86_64. Build from source for $architecture." }

    $asset = "oxid-windows-$architecture.zip"
    $baseUrl = if ($Version -eq "latest") {
        "https://github.com/YanagiKH/Oxid/releases/latest/download"
    } else {
        "https://github.com/YanagiKH/Oxid/releases/download/$Version"
    }

    $tempDir = Join-Path ([System.IO.Path]::GetTempPath()) ("oxid-install-" + [guid]::NewGuid())
    $tempOwned = $false
    $stagingDir = $null
    $keepStaging = $false
    try {
        New-Item -ItemType Directory -Path $tempDir | Out-Null
        $tempOwned = $true
        $archivePath = Join-Path $tempDir $asset
        $checksumFile = "$archivePath.sha256"
        if ($Archive) {
            $localArchive = $ExecutionContext.SessionState.Path.GetUnresolvedProviderPathFromPSPath($Archive)
            [System.IO.File]::Copy($localArchive, $archivePath, $false)
            [System.IO.File]::Copy("$localArchive.sha256", $checksumFile, $false)
        } else {
            Invoke-WebRequest "$baseUrl/$asset" -OutFile $archivePath
            Invoke-WebRequest "$baseUrl/$asset.sha256" -OutFile $checksumFile
        }

        $checksumText = [string](Get-Content -LiteralPath $checksumFile -Raw)
        $expected = ($checksumText.Trim() -split "\s+")[0]
        if ($expected -cnotmatch '\A[0-9a-fA-F]{64}\z') { throw "Invalid SHA-256 checksum." }
        $actual = (Get-FileHash -LiteralPath $archivePath -Algorithm SHA256).Hash
        if ($expected -ine $actual) { throw "Checksum verification failed." }

        Add-Type -AssemblyName System.IO.Compression
        $archiveStream = $null
        $zip = $null
        try {
            $archiveStream = [System.IO.File]::OpenRead($archivePath)
            $zip = [System.IO.Compression.ZipArchive]::new(
                $archiveStream, [System.IO.Compression.ZipArchiveMode]::Read, $false)
            $binaries = @($zip.Entries | Where-Object {
                [string]::Equals($_.FullName, "oxid.exe", [System.StringComparison]::Ordinal)
            })
            if ($binaries.Count -ne 1) { throw "The archive must contain exactly one root oxid.exe." }
            $binary = $binaries[0]
            $attributes = [uint32]([int64]$binary.ExternalAttributes -band 0xffffffffL)
            $unixType = ($attributes -shr 16) -band 0xf000
            if (($unixType -ne 0 -and $unixType -ne 0x8000) -or ($attributes -band 0x450)) {
                throw "The archive must contain a regular oxid.exe executable."
            }

            [System.IO.Directory]::CreateDirectory($InstallDir) | Out-Null
            $null = Test-OxidDestination $destination
            $privatePath = Join-Path $InstallDir (".oxid-install-" + [guid]::NewGuid())
            New-Item -ItemType Directory -Path $privatePath | Out-Null
            $stagingDir = $privatePath
            $staged = Join-Path $stagingDir "oxid.exe"
            $backup = Join-Path $stagingDir "previous.exe"
            $entryStream = $null
            $stageStream = $null
            try {
                $stageStream = [System.IO.FileStream]::new(
                    $staged, [System.IO.FileMode]::CreateNew,
                    [System.IO.FileAccess]::Write, [System.IO.FileShare]::None)
                $entryStream = $binary.Open()
                Copy-OxidStream $entryStream $stageStream
                if ($stageStream.Length -ne $binary.Length) { throw "The archive executable is truncated." }
                $stageStream.Flush($true)
            } finally {
                try {
                    if ($entryStream) { $entryStream.Dispose() }
                } finally {
                    if ($stageStream) { $stageStream.Dispose() }
                }
            }
        } finally {
            try {
                if ($zip) { $zip.Dispose() }
            } finally {
                if ($archiveStream) { $archiveStream.Dispose() }
            }
        }

        $replace = Test-OxidDestination $destination
        try {
            Move-OxidExecutable $staged $destination $backup $replace
        } catch {
            $publicationError = $_
            if ([System.IO.File]::Exists($backup)) {
                # ReplaceFile may fail after moving the original to its backup.
                # Keep that copy until restoration has completed successfully.
                $keepStaging = $true
                try {
                    $restoreReplace = Test-OxidDestination $destination
                    Move-OxidExecutable $backup $destination $null $restoreReplace
                    $keepStaging = $false
                } catch {
                    throw "Installation failed: $publicationError Previous executable kept at $backup; automatic restoration failed: $_"
                }
            }
            throw $publicationError
        }
    } finally {
        # Cleanup must not replace a publication error or fail a completed install.
        if ($stagingDir -and -not $keepStaging) {
            try { Remove-Item -LiteralPath $stagingDir -Recurse -Force -ErrorAction Stop } catch {}
        }
        if ($tempOwned) {
            try { Remove-Item -LiteralPath $tempDir -Recurse -Force -ErrorAction Stop } catch {}
        }
    }

    Write-Host "Installed Oxid to $destination"
    Write-Host "Add $InstallDir to PATH if it is not already available."
}

Install-OxidRelease -Version $Version -InstallDir $InstallDir -Archive $Archive
