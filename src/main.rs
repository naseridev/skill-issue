use std::env;
use std::fs::File;
use std::io::{self, BufReader, BufWriter, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::{Duration, Instant};

use crossterm::cursor::{Hide, MoveUp, Show};
use crossterm::event::{self, Event, KeyCode, KeyModifiers};
use crossterm::terminal::size;
use crossterm::QueueableCommand;
use image::codecs::gif::GifDecoder;
use image::{AnimationDecoder, DynamicImage, ImageDecoder};

const FRAME_MARGIN_COLS: u16 = 2;
const FRAME_MARGIN_ROWS: u16 = 2;
const MAX_FRAMES: usize = 2000;
const MAX_GIF_BYTES: u64 = 64 * 1024 * 1024;
const MIN_DELAY: Duration = Duration::from_millis(20);
const POLL_GRANULARITY: Duration = Duration::from_millis(5);

struct Config {
    gif_path: PathBuf,
    max_loops: Option<u32>,
}

struct Frame {
    bytes: Vec<u8>,
    delay: Duration,
}

fn usage(program: &str) -> String {
    format!(
        "Usage: {program} [GIF_PATH] [--loops N]\n\nPlays a GIF with half-block characters.\nDefaults to FAIL_GIF_PATH when set.\n\nKeys while playing: q, Esc, Enter, or Ctrl-C to stop."
    )
}

fn parse_args(argv: &[String]) -> Result<Option<Config>, String> {
    let program = argv.first().map(String::as_str).unwrap_or("skill-issue");
    let mut gif_path: Option<PathBuf> = None;
    let mut max_loops: Option<u32> = None;
    let mut iter = argv.iter().skip(1).peekable();

    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "-h" | "--help" => return Err(usage(program)),
            "--loops" => {
                let value = iter.next().ok_or("--loops requires a value")?;
                let parsed: u32 = value
                    .parse()
                    .map_err(|_| format!("Invalid --loops value: {value}"))?;
                if parsed == 0 {
                    return Err("--loops must be at least 1".to_string());
                }
                max_loops = Some(parsed);
            }
            other if other.starts_with('-') => {
                return Err(format!("Unknown option: {other}"));
            }
            other => {
                if gif_path.is_some() {
                    return Err("Too many arguments".to_string());
                }
                gif_path = Some(PathBuf::from(other));
            }
        }
    }

    if gif_path.is_none() {
        if let Ok(env_path) = env::var("FAIL_GIF_PATH") {
            if !env_path.trim().is_empty() {
                gif_path = Some(PathBuf::from(env_path));
            }
        }
    }

    match gif_path {
        Some(path) => Ok(Some(Config {
            gif_path: path,
            max_loops,
        })),
        None => Err(usage(program)),
    }
}

fn validate_gif_path(path: &Path) -> Result<(), String> {
    let metadata = path
        .metadata()
        .map_err(|_| format!("Cannot read GIF: {}", path.display()))?;
    if !metadata.is_file() {
        return Err(format!("Not a file: {}", path.display()));
    }
    if metadata.len() == 0 {
        return Err(format!("Empty file: {}", path.display()));
    }
    if metadata.len() > MAX_GIF_BYTES {
        return Err("GIF is too large".to_string());
    }
    Ok(())
}

fn resolve_size(term_cols: u16, term_rows: u16, src_w: u32, src_h: u32) -> (u32, u32, u16) {
    let avail_cols = u32::from(term_cols.saturating_sub(FRAME_MARGIN_COLS)).max(8);
    let avail_rows = u32::from(term_rows.saturating_sub(FRAME_MARGIN_ROWS)).max(4);
    let avail_px_h = avail_rows.saturating_mul(2).max(8);
    let src_aspect = f64::from(src_w.max(1)) / f64::from(src_h.max(1));
    let cell_aspect = 0.5;
    let mut target_cols =
        ((f64::from(avail_px_h) * src_aspect / cell_aspect).round() as u32).clamp(1, avail_cols);
    let mut target_px_h =
        ((f64::from(target_cols) * cell_aspect / src_aspect).round() as u32).clamp(2, avail_px_h);
    if target_px_h % 2 != 0 {
        target_px_h = target_px_h.saturating_add(1).min(avail_px_h & !1).max(2);
    }
    target_cols = target_cols.min(src_w.saturating_mul(4).max(1));
    let line_count = (target_px_h / 2).min(u32::from(u16::MAX)) as u16;
    (target_cols.max(1), target_px_h.max(2), line_count.max(1))
}

fn prepare_frames(
    path: &Path,
    term_cols: u16,
    term_rows: u16,
) -> Result<(Vec<Frame>, u16), String> {
    validate_gif_path(path)?;
    let file = File::open(path).map_err(|_| format!("Cannot read GIF: {}", path.display()))?;
    let decoder = GifDecoder::new(BufReader::new(file)).map_err(|e| format!("Invalid GIF: {e}"))?;
    let (src_w, src_h) = decoder.dimensions();
    if src_w == 0 || src_h == 0 || src_w > 8192 || src_h > 8192 {
        return Err("Unsupported GIF dimensions".to_string());
    }
    let raw_frames = decoder
        .into_frames()
        .collect_frames()
        .map_err(|e| format!("Cannot decode GIF: {e}"))?;
    if raw_frames.is_empty() {
        return Err("GIF contains no frames".to_string());
    }
    if raw_frames.len() > MAX_FRAMES {
        return Err("GIF has too many frames".to_string());
    }

    let (target_cols, target_px_h, line_count) = resolve_size(term_cols, term_rows, src_w, src_h);
    let filter = if target_cols > src_w || target_px_h > src_h {
        image::imageops::FilterType::Triangle
    } else {
        image::imageops::FilterType::Lanczos3
    };
    let mut frames = Vec::with_capacity(raw_frames.len());

    for raw in raw_frames {
        let delay = Duration::from(raw.delay()).max(MIN_DELAY);
        let resized = DynamicImage::ImageRgba8(raw.into_buffer()).resize_exact(
            target_cols,
            target_px_h,
            filter,
        );

        let rgba = resized.to_rgba8();
        let raw = rgba.into_raw();
        let mut bytes = Vec::with_capacity((target_cols * target_px_h) as usize);
        let mut last_fg: Option<[u8; 3]> = None;
        let mut last_bg: Option<[u8; 3]> = None;

        for row in 0..line_count {
            if row > 0 {
                bytes.extend_from_slice(b"\x1b[0m\r\n");
                last_fg = None;
                last_bg = None;
            }
            let y = u32::from(row) * 2;
            for x in 0..target_cols {
                let top = pixel_at(&raw, target_cols, x, y);
                let bottom = pixel_at(&raw, target_cols, x, y + 1);
                if last_fg != Some(top) {
                    push_color(&mut bytes, 38, top);
                    last_fg = Some(top);
                }
                if last_bg != Some(bottom) {
                    push_color(&mut bytes, 48, bottom);
                    last_bg = Some(bottom);
                }
                bytes.extend_from_slice("▀".as_bytes());
            }
        }
        bytes.extend_from_slice(b"\x1b[0m\r\n");
        frames.push(Frame { bytes, delay });
    }

    Ok((frames, line_count))
}

fn pixel_at(raw: &[u8], stride: u32, x: u32, y: u32) -> [u8; 3] {
    let i = ((y * stride + x) * 4) as usize;
    let alpha = u16::from(raw[i + 3]);
    if alpha >= 255 {
        return [raw[i], raw[i + 1], raw[i + 2]];
    }
    if alpha == 0 {
        return [0, 0, 0];
    }
    [
        (u16::from(raw[i]) * alpha / 255) as u8,
        (u16::from(raw[i + 1]) * alpha / 255) as u8,
        (u16::from(raw[i + 2]) * alpha / 255) as u8,
    ]
}

fn push_color(out: &mut Vec<u8>, layer: u8, rgb: [u8; 3]) {
    let mut buf = [0u8; 19];
    let mut pos = 0usize;
    pos += write_bytes(&mut buf[pos..], b"\x1b[");
    pos += write_bytes(&mut buf[pos..], if layer == 38 { b"38" } else { b"48" });
    pos += write_bytes(&mut buf[pos..], b";2;");
    pos += write_number(&mut buf[pos..], rgb[0]);
    pos += write_bytes(&mut buf[pos..], b";");
    pos += write_number(&mut buf[pos..], rgb[1]);
    pos += write_bytes(&mut buf[pos..], b";");
    pos += write_number(&mut buf[pos..], rgb[2]);
    pos += write_bytes(&mut buf[pos..], b"m");
    out.extend_from_slice(&buf[..pos]);
}

fn write_bytes(dst: &mut [u8], src: &[u8]) -> usize {
    let len = src.len().min(dst.len());
    dst[..len].copy_from_slice(&src[..len]);
    len
}

fn write_number(dst: &mut [u8], mut n: u8) -> usize {
    let mut digits = [0u8; 3];
    let mut len = 0;
    loop {
        digits[len] = b'0' + n % 10;
        len += 1;
        n /= 10;
        if n == 0 {
            break;
        }
    }
    let mut written = 0;
    for i in (0..len).rev() {
        if written < dst.len() {
            dst[written] = digits[i];
            written += 1;
        }
    }
    written
}

fn stop_requested() -> io::Result<bool> {
    if !event::poll(Duration::ZERO)? {
        return Ok(false);
    }
    match event::read()? {
        Event::Key(key) => Ok(matches!(
            key.code,
            KeyCode::Esc | KeyCode::Enter | KeyCode::Char('q') | KeyCode::Char('Q')
        ) || (key.code == KeyCode::Char('c')
            && key.modifiers.contains(KeyModifiers::CONTROL))),
        _ => Ok(false),
    }
}

struct TerminalGuard;

impl TerminalGuard {
    fn hide() -> io::Result<Self> {
        io::stdout().queue(Hide)?.flush()?;
        Ok(Self)
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let mut out = io::stdout();
        let _ = out.queue(Show);
        let _ = out.flush();
    }
}

fn play(frames: &[Frame], line_count: u16, max_loops: Option<u32>) -> io::Result<()> {
    let _guard = TerminalGuard::hide()?;
    let mut out = BufWriter::with_capacity(256 * 1024, io::stdout().lock());

    for _ in 0..line_count {
        out.write_all(b"\r\n")?;
    }
    out.flush()?;

    let mut loops = 0u32;
    'outer: loop {
        for frame in frames {
            let start = Instant::now();
            out.queue(MoveUp(line_count))?;
            out.write_all(&frame.bytes)?;
            out.flush()?;

            while start.elapsed() < frame.delay {
                let remaining = frame.delay.saturating_sub(start.elapsed());
                event::poll(remaining.min(POLL_GRANULARITY))?;
                if stop_requested()? {
                    break 'outer;
                }
            }
        }
        loops = loops.saturating_add(1);
        if let Some(limit) = max_loops {
            if loops >= limit {
                break;
            }
        }
    }

    out.flush()?;
    Ok(())
}

fn run() -> Result<(), String> {
    let argv: Vec<String> = env::args().collect();
    let config = match parse_args(&argv)? {
        Some(config) => config,
        None => return Ok(()),
    };
    if !is_terminal() {
        return Err("Output is not a terminal".to_string());
    }
    let (cols, rows) = size().map_err(|e| format!("Cannot query terminal size: {e}"))?;
    if cols < 10 || rows < 6 {
        return Err("Terminal is too small to play the GIF".to_string());
    }
    let (frames, line_count) = prepare_frames(&config.gif_path, cols, rows)?;
    play(&frames, line_count, config.max_loops).map_err(|e| format!("Playback failed: {e}"))?;
    Ok(())
}

#[cfg(unix)]
fn is_terminal() -> bool {
    use std::io::IsTerminal;
    io::stdout().is_terminal()
}

#[cfg(not(unix))]
fn is_terminal() -> bool {
    true
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            if message.starts_with("Usage:") {
                println!("{message}");
                return ExitCode::SUCCESS;
            }
            eprintln!("skill-issue: {message}");
            ExitCode::from(2)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn argv(parts: &[&str]) -> Vec<String> {
        parts.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn parses_explicit_path() {
        let config = parse_args(&argv(&["skill-issue", "/tmp/a.gif"]))
            .expect("args")
            .expect("config");
        assert_eq!(config.gif_path, PathBuf::from("/tmp/a.gif"));
        assert_eq!(config.max_loops, None);
    }

    #[test]
    fn parses_loops_option() {
        let config = parse_args(&argv(&["skill-issue", "/tmp/a.gif", "--loops", "3"]))
            .expect("args")
            .expect("config");
        assert_eq!(config.max_loops, Some(3));
    }

    #[test]
    fn rejects_zero_loops() {
        assert!(parse_args(&argv(&["skill-issue", "/tmp/a.gif", "--loops", "0"])).is_err());
    }

    #[test]
    fn resolve_size_matches_terminal_bounds() {
        let (cols, px_h, lines) = resolve_size(80, 24, 752, 352);
        assert!(cols <= 78);
        assert!(px_h <= 44);
        assert_eq!(lines, (px_h / 2) as u16);
        assert_eq!(px_h % 2, 0);
    }

    #[test]
    fn transparent_pixels_render_as_black() {
        let blended = pixel_at(&[200, 100, 50, 0], 1, 0, 0);
        assert_eq!(blended, [0, 0, 0]);
        let opaque = pixel_at(&[10, 20, 30, 255], 1, 0, 0);
        assert_eq!(opaque, [10, 20, 30]);
    }
}
