$ErrorActionPreference = "Stop"
Set-Location $PSScriptRoot

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
Invoke-WebRequest "$Base/$Archive" -OutFile $ZipPath
Invoke-WebRequest "$Base/$ChecksumFile" -OutFile $ChecksumPath

$Raw = Get-Content $ChecksumPath -Raw
$EscapedArchive = [regex]::Escape($Archive)
$Match = [regex]::Match($Raw, "(?im)^([0-9a-f]{64})\s+\*?$EscapedArchive\s*$")

if (-not $Match.Success) {
    throw "Official SHA-256 for $Archive was not found in CHECKSUMS.asc."
}

$Expected = $Match.Groups[1].Value.ToLowerInvariant()
$Actual = (Get-FileHash $ZipPath -Algorithm SHA256).Hash.ToLowerInvariant()

Write-Host "Expected SHA256: $Expected"
Write-Host "Actual SHA256:   $Actual"

if ($Expected -ne $Actual) {
    throw "SHA-256 verification FAILED."
}

Write-Host "SHA-256 verified." -ForegroundColor Green

if (Test-Path $Extracted) {
    Remove-Item $Extracted -Recurse -Force
}

Expand-Archive $ZipPath -DestinationPath $Extracted -Force

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
