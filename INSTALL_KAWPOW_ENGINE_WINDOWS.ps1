$ErrorActionPreference = "Stop"
$ProgressPreference = "SilentlyContinue"
Set-Location $PSScriptRoot
Start-Transcript -Path (Join-Path $PSScriptRoot 'kawpow-install.log') -Force | Out-Null

$Version = "1.2.4"
$Archive = "kawpowminer-windows-cuda11-$Version.zip"
$ChecksumFile = "CHECKSUMS.asc"
$Base = "https://github.com/RavenCommunity/kawpowminer/releases/download/$Version"

$Dest = Join-Path $PSScriptRoot "engines\kawpow"
$Tmp = Join-Path $Dest ".download"
$ZipPath = Join-Path $Tmp $Archive
$ChecksumPath = Join-Path $Tmp $ChecksumFile
$Extracted = Join-Path $Tmp "extracted"

New-Item -ItemType Directory -Force $Dest | Out-Null
New-Item -ItemType Directory -Force $Tmp | Out-Null

Write-Host "Downloading official kawpowminer $Version..." -ForegroundColor Cyan
Invoke-WebRequest "$Base/$Archive" -OutFile $ZipPath -UseBasicParsing
Invoke-WebRequest "$Base/$ChecksumFile" -OutFile $ChecksumPath -UseBasicParsing

$Raw = Get-Content $ChecksumPath -Raw
$EscapedArchive = [regex]::Escape($Archive)
$Match = [regex]::Match($Raw, "(?im)^([0-9a-f]{64})\s+\*?$EscapedArchive\s*$")

if (-not $Match.Success) {
    throw "Official SHA-256 for $Archive was not found in CHECKSUMS.asc."
}

$Expected = $Match.Groups[1].Value.ToLowerInvariant()
$HashAlgorithm = [Security.Cryptography.SHA256]::Create()
$ArchiveStream = [IO.File]::OpenRead($ZipPath)
try {
    $Actual = [BitConverter]::ToString($HashAlgorithm.ComputeHash($ArchiveStream)).Replace('-', '').ToLowerInvariant()
} finally {
    $ArchiveStream.Dispose()
    $HashAlgorithm.Dispose()
}

Write-Host "Expected SHA256: $Expected"
Write-Host "Actual SHA256:   $Actual"

if ($Expected -ne $Actual) {
    throw "SHA-256 verification FAILED."
}

Write-Host "SHA-256 verified." -ForegroundColor Green

if (Test-Path $Extracted) {
    $SafeRoot = [IO.Path]::GetFullPath($Dest).TrimEnd('\') + '\'
    if (-not [IO.Path]::GetFullPath($Extracted).StartsWith($SafeRoot, [StringComparison]::OrdinalIgnoreCase)) {
        throw "Extraction directory must stay inside the KAWPOW directory"
    }
    Remove-Item -LiteralPath $Extracted -Recurse -Force
}

Add-Type -AssemblyName System.IO.Compression.FileSystem
[IO.Compression.ZipFile]::ExtractToDirectory($ZipPath, $Extracted)

$Exe = Get-ChildItem $Extracted -Recurse -Filter "kawpowminer.exe" |
    Select-Object -First 1

if (-not $Exe) {
    throw "kawpowminer.exe not found."
}

Get-ChildItem $Exe.Directory.FullName -File | ForEach-Object {
    Copy-Item $_.FullName $Dest -Force
}

Write-Host ""
Write-Host "KAWPOW GPU engine installed." -ForegroundColor Green
Write-Host "Location: $Dest\kawpowminer.exe"
foreach ($Required in @('kawpowminer.exe', 'nvrtc64_112_0.dll', 'nvrtc-builtins64_112.dll')) {
    if (-not (Test-Path -LiteralPath (Join-Path $Dest $Required))) {
        throw "KAWPOW installation incomplete: $Required"
    }
}
Stop-Transcript | Out-Null
