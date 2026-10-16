function Get-WixToolset {
    $cacheRoot = Join-Path $env:LOCALAPPDATA 'Latch/BuildTools'
    $cache = Join-Path $cacheRoot 'WiX-3.14.1'
    if ((Test-Path -LiteralPath (Join-Path $cache 'candle.exe')) -and
        (Test-Path -LiteralPath (Join-Path $cache 'light.exe'))) { return $cache }
    if (Test-Path -LiteralPath $cache) { throw "Incomplete WiX cache. Remove '$cache' and retry." }

    $staging = Join-Path $cacheRoot ('wix-' + [Guid]::NewGuid().ToString('N'))
    $null = New-Item -ItemType Directory -Path $staging -Force
    try {
        $archive = Join-Path $staging 'wix.zip'
        $runtime = Join-Path $staging 'runtime'
        $url = 'https://github.com/wixtoolset/wix3/releases/download/wix3141rtm/wix314-binaries.zip'
        $sha256 = '6ac824e1642d6f7277d0ed7ea09411a508f6116ba6fae0aa5f2c7daa2ff43d31'
        Write-Host 'Downloading WiX Toolset 3.14.1...'
        $ProgressPreference = 'SilentlyContinue'
        [Net.ServicePointManager]::SecurityProtocol = [Net.ServicePointManager]::SecurityProtocol -bor [Net.SecurityProtocolType]::Tls12
        Invoke-WebRequest -Uri $url -OutFile $archive -UseBasicParsing
        if ((Get-FileHash -LiteralPath $archive -Algorithm SHA256).Hash -ne $sha256) {
            throw 'WiX download failed SHA256 verification.'
        }
        Expand-Archive -LiteralPath $archive -DestinationPath $runtime
        if (!(Test-Path -LiteralPath (Join-Path $runtime 'candle.exe')) -or
            !(Test-Path -LiteralPath (Join-Path $runtime 'light.exe'))) { throw 'WiX archive is incomplete.' }
        [IO.Directory]::Move($runtime, $cache)
        return $cache
    } finally {
        $resolvedStaging = [IO.Path]::GetFullPath($staging)
        $cachePrefix = [IO.Path]::GetFullPath($cacheRoot) + [IO.Path]::DirectorySeparatorChar
        if ($resolvedStaging.StartsWith($cachePrefix, [StringComparison]::OrdinalIgnoreCase)) {
            Remove-Item -LiteralPath $resolvedStaging -Recurse -Force
        }
    }
}
