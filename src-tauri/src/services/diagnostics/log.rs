//! A tiny local log: `logs/islandy.log` in Islandy's data directory.
//!
//! - Levels: error, warn, info (default) and debug (only with the
//!   "verbose logging" setting).
//! - Bounded: the file rotates at [`MAX_FILE_BYTES`] and at most
//!   [`KEEP_FILES`] files exist (≈ 3 MiB in total).
//! - Sanitized: control characters are stripped, the user's home directory is
//!   replaced by `~`, long hex/base64-like runs (tokens) are redacted and
//!   lines are capped. Callers must still never log hook payloads, prompts,
//!   tokens or environment variables; this is a second line of defence.
//! - Nothing leaves the machine.

use std::{
    collections::VecDeque,
    fmt,
    fs::{File, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Mutex, OnceLock,
    },
    time::{SystemTime, UNIX_EPOCH},
};

pub const FILE: &str = "islandy.log";
pub const MAX_FILE_BYTES: u64 = 1024 * 1024;
pub const KEEP_FILES: usize = 3;
const MAX_LINE_CHARS: usize = 600;
const RECENT_PROBLEMS: usize = 20;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Level {
    Error,
    Warn,
    Info,
    Debug,
}

impl Level {
    fn label(self) -> &'static str {
        match self {
            Level::Error => "ERROR",
            Level::Warn => "WARN",
            Level::Info => "INFO",
            Level::Debug => "DEBUG",
        }
    }
}

struct Sink {
    dir: PathBuf,
    file: Option<File>,
    size: u64,
}

struct Logger {
    sink: Mutex<Option<Sink>>,
    verbose: AtomicBool,
    recent: Mutex<VecDeque<String>>,
    home: Option<String>,
}

static LOGGER: OnceLock<Logger> = OnceLock::new();

fn logger() -> &'static Logger {
    LOGGER.get_or_init(|| Logger {
        sink: Mutex::new(None),
        verbose: AtomicBool::new(false),
        recent: Mutex::new(VecDeque::new()),
        home: std::env::var("USERPROFILE")
            .or_else(|_| std::env::var("HOME"))
            .ok()
            .filter(|h| h.len() > 3),
    })
}

/// Starts writing to `<dir>/islandy.log`. Without a directory, entries are
/// only kept in memory (and printed in debug builds).
pub fn init(dir: Option<PathBuf>, verbose: bool) {
    let l = logger();
    l.verbose.store(verbose, Ordering::Relaxed);
    let sink = dir.and_then(|dir| {
        crate::services::paths::ensure_private_dir(&dir).ok()?;
        let (file, size) = open(&dir)?;
        Some(Sink {
            dir,
            file: Some(file),
            size,
        })
    });
    *l.sink.lock().unwrap_or_else(|e| e.into_inner()) = sink;
}

pub fn set_verbose(verbose: bool) {
    logger().verbose.store(verbose, Ordering::Relaxed);
}

pub fn enabled(level: Level) -> bool {
    level <= Level::Info || logger().verbose.load(Ordering::Relaxed)
}

fn open(dir: &Path) -> Option<(File, u64)> {
    let path = dir.join(FILE);
    let mut options = OpenOptions::new();
    options.create(true).append(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let file = options.open(&path).ok()?;
    let size = file.metadata().map(|m| m.len()).unwrap_or(0);
    Some((file, size))
}

fn rotated(dir: &Path, n: usize) -> PathBuf {
    dir.join(format!("islandy.{n}.log"))
}

/// `islandy.log` → `islandy.1.log` → … (the oldest is dropped).
fn rotate(sink: &mut Sink) {
    sink.file = None;
    let _ = std::fs::remove_file(rotated(&sink.dir, KEEP_FILES - 1));
    for n in (1..KEEP_FILES - 1).rev() {
        let _ = std::fs::rename(rotated(&sink.dir, n), rotated(&sink.dir, n + 1));
    }
    let _ = std::fs::rename(sink.dir.join(FILE), rotated(&sink.dir, 1));
    if let Some((file, size)) = open(&sink.dir) {
        sink.file = Some(file);
        sink.size = size;
    }
}

/// Removes characters and values that must never reach the log.
pub fn sanitize(message: &str, home: Option<&str>) -> String {
    let mut text: String = message
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect();
    if let Some(home) = home {
        text = text.replace(home, "~");
    }
    // Redact long runs of token-like characters (hex, base64url).
    let mut out = String::with_capacity(text.len());
    let mut run = String::new();
    let flush = |run: &mut String, out: &mut String| {
        if run.len() >= 32 {
            out.push_str("[redacted]");
        } else {
            out.push_str(run);
        }
        run.clear();
    };
    for c in text.chars() {
        if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
            run.push(c);
        } else {
            flush(&mut run, &mut out);
            out.push(c);
        }
    }
    flush(&mut run, &mut out);
    if out.chars().count() > MAX_LINE_CHARS {
        out = out.chars().take(MAX_LINE_CHARS).collect::<String>() + "…";
    }
    out
}

/// UTC timestamp `YYYY-MM-DDTHH:MM:SS.mmmZ` without a date library.
pub fn timestamp(ms: u64) -> String {
    let secs = ms / 1000;
    let (days, rem) = (secs / 86_400, secs % 86_400);
    // Civil-from-days (Howard Hinnant's algorithm).
    let z = days as i64 + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}.{:03}Z",
        rem / 3600,
        rem % 3600 / 60,
        rem % 60,
        ms % 1000
    )
}

pub fn write(level: Level, target: &str, args: fmt::Arguments<'_>) {
    if !enabled(level) {
        return;
    }
    let l = logger();
    let ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    let message = sanitize(&args.to_string(), l.home.as_deref());
    let line = format!("{} {:<5} {target}: {message}", timestamp(ms), level.label());
    #[cfg(debug_assertions)]
    eprintln!("[islandy] {line}");
    if level <= Level::Warn {
        let mut recent = l.recent.lock().unwrap_or_else(|e| e.into_inner());
        if recent.len() >= RECENT_PROBLEMS {
            recent.pop_front();
        }
        recent.push_back(line.clone());
    }
    let mut guard = l.sink.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(sink) = guard.as_mut() {
        if sink.size >= MAX_FILE_BYTES {
            rotate(sink);
        }
        if let Some(file) = sink.file.as_mut() {
            if writeln!(file, "{line}").is_ok() {
                sink.size += line.len() as u64 + 1;
            }
        }
    }
}

/// The most recent warnings and errors (already sanitized), oldest first.
pub fn recent_problems() -> Vec<String> {
    logger()
        .recent
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .iter()
        .cloned()
        .collect()
}

#[macro_export]
macro_rules! log_error {
    ($target:expr, $($arg:tt)+) => {
        $crate::services::diagnostics::log::write(
            $crate::services::diagnostics::log::Level::Error, $target, format_args!($($arg)+))
    };
}
#[macro_export]
macro_rules! log_warn {
    ($target:expr, $($arg:tt)+) => {
        $crate::services::diagnostics::log::write(
            $crate::services::diagnostics::log::Level::Warn, $target, format_args!($($arg)+))
    };
}
#[macro_export]
macro_rules! log_info {
    ($target:expr, $($arg:tt)+) => {
        $crate::services::diagnostics::log::write(
            $crate::services::diagnostics::log::Level::Info, $target, format_args!($($arg)+))
    };
}
#[macro_export]
macro_rules! log_debug {
    ($target:expr, $($arg:tt)+) => {
        if $crate::services::diagnostics::log::enabled(
            $crate::services::diagnostics::log::Level::Debug,
        ) {
            $crate::services::diagnostics::log::write(
                $crate::services::diagnostics::log::Level::Debug, $target, format_args!($($arg)+))
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitizes_control_characters_tokens_and_home() {
        let token = "a".repeat(64);
        let s = sanitize(
            &format!("line\nbreak C:\\Users\\Ann\\proj token={token} short=abc123"),
            Some("C:\\Users\\Ann"),
        );
        assert_eq!(s, "line break ~\\proj token=[redacted] short=abc123");
        let long = "word ".repeat(400);
        assert!(sanitize(&long, None).chars().count() <= MAX_LINE_CHARS + 1);
    }

    #[test]
    fn formats_utc_timestamps() {
        assert_eq!(timestamp(0), "1970-01-01T00:00:00.000Z");
        assert_eq!(timestamp(951_782_400_123), "2000-02-29T00:00:00.123Z");
        assert_eq!(timestamp(1_794_528_000_000), "2026-11-13T00:00:00.000Z");
    }

    #[test]
    fn rotation_keeps_a_bounded_number_of_files() {
        let dir = std::env::temp_dir().join(format!("islandy-log-rotate-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let (file, size) = open(&dir).unwrap();
        let mut sink = Sink {
            dir: dir.clone(),
            file: Some(file),
            size,
        };
        for i in 0..6 {
            writeln!(sink.file.as_mut().unwrap(), "generation {i}").unwrap();
            rotate(&mut sink);
        }
        let mut names: Vec<_> = std::fs::read_dir(&dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().into_string().unwrap())
            .collect();
        names.sort();
        assert_eq!(names, ["islandy.1.log", "islandy.2.log", "islandy.log"]);
        assert_eq!(
            std::fs::read_to_string(dir.join("islandy.1.log")).unwrap(),
            "generation 5\n"
        );
        assert_eq!(
            std::fs::read_to_string(dir.join("islandy.2.log")).unwrap(),
            "generation 4\n"
        );
    }

    #[test]
    fn debug_entries_need_verbose_logging() {
        assert!(enabled(Level::Info));
        assert!(enabled(Level::Error));
    }
}
