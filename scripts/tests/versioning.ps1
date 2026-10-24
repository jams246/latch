$ErrorActionPreference = 'Stop'
$repository = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
$fixture = Join-Path $repository ('.verification/versioning/' + [guid]::NewGuid())
$null = New-Item -ItemType Directory -Path "$fixture/scripts", "$fixture/src", "$fixture/helper/src" -Force
foreach ($script in @('version.ps1', 'set-version.ps1', 'check-version.ps1')) {
    Copy-Item -LiteralPath (Join-Path $repository "scripts/$script") -Destination "$fixture/scripts/$script"
}
$manifestPath = Join-Path $fixture 'Cargo.toml'
$lockPath = Join-Path $fixture 'Cargo.lock'
[IO.File]::WriteAllText($manifestPath, @'
[package]
name = "latch"
version = "0.1.0" # Keep this comment.
edition = "2024"

[workspace]

[dependencies]
fixture-helper = { path = "helper", version = "0.1.0" }
'@)
[IO.File]::WriteAllText("$fixture/src/lib.rs", '')
[IO.File]::WriteAllText("$fixture/helper/Cargo.toml", @'
[package]
name = "fixture-helper"
version = "0.1.0"
edition = "2024"
'@)
[IO.File]::WriteAllText("$fixture/helper/src/lib.rs", '')
& cargo generate-lockfile --offline --manifest-path $manifestPath
if ($LASTEXITCODE -ne 0) { throw 'Cannot create the version test fixture.' }
$originalManifest = [IO.File]::ReadAllText($manifestPath)
$originalLock = [IO.File]::ReadAllText($lockPath)
$setVersion = Join-Path $fixture 'scripts/set-version.ps1'
$checkVersion = Join-Path $fixture 'scripts/check-version.ps1'

function Assert-Throws([scriptblock]$Action, [string]$Message) {
    try { & $Action }
    catch {
        if ($_.Exception.Message -like $Message) { return }
        throw "Unexpected failure: $($_.Exception.Message)"
    }
    throw "Expected failure: $Message"
}

function Reset-Fixture {
    [IO.File]::WriteAllText($manifestPath, $originalManifest)
    [IO.File]::WriteAllText($lockPath, $originalLock)
}

function Assert-Unchanged([string]$Manifest = $originalManifest, [string]$Lock = $originalLock) {
    if ([IO.File]::ReadAllText($manifestPath) -cne $Manifest -or
        [IO.File]::ReadAllText($lockPath) -cne $Lock) {
        throw 'Rejected version update changed the manifest or lockfile.'
    }
}

if ((& $checkVersion) -cne '0.1.0') { throw 'Incorrect initial version.' }
$null = & $checkVersion -Tag 'v0.1.0'
foreach ($tag in @('v0.2.0', '0.1.0', 'V0.1.0', 'v0.1.0-beta.1', '')) {
    Assert-Throws { & $checkVersion -Tag $tag } 'Release tag must be v0.1.0*'
}
foreach ($version in @('0.1.1', '0.2.0', '1.0.0', '255.255.65535')) {
    Reset-Fixture
    & $setVersion -Version $version
    if ((& $checkVersion -Tag "v$version") -cne $version) { throw "Version update failed: $version" }
    $expectedManifest = $originalManifest.Replace('version = "0.1.0" #', "version = `"$version`" #")
    $expectedLock = $originalLock.Replace("name = `"latch`"`nversion = `"0.1.0`"", "name = `"latch`"`nversion = `"$version`"")
    Assert-Unchanged $expectedManifest $expectedLock
}
Reset-Fixture
foreach ($version in @('0.1.0', '0.0.9')) {
    Assert-Throws { & $setVersion -Version $version } 'Version must be greater than 0.1.0.'
    Assert-Unchanged
}
foreach ($version in @('v0.2.0', '0.2', '0.2.0.1', '0.2.0-beta.1', '0.2.0+sha', '00.2.0',
        '0.02.0', '0.2.00', '256.0.0', '0.256.0', '0.0.65536', '999999999999.0.0', "0.2.0`n")) {
    Assert-Throws { & $setVersion -Version $version } 'Use major.minor.patch*'
    Assert-Unchanged
}
$staleLock = $originalLock.Replace('name = "latch"', 'name = "other"')
[IO.File]::WriteAllText($lockPath, $staleLock)
Assert-Throws { & $checkVersion } 'Cargo.toml and Cargo.lock must contain the same latch version.'
Assert-Throws { & $setVersion -Version '0.2.0' } 'Cargo.toml and Cargo.lock must contain the same latch version.'
Assert-Unchanged $originalManifest $staleLock
Reset-Fixture
$staleLock = $originalLock.Replace("name = `"latch`"`nversion = `"0.1.0`"", "name = `"latch`"`nversion = `"0.0.9`"")
[IO.File]::WriteAllText($lockPath, $staleLock)
Assert-Throws { & $checkVersion } 'Cargo.toml and Cargo.lock must contain the same latch version.'
Reset-Fixture

$cargoPath = (Get-Command cargo -CommandType Application).Source
$script:metadataCalls = 0
function cargo {
    $script:metadataCalls++
    if ($script:metadataCalls -eq 2) { $global:LASTEXITCODE = 7; return }
    & $cargoPath @args
}
try {
    Assert-Throws { & $setVersion -Version '0.2.0' } 'Cannot read Cargo package metadata.'
    Assert-Unchanged
} finally { Remove-Item Function:cargo }

'Passed: explicit bumps, unchanged dependencies/comments, numeric bounds, rejected suffixes/downgrades, lock consistency, tag matching, and rollback.'
