[CmdletBinding()]
param([switch]$Launch, [switch]$Validate, [string]$BuildDirectory = (Join-Path $PSScriptRoot 'target'))
$ErrorActionPreference = 'Stop'
$projectRoot = $PSScriptRoot
$previousTarget = $env:CARGO_TARGET_DIR
Push-Location -LiteralPath $projectRoot
try {
    if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
        throw 'Install Rust 1.95 or newer (MSVC toolchain) and Visual Studio C++ Build Tools with the Windows SDK, then run this file again.'
    }
    $env:CARGO_TARGET_DIR = [IO.Path]::GetFullPath($BuildDirectory)
    $rustHost = (& rustc -vV | Select-String '^host:').ToString()
    if ($LASTEXITCODE -ne 0 -or $rustHost -notmatch 'x86_64-pc-windows-msvc') { throw 'Use the Windows x64 MSVC Rust toolchain.' }
    Write-Host 'Building the Windows x64 portable GUI...' -ForegroundColor Cyan
    & cargo build --release --locked
    if ($LASTEXITCODE -ne 0) { throw 'Cargo release build failed.' }
    if ($Validate) {
        & cargo test --release --locked
        if ($LASTEXITCODE -ne 0) { throw 'Release tests failed.' }
    }
    $portable = Join-Path $projectRoot 'dist/portable'
    New-Item -ItemType Directory -Path $portable -Force | Out-Null
    Copy-Item -LiteralPath (Join-Path $env:CARGO_TARGET_DIR 'release/mipoint-dummy-generator.exe') -Destination $portable -Force
    & (Join-Path $projectRoot 'tools/Write-LicenceNotices.ps1') -Destination (Join-Path $projectRoot 'THIRD_PARTY_NOTICES.txt')
    foreach ($name in @('LICENSE', 'README.md', 'VALIDATION.md', 'THIRD_PARTY_NOTICES.txt')) {
        Copy-Item -LiteralPath (Join-Path $projectRoot $name) -Destination $portable -Force
    }
    New-Item -ItemType Directory -Path (Join-Path $portable 'validation') -Force | Out-Null
    Copy-Item -LiteralPath (Join-Path $projectRoot 'validation/benchmark.txt') -Destination (Join-Path $portable 'validation/benchmark.txt') -Force
    $archive = Join-Path $projectRoot 'dist/MiPoint-Generator-Windows-x64.zip'
    $packageFiles = @('mipoint-dummy-generator.exe','LICENSE','README.md','VALIDATION.md','THIRD_PARTY_NOTICES.txt','validation') | ForEach-Object { Join-Path $portable $_ }
    Compress-Archive -LiteralPath $packageFiles -DestinationPath $archive -Force
    $hash = (Get-FileHash -LiteralPath $archive -Algorithm SHA256).Hash.ToLowerInvariant()
    [IO.File]::WriteAllText((Join-Path $projectRoot 'dist/SHA256SUMS.txt'), "$hash  MiPoint-Generator-Windows-x64.zip`r`n", [Text.UTF8Encoding]::new($false))
    Write-Host "Portable application: $portable" -ForegroundColor Green
    Write-Host "Distribution ZIP: $archive"
    if ($Launch) { Start-Process -FilePath (Join-Path $portable 'mipoint-dummy-generator.exe') -WorkingDirectory $portable }
} finally {
    $env:CARGO_TARGET_DIR = $previousTarget
    Pop-Location
}
