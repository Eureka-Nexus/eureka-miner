$ErrorActionPreference = "Stop"
Set-Location $PSScriptRoot

$Version = (Get-Content (Join-Path $PSScriptRoot "VERSION") -Raw).Trim()
if ($Version -notmatch "^\d+\.\d+\.\d+$") { throw "Invalid VERSION" }
$PackageName = "Eureka-Nexus-Miner-Official-$Version-Windows-x86_64"
$TargetDir = Join-Path $PSScriptRoot "target-windows"
$DistDir = Join-Path $PSScriptRoot "dist"
$PackageDir = Join-Path $DistDir $PackageName
$ZipPath = Join-Path $DistDir "$PackageName.zip"
$ZipHashPath = Join-Path $DistDir "$PackageName.zip.sha256.txt"

$env:CARGO_TARGET_DIR = $TargetDir

Write-Host "=== Building Eureka Nexus Miner Official $Version ===" -ForegroundColor Cyan
cargo build --release --locked --bin eureka-nexus-miner-official --bin EurekaNexusMiner
if ($LASTEXITCODE -ne 0) { throw "cargo build failed" }

$BackendExe = Join-Path $TargetDir "release\eureka-nexus-miner-official.exe"
$DesktopExe = Join-Path $TargetDir "release\EurekaNexusMiner.exe"

if (-not (Test-Path $BackendExe)) {
    throw "Windows backend executable not found: $BackendExe"
}
if (-not (Test-Path $DesktopExe)) {
    throw "Windows desktop executable not found: $DesktopExe"
}

$ResolvedDist = [IO.Path]::GetFullPath($DistDir).TrimEnd('\') + '\'
foreach ($Candidate in @($PackageDir, $ZipPath, $ZipHashPath)) {
    if (-not [IO.Path]::GetFullPath($Candidate).StartsWith($ResolvedDist, [StringComparison]::OrdinalIgnoreCase)) {
        throw "Release output must stay inside dist"
    }
}
if (Test-Path -LiteralPath $PackageDir) {
    Remove-Item -LiteralPath $PackageDir -Recurse -Force
}
if (Test-Path $ZipPath) {
    Remove-Item -LiteralPath $ZipPath -Force
}
if (Test-Path $ZipHashPath) {
    Remove-Item -LiteralPath $ZipHashPath -Force
}

New-Item -ItemType Directory -Force $PackageDir | Out-Null
New-Item -ItemType Directory -Force (Join-Path $PackageDir "LICENSES") | Out-Null
New-Item -ItemType Directory -Force (Join-Path $PackageDir "engines\kawpow") | Out-Null

Copy-Item $BackendExe (Join-Path $PackageDir "eureka-nexus-miner-official.exe") -Force
Copy-Item $DesktopExe (Join-Path $PackageDir "EurekaNexusMiner.exe") -Force

$Files = @(
    "README.md",
    "CHANGELOG.md",
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
Write-Host "The Windows Setup installs verified KAWPOW automatically (internet required)."

$IsccCandidates = @(
    (Join-Path ${env:LOCALAPPDATA} "Programs\Inno Setup 6\ISCC.exe"),
    (Join-Path ${env:ProgramFiles(x86)} "Inno Setup 6\ISCC.exe")
)
$Iscc = $IsccCandidates | Where-Object { Test-Path -LiteralPath $_ } | Select-Object -First 1
if (-not $Iscc) { throw "Inno Setup 6 compiler not found" }
& $Iscc (Join-Path $PSScriptRoot "installer\Eureka-Nexus-Miner-$Version.iss")
if ($LASTEXITCODE -ne 0) { throw "Windows Setup compilation failed" }
$SetupName = "Eureka-Nexus-Miner-Setup-$Version.exe"
$SetupPath = Join-Path $DistDir "installer\$SetupName"
$SetupHash = (Get-FileHash -LiteralPath $SetupPath -Algorithm SHA256).Hash.ToLowerInvariant()
"$SetupHash  $SetupName" | Set-Content "$SetupPath.sha256.txt" -Encoding ascii
Write-Host "Setup and SHA-256 created: $SetupPath"
