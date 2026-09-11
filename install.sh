#!/usr/bin/env bash
set -euo pipefail

APP_NAME="skill-issue"
VERSION="0.2.0"
REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BIN_DIR="${BIN_DIR:-$HOME/.local/bin}"
CONFIG_DIR="${CONFIG_DIR:-$HOME/.config/skill-issue}"
TARGET_GIF="$CONFIG_DIR/fail.gif"
INPUT_FILE="${1:-}"
STEP=0

fail() {
  echo "error: $1" >&2
  exit "${2:-1}"
}

step() {
  STEP=$((STEP + 1))
  echo "[$STEP/6] $1"
}

need_cmd() {
  command -v "$1" >/dev/null 2>&1 || fail "required command not found: $1"
}

resolve_input() {
  local src="$1"
  [ -f "$src" ] || fail "file not found: $src"
  case "$src" in
    *.gif|*.GIF)
      cp -- "$src" "$TARGET_GIF"
      ;;
    *.mp4|*.MP4|*.webm|*.WEBM|*.mov|*.MOV|*.mkv|*.MKV|*.m4v|*.M4V)
      need_cmd ffmpeg
      ffmpeg -y -hide_banner -loglevel error -i "$src" \
        -vf "fps=15,scale=320:-2:flags=lanczos,split[s0][s1];[s0]palettegen=max_colors=128[p];[s1][p]paletteuse" \
        "$TARGET_GIF" || fail "ffmpeg could not convert $src"
      ;;
    *)
      fail "unsupported input type (use gif, mp4, webm, mov, mkv): $src"
      ;;
  esac
}

write_default_gif() {
  if [ ! -f "$TARGET_GIF" ] && [ -f "$REPO_ROOT/assets/default.gif" ]; then
    cp -- "$REPO_ROOT/assets/default.gif" "$TARGET_GIF"
  fi
}

verify_gif() {
  python3 - "$TARGET_GIF" <<'PY'
import sys
path = sys.argv[1]
try:
    with open(path, "rb") as f:
        header = f.read(6)
        if header not in (b"GIF87a", b"GIF89a"):
            sys.exit(1)
        f.seek(0, 2)
        if f.tell() == 0 or f.tell() > 64 * 1024 * 1024:
            sys.exit(1)
except OSError:
    sys.exit(1)
PY
}

hook_bash() {
  local rc="$1"
  touch "$rc"
  grep -q "skill-issue bash hook" "$rc" 2>/dev/null && return 0
  cat >> "$rc" <<'EOF'

# skill-issue bash hook (managed by install.sh, safe to remove)
export PATH="$HOME/.local/bin:$PATH"
export FAIL_GIF_PATH="$HOME/.config/skill-issue/fail.gif"
__skill_issue_hook() {
  local r=$?
  if [ $r -ne 0 ] && [ $r -ne 130 ] && [ -t 1 ] && [ -z "${SKILL_ISSUE_ACTIVE:-}" ]; then
    SKILL_ISSUE_ACTIVE=1 skill-issue || true
  fi
  return $r
}
if [[ "${PROMPT_COMMAND:-}" != *"__skill_issue_hook"* ]]; then
  PROMPT_COMMAND="__skill_issue_hook${PROMPT_COMMAND:+; $PROMPT_COMMAND}"
fi
EOF
}

hook_zsh() {
  local rc="$1"
  touch "$rc"
  grep -q "skill-issue zsh hook" "$rc" 2>/dev/null && return 0
  cat >> "$rc" <<'EOF'

# skill-issue zsh hook (managed by install.sh, safe to remove)
export PATH="$HOME/.local/bin:$PATH"
export FAIL_GIF_PATH="$HOME/.config/skill-issue/fail.gif"
autoload -Uz add-zsh-hook
__skill_issue_hook() {
  local r=$?
  if [ $r -ne 0 ] && [ $r -ne 130 ] && [ -t 1 ] && [ -z "${SKILL_ISSUE_ACTIVE:-}" ]; then
    SKILL_ISSUE_ACTIVE=1 skill-issue || true
  fi
  return $r
}
add-zsh-hook precmd __skill_issue_hook
EOF
}

hook_fish() {
  local dir="$HOME/.config/fish/conf.d"
  mkdir -p "$dir"
  cat > "$dir/skill_issue.fish" <<'EOF'
# skill-issue fish hook (managed by install.sh, safe to remove)
if not contains -- $HOME/.local/bin $PATH
  fish_add_path $HOME/.local/bin
end
set -gx FAIL_GIF_PATH "$HOME/.config/skill-issue/fail.gif"
function __skill_issue_hook --on-event fish_postexec
  set -l r $status
  if test $r -ne 0 -a $r -ne 130
    and isatty stdout
    and test -z "$SKILL_ISSUE_ACTIVE"
    SKILL_ISSUE_ACTIVE=1 skill-issue 2>/dev/null < /dev/tty
  end
end
EOF
}

check_input_early() {
  [ -n "$INPUT_FILE" ] || return 0
  [ -f "$INPUT_FILE" ] || fail "file not found: $INPUT_FILE"
  case "$INPUT_FILE" in
    *.gif|*.GIF|*.mp4|*.MP4|*.webm|*.WEBM|*.mov|*.MOV|*.mkv|*.MKV|*.m4v|*.M4V) ;;
    *) fail "unsupported input type (use gif, mp4, webm, mov, mkv): $INPUT_FILE" ;;
  esac
}

main() {
  need_cmd cargo
  check_input_early
  if ! command -v rustc >/dev/null 2>&1 && [ -f "$HOME/.cargo/env" ]; then
    . "$HOME/.cargo/env"
  fi

  step "Building $APP_NAME $VERSION (release)"
  (cd "$REPO_ROOT" && cargo build --release --locked)

  step "Installing binary to $BIN_DIR"
  mkdir -p "$BIN_DIR" "$CONFIG_DIR"
  install -m 755 "$REPO_ROOT/target/release/$APP_NAME" "$BIN_DIR/$APP_NAME"

  step "Configuring media"
  if [ -n "$INPUT_FILE" ]; then
    resolve_input "$INPUT_FILE"
  fi
  write_default_gif

  step "Validating installation"
  [ -x "$BIN_DIR/$APP_NAME" ] || fail "binary not installed"
  if [ -f "$TARGET_GIF" ]; then
    verify_gif || fail "installed GIF failed validation: $TARGET_GIF"
  fi
  "$BIN_DIR/$APP_NAME" --help >/dev/null 2>&1 || fail "binary self-check failed"

  step "Wiring shell hooks"
  if [ -f "$HOME/.bashrc" ] || command -v bash >/dev/null 2>&1; then
    hook_bash "$HOME/.bashrc"
  fi
  if [ -f "$HOME/.zshrc" ] || command -v zsh >/dev/null 2>&1; then
    hook_zsh "$HOME/.zshrc"
  fi
  if [ -d "$HOME/.config/fish" ] || command -v fish >/dev/null 2>&1; then
    hook_fish
  fi

  step "Done"
  echo "Installed $APP_NAME $VERSION to $BIN_DIR/$APP_NAME"
  if [ ! -f "$TARGET_GIF" ]; then
    echo "No GIF configured. Rerun as: ./install.sh /path/to/clip.gif"
  else
    echo "GIF: $TARGET_GIF"
  fi
  echo "Restart your shell or run: export FAIL_GIF_PATH=\"$TARGET_GIF\""
}

main "$@"
