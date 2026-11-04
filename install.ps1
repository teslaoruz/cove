# Install or update Cove on Windows, for the current user (no admin needed).
#   irm https://github.com/teslaoruz/cove/releases/latest/download/install.ps1 | iex
# Set $env:COVE_VERSION = 'v0.2.0' to install a given release instead of the newest.
# Runs inside a script block and never calls `exit`, so piping it to iex cannot close your PowerShell window.
& {
    $ErrorActionPreference = 'Stop'
    $ProgressPreference = 'SilentlyContinue' # the progress bar makes downloads very slow in Windows PowerShell 5.1
    [Net.ServicePointManager]::SecurityProtocol = [Net.ServicePointManager]::SecurityProtocol -bor [Net.SecurityProtocolType]::Tls12

    $repo = 'teslaoruz/cove'
    $asset = 'cove-windows-x86_64.zip'
    if (-not [Environment]::Is64BitOperatingSystem) { throw 'Cove needs 64-bit Windows.' }
    $base = if ($env:COVE_VERSION) { "https://github.com/$repo/releases/download/$env:COVE_VERSION" } else { "https://github.com/$repo/releases/latest/download" }

    $dir = Join-Path $env:LOCALAPPDATA 'Programs\Cove'
    $exe = Join-Path $dir 'cove.exe'
    $tmp = Join-Path ([IO.Path]::GetTempPath()) ('cove-' + [guid]::NewGuid())
    New-Item -ItemType Directory -Force $tmp | Out-Null
    try {
        Write-Host 'Downloading Cove...'
        Invoke-WebRequest -UseBasicParsing "$base/$asset" -OutFile "$tmp\$asset"
        Invoke-WebRequest -UseBasicParsing "$base/SHA256SUMS" -OutFile "$tmp\SHA256SUMS"
        $line = Get-Content "$tmp\SHA256SUMS" | Where-Object { $_ -match ('\s\*?' + [regex]::Escape($asset) + '$') } | Select-Object -First 1
        if (-not $line) { throw "No checksum for $asset. Nothing was installed." }
        $want = ($line -split '\s+')[0]
        $got = (Get-FileHash "$tmp\$asset" -Algorithm SHA256).Hash
        if ($got -ne $want) { throw 'The download does not match its checksum. Nothing was installed.' }

        Expand-Archive "$tmp\$asset" -DestinationPath "$tmp\x" -Force
        New-Item -ItemType Directory -Force $dir | Out-Null
        # a running exe cannot be overwritten, but it can be renamed; Cove deletes the .old copy on its next start
        if (Test-Path $exe) {
            Remove-Item "$exe.old" -Force -ErrorAction SilentlyContinue
            Move-Item $exe "$exe.old"
        }
        Copy-Item "$tmp\x\cove\*" $dir -Recurse -Force

        $lnk = Join-Path ([Environment]::GetFolderPath('Programs')) 'Cove.lnk'
        $shortcut = (New-Object -ComObject WScript.Shell).CreateShortcut($lnk)
        $shortcut.TargetPath = $exe
        $shortcut.WorkingDirectory = $dir
        $shortcut.Description = 'Your projects and coding agents in one place'
        $shortcut.Save()

        $path = [Environment]::GetEnvironmentVariable('Path', 'User')
        if (($path -split ';') -notcontains $dir) {
            [Environment]::SetEnvironmentVariable('Path', ((@($path, $dir) | Where-Object { $_ }) -join ';'), 'User')
        }
        Write-Host 'Cove is installed. Start it from the Start menu, or run "cove" in a new terminal.'
    } finally {
        Remove-Item $tmp -Recurse -Force -ErrorAction SilentlyContinue
    }
}
