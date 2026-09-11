#!/usr/bin/env bash
set -euo pipefail

BIN_PATH="$HOME/.local/bin/skill-issue"
CONFIG_DIR="$HOME/.config/skill-issue"
FISH_PLUGIN="$HOME/.config/fish/conf.d/skill_issue.fish"

remove_block() {
  local file="$1"
  local marker="$2"
  [ -f "$file" ] || return 0
  grep -q "$marker" "$file" 2>/dev/null || return 0
  local tmp
  tmp="$(mktemp)"
  awk -v marker="$marker" '
    $0 ~ marker { skip = 1 }
    skip && /__skill_issue_hook\(\)/ { depth = 1; next }
    skip && /PROMPT_COMMAND=.*__skill_issue_hook/ { skip = 0; next }
    skip && /add-zsh-hook precmd __skill_issue_hook/ { skip = 0; next }
    skip && /^\}/ { if (depth) { depth = 0; next } }
    skip && /^$/ && !printed { next }
    { print }
  ' "$file" > "$tmp" || true
  grep -v "$marker" "$tmp" | grep -v "FAIL_GIF_PATH.*skill-issue" > "$file.tmp" || true
  mv "$file.tmp" "$file"
  rm -f "$tmp"
}

rm -f "$BIN_PATH"
rm -f "$FISH_PLUGIN"

if [ -d "$CONFIG_DIR" ]; then
  rm -f "$CONFIG_DIR/fail.gif"
  rmdir "$CONFIG_DIR" 2>/dev/null || true
fi

remove_block "$HOME/.bashrc" "skill-issue bash hook"
remove_block "$HOME/.zshrc" "skill-issue zsh hook"

if [ -f "$HOME/.config/fish/config.fish" ]; then
  grep -v "skill_issue\|FAIL_GIF_PATH.*fail.gif" "$HOME/.config/fish/config.fish" > "$HOME/.config/fish/config.fish.tmp" || true
  mv "$HOME/.config/fish/config.fish.tmp" "$HOME/.config/fish/config.fish"
fi

echo "skill-issue uninstalled"
