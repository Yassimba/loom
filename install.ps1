$ErrorActionPreference = "Stop"
# Loom bootstrap: ensure mise, sync the published tool manifest into
# mise's conf.d, install its exact pins (including the Loom CLI itself),
# then hand off to the guided setup. Tools update only when a new manifest
# lands on main and `loom update` re-syncs it.

# TLS 1.2 for downloads on hosts whose .NET default predates it; additive
# (-bor) so stronger protocols stay enabled.
[Net.ServicePointManager]::SecurityProtocol = [Net.ServicePointManager]::SecurityProtocol -bor 3072
# Invoke-WebRequest's progress bar slows downloads badly under PowerShell 5.1.
$script:ProgressPreference = "SilentlyContinue"

$Name = "loom"
$Repo = "Yassimba/loom"
$ManifestUrl = "https://raw.githubusercontent.com/$Repo/main/manifest/loom.toml"
$ConfD = Join-Path $HOME ".config\mise\conf.d"

function Get-Url([string]$Url, [string]$OutFile) {
  for ($attempt = 1; $attempt -le 5; $attempt++) {
    try {
      Invoke-WebRequest -Uri $Url -OutFile $OutFile -MaximumRedirection 5 -UseBasicParsing
      return
    } catch {
      if ($attempt -eq 5) { throw }
      Start-Sleep -Seconds 3
    }
  }
}

# 1. mise - the only thing this script installs itself.
if (-not (Get-Command mise -ErrorAction SilentlyContinue)) {
  Write-Host "$Name`: installing mise (https://mise.jdx.dev)..."
  $installed = $false
  if (Get-Command winget -ErrorAction SilentlyContinue) {
    winget install --id jdx.mise --silent --accept-package-agreements --accept-source-agreements
    if ($LASTEXITCODE -eq 0) { $installed = $true }
  }
  if (-not $installed -and (Get-Command scoop -ErrorAction SilentlyContinue)) {
    scoop install mise
    if ($LASTEXITCODE -eq 0) { $installed = $true }
  }
  if (-not $installed) {
    throw "$Name`: could not install mise automatically; install it from https://mise.jdx.dev/installing-mise.html and rerun"
  }
  # The nested PowerShell of the README one-liner keeps its pre-install PATH.
  # Pull in the paths that WinGet or Scoop just persisted for future shells.
  $persistentPaths = @(
    [Environment]::GetEnvironmentVariable("Path", [System.EnvironmentVariableTarget]::Machine)
    [Environment]::GetEnvironmentVariable("Path", [System.EnvironmentVariableTarget]::User)
  ) | Where-Object { $_ }
  foreach ($path in $persistentPaths) {
    $env:Path = "$env:Path".TrimEnd(';') + ";" + $path
  }

  # Keep the common install homes as fallbacks for package managers that did
  # not persist their shim directory.
  $candidates = @(
    (Join-Path $env:LOCALAPPDATA "Microsoft\WinGet\Links"),
    (Join-Path $HOME "scoop\shims")
  )
  foreach ($dir in $candidates) {
    if (Test-Path $dir) { $env:Path = "$env:Path".TrimEnd(';') + ";" + $dir }
  }
}
if (-not (Get-Command mise -ErrorAction SilentlyContinue)) {
  throw "$Name`: mise is installed but not on PATH yet; open a new terminal and rerun this installer"
}

# 2. The manifest's core block only - node and the Loom CLI. Everything
#    else is optional and chosen in the wizard, which appends to this file.
#    An existing Loom selection is left alone (loom update refreshes it).
New-Item -ItemType Directory -Path $ConfD -Force | Out-Null
$Selection = Join-Path $ConfD "loom.toml"
if (-not (Test-Path $Selection)) {
  $TmpManifest = Join-Path ([System.IO.Path]::GetTempPath()) ([System.Guid]::NewGuid().ToString() + ".toml")
  try {
    Get-Url $ManifestUrl $TmpManifest
    $lines = Get-Content $TmpManifest
    $begin = -1
    $end = -1
    for ($i = 0; $i -lt $lines.Count; $i++) {
      if ($begin -lt 0 -and $lines[$i].StartsWith("# core:begin")) { $begin = $i }
      if ($begin -ge 0 -and $lines[$i].StartsWith("# core:end")) { $end = $i; break }
    }
    if ($begin -lt 0 -or $end -lt $begin) { throw "$Name`: manifest is missing its core block" }
    $core = $lines[$begin..$end]
    @("# Managed by Loom: the selected tools from the published manifest.", "", "[tools]") + $core |
      Set-Content -Path $Selection
    Write-Host "$Name`: core tools synced to $Selection"
  } finally {
    Remove-Item $TmpManifest -Force -ErrorAction SilentlyContinue
  }
}

# 3. Install the pins - node and the Loom CLI (plus any prior selection).
mise install --yes
if ($LASTEXITCODE -ne 0) { throw "$Name`: mise install failed" }

# 4. Hand off to the guided setup with the freshly installed tools on PATH.
Write-Host ""
mise exec -- loom setup
exit $LASTEXITCODE
