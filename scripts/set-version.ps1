param([Parameter(Mandatory)][string]$Version)

$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'version.ps1')
$repository = Split-Path -Parent $PSScriptRoot
$next = ConvertTo-LatchVersion $Version
$current = Get-LatchVersion $repository
if ($next -le [version]$current) { throw "Version must be greater than $current." }

$manifestPath = Join-Path $repository 'Cargo.toml'
$lockPath = Join-Path $repository 'Cargo.lock'
$manifestBytes = [IO.File]::ReadAllBytes($manifestPath)
$lockBytes = [IO.File]::ReadAllBytes($lockPath)
$manifest = [IO.File]::ReadAllText($manifestPath)
$lock = [IO.File]::ReadAllText($lockPath)
$manifestPattern = '(?ms)(^\[package\]\r?\n(?:(?!^\[).)*?^version\s*=\s*")[^"]+("[^\r\n]*)'
$lockPattern = '(?m)(^\[\[package\]\]\r?\nname = "latch"\r?\nversion = ")[^"]+(")'
if ([regex]::Matches($manifest, $manifestPattern).Count -ne 1 -or
    [regex]::Matches($lock, $lockPattern).Count -ne 1) {
    throw 'Expected one explicit package version in Cargo.toml and Cargo.lock.'
}
$replacement = '${1}' + $Version + '${2}'
try {
    [IO.File]::WriteAllText($manifestPath, [regex]::Replace($manifest, $manifestPattern, $replacement))
    [IO.File]::WriteAllText($lockPath, [regex]::Replace($lock, $lockPattern, $replacement))
    $null = Get-LatchVersion $repository
} catch {
    [IO.File]::WriteAllBytes($manifestPath, $manifestBytes)
    [IO.File]::WriteAllBytes($lockPath, $lockBytes)
    throw
}
Write-Host "Updated Latch from $current to $Version. Review Cargo.toml, Cargo.lock, and CHANGELOG.md before committing."
