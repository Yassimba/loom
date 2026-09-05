param()

$ErrorActionPreference = "Stop"
$Workspace = $env:GITHUB_WORKSPACE
$EvidenceDir = Join-Path $env:RUNNER_TEMP "loom-install-e2e"
New-Item -ItemType Directory -Path $EvidenceDir -Force | Out-Null
$PowerShellExe = (Get-Command $env:LOOM_E2E_POWERSHELL -ErrorAction Stop).Source
$TargetProfile = (& $PowerShellExe -NoProfile -Command '$PROFILE').Trim()
$Documents = [Environment]::GetFolderPath([Environment+SpecialFolder]::MyDocuments)
$ExpectedProfiles = @(
  $TargetProfile,
  (Join-Path $Documents "WindowsPowerShell\profile.ps1"),
  (Join-Path $Documents "PowerShell\profile.ps1")
)
$Skill = if ($env:LOOM_E2E_SKILL) { $env:LOOM_E2E_SKILL } else { "implement" }
$ExpectBeads = $Skill -eq "implement"

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
$Project = Join-Path $env:RUNNER_TEMP "loom first project with spaces"
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
$SelectionBefore = Get-Content $Selection -Raw
if ([regex]::Matches($SelectionBefore, '(?m)^# core:begin').Count -ne 1) {
  throw "selection must contain exactly one core start marker"
}
if ([regex]::Matches($SelectionBefore, '(?m)^# core:end').Count -ne 1) {
  throw "selection must contain exactly one core end marker"
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
# Recreate the exact state left when a package manager installed mise but the
# first process could not discover it: ownership is pending and the ledger has
# not recorded core:mise yet. Also route Documents to a path with spaces.
$StatePath = Join-Path $HOME ".config\loom\install-state.json"
$State = Get-Content $StatePath -Raw | ConvertFrom-Json
$State.resources.PSObject.Properties.Remove("core:mise")
[IO.File]::WriteAllText(
  $StatePath,
  ($State | ConvertTo-Json -Depth 20),
  [Text.UTF8Encoding]::new($false)
)
$SelectionBackup = Join-Path (Split-Path -Parent $Selection) ".loom.toml.loom-old"
Move-Item -LiteralPath $Selection -Destination $SelectionBackup
$ProfileBackup = Join-Path (Split-Path -Parent $TargetProfile) ("." + (Split-Path -Leaf $TargetProfile) + ".loom-old")
Move-Item -LiteralPath $TargetProfile -Destination $ProfileBackup
$PendingMise = Join-Path $HOME ".config\loom\bootstrap-mise-pending.json"
[IO.File]::WriteAllText(
  $PendingMise,
  (@{ manager = "direct"; pathAdded = $false } | ConvertTo-Json -Compress),
  [Text.UTF8Encoding]::new($false)
)
$RedirectedDocuments = Join-Path $env:RUNNER_TEMP "e2e redirected Documents"
$env:LOOM_E2E_DOCUMENTS_DIR = $RedirectedDocuments
$RerunLog = Join-Path $EvidenceDir "rerun.txt"
try {
  Invoke-Bootstrap *> $RerunLog
} finally {
  Remove-Item Env:LOOM_E2E_DOCUMENTS_DIR -ErrorAction SilentlyContinue
}
$State = Get-Content $StatePath -Raw | ConvertFrom-Json
$MiseReceipt = $State.resources."core:mise".receipts | Where-Object { $_.kind -eq "mise-installation" } | Select-Object -First 1
if (-not $MiseReceipt -or $MiseReceipt.manager -ne "direct") {
  throw "pending mise ownership was not restored on rerun"
}
if (Test-Path $PendingMise) { throw "successful rerun left pending mise ownership behind" }
if (-not (Test-Path $Selection) -or (Test-Path $SelectionBackup)) {
  throw "rerun did not recover the interrupted mise selection"
}
if (-not (Test-Path $TargetProfile) -or (Test-Path $ProfileBackup)) {
  throw "rerun did not recover the interrupted PowerShell profile"
}
$ExpectedProfiles += @(
  (Join-Path $RedirectedDocuments "WindowsPowerShell\profile.ps1"),
  (Join-Path $RedirectedDocuments "PowerShell\profile.ps1")
)
$ExpectedProfiles = $ExpectedProfiles | Select-Object -Unique

$MiseQuoted = $Mise.Replace("'", "''")
$MiseDirQuoted = (Split-Path -Parent $Mise).Replace("'", "''")
$Activation = "`$env:Path = '$MiseDirQuoted;' + `$env:Path; (& '$MiseQuoted' activate pwsh) | Out-String | Invoke-Expression"
foreach ($ExpectedProfile in $ExpectedProfiles) {
  if (-not (Test-Path $ExpectedProfile)) { throw "installer omitted PowerShell profile $ExpectedProfile" }
  $ProfileText = Get-Content $ExpectedProfile -Raw
  $ProfileText | Set-Content (Join-Path $EvidenceDir ("profile-" + [IO.Path]::GetFileName((Split-Path -Parent $ExpectedProfile)) + ".txt"))
  if ([regex]::Matches($ProfileText, [regex]::Escape($Activation)).Count -ne 1) {
    throw "mise activation must appear exactly once in $ExpectedProfile"
  }
}
$SelectionText = Get-Content $Selection -Raw
if ($SelectionText.Replace("`r`n", "`n") -ne $SelectionBefore.Replace("`r`n", "`n")) {
  throw "rerun changed the selected tool set"
}
$SelectionLines = $SelectionText -split "`r?`n"
$SkillHashAfter = (Get-FileHash (Join-Path $SkillRoots[0] "$Skill\SKILL.md") -Algorithm SHA256).Hash
if ($SkillHashAfter -ne $SkillHashBefore) { throw "rerun changed the installed skill" }
$Manifest = Join-Path $(if ($env:LOOM_REPO_DIR) { $env:LOOM_REPO_DIR } else { $Workspace }) "manifest\loom.toml"
$ManifestLines = Get-Content $Manifest
$InsideCore = $false
foreach ($Line in $ManifestLines) {
  if ($Line.StartsWith("# core:begin")) { $InsideCore = $true; continue }
  if ($Line.StartsWith("# core:end")) { $InsideCore = $false; continue }
  if ($InsideCore -and $Line -match '^[^#\s].*=') {
    if (-not $SelectionLines.Contains($Line)) { throw "selection changed exact core pin: $Line" }
  }
}
if ($env:LOOM_E2E_TOKEI -ne "skip") {
  $TokeiPin = $ManifestLines | Where-Object { $_ -match '^"aqua:XAMPPRocky/tokei"' } | Select-Object -First 1
  if (-not $SelectionLines.Contains($TokeiPin)) { throw "selection changed the exact Tokei pin" }
  if (-not (Select-String -Quiet -Path (Join-Path $EvidenceDir "tokei-version.txt") -SimpleMatch 'tokei 12.1.2')) {
    throw "Tokei version did not match its exact pin"
  }
}
if ($env:LOOM_E2E_PUBLISHED -eq "true") {
  $LoomVersion = ([regex]::Match(($ManifestLines -join "`n"), '(?m)^"github:Yassimba/loom\[exe=loom\]" = \{ version = "loom-v([^"]+)"')).Groups[1].Value
  if (-not $LoomVersion) { throw "missing published Loom pin" }
} else {
  $Cargo = Get-Content (Join-Path $Workspace "cli\loom\Cargo.toml")
  $LoomVersion = ([regex]::Match(($Cargo -join "`n"), '(?m)^version = "([^"]+)"')).Groups[1].Value
}
if (-not (Select-String -Quiet -Path (Join-Path $EvidenceDir "loom-version.txt") -SimpleMatch "loom $LoomVersion")) {
  throw "loom version did not match the expected candidate or published pin"
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
