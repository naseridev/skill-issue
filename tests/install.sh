#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TEST_HOME="$(mktemp -d)"
export HOME="$TEST_HOME"
export BIN_DIR="$HOME/.local/bin"
export CONFIG_DIR="$HOME/.config/skill-issue"

PASS=0
FAIL=0

ok() {
  PASS=$((PASS + 1))
  echo "ok: $1"
}

bad() {
  FAIL=$((FAIL + 1))
  echo "FAIL: $1" >&2
}

cleanup() {
  rm -rf "$TEST_HOME"
}
trap cleanup EXIT

bash -n "$REPO_ROOT/install.sh" && ok "install.sh parses" || bad "install.sh parses"
bash -n "$REPO_ROOT/uninstall.sh" && ok "uninstall.sh parses" || bad "uninstall.sh parses"

if command -v fish >/dev/null 2>&1; then
  TMP_FISH="$(mktemp -d)/skill_issue.fish"
  awk '/^hook_fish\(\)/,/^\}/' "$REPO_ROOT/install.sh" | sed -n "/cat > /,/^EOF/p" | sed '1d;$d' > "$TMP_FISH"
  if fish -n "$TMP_FISH" 2>/dev/null; then
    ok "fish hook parses"
  else
    bad "fish hook parses"
  fi
  if grep -q "isatty stdout" "$TMP_FISH" && grep -q "SKILL_ISSUE_ACTIVE" "$TMP_FISH"; then
    ok "fish hook has tty and recursion guards"
  else
    bad "fish hook has tty and recursion guards"
  fi
  if grep -q "skill-issue &>/dev/null" "$TMP_FISH"; then
    bad "fish hook keeps stdout on the terminal"
  else
    ok "fish hook keeps stdout on the terminal"
  fi
fi

mkdir -p "$HOME"
touch "$HOME/.bashrc" "$HOME/.zshrc"

echo data > "$TEST_HOME/note.txt"
if bash "$REPO_ROOT/install.sh" "$TEST_HOME/note.txt" >/dev/null 2>&1; then
  bad "installer should reject unsupported input"
else
  ok "installer rejects unsupported input"
fi

if bash "$REPO_ROOT/install.sh" /nonexistent/clip.gif >/dev/null 2>&1; then
  bad "installer should reject missing input"
else
  ok "installer rejects missing input"
fi

mkdir -p "$BIN_DIR" "$CONFIG_DIR"
touch "$BIN_DIR/skill-issue" "$CONFIG_DIR/fail.gif"
echo "# skill-issue bash hook (managed by install.sh, safe to remove)" >> "$HOME/.bashrc"
bash "$REPO_ROOT/uninstall.sh" >/dev/null 2>&1
[ ! -f "$BIN_DIR/skill-issue" ] && ok "uninstall removes binary" || bad "uninstall removes binary"
[ ! -f "$CONFIG_DIR/fail.gif" ] && ok "uninstall removes gif" || bad "uninstall removes gif"
! grep -q "skill-issue bash hook" "$HOME/.bashrc" && ok "uninstall removes hook" || bad "uninstall removes hook"

echo "pass=$PASS fail=$FAIL"
[ "$FAIL" -eq 0 ]
