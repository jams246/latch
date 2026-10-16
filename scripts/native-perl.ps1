function Test-NativePerl([string]$Path) {
    if (!$Path -or !(Test-Path -LiteralPath $Path -PathType Leaf)) { return $false }
    try {
        & $Path -MIPC::Cmd -MLocale::Maketext::Simple -e 'exit($^O ne q(MSWin32))' 2>$null
        return $LASTEXITCODE -eq 0
    } catch { return $false }
}

function Get-NativePerl {
    $version = '5.42.3.1'
    $cacheRoot = Join-Path $env:LOCALAPPDATA 'Latch\BuildTools'
    $cache = Join-Path $cacheRoot "StrawberryPerl-$version"
    $cachedPerl = Join-Path $cache 'perl\bin\perl.exe'
    $candidates = @($env:OPENSSL_SRC_PERL, $env:PERL) +
        @(Get-Command perl.exe -CommandType Application -All -ErrorAction SilentlyContinue | ForEach-Object Source) +
        @('C:\Strawberry\perl\bin\perl.exe', $cachedPerl)
    foreach ($candidate in $candidates) {
        if (Test-NativePerl $candidate) { return $candidate }
    }
    if (Test-Path -LiteralPath $cache) {
        throw "Cached Perl is incomplete. Remove '$cache' and retry, or set OPENSSL_SRC_PERL to a working native perl.exe."
    }

    $null = New-Item -ItemType Directory -Path $cacheRoot -Force
    $staging = Join-Path $cacheRoot ('strawberry-' + [Guid]::NewGuid().ToString('N'))
    $null = New-Item -ItemType Directory -Path $staging
    try {
        $archive = Join-Path $staging 'perl.zip'
        $runtime = Join-Path $staging 'runtime'
        $url = "https://github.com/StrawberryPerl/Perl-Dist-Strawberry/releases/download/SP_54231_64bit/strawberry-perl-$version-64bit-portable.zip"
        $sha256 = '6a081a811781c30aca51dbc036afd93092af91e3297901f02c17043795a10690'
        Write-Host "Downloading native Strawberry Perl $version for the OpenSSL build (291 MiB)..."
        $ProgressPreference = 'SilentlyContinue'
        [Net.ServicePointManager]::SecurityProtocol = [Net.ServicePointManager]::SecurityProtocol -bor [Net.SecurityProtocolType]::Tls12
        Invoke-WebRequest -Uri $url -OutFile $archive -UseBasicParsing
        if ((Get-FileHash -LiteralPath $archive -Algorithm SHA256).Hash -ne $sha256) {
            throw 'Strawberry Perl download failed SHA256 verification.'
        }

        Add-Type -AssemblyName System.IO.Compression.FileSystem
        $zip = [IO.Compression.ZipFile]::OpenRead($archive)
        try {
            $prefix = [IO.Path]::GetFullPath($runtime) + [IO.Path]::DirectorySeparatorChar
            foreach ($entry in $zip.Entries) {
                if ($entry.FullName -notmatch '^(perl|licenses)/' -or !$entry.Name) { continue }
                $destination = [IO.Path]::GetFullPath((Join-Path $runtime $entry.FullName))
                if (!$destination.StartsWith($prefix, [StringComparison]::OrdinalIgnoreCase)) {
                    throw 'Unexpected path in Strawberry Perl archive.'
                }
                $null = [IO.Directory]::CreateDirectory([IO.Path]::GetDirectoryName($destination))
                [IO.Compression.ZipFileExtensions]::ExtractToFile($entry, $destination)
            }
        } finally { $zip.Dispose() }
        if (!(Test-NativePerl (Join-Path $runtime 'perl\bin\perl.exe'))) {
            throw 'Downloaded Perl cannot run or is missing required core modules.'
        }
        try { [IO.Directory]::Move($runtime, $cache) }
        catch { if (!(Test-NativePerl $cachedPerl)) { throw } }
        if (!(Test-NativePerl $cachedPerl)) { throw 'Cannot use the native Perl cache.' }
        return $cachedPerl
    } catch {
        throw "Cannot prepare native Perl: $($_.Exception.Message) Install Strawberry Perl or set OPENSSL_SRC_PERL to its perl.exe. See docs/windows-build.md."
    } finally {
        $resolvedStaging = [IO.Path]::GetFullPath($staging)
        $cachePrefix = [IO.Path]::GetFullPath($cacheRoot) + [IO.Path]::DirectorySeparatorChar
        if ($resolvedStaging.StartsWith($cachePrefix, [StringComparison]::OrdinalIgnoreCase)) {
            Remove-Item -LiteralPath $resolvedStaging -Recurse -Force
        }
    }
}
