[CmdletBinding()]
param([Parameter(Mandatory = $true)][string]$Destination)
$ErrorActionPreference = 'Stop'
Write-Host 'Collecting dependency licence notices...'
$metadataText = & cargo metadata --locked --format-version 1 --filter-platform x86_64-pc-windows-msvc
if ($LASTEXITCODE -ne 0) { throw 'Could not read Cargo dependency metadata.' }
$metadata = ($metadataText -join "`n") | ConvertFrom-Json
$included = @{}
foreach ($node in $metadata.resolve.nodes) { $included[$node.id] = $true }
$text = [Text.StringBuilder]::new()
[void]$text.AppendLine('MiPoint Dummy Migration Set Generator - third-party notices')
[void]$text.AppendLine('The application source is MIT licensed. Dependencies retain their own licences.')
foreach ($package in ($metadata.packages | Sort-Object name, version)) {
    if ($package.name -eq 'mipoint-dummy-generator' -or -not $included.ContainsKey($package.id)) { continue }
    [void]$text.AppendLine("`n================================================================")
    [void]$text.AppendLine("$($package.name) $($package.version) | $($package.license)")
    [void]$text.AppendLine([string]$package.repository)
    $sourceRoot = Split-Path -Parent $package.manifest_path
    $licences = @(Get-ChildItem -LiteralPath $sourceRoot -File -Recurse | Where-Object {
        $_.Name -match '^(?i:licen[cs]e|copying|notice|copyright)([._-].*)?$|^(OFL|UFL)(\.txt)?$'
    } | Sort-Object FullName)
    if ($package.license_file) {
        $specified = Join-Path $sourceRoot $package.license_file
        if (Test-Path -LiteralPath $specified -PathType Leaf) { $licences += Get-Item -LiteralPath $specified }
    }
    if ($licences.Count -eq 0) {
        $vendored = Join-Path $PSScriptRoot "licences/$($package.name)-$($package.version)"
        if (Test-Path -LiteralPath $vendored) {
            $sourceRoot = $vendored
            $licences = @(Get-ChildItem -LiteralPath $vendored -File)
        }
    }
    if ($licences.Count -eq 0) { throw "No licence text found for $($package.name) $($package.version). Review before distributing." }
    foreach ($file in ($licences | Sort-Object FullName -Unique)) {
        [void]$text.AppendLine("`n--- $($file.FullName.Substring($sourceRoot.Length + 1)) ---")
        [void]$text.AppendLine([IO.File]::ReadAllText($file.FullName))
    }
}
[IO.File]::WriteAllText($Destination, $text.ToString(), [Text.UTF8Encoding]::new($false))
