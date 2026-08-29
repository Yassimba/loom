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
  "--agent", "cursor",
  "--agent", "grok",
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
function Invoke-Loom {
  $WorkingDirectory = (Get-Location).Path.Replace("'", "''")
  $Loom = $(if ($env:LOOM_E2E_LOOM_BIN) { $env:LOOM_E2E_LOOM_BIN } else { "loom" }).Replace("'", "''")
  $Arguments = ($args | ForEach-Object { "'" + $_.Replace("'", "''") + "'" }) -join " "
  $Command = "Set-Location -LiteralPath '$WorkingDirectory'; & '$Loom' $Arguments; exit `$LASTEXITCODE"
  & $Mise -C $HOME exec -- $PowerShellExe -NoProfile -Command $Command
}
Invoke-Loom --version *> (Join-Path $EvidenceDir "loom-version.txt")
if ($LASTEXITCODE -ne 0) { throw "installed Loom did not run" }
if ($env:LOOM_E2E_TOKEI -eq "skip") {
  "unsupported: upstream Tokei publishes no Windows ARM64 asset" | Set-Content (Join-Path $EvidenceDir "tokei-version.txt")
} else {
  Invoke-Loom add --tool tokei --yes *> (Join-Path $EvidenceDir "tokei-install.txt")
  if ($LASTEXITCODE -ne 0) { throw "tokei install failed" }
  & $Mise -C $HOME exec -- tokei --version *> (Join-Path $EvidenceDir "tokei-version.txt")
  if ($LASTEXITCODE -ne 0) { throw "tokei did not run" }
}
$StatusLog = Join-Path $EvidenceDir "loom-status.txt"
Invoke-Loom status *> $StatusLog
if ($LASTEXITCODE -ne 0) { throw "loom status failed" }
$Project = Join-Path $env:RUNNER_TEMP "loom-first-project"
New-Item -ItemType Directory -Path $Project -Force | Out-Null
Push-Location $Project
try {
  Invoke-Loom init --yes *> (Join-Path $EvidenceDir "loom-init.txt")
  if ($LASTEXITCODE -ne 0) { throw "loom init failed" }
} finally {
  Pop-Location
}
if (-not (Test-Path (Join-Path $Project "AGENTS.md"))) { throw "loom init omitted AGENTS.md" }
if (-not (Test-Path (Join-Path $Project "CLAUDE.md"))) { throw "loom init omitted CLAUDE.md" }
Invoke-Loom update --yes *> (Join-Path $EvidenceDir "loom-update.txt")
if ($LASTEXITCODE -ne 0) { throw "loom update failed" }
if (-not (Select-String -Quiet -Path $StatusLog -Pattern "Selected resources and runtimes verified")) {
  throw "status omitted the verified-resource verdict"
}
if ($ExpectBeads) {
  & $Mise -C $HOME exec -- br --version *> (Join-Path $EvidenceDir "br-version.txt")
  if ($LASTEXITCODE -ne 0) { throw "br did not run" }
  & $Mise -C $HOME exec -- bv --version *> (Join-Path $EvidenceDir "bv-version.txt")
  if ($LASTEXITCODE -ne 0) { throw "bv did not run" }
}
& $Mise doctor *> (Join-Path $EvidenceDir "mise-doctor.txt")

$Selection = Join-Path $HOME ".config\mise\conf.d\loom.toml"
if (-not (Test-Path $Selection)) { throw "mise selection was not created" }
$SelectionText = Get-Content $Selection -Raw
$SelectionBefore = $SelectionText
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
if ($env:LOOM_E2E_TOKEI -ne "skip" -and -not $SelectionText.Contains('"aqua:XAMPPRocky/tokei"')) {
  throw "selection is missing the native Tokei backend"
}

$SkillRoots = @(
  (Join-Path $HOME ".agents\skills"),
  (Join-Path $HOME ".claude\skills"),
  (Join-Path $HOME ".codex\skills"),
  (Join-Path $HOME ".config\opencode\skills"),
  (Join-Path $HOME ".cursor\skills"),
  (Join-Path $HOME ".grok\skills"),
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

$SkillHashBefore = (Get-FileHash (Join-Path $SkillRoots[0] "$Skill\SKILL.md") -Algorithm SHA256).Hash
$RerunLog = Join-Path $EvidenceDir "rerun.txt"
Invoke-Bootstrap *> $RerunLog

$ProfileText = Get-Content $TargetProfile -Raw
$ProfileText | Set-Content (Join-Path $EvidenceDir "profile.txt")
$MiseQuoted = $Mise.Replace("'", "''")
$MiseDirQuoted = (Split-Path -Parent $Mise).Replace("'", "''")
$Activation = "`$env:Path = '$MiseDirQuoted;' + `$env:Path; (& '$MiseQuoted' activate pwsh) | Out-String | Invoke-Expression"
if ([regex]::Matches($ProfileText, [regex]::Escape($Activation)).Count -ne 1) {
  throw "mise activation must appear exactly once in the PowerShell profile"
}
$SelectionText = Get-Content $Selection -Raw
if ($SelectionText.Replace("`r`n", "`n") -ne $SelectionBefore.Replace("`r`n", "`n")) {
  throw "rerun changed the selected tool set"
}
$SkillHashAfter = (Get-FileHash (Join-Path $SkillRoots[0] "$Skill\SKILL.md") -Algorithm SHA256).Hash
if ($SkillHashAfter -ne $SkillHashBefore) { throw "rerun changed the installed skill" }
$Manifest = Join-Path $(if ($env:LOOM_REPO_DIR) { $env:LOOM_REPO_DIR } else { $Workspace }) "manifest\loom.toml"
$ManifestLines = Get-Content $Manifest
$InsideCore = $false
foreach ($Line in $ManifestLines) {
  if ($Line.StartsWith("# core:begin")) { $InsideCore = $true; continue }
  if ($Line.StartsWith("# core:end")) { $InsideCore = $false; continue }
  if ($InsideCore -and $Line -match '^[^#\s].*=') {
    if (-not ($SelectionText -split "`r?`n").Contains($Line)) { throw "selection changed exact core pin: $Line" }
  }
}
if ($env:LOOM_E2E_TOKEI -ne "skip") {
  $TokeiPin = $ManifestLines | Where-Object { $_ -match '^"aqua:XAMPPRocky/tokei"' } | Select-Object -First 1
  if (-not ($SelectionText -split "`r?`n").Contains($TokeiPin)) { throw "selection changed the exact Tokei pin" }
  if (-not (Select-String -Quiet -Path (Join-Path $EvidenceDir "tokei-version.txt") -SimpleMatch 'tokei 12.1.2')) {
    throw "Tokei version did not match its exact pin"
  }
}
$Cargo = Get-Content (Join-Path $(if ($env:LOOM_REPO_DIR) { $env:LOOM_REPO_DIR } else { $Workspace }) "cli\loom\Cargo.toml")
$LoomVersion = ([regex]::Match(($Cargo -join "`n"), '(?m)^version = "([^"]+)"')).Groups[1].Value
if (-not (Select-String -Quiet -Path (Join-Path $EvidenceDir "loom-version.txt") -SimpleMatch "loom $LoomVersion")) {
  throw "loom version did not match the checked-out binary"
}
if (-not (Select-String -Quiet -Path $RerunLog -Pattern 'loom setup')) { throw "rerun omitted setup output" }
if (-not (Select-String -Quiet -Path $RerunLog -SimpleMatch 'Everything selected is already set up; no changes made')) {
  throw "rerun did not prove the no-change path"
}
if (-not (Select-String -Quiet -Path $BootstrapLog -SimpleMatch 'next run `loom status` to verify the setup')) {
  throw "setup omitted the status action"
}
if (-not (Select-String -Quiet -Path $BootstrapLog -SimpleMatch 'next run `loom init` inside your first project')) {
  throw "setup omitted the init action"
}
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
