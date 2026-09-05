param(
  [string[]]$SetupArgs = @()
)

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

function Restore-AtomicPath([string]$Path) {
  $Parent = Split-Path -Parent $Path
  $Name = Split-Path -Leaf $Path
  $Backup = Join-Path $Parent ".$Name.loom-old"
  if (-not (Test-Path $Path) -and (Test-Path $Backup)) {
    Move-Item -LiteralPath $Backup -Destination $Path
  } elseif (Test-Path $Path) {
    Remove-Item -LiteralPath $Backup -Force -ErrorAction SilentlyContinue
  }
}

function Set-AtomicLines([string]$Path, [string[]]$Lines) {
  $Parent = Split-Path -Parent $Path
  New-Item -ItemType Directory -Path $Parent -Force | Out-Null
  $Name = Split-Path -Leaf $Path
  $Incoming = Join-Path $Parent ".$Name.loom-new"
  $Backup = Join-Path $Parent ".$Name.loom-old"
  Remove-Item -LiteralPath $Incoming -Force -ErrorAction SilentlyContinue
  $Text = ($Lines -join [Environment]::NewLine) + [Environment]::NewLine
  [System.IO.File]::WriteAllText($Incoming, $Text, [System.Text.UTF8Encoding]::new($false))
  if (Test-Path $Path) {
    [System.IO.File]::Replace($Incoming, $Path, $Backup)
    Remove-Item -LiteralPath $Backup -Force -ErrorAction SilentlyContinue
  } else {
    Move-Item -LiteralPath $Incoming -Destination $Path
  }
}

function Install-MiseRelease {
  $Headers = @{
    Accept = "application/vnd.github+json"
    "User-Agent" = "loom-installer"
  }
  if ($env:GITHUB_TOKEN) { $Headers.Authorization = "Bearer $($env:GITHUB_TOKEN)" }
  $Release = Invoke-RestMethod -Uri "https://api.github.com/repos/jdx/mise/releases/latest" -Headers $Headers
  $Architecture = [Runtime.InteropServices.RuntimeInformation]::OSArchitecture.ToString().ToLowerInvariant()
  if ($Architecture -notin @("arm64", "x64")) {
    throw "$Name`: mise has no supported Windows asset for $Architecture"
  }
  $AssetName = "mise-$($Release.tag_name)-windows-$Architecture.zip"
  $Asset = $Release.assets | Where-Object { $_.name -eq $AssetName } | Select-Object -First 1
  $Checksums = $Release.assets | Where-Object { $_.name -eq "SHASUMS256.txt" } | Select-Object -First 1
  if (-not $Asset -or -not $Checksums) { throw "$Name`: mise release assets are incomplete" }

  $TmpDirectory = Join-Path ([System.IO.Path]::GetTempPath()) ([System.Guid]::NewGuid().ToString())
  New-Item -ItemType Directory -Path $TmpDirectory | Out-Null
  try {
    $TmpArchive = Join-Path $TmpDirectory $AssetName
    $TmpChecksums = Join-Path $TmpDirectory "SHASUMS256.txt"
    Get-Url $Asset.browser_download_url $TmpArchive
    Get-Url $Checksums.browser_download_url $TmpChecksums
    $ChecksumLine = Get-Content $TmpChecksums | Where-Object { $_.EndsWith($AssetName) } | Select-Object -First 1
    if (-not $ChecksumLine) { throw "$Name`: mise checksum is missing for $AssetName" }
    $Expected = ($ChecksumLine -split '\s+')[0]
    $Actual = (Get-FileHash -Path $TmpArchive -Algorithm SHA256).Hash
    if ($Actual -ne $Expected) { throw "$Name`: mise checksum verification failed" }

    $InstallDirectory = Join-Path $HOME ".local\bin"
    New-Item -ItemType Directory -Path $InstallDirectory -Force | Out-Null
    $Extracted = Join-Path $TmpDirectory "extracted"
    Expand-Archive -Path $TmpArchive -DestinationPath $Extracted
    Copy-Item (Join-Path $Extracted "mise\bin\mise.exe") $InstallDirectory -Force
    Copy-Item (Join-Path $Extracted "mise\bin\mise-shim.exe") $InstallDirectory -Force
    $UserPath = [Environment]::GetEnvironmentVariable("Path", [System.EnvironmentVariableTarget]::User)
    $UserEntries = @($UserPath -split ';' | Where-Object { $_ })
    if ($InstallDirectory -notin $UserEntries) {
      $NewUserPath = (@($InstallDirectory) + $UserEntries) -join ';'
      [Environment]::SetEnvironmentVariable("Path", $NewUserPath, [System.EnvironmentVariableTarget]::User)
      $script:MisePathAdded = $true
    }
    $env:Path = "$InstallDirectory;" + $env:Path
  } finally {
    Remove-Item $TmpDirectory -Recurse -Force -ErrorAction SilentlyContinue
  }
}

# 1. mise - the only thing this script installs itself.
$MiseInstalledByLoom = $false
$MiseInstallMethod = ""
$MisePathAdded = $false
$PendingMise = Join-Path $HOME ".config\loom\bootstrap-mise-pending.json"
Restore-AtomicPath $PendingMise
if ((Get-Command mise -ErrorAction SilentlyContinue) -and (Test-Path $PendingMise)) {
  try {
    $Pending = Get-Content $PendingMise -Raw | ConvertFrom-Json
    $MiseInstalledByLoom = $true
    $MiseInstallMethod = [string]$Pending.manager
    $MisePathAdded = [bool]$Pending.pathAdded
  } catch {
    throw "$Name`: could not read pending mise ownership from $PendingMise`: $($_.Exception.Message)"
  }
}
if (-not (Get-Command mise -ErrorAction SilentlyContinue)) {
  $MiseInstalledByLoom = $true
  Write-Host "$Name`: installing mise (https://mise.jdx.dev)..."
  $installed = $false
  if (Get-Command winget -ErrorAction SilentlyContinue) {
    winget install --id jdx.mise --silent --accept-package-agreements --accept-source-agreements
    if ($LASTEXITCODE -eq 0) { $installed = $true; $MiseInstallMethod = "winget" }
  }
  if (-not $installed -and (Get-Command scoop -ErrorAction SilentlyContinue)) {
    scoop install mise
    if ($LASTEXITCODE -eq 0) { $installed = $true; $MiseInstallMethod = "scoop" }
  }
  if (-not $installed) {
    Install-MiseRelease
    $MiseInstallMethod = "direct"
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
if ($MiseInstalledByLoom) {
  $PendingJson = ConvertTo-Json -Compress @{
    manager = $MiseInstallMethod
    pathAdded = $MisePathAdded
  }
  Set-AtomicLines $PendingMise @($PendingJson)
}
if (-not (Get-Command mise -ErrorAction SilentlyContinue)) {
  throw "$Name`: mise is installed but not on PATH yet; open a new terminal and rerun this installer"
}

# 2. Refresh the required core block - node and the Loom CLI - while keeping
#    any optional tools already chosen through the wizard. This also repairs
#    selections left incomplete by an interrupted or older bootstrap.
$Selection = Join-Path $ConfD "loom.toml"
Restore-AtomicPath $Selection
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

  if (Test-Path $Selection) {
    $updated = [System.Collections.Generic.List[string]]::new()
    $inserted = $false
    $skippingCore = $false
    foreach ($line in Get-Content $Selection) {
      if ($line.StartsWith("# core:begin")) { $skippingCore = $true; continue }
      if ($skippingCore) {
        if ($line.StartsWith("# core:end")) { $skippingCore = $false }
        continue
      }
      $updated.Add($line)
      if (-not $inserted -and $line -eq "[tools]") {
        $updated.AddRange([string[]]$core)
        $inserted = $true
      }
    }
    if ($skippingCore) { throw "$Name`: existing selection has an incomplete core block" }
    if (-not $inserted) { throw "$Name`: existing selection is missing its [tools] table" }
    Set-AtomicLines $Selection @($updated)
  } else {
    Set-AtomicLines $Selection @(@("# Managed by Loom: the selected tools from the published manifest.", "", "[tools]") + $core)
  }
  Write-Host "$Name`: core tools synced to $Selection"
} finally {
  Remove-Item $TmpManifest -Force -ErrorAction SilentlyContinue
}

# 3. Install the pins - node and the Loom CLI (plus any prior selection).
mise -C $HOME install --yes
if ($LASTEXITCODE -ne 0) { throw "$Name`: mise install failed" }

# 4. Activate mise now and persist it for both Windows PowerShell and pwsh.
$MiseCommand = (Get-Command mise -ErrorAction Stop).Source
$MiseExe = $MiseCommand.Replace("'", "''")
$MiseDir = (Split-Path -Parent $MiseCommand).Replace("'", "''")
$Activation = "`$env:Path = '$MiseDir;' + `$env:Path; (& '$MiseExe' activate pwsh) | Out-String | Invoke-Expression"
(& $MiseCommand activate pwsh) | Out-String | Invoke-Expression
$Documents = if ($env:LOOM_E2E_DOCUMENTS_DIR) {
  $env:LOOM_E2E_DOCUMENTS_DIR
} else {
  [Environment]::GetFolderPath([Environment+SpecialFolder]::MyDocuments)
}
if (-not $Documents) { $Documents = Join-Path $HOME "Documents" }
$Profiles = @(
  [string]$PROFILE,
  (Join-Path $Documents "WindowsPowerShell\profile.ps1"),
  (Join-Path $Documents "PowerShell\profile.ps1")
) | Select-Object -Unique
$ChangedProfiles = @()
foreach ($ProfilePath in $Profiles) {
  Restore-AtomicPath $ProfilePath
  $ProfileContent = if (Test-Path $ProfilePath) { [string](Get-Content $ProfilePath -Raw) } else { "" }
  if (-not $ProfileContent.Contains($Activation)) {
    $ExistingProfile = $ProfileContent.TrimEnd("`r", "`n")
    $ProfileLines = if ($ExistingProfile) { @($ExistingProfile, $Activation) } else { @($Activation) }
    Set-AtomicLines $ProfilePath $ProfileLines
    $ChangedProfiles += $ProfilePath
  }
}
if ($ChangedProfiles.Count -gt 0) {
  Write-Host "$Name`: added mise activation to $($ChangedProfiles -join ', ')"
  Write-Host "$Name`: open a new PowerShell, or run: . `$PROFILE"
}

# 5. Hand off to the guided setup with the freshly installed tools on PATH.
# CI may point this handoff at the checked-out binary while keeping the real
# bootstrap and manifest path intact.
$env:LOOM_BOOTSTRAP = "1"
$env:LOOM_BOOTSTRAP_MISE_INSTALLED = if ($MiseInstalledByLoom) { "1" } else { "0" }
$env:LOOM_BOOTSTRAP_MISE_ROOT = if ($env:MISE_DATA_DIR) { $env:MISE_DATA_DIR } elseif ($env:XDG_DATA_HOME) { Join-Path $env:XDG_DATA_HOME "mise" } else { Join-Path $env:LOCALAPPDATA "mise" }
$env:LOOM_BOOTSTRAP_MISE_EXECUTABLE = $MiseCommand
$env:LOOM_BOOTSTRAP_MISE_MANAGER = $MiseInstallMethod
$env:LOOM_BOOTSTRAP_MISE_PATH_ADDED = if ($MisePathAdded) { "1" } else { "0" }
$env:LOOM_BOOTSTRAP_MISE_PATH_ENTRY = Split-Path -Parent $MiseCommand
$env:LOOM_BOOTSTRAP_ACTIVATION_LINE = $Activation
$env:LOOM_BOOTSTRAP_ACTIVATION_PATHS_JSON = ConvertTo-Json -Compress @($ChangedProfiles)
Write-Host ""
if ($env:LOOM_E2E_LOOM_BIN) {
  & $env:LOOM_E2E_LOOM_BIN setup @SetupArgs
} else {
  mise -C $HOME exec -- loom setup @SetupArgs
}
$SetupExit = $LASTEXITCODE
if ($SetupExit -eq 0) {
  Remove-Item -LiteralPath $PendingMise -Force -ErrorAction SilentlyContinue
}
exit $SetupExit
