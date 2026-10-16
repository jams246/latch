# Windows production build

Run `make build:production` from the repository root.

Install the Rust MSVC toolchain, Visual Studio Build Tools with **Desktop development with C++**, and a Windows SDK. SQLCipher's vendored OpenSSL build requires native Windows Perl; Git's MSYS Perl is unsuitable.

The Windows target uses Windows PowerShell's own module path, initializes installed Visual Studio tools, and discovers native Perl from `OPENSSL_SRC_PERL`, `PERL`, `PATH`, `C:\Strawberry\perl\bin\perl.exe`, or its per-user cache. This avoids incompatible module paths inherited from PowerShell 7 through Make. It verifies that Perl runs natively and includes the required core modules.

If native Perl is missing, the first build downloads the official [Strawberry Perl 5.42.3.1 portable release](https://github.com/StrawberryPerl/Perl-Dist-Strawberry/releases/tag/SP_54231_64bit) (291 MiB), verifies its pinned SHA256 from [Strawberry's release metadata](https://strawberryperl.com/releases.json), and caches only `perl` and license files under `%LOCALAPPDATA%\Latch\BuildTools\StrawberryPerl-5.42.3.1`. The bundled compiler is excluded. Partial downloads are discarded; later builds reuse the verified runtime without downloading it again.

For offline builds, install [Strawberry Perl](https://strawberryperl.com/) first or set `OPENSSL_SRC_PERL` to the full path of a complete portable installation's `perl.exe`. The build needs no administrator access and changes only its child process environment. The Make target uses a process-scoped `RemoteSigned` execution policy for the local build script; it does not change the system policy or global `PATH`.

OpenSSL and SQLCipher remain statically bundled. The application does not require Perl or separate OpenSSL/SQLCipher DLLs at runtime. Latch supports Windows only.

Windows MSVC release builds with debug information disabled omit the final PDB debugging-symbol file. This prevents repeated `LNK4099` warnings for OpenSSL's separate compiler PDB. Detailed production crash symbolization is unavailable in this mode. Development builds and release builds with debug information explicitly enabled retain normal symbol generation.
