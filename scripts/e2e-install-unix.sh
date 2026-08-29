#!/usr/bin/env bash
set -euo pipefail

workspace=${GITHUB_WORKSPACE:?GITHUB_WORKSPACE is required}
evidence_dir=${RUNNER_TEMP:?RUNNER_TEMP is required}/loom-install-e2e
mkdir -p "$evidence_dir"

case "$(uname -s)" in
  Darwin)
    base_path=/usr/bin:/bin:/usr/sbin:/sbin
    test_shell=/bin/zsh
    profile=${HOME}/.zshrc
    shell_name=zsh
    ;;
  Linux)
    base_path=/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin
    test_shell=/bin/bash
    profile=${HOME}/.bashrc
    shell_name=bash
    ;;
  *)
    echo "unsupported Unix test host: $(uname -s)" >&2
    exit 1
    ;;
esac

setup_args=(
  --skill next
  --agent agents
  --agent claude
  --agent codex
  --agent cursor
  --agent grok
  --agent opencode
  --agent pi
  --yes
)

env PATH="$base_path" SHELL="$test_shell" \
  sh "$workspace/install.sh" "${setup_args[@]}" \
  >"$evidence_dir/bootstrap-stdout.txt" \
  2>"$evidence_dir/bootstrap-stderr.txt"

mise_bin=${HOME}/.local/bin/mise
if [[ ! -x "$mise_bin" ]]; then
  mise_bin=$(PATH="$base_path" command -v mise)
fi
run_loom() {
  local working_dir=$PWD
  local loom_bin=${LOOM_E2E_LOOM_BIN:-loom}
  "$mise_bin" -C "$HOME" exec -- sh -c \
    'cd "$1" && shift && exec "$@"' sh "$working_dir" "$loom_bin" "$@"
}
run_loom --version >"$evidence_dir/loom-version.txt" 2>&1
run_loom add --tool tokei --yes >"$evidence_dir/tokei-install.txt" 2>&1
"$mise_bin" -C "$HOME" exec -- tokei --version >"$evidence_dir/tokei-version.txt" 2>&1
run_loom status >"$evidence_dir/loom-status.txt" 2>&1
project=${RUNNER_TEMP}/loom-first-project
mkdir -p "$project"
(
  cd "$project"
  run_loom init --yes
) >"$evidence_dir/loom-init.txt" 2>&1
test -f "$project/AGENTS.md"
test -f "$project/CLAUDE.md"
run_loom update --yes >"$evidence_dir/loom-update.txt" 2>&1
"$mise_bin" -C "$HOME" exec -- br --version >"$evidence_dir/br-version.txt" 2>&1
"$mise_bin" -C "$HOME" exec -- bv --version >"$evidence_dir/bv-version.txt" 2>&1
"$mise_bin" doctor >"$evidence_dir/mise-doctor.txt" 2>&1 || true

selection=${HOME}/.config/mise/conf.d/loom.toml
test -f "$selection"
selection_before=$(cksum <"$selection")
test "$(grep -c '^# core:begin' "$selection")" -eq 1
test "$(grep -c '^# core:end' "$selection")" -eq 1
grep -Fq 'beads_rust' "$selection"
grep -Fq 'beads_viewer' "$selection"
if [[ $(uname -s) == Darwin ]]; then
  grep -Fq '"cargo:tokei"' "$selection"
else
  grep -Fq '"aqua:XAMPPRocky/tokei"' "$selection"
fi

for skill_root in \
  "${HOME}/.agents/skills" \
  "${HOME}/.claude/skills" \
  "${HOME}/.codex/skills" \
  "${HOME}/.config/opencode/skills" \
  "${HOME}/.cursor/skills" \
  "${HOME}/.grok/skills" \
  "${HOME}/.pi/agent/skills"; do
  test -f "$skill_root/next/SKILL.md"
done
test -f "${HOME}/.config/opencode/plugins/loom-session-env.js"

case "$shell_name" in
  bash)
    env -i HOME="$HOME" PATH="$base_path" SHELL="$test_shell" \
      "$test_shell" --noprofile --rcfile "$profile" -ic \
      'command -v mise && command -v loom && loom --version' \
      >"$evidence_dir/fresh-shell.txt" 2>&1
    ;;
  zsh)
    env -i HOME="$HOME" PATH="$base_path" SHELL="$test_shell" \
      "$test_shell" -d -i -c 'command -v mise && command -v loom && loom --version' \
      >"$evidence_dir/fresh-shell.txt" 2>&1
    ;;
esac

skill_before=$(cksum <"${HOME}/.agents/skills/next/SKILL.md")
env PATH="$(dirname "$mise_bin"):$base_path" SHELL="$test_shell" \
  sh "$workspace/install.sh" "${setup_args[@]}" \
  >"$evidence_dir/rerun-stdout.txt" \
  2>"$evidence_dir/rerun-stderr.txt"

activation=$(printf 'export PATH="%s:$PATH"; eval "$("%s" activate %s)"' "$(dirname "$mise_bin")" "$mise_bin" "$shell_name")
test "$(grep -Fxc "$activation" "$profile")" -eq 1
grep -Fq 'beads_rust' "$selection"
grep -Fq 'beads_viewer' "$selection"
test "$(cksum <"$selection")" = "$selection_before"
test "$(cksum <"${HOME}/.agents/skills/next/SKILL.md")" = "$skill_before"
manifest="${LOOM_REPO_DIR:-$workspace}/manifest/loom.toml"
awk '/^# core:begin/{inside=1; next} /^# core:end/{inside=0} inside && /^[^#[:space:]].*=/{print}' "$manifest" |
  while IFS= read -r pin; do grep -Fqx "$pin" "$selection"; done
grep -Fxf <(grep -E '^"(aqua:XAMPPRocky/tokei|cargo:tokei)"' "$manifest") "$selection" >/dev/null
loom_version=$(awk -F'"' '/^version = /{print $2; exit}' "${LOOM_REPO_DIR:-$workspace}/cli/loom/Cargo.toml")
grep -Fqx "loom $loom_version" "$evidence_dir/loom-version.txt"
grep -Fq 'tokei 12.1.2' "$evidence_dir/tokei-version.txt"
grep -Fq 'loom setup' "$evidence_dir/rerun-stdout.txt"
grep -Fq 'Everything selected is already set up; no changes made' "$evidence_dir/rerun-stdout.txt"
grep -Fq 'next run `loom status` to verify the setup' "$evidence_dir/bootstrap-stdout.txt"
grep -Fq 'next run `loom init` inside your first project' "$evidence_dir/bootstrap-stdout.txt"
grep -Fq 'Selected resources and runtimes verified' "$evidence_dir/loom-status.txt"
grep -Fq 'Done' "$evidence_dir/loom-init.txt"
grep -Eq 'Up to date|refreshed' "$evidence_dir/loom-update.txt"

find "${HOME}/.agents/skills/next" \
  "${HOME}/.claude/skills/next" \
  "${HOME}/.codex/skills/next" \
  "${HOME}/.config/opencode/skills/next" \
  "${HOME}/.cursor/skills/next" \
  "${HOME}/.grok/skills/next" \
  "${HOME}/.pi/agent/skills/next" \
  -type f -print | sort >"$evidence_dir/installed-files.txt"
cp "$selection" "$evidence_dir/loom.toml"

for artifact in "$evidence_dir"/*.txt; do
  echo "===== $(basename "$artifact") ====="
  sed -n '1,240p' "$artifact"
done
