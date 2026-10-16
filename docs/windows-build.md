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
