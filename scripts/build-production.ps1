$ErrorActionPreference = 'Stop'
# Make can pass PowerShell 7 module paths to Windows PowerShell.
$env:PSModulePath = [IO.Path]::Combine($PSHOME, 'Modules')

if (!(Get-Command cl.exe -ErrorAction SilentlyContinue) -or
    !(Get-Command nmake.exe -ErrorAction SilentlyContinue)) {
    $vswhere = Join-Path ([Environment]::GetFolderPath('ProgramFilesX86')) 'Microsoft Visual Studio\Installer\vswhere.exe'
    if (!(Test-Path -LiteralPath $vswhere)) {
        throw 'Install Visual Studio Build Tools with Desktop development with C++ and a Windows SDK.'
    }
    $installation = & $vswhere -latest -products '*' -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
    if (!$installation) {
        throw 'Install the Visual Studio C++ build tools and a Windows SDK.'
    }
    $setup = Join-Path $installation 'Common7\Tools\VsDevCmd.bat'
    $nativeEnvironment = & $env:ComSpec /d /c ('"{0}" -no_logo -arch=x64 -host_arch=x64 && set' -f $setup)
    if ($LASTEXITCODE -ne 0) { throw 'Cannot initialize the Visual Studio build environment.' }
    foreach ($entry in $nativeEnvironment) {
        $separator = $entry.IndexOf('=')
        if ($separator -gt 0) {
            [Environment]::SetEnvironmentVariable($entry.Substring(0, $separator), $entry.Substring($separator + 1), 'Process')
        }
    }
}

$env:LC_ALL = 'C'
$env:LANG = 'C'
. (Join-Path $PSScriptRoot 'native-perl.ps1')
$nativePerl = Get-NativePerl
if (!(Get-Command cl.exe -ErrorAction SilentlyContinue) -or
    !(Get-Command nmake.exe -ErrorAction SilentlyContinue)) {
    throw 'The Visual Studio environment does not provide cl.exe and nmake.exe.'
}

$env:OPENSSL_SRC_PERL = $nativePerl
$env:PERL = $nativePerl
$env:PATH = (Split-Path -Parent $nativePerl) + ';' + $env:PATH
Write-Host "Building with native Perl: $nativePerl"
& cargo build --release --locked --bin latch
exit $LASTEXITCODE
