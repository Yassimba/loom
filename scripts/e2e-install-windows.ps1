param()

$ErrorActionPreference = "Stop"
$Workspace = $env:GITHUB_WORKSPACE
$EvidenceDir = Join-Path $env:RUNNER_TEMP "loom-install-e2e"
New-Item -ItemType Directory -Path $EvidenceDir -Force | Out-Null
$PowerShellExe = (Get-Command $env:LOOM_E2E_POWERSHELL -ErrorAction Stop).Source
$TargetProfile = (& $PowerShellExe -NoProfile -Command '$PROFILE').Trim()
$Skill = if ($env:LOOM_E2E_SKILL) { $env:LOOM_E2E_SKILL } else { "next" }
$ExpectBeads = $Skill -eq "next"

$SetupArgs = @(
  "--skill", $Skill,
  "--agent", "agents",
  "--agent", "claude",
  "--agent", "codex",
  "--agent", "opencode",
  "--agent", "pi",
  "--yes"
)

function Invoke-Bootstrap {
  $Installer = (Join-Path $Workspace "install.ps1").Replace("'", "''")
  $ArgumentLiterals = ($SetupArgs | ForEach-Object {
    $EscapedArgument = $_.Replace("'", "''")
    "'$EscapedArgument'"
  }) -join ", "
  $Command = "& '$Installer' -SetupArgs @($ArgumentLiterals)"
  & $PowerShellExe -NoProfile -ExecutionPolicy Bypass -Command $Command
  if ($LASTEXITCODE -ne 0) { throw "bootstrap failed with exit code $LASTEXITCODE" }
}

$BootstrapLog = Join-Path $EvidenceDir "bootstrap.txt"
Invoke-Bootstrap *> $BootstrapLog

$PersistentPaths = @(
  [Environment]::GetEnvironmentVariable("Path", [System.EnvironmentVariableTarget]::Machine)
  [Environment]::GetEnvironmentVariable("Path", [System.EnvironmentVariableTarget]::User)
) | Where-Object { $_ }
foreach ($PersistentPath in $PersistentPaths) {
  $env:Path = $env:Path.TrimEnd(';') + ";" + $PersistentPath
}
foreach ($Candidate in @(
  (Join-Path $env:LOCALAPPDATA "Microsoft\WinGet\Links"),
  (Join-Path $HOME "scoop\shims"),
  (Join-Path $HOME ".local\bin")
)) {
  if (Test-Path $Candidate) { $env:Path = $env:Path.TrimEnd(';') + ";" + $Candidate }
}
$Mise = (Get-Command mise -ErrorAction Stop).Source
& $Mise exec -- loom --version *> (Join-Path $EvidenceDir "loom-version.txt")
if ($LASTEXITCODE -ne 0) { throw "installed Loom did not run" }
$LoomVersion = ((Get-Content (Join-Path $EvidenceDir "loom-version.txt") -Raw) -split '\s+')[1]
if ($LoomVersion -eq "0.10.0") {
  "deferred until Loom 0.10.1 is published" | Set-Content (Join-Path $EvidenceDir "tokei-version.txt")
} elseif ($env:LOOM_E2E_TOKEI -eq "skip") {
  "unsupported: upstream Tokei publishes no Windows ARM64 asset" | Set-Content (Join-Path $EvidenceDir "tokei-version.txt")
} else {
  & $Mise exec -- loom add --tool tokei --yes *> (Join-Path $EvidenceDir "tokei-install.txt")
  if ($LASTEXITCODE -ne 0) { throw "tokei install failed" }
  & $Mise exec -- tokei --version *> (Join-Path $EvidenceDir "tokei-version.txt")
  if ($LASTEXITCODE -ne 0) { throw "tokei did not run" }
}
& $Mise exec -- loom status *> (Join-Path $EvidenceDir "loom-status.txt")
if ($LASTEXITCODE -ne 0) { throw "loom status failed" }
if ($ExpectBeads) {
  & $Mise exec -- br --version *> (Join-Path $EvidenceDir "br-version.txt")
  if ($LASTEXITCODE -ne 0) { throw "br did not run" }
  & $Mise exec -- bv --version *> (Join-Path $EvidenceDir "bv-version.txt")
  if ($LASTEXITCODE -ne 0) { throw "bv did not run" }
}
& $Mise doctor *> (Join-Path $EvidenceDir "mise-doctor.txt")

$Selection = Join-Path $HOME ".config\mise\conf.d\loom.toml"
if (-not (Test-Path $Selection)) { throw "mise selection was not created" }
$SelectionText = Get-Content $Selection -Raw
if ([regex]::Matches($SelectionText, '(?m)^# core:begin').Count -ne 1) {
  throw "selection must contain exactly one core start marker"
}
if ([regex]::Matches($SelectionText, '(?m)^# core:end').Count -ne 1) {
  throw "selection must contain exactly one core end marker"
}
if ($ExpectBeads) {
  foreach ($Expected in @("beads_rust", "beads_viewer")) {
    if (-not $SelectionText.Contains($Expected)) { throw "selection is missing $Expected" }
  }
}
if ($LoomVersion -ne "0.10.0" -and $env:LOOM_E2E_TOKEI -ne "skip" -and -not $SelectionText.Contains('"aqua:XAMPPRocky/tokei"')) {
  throw "selection is missing the native Tokei backend"
}

$SkillRoots = @(
  (Join-Path $HOME ".agents\skills"),
  (Join-Path $HOME ".claude\skills"),
  (Join-Path $HOME ".codex\skills"),
  (Join-Path $HOME ".config\opencode\skills"),
  (Join-Path $HOME ".pi\agent\skills")
)
foreach ($SkillRoot in $SkillRoots) {
  if (-not (Test-Path (Join-Path $SkillRoot "$Skill\SKILL.md"))) {
    throw "$Skill skill is missing from $SkillRoot"
  }
}
if (-not (Test-Path (Join-Path $HOME ".config\opencode\plugins\loom-session-env.js"))) {
  throw "OpenCode adapter was not installed"
}

$FreshShell = Join-Path $EvidenceDir "fresh-shell.txt"
& $PowerShellExe -Command "Get-Command mise -ErrorAction Stop; Get-Command loom -ErrorAction Stop; loom --version" *> $FreshShell
if ($LASTEXITCODE -ne 0) { throw "Loom is unavailable in a fresh PowerShell" }

Invoke-Bootstrap *> (Join-Path $EvidenceDir "rerun.txt")

$ProfileText = Get-Content $TargetProfile -Raw
$MiseQuoted = $Mise.Replace("'", "''")
$MiseDirQuoted = (Split-Path -Parent $Mise).Replace("'", "''")
$Activation = "`$env:Path = '$MiseDirQuoted;' + `$env:Path; (& '$MiseQuoted' activate pwsh) | Out-String | Invoke-Expression"
if (($ProfileText | Select-String -SimpleMatch $Activation -AllMatches).Matches.Count -ne 1) {
  throw "mise activation must appear exactly once in the PowerShell profile"
}
$SelectionText = Get-Content $Selection -Raw
if ($ExpectBeads) {
  foreach ($Expected in @("beads_rust", "beads_viewer")) {
    if (-not $SelectionText.Contains($Expected)) { throw "rerun dropped $Expected" }
  }
}

$InstalledFiles = foreach ($SkillRoot in $SkillRoots) {
  Get-ChildItem (Join-Path $SkillRoot $Skill) -File -Recurse | Select-Object -ExpandProperty FullName
}
$InstalledFiles | Sort-Object | Set-Content (Join-Path $EvidenceDir "installed-files.txt")
Copy-Item $Selection (Join-Path $EvidenceDir "loom.toml")

Get-ChildItem $EvidenceDir -Filter *.txt | Sort-Object Name | ForEach-Object {
  Write-Host "===== $($_.Name) ====="
  Get-Content $_.FullName
}
