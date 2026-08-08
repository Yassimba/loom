#!/bin/sh

set -eu

if [ -n "${BEADS_ACTOR:-}" ]; then
    actor=$BEADS_ACTOR
elif [ -n "${CLAUDE_CODE_SESSION_ID:-}" ]; then
    actor="claude-$CLAUDE_CODE_SESSION_ID"
elif [ -n "${CODEX_SESSION_ID:-}" ]; then
    actor="codex-$CODEX_SESSION_ID"
elif [ -n "${CODEX_THREAD_ID:-}" ]; then
    actor="codex-$CODEX_THREAD_ID"
elif [ -n "${OPENCODE_SESSION_ID:-}" ]; then
    actor="opencode-$OPENCODE_SESSION_ID"
elif [ -n "${PI_SESSION_ID:-}" ]; then
    actor="pi-$PI_SESSION_ID"
else
    echo "br-agent: no session-unique actor found; set BEADS_ACTOR explicitly" >&2
    exit 64
fi

for argument in "$@"; do
    case $argument in
        --actor | --actor=*)
            echo "br-agent: pass an explicit actor through BEADS_ACTOR, not --actor" >&2
            exit 64
            ;;
    esac
done

if [ -n "${BR_AGENT_REAL_BR:-}" ]; then
    real_br=$BR_AGENT_REAL_BR
else
    real_br=$(command -v br || true)
fi

if [ -z "$real_br" ] || [ "$real_br" = "$0" ]; then
    echo "br-agent: could not find the real br executable" >&2
    exit 127
fi

exec "$real_br" --actor "$actor" "$@"
