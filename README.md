# skill-issue

Terminal GIF player that runs when a shell command fails. A shell hook watches
the exit status of each command and plays a configured GIF with half-block
characters when the status is nonzero.

## Install

The recommended installation method is to download a release package from the
[Releases](https://github.com/naseridev/skill-issue/releases) section, unzip
it, and run the installer from the extracted directory:

```sh
unzip skill-issue-vX.Y.Z-x86_64-unknown-linux-gnu.zip
cd skill-issue-vX.Y.Z-x86_64-unknown-linux-gnu
./install.sh [CLIP]
```

The release package contains a prebuilt binary, so no Rust toolchain is
needed. Pick the archive matching your operating system and architecture. If
no release matches your platform, fall back to the manual installation below,
which builds from source.

### Manual install

Clone the repository and run:

```sh
./install.sh [CLIP]
```

This requires a Rust toolchain with cargo (release build uses `--locked`).

`CLIP` may be a gif file or a video file (mp4, webm, mov, mkv, m4v). Video
clips are converted to gif at 15 fps and 320 pixels wide. Without `CLIP`,
the installer uses `assets/default.gif`. The binary goes to
`~/.local/bin/skill-issue` and media goes to `~/.config/skill-issue/fail.gif`.
Override either location with `BIN_DIR` or `CONFIG_DIR`.

## Layout

- `src/main.rs` - GIF decoding, terminal sizing, rendering, playback
- `install.sh` - release build, binary install, GIF setup, shell hooks
- `uninstall.sh` - removes the binary, media, and managed shell hooks
- `assets/default.gif` - bundled fallback clip used when no input is given
- `tests/install.sh` - installer and uninstaller behavior checks

## Requirements

- ffmpeg only when converting video input (gif input needs no ffmpeg)
- python3 used by the installer to validate the installed GIF header
- bash, zsh, or fish for the failure hook
- Rust toolchain with cargo only for the manual install from source

The installer appends a hook to `~/.bashrc` and `~/.zshrc` when those shells
are present, and writes `~/.config/fish/conf.d/skill_issue.fish` for fish.
The hook runs only on nonzero exit status, only on a tty, and never recurses
through `SKILL_ISSUE_ACTIVE`. Exit status 130 (Ctrl-C) never triggers playback.

## Usage

```sh
skill-issue [GIF_PATH] [--loops N]
skill-issue --help
```

With no path, `FAIL_GIF_PATH` is used. `--loops N` stops after N loops and is
useful for scripting. While playing, `q`, `Esc`, `Enter`, or `Ctrl-C` stops.
Piped output exits with status 2 instead of writing escape codes.

## Uninstall

```sh
./uninstall.sh
```

This removes the binary, the configured gif, the fish plugin, and the managed
hook blocks from bash and zsh startup files.

## License

This project is licensed under the GNU General Public License v3.0 or later.
See the `LICENSE` file for the full text.

## Development

```sh
cargo fmt --check
cargo test
cargo clippy --all-targets -- -D warnings
cargo build --release --locked
bash tests/install.sh
```
