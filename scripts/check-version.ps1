param([string]$Tag)

$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'version.ps1')
$version = Get-LatchVersion (Split-Path -Parent $PSScriptRoot)
if ($PSBoundParameters.ContainsKey('Tag') -and $Tag -cne "v$version") {
    throw "Release tag must be v$version; received '$Tag'."
}
$version
