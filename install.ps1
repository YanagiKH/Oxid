[CmdletBinding()]
param(
    [string]$Version = $env:OXID_VERSION,
    [string]$InstallDir = $env:OXID_INSTALL_DIR
)

$ErrorActionPreference = "Stop"
if (-not $Version) { $Version = "latest" }
if (-not $InstallDir) { $InstallDir = Join-Path $env:LOCALAPPDATA "Oxid\bin" }

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
New-Item -ItemType Directory -Path $tempDir | Out-Null
try {
    $archive = Join-Path $tempDir $asset
    $checksumFile = "$archive.sha256"
    Invoke-WebRequest "$baseUrl/$asset" -OutFile $archive
    Invoke-WebRequest "$baseUrl/$asset.sha256" -OutFile $checksumFile

    $expected = ((Get-Content $checksumFile -Raw).Trim() -split "\s+")[0].ToLowerInvariant()
    $actual = (Get-FileHash $archive -Algorithm SHA256).Hash.ToLowerInvariant()
    if ($expected -ne $actual) { throw "Checksum verification failed." }

    Expand-Archive $archive -DestinationPath $tempDir -Force
    New-Item -ItemType Directory -Path $InstallDir -Force | Out-Null
    Copy-Item (Join-Path $tempDir "oxid.exe") (Join-Path $InstallDir "oxid.exe") -Force
} finally {
    Remove-Item $tempDir -Recurse -Force -ErrorAction SilentlyContinue
}

Write-Host "Installed Oxid to $(Join-Path $InstallDir 'oxid.exe')"
Write-Host "Add $InstallDir to PATH if it is not already available."
