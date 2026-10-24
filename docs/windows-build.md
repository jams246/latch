# Windows production build

Install these tools on 64-bit Windows:

- Rust with the `x86_64-pc-windows-msvc` toolchain.
- Visual Studio Build Tools with **Desktop development with C++**, x64 build tools, and a Windows SDK that includes `fxc.exe`.
- GNU Make, available as `make` in your terminal.

Installing these tools may need administrator access. Building Latch and downloading Perl do not.

From the project folder, run:

```powershell
make build:production
```

Make finds the SDK's `fxc.exe` automatically. If needed, set `GPUI_FXC_PATH` to its full path before running Make.

The build sets up the Visual Studio tools and uses Windows PowerShell. It does not change your system settings or global `PATH`.

OpenSSL needs native Windows Perl to build. Git's MSYS Perl will not work. The build checks `OPENSSL_SRC_PERL`, `PERL`, `PATH`, `C:\Strawberry\perl\bin\perl.exe`, then its saved Perl copy.

If no suitable Perl is found, the build downloads [Strawberry Perl 5.42.3.1](https://github.com/StrawberryPerl/Perl-Dist-Strawberry/releases/tag/SP_54231_64bit) (291 MiB), checks its SHA256, and saves the Perl runtime and licenses under `%LOCALAPPDATA%\Latch\BuildTools\StrawberryPerl-5.42.3.1`. Later builds reuse it.

For offline builds, install [Strawberry Perl](https://strawberryperl.com/) first or set `OPENSSL_SRC_PERL` to a complete installation's `perl.exe`. Cargo dependencies must also be downloaded in advance; run `cargo fetch --locked` while online.

The built app is `target\release\latch.exe`. OpenSSL and SQLCipher are included; running Latch needs no Perl or separate OpenSSL/SQLCipher DLLs. Latch supports Windows only.

Release builds omit the PDB debug-symbol file unless debug information is enabled. This avoids OpenSSL's `LNK4099` warnings but limits crash diagnosis. Development builds keep debug symbols.

## MSI installer

With the same Windows build tools installed, run either command from the project folder:

```powershell
make build:msi
# Or without GNU Make:
powershell.exe -NoProfile -ExecutionPolicy RemoteSigned -File scripts/build-msi.ps1
```

The script builds the release app first and discovers `fxc.exe` without Make. It requires the x64 MSVC Rust toolchain and no `CARGO_BUILD_TARGET` override. Normal Cargo output is `target\wix\latch-<version>-x64.msi`; a configured Cargo target directory is respected.

The first build downloads the [WiX 3.14.1 binaries](https://github.com/wixtoolset/wix3/releases/tag/wix3141rtm), verifies SHA256, and installs [`cargo-wix` 0.3.9](https://docs.rs/crate/cargo-wix/0.3.9) with locked dependencies. Both tools are cached under `%LOCALAPPDATA%\Latch\BuildTools`, alongside the existing Perl cache. No administrator access or global `PATH` changes are needed. Later builds reuse these tools. WiX 3 is the legacy toolset supported by this published `cargo-wix` version; newer WiX releases need a packaging migration.

The MSI installs for the current Windows user under `%LOCALAPPDATA%\Programs\Latch`, adds a Start menu shortcut and an Installed Apps entry, and supports repair and uninstall. Windows 10 version 1703 or later, including Windows 11, is required. The payload contains only `latch.exe`, `LICENSE.txt`, and Microsoft's app-local `vcruntime140.dll` from the Visual Studio C++ x64 redistributable files. Runtime updates ship through a rebuilt MSI.

Latch keeps its encrypted profile and trusted host keys beside its executable. Upgrade and uninstall preserve these user-created files. An existing portable copy has a separate profile; use Latch's export/import to move connections. Never package the entire release directory, because it can contain private profile data.

Before distributing a newer version, run `./scripts/set-version.ps1 -Version X.Y.Z`. MSI versions must contain exactly three numeric components: major and minor at most 255, patch at most 65535. Keep `UpgradeCode` and component GUIDs in `wix/main.wxs` stable. Newer versions and same-version rebuilds replace the existing installation; downgrades are rejected.

These installers are unsigned. Windows may show an unknown-publisher or reputation warning. Code signing is a separate release step.

## Manual GitHub Actions build

The workflow in `.github/workflows/build-msi.yml` uses the same MSI script on a Windows runner. It checks version consistency and, when run for a tag, requires `vX.Y.Z` to match Cargo. It runs version-script tests, formatting, Clippy, and Rust tests before packaging, then uploads the MSI as the `latch-X.Y.Z-windows-x64-msi` artifact. It does not publish a release.

After the workflow reaches the repository's default branch:

1. Open **Actions**, then **Build Windows MSI**.
2. Select **Run workflow**, choose the branch, and run it. For tagged builds, run `gh workflow run build-msi.yml --ref vX.Y.Z`.
3. Open the completed run and download `latch-X.Y.Z-windows-x64-msi` from **Artifacts**. Extract the ZIP to get the MSI.

Only `workflow_dispatch` triggers builds; pushes and tags do not start this workflow.
