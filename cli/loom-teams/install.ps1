# loom-teams standalone installer (Windows).
#   powershell -NoProfile -ExecutionPolicy Bypass -Command "irm https://raw.githubusercontent.com/Yassimba/loom/main/cli/loom-teams/install.ps1 | iex"
# Downloads the release binary pinned by the published manifest, verifies its
# checksum, and installs it to %USERPROFILE%\.local\bin.

$ErrorActionPreference = "Stop"

$Name = "loom-teams"
$Repo = "Yassimba/loom"
$ManifestUrl = "https://raw.githubusercontent.com/$Repo/main/manifest/loom.toml"
$BinDir = if ($env:LOOM_TEAMS_INSTALL_DIR) { $env:LOOM_TEAMS_INSTALL_DIR } else { Join-Path $env:USERPROFILE ".local\bin" }

$target = switch ($env:PROCESSOR_ARCHITECTURE) {
    "ARM64" { "aarch64-pc-windows-msvc" }
    "AMD64" { "x86_64-pc-windows-msvc" }
    default { throw "${Name}: unsupported architecture $env:PROCESSOR_ARCHITECTURE" }
}

# Windows PowerShell 5.1 has no -MaximumRetryCount; retry by hand.
function Invoke-WithRetry([scriptblock]$Action) {
    for ($attempt = 1; $attempt -le 5; $attempt++) {
        try { return & $Action }
        catch {
            if ($attempt -eq 5) { throw }
            Start-Sleep -Seconds 3
        }
    }
}

# The published manifest pins the released tag; install exactly that.
$manifest = Invoke-WithRetry { Invoke-RestMethod -Uri $ManifestUrl }
$tag = [regex]::Match(
    $manifest,
    '(?m)^"github:Yassimba/loom\[exe=loom-teams\]" = \{ version = "([^"]+)"'
).Groups[1].Value
if (-not $tag) { throw "${Name}: could not read the release pin from the manifest" }

$asset = "$Name-$target.zip"
$url = "https://github.com/$Repo/releases/download/$tag/$asset"
$tmp = Join-Path ([System.IO.Path]::GetTempPath()) ([System.IO.Path]::GetRandomFileName())
New-Item -ItemType Directory -Path $tmp | Out-Null

try {
    Write-Host "${Name}: downloading $tag for $target..."
    Invoke-WithRetry { Invoke-WebRequest -Uri $url -OutFile (Join-Path $tmp $asset) -UseBasicParsing }
    Invoke-WithRetry { Invoke-WebRequest -Uri "$url.sha256" -OutFile (Join-Path $tmp "$asset.sha256") -UseBasicParsing }

    $expected = ((Get-Content (Join-Path $tmp "$asset.sha256") -Raw).Trim() -split "\s+")[0]
    $actual = (Get-FileHash (Join-Path $tmp $asset) -Algorithm SHA256).Hash
    if ($expected -ne $actual) { throw "${Name}: checksum mismatch for $asset" }

    Expand-Archive -Path (Join-Path $tmp $asset) -DestinationPath $tmp -Force
    New-Item -ItemType Directory -Path $BinDir -Force | Out-Null
    Copy-Item (Join-Path $tmp "$Name.exe") (Join-Path $BinDir "$Name.exe") -Force
    Write-Host "${Name}: installed $tag to $BinDir\$Name.exe"
}
finally {
    Remove-Item -Recurse -Force $tmp -ErrorAction SilentlyContinue
}

$userPath = [Environment]::GetEnvironmentVariable("Path", "User")
if (($userPath -split ";") -notcontains $BinDir) {
    [Environment]::SetEnvironmentVariable("Path", "$BinDir;$userPath", "User")
    Write-Host "${Name}: added $BinDir to your user PATH (open a new terminal to pick it up)"
}

Write-Host ""
Write-Host "Next: run ``$Name setup`` and sign in to Teams once."
