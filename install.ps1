$ErrorActionPreference = "Stop"
# ai-setup bootstrap: ensure mise, sync the published tool manifest into
# mise's conf.d, install its exact pins (including the ai-setup CLI itself),
# then hand off to the guided setup. Tools update only when a new manifest
# lands on main and `ai-setup update` re-syncs it.

# TLS 1.2 for downloads on hosts whose .NET default predates it; additive
# (-bor) so stronger protocols stay enabled.
[Net.ServicePointManager]::SecurityProtocol = [Net.ServicePointManager]::SecurityProtocol -bor 3072
# Invoke-WebRequest's progress bar slows downloads badly under PowerShell 5.1.
$script:ProgressPreference = "SilentlyContinue"

$Name = "ai-setup"
$Repo = "Yassimba/ai-setup"
$ManifestUrl = "https://raw.githubusercontent.com/$Repo/main/manifest/ai-setup.toml"
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
  # The nested powershell of the README one-liner inherits the pre-install
  # environment; extend PATH for this session from the usual install homes.
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

# 2. The published manifest, merged by mise without touching the user's own
#    config.toml (that file stays theirs, as a personal overlay).
New-Item -ItemType Directory -Path $ConfD -Force | Out-Null
Get-Url $ManifestUrl (Join-Path $ConfD "ai-setup.toml")
Write-Host "$Name`: manifest synced to $(Join-Path $ConfD "ai-setup.toml")"

# 3. Install the pins - node, pi, the ai-setup CLI, and the rest.
mise install --yes
if ($LASTEXITCODE -ne 0) { throw "$Name`: mise install failed" }

# 4. Hand off to the guided setup with the freshly installed tools on PATH.
Write-Host ""
mise exec -- ai-setup setup
exit $LASTEXITCODE
