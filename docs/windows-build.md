# Windows production build

Run `make build:production` from the repository root.

Install the Rust MSVC toolchain, Visual Studio Build Tools with **Desktop development with C++**, a Windows SDK, and native [Strawberry Perl](https://strawberryperl.com/). SQLCipher's vendored OpenSSL build requires native Windows Perl; Git's MSYS Perl is unsuitable.

The Windows target initializes installed Visual Studio tools and discovers native Perl from `OPENSSL_SRC_PERL`, `PERL`, `PATH`, `C:\Strawberry\perl\bin\perl.exe`, or `%LOCALAPPDATA%\Latch\BuildTools\StrawberryPerl\perl\bin\perl.exe`. A portable Strawberry distribution can be extracted into that cache. Keep its `perl` directory and license files; its bundled compiler is unnecessary.

For another portable installation, set `OPENSSL_SRC_PERL` to the full path of `perl.exe`. The build configures its child process without changing the global `PATH` or installing software automatically. The Make target uses a process-scoped `RemoteSigned` execution policy for the local build script; it does not change the system policy.

OpenSSL and SQLCipher remain statically bundled. The application does not require Perl or separate OpenSSL/SQLCipher DLLs at runtime. Non-Windows Make builds retain the direct Cargo command.
