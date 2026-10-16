$ErrorActionPreference = 'Stop'
$env:PSModulePath = [IO.Path]::Combine($PSHOME, 'Modules')
$repository = Split-Path -Parent $PSScriptRoot
Push-Location $repository
try {
    $metadata = & cargo metadata --no-deps --locked --format-version 1 | ConvertFrom-Json
    if ($LASTEXITCODE -ne 0) { throw 'Cannot read Cargo package metadata.' }
    $package = $metadata.packages | Where-Object name -EQ 'latch'
    $version = $package.version
    if ($version -notmatch '^(\d+)\.(\d+)\.(\d+)$' -or
        [int]$Matches[1] -gt 255 -or [int]$Matches[2] -gt 255 -or [int]$Matches[3] -gt 65535) {
        throw 'MSI requires a Cargo version with three numeric components: major <=255, minor <=255, patch <=65535.'
    }
    $hostInfo = & rustc -vV
    if ($LASTEXITCODE -ne 0 -or $hostInfo -notcontains 'host: x86_64-pc-windows-msvc') {
        throw 'Build the MSI with the x86_64-pc-windows-msvc Rust toolchain.'
    }
    if ($env:CARGO_BUILD_TARGET) { throw 'Unset CARGO_BUILD_TARGET before building the MSI.' }

    if (!$env:GPUI_FXC_PATH) {
        $env:GPUI_FXC_PATH = Get-ChildItem -Path ([IO.Path]::Combine(
            [Environment]::GetFolderPath('ProgramFilesX86'), 'Windows Kits/10/bin/*/x64/fxc.exe')) |
            Sort-Object FullName -Descending | Select-Object -First 1 -ExpandProperty FullName
    }
    & powershell.exe -NoProfile -ExecutionPolicy RemoteSigned -File (Join-Path $PSScriptRoot 'build-production.ps1')
    if ($LASTEXITCODE -ne 0) { throw "Production build failed with exit code $LASTEXITCODE." }

    $vswhere = Join-Path ([Environment]::GetFolderPath('ProgramFilesX86')) 'Microsoft Visual Studio/Installer/vswhere.exe'
    $visualStudio = & $vswhere -latest -products '*' -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
    $runtime = Get-ChildItem -Path (Join-Path $visualStudio 'VC/Redist/MSVC/*/x64/Microsoft.VC*.CRT/vcruntime140.dll') |
        Sort-Object FullName -Descending | Select-Object -First 1
    if (!$runtime) { throw 'Install the Visual Studio C++ x64 redistributable files to package vcruntime140.dll.' }

    $output = Join-Path $metadata.target_directory 'wix'
    $null = New-Item -ItemType Directory -Path $output -Force
    Copy-Item -LiteralPath $runtime.FullName -Destination (Join-Path $output 'vcruntime140.dll') -Force
    $license = (Get-Content -LiteralPath 'LICENSE' -Raw).Replace('\', '\\').Replace('{', '\{').Replace('}', '\}')
    $license = $license -replace '\r?\n', '\par '
    [IO.File]::WriteAllText((Join-Path $output 'license.rtf'), ('{\rtf1\ansi\deff0{\fonttbl{\f0 Segoe UI;}}\fs18 ' + $license + '}'))

    . (Join-Path $PSScriptRoot 'wix-toolset.ps1')
    $wix = Get-WixToolset
    $cargoWixRoot = Join-Path $env:LOCALAPPDATA 'Latch/BuildTools/cargo-wix-0.3.9'
    $cargoWix = Join-Path $cargoWixRoot 'bin/cargo-wix.exe'
    if (!(Test-Path -LiteralPath $cargoWix)) {
        & cargo install cargo-wix --version 0.3.9 --locked --root $cargoWixRoot
        if ($LASTEXITCODE -ne 0) { throw 'Cannot install cargo-wix 0.3.9.' }
    }
    $installer = Join-Path $output "latch-$version-x64.msi"
    # ICE91 warns about per-machine installs; this package supports per-user installs only.
    # ICE61 warns about equal-version replacements; the three-component version guard prevents fourth-component downgrades.
    & $cargoWix wix --package latch --no-build --target x86_64-pc-windows-msvc `
        --target-bin-dir (Join-Path $metadata.target_directory 'release') --bin-path $wix --output $installer `
        --linker-arg=-sice:ICE91 --linker-arg=-sice:ICE61 --nocapture
    if ($LASTEXITCODE -ne 0) { throw "MSI build failed with exit code $LASTEXITCODE." }
    Write-Host "Installer: $installer"
} finally { Pop-Location }
