$ErrorActionPreference = "Stop"
Set-Location $PSScriptRoot

$PackageName = "Eureka-Nexus-Miner-Official-1.0-Windows-x86_64"
$TargetDir = Join-Path $PSScriptRoot "target-windows"
$DistDir = Join-Path $PSScriptRoot "dist"
$PackageDir = Join-Path $DistDir $PackageName
$ZipPath = Join-Path $DistDir "$PackageName.zip"
$ZipHashPath = Join-Path $DistDir "$PackageName.zip.sha256.txt"

$env:CARGO_TARGET_DIR = $TargetDir

Write-Host "=== Building Eureka Nexus Miner Official 1.0 ===" -ForegroundColor Cyan
cargo build --release --bin eureka-nexus-miner-official
if ($LASTEXITCODE -ne 0) { throw "cargo build failed" }

$Exe = Join-Path $TargetDir "release\eureka-nexus-miner-official.exe"
if (-not (Test-Path $Exe)) {
    throw "Windows executable not found: $Exe"
}

if (Test-Path $PackageDir) {
    Remove-Item $PackageDir -Recurse -Force
}
if (Test-Path $ZipPath) {
    Remove-Item $ZipPath -Force
}
if (Test-Path $ZipHashPath) {
    Remove-Item $ZipHashPath -Force
}

New-Item -ItemType Directory -Force $PackageDir | Out-Null
New-Item -ItemType Directory -Force (Join-Path $PackageDir "LICENSES") | Out-Null
New-Item -ItemType Directory -Force (Join-Path $PackageDir "engines\kawpow") | Out-Null

Copy-Item $Exe (Join-Path $PackageDir "eureka-nexus-miner-official.exe") -Force

$Files = @(
    "README.md",
    "LICENSE",
    "SECURITY.md",
    "THIRD_PARTY_NOTICES.md",
    "THIRD_PARTY_RUST_LICENSES.html",
    "THIRD_PARTY_RUST_LICENSES.tsv",
    "eureka-public.json",
    "VERSION",
    "INSTALL_KAWPOW_ENGINE_WINDOWS.ps1"
)

foreach ($File in $Files) {
    $Source = Join-Path $PSScriptRoot $File
    if (-not (Test-Path $Source)) {
        throw "Required release file missing: $File"
    }
    Copy-Item $Source $PackageDir -Force
}

Copy-Item (Join-Path $PSScriptRoot "LICENSES\*") (Join-Path $PackageDir "LICENSES") -Force
Copy-Item (Join-Path $PSScriptRoot "engines\kawpow\NOTICE.txt") (Join-Path $PackageDir "engines\kawpow\NOTICE.txt") -Force

$ChecksumFile = Join-Path $PackageDir "SHA256SUMS.txt"
$RootResolved = (Resolve-Path $PackageDir).Path.TrimEnd('\')

$Rows = Get-ChildItem $PackageDir -Recurse -File |
    Where-Object { $_.FullName -ne $ChecksumFile } |
    Sort-Object FullName |
    ForEach-Object {
        $Hash = (Get-FileHash $_.FullName -Algorithm SHA256).Hash.ToLowerInvariant()
        $Relative = $_.FullName.Substring($RootResolved.Length + 1).Replace('\','/')
        "$Hash  $Relative"
    }

$Rows | Set-Content $ChecksumFile -Encoding ascii

Compress-Archive -Path (Join-Path $PackageDir "*") -DestinationPath $ZipPath -CompressionLevel Optimal

$ZipHash = (Get-FileHash $ZipPath -Algorithm SHA256).Hash.ToLowerInvariant()
"$ZipHash  $PackageName.zip" | Set-Content $ZipHashPath -Encoding ascii

Write-Host ""
Write-Host "Release package created:" -ForegroundColor Green
Write-Host $ZipPath
Write-Host "ZIP SHA256:"
Write-Host $ZipHash
Write-Host ""
Write-Host "KAWPOW is not bundled. Users install it with INSTALL_KAWPOW_ENGINE_WINDOWS.ps1."
