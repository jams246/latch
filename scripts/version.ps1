function ConvertTo-LatchVersion([string]$Version) {
    if ($Version -cnotmatch '\A(0|[1-9][0-9]{0,2})\.(0|[1-9][0-9]{0,2})\.(0|[1-9][0-9]{0,4})\z' -or
        [int]$Matches[1] -gt 255 -or [int]$Matches[2] -gt 255 -or [int]$Matches[3] -gt 65535) {
        throw 'Use major.minor.patch without leading zeros or suffixes; major <=255, minor <=255, patch <=65535.'
    }
    return [version]$Version
}

function Get-LatchVersion([string]$Repository) {
    $metadata = & cargo metadata --manifest-path (Join-Path $Repository 'Cargo.toml') --no-deps --locked --format-version 1 |
        ConvertFrom-Json
    if ($LASTEXITCODE -ne 0) { throw 'Cannot read Cargo package metadata.' }
    $package = @($metadata.packages | Where-Object { $_.name -ceq 'latch' })
    if ($package.Count -ne 1) { throw 'Expected exactly one latch package.' }
    $version = $package[0].version
    $null = ConvertTo-LatchVersion $version
    $lock = [IO.File]::ReadAllText((Join-Path $Repository 'Cargo.lock'))
    $entries = [regex]::Matches($lock, '(?m)^\[\[package\]\]\r?\nname = "latch"\r?\nversion = "([^"]+)"')
    if ($entries.Count -ne 1 -or $entries[0].Groups[1].Value -cne $version) {
        throw 'Cargo.toml and Cargo.lock must contain the same latch version.'
    }
    return $version
}
