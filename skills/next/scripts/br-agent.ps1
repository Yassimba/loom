$ErrorActionPreference = "Stop"

if ($env:BEADS_ACTOR) {
    $actor = $env:BEADS_ACTOR
} elseif ($env:CLAUDE_CODE_SESSION_ID) {
    $actor = "claude-$($env:CLAUDE_CODE_SESSION_ID)"
} elseif ($env:CODEX_SESSION_ID) {
    $actor = "codex-$($env:CODEX_SESSION_ID)"
} elseif ($env:CODEX_THREAD_ID) {
    $actor = "codex-$($env:CODEX_THREAD_ID)"
} elseif ($env:OPENCODE_SESSION_ID) {
    $actor = "opencode-$($env:OPENCODE_SESSION_ID)"
} elseif ($env:PI_SESSION_ID) {
    $actor = "pi-$($env:PI_SESSION_ID)"
} else {
    [Console]::Error.WriteLine("br-agent: no session-unique actor found; set BEADS_ACTOR explicitly")
    exit 64
}

foreach ($argument in $args) {
    if ($argument -eq "--actor" -or $argument -like "--actor=*") {
        [Console]::Error.WriteLine("br-agent: pass an explicit actor through BEADS_ACTOR, not --actor")
        exit 64
    }
}

if ($env:BR_AGENT_REAL_BR) {
    $realBr = $env:BR_AGENT_REAL_BR
} else {
    $command = Get-Command br -CommandType Application -ErrorAction SilentlyContinue
    if ($command) {
        $realBr = $command.Source
    }
}

if (-not $realBr -or $realBr -eq $PSCommandPath) {
    [Console]::Error.WriteLine("br-agent: could not find the real br executable")
    exit 127
}

& $realBr --actor $actor @args
exit $LASTEXITCODE
