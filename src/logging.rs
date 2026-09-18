//! Minimal file + stdout logging for the GBOS agent.
//!
//! The agent runs as a root launchd daemon on macOS and as a Session 0 Windows
//! service. Neither has a console, so `println!` is either lost or — on
//! Windows, where the service's stdout handle is invalid — panics on write.
//! Every log call therefore goes through [`log_line!`], which appends a
//! timestamped line to a file and *best-effort* echoes it to stdout, ignoring
//! write errors instead of unwrapping them.
//!
//! No logging crates: the whole thing is a `Mutex<File>` plus a size-based
//! rotation check, which is proportionate for an agent that emits a handful of
//! lines every few seconds.

use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

/// Rotate once the active file passes this size. One `.1` backup is kept.
const MAX_LOG_BYTES: u64 = 5 * 1024 * 1024;

/// Path of the currently open log file, for `--status` to report.
static ACTIVE_PATH: OnceLock<PathBuf> = OnceLock::new();

struct Logger {
    file: Option<File>,
    bytes_written: u64,
    echo_stdout: bool,
}

static LOGGER: OnceLock<Mutex<Logger>> = OnceLock::new();

fn logger() -> &'static Mutex<Logger> {
    LOGGER.get_or_init(|| {
        Mutex::new(Logger {
            file: None,
            bytes_written: 0,
            echo_stdout: true,
        })
    })
}

/// Default log file location for the current platform.
///
/// macOS uses `/var/log` rather than `~/Library/Logs` because the daemon runs
/// as root and has no home directory worth writing to. Windows uses
/// `ProgramData` for the same reason — a service has no per-user profile.
pub fn default_log_path() -> PathBuf {
    #[cfg(target_os = "windows")]
    {
        PathBuf::from(r"C:\ProgramData\GBOS\Agent\agent.log")
    }

    #[cfg(not(target_os = "windows"))]
    {
        PathBuf::from("/var/log/gbos-agent.log")
    }
}

/// Open `path` for appending and route all logging there.
///
/// Falls back to stdout-only if the file cannot be opened, so a permissions
/// problem degrades logging rather than killing the agent.
pub fn init(path: PathBuf) {
    let _ = ACTIVE_PATH.set(path.clone());

    let existing_bytes = fs::metadata(&path).map(|meta| meta.len()).unwrap_or(0);

    let file = match open_append(&path) {
        Ok(file) => Some(file),
        Err(error) => {
            let _ = writeln!(
                std::io::stdout(),
                "GBOS agent could not open log file {}: {}",
                path.display(),
                error
            );

            None
        }
    };

    let mut guard = match logger().lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };

    guard.file = file;
    guard.bytes_written = existing_bytes;
}

fn open_append(path: &Path) -> std::io::Result<File> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    OpenOptions::new().create(true).append(true).open(path)
}

/// Write one timestamped line. Never panics and never returns an error — a
/// failed log write must not take down the agent.
pub fn log_line(message: &str) {
    let line = format!("{} {}\n", timestamp(), message);

    let mut guard = match logger().lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };

    if guard.echo_stdout {
        // Ignore the error: a service has no valid stdout handle.
        let _ = std::io::stdout().write_all(line.as_bytes());
    }

    let Some(file) = guard.file.as_mut() else {
        return;
    };

    if file.write_all(line.as_bytes()).is_err() {
        return;
    }

    guard.bytes_written += line.len() as u64;

    if guard.bytes_written >= MAX_LOG_BYTES {
        rotate(&mut guard);
    }
}

/// Rename the active file to `<path>.1` and reopen a fresh one.
fn rotate(guard: &mut Logger) {
    let Some(path) = ACTIVE_PATH.get() else {
        return;
    };

    let backup = path.with_extension("log.1");

    // Drop the handle before renaming; Windows will not rename an open file.
    guard.file = None;

    if fs::rename(path, &backup).is_err() {
        // If the rename failed, keep appending to the original file rather
        // than silently losing output.
        guard.file = open_append(path).ok();
        return;
    }

    guard.file = open_append(path).ok();
    guard.bytes_written = 0;
}

/// Enable or disable echoing to stdout.
///
/// The Windows service disables this: its stdout handle is invalid, and every
/// write is a wasted syscall.
#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
pub fn set_stdout_echo(enabled: bool) {
    let mut guard = match logger().lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };

    guard.echo_stdout = enabled;
}

/// `YYYY-MM-DDTHH:MM:SSZ`, computed from the Unix epoch.
///
/// Hand-rolled rather than pulling in `chrono`: the agent needs one format and
/// always logs in UTC.
fn timestamp() -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();

    let total_seconds = now.as_secs() as i64;

    let days = total_seconds.div_euclid(86_400);
    let seconds_of_day = total_seconds.rem_euclid(86_400);

    let (year, month, day) = civil_from_days(days);

    let hour = seconds_of_day / 3_600;
    let minute = (seconds_of_day % 3_600) / 60;
    let second = seconds_of_day % 60;

    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        year, month, day, hour, minute, second
    )
}

/// Convert a count of days since 1970-01-01 into a civil (year, month, day).
///
/// Howard Hinnant's `civil_from_days` algorithm, shifted to a 0000-03-01 era
/// so leap years fall at the end of the cycle.
fn civil_from_days(days_since_epoch: i64) -> (i64, u32, u32) {
    let z = days_since_epoch + 719_468;

    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let day_of_era = (z - era * 146_097) as u64;

    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;

    let year = year_of_era as i64 + era * 400;

    let day_of_year =
        day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);

    let month_prime = (5 * day_of_year + 2) / 153;

    let day = (day_of_year - (153 * month_prime + 2) / 5 + 1) as u32;

    let month = if month_prime < 10 {
        month_prime + 3
    } else {
        month_prime - 9
    } as u32;

    let year = if month <= 2 { year + 1 } else { year };

    (year, month, day)
}

/// Log one timestamped line, with `format!`-style arguments.
///
/// ```ignore
/// log_line!("Heartbeat sent 💓");
/// log_line!("Connected to {}", url);
/// ```
///
/// `$crate` rather than `crate` is required: this macro is defined in the
/// library but called from the binary and its modules too, and `crate` would
/// resolve against whichever crate the call site is in.
#[macro_export]
macro_rules! log_line {
    ($($arg:tt)*) => {
        $crate::logging::log_line(&format!($($arg)*))
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_epoch_to_civil_dates() {
        assert_eq!(civil_from_days(0), (1970, 1, 1));
        assert_eq!(civil_from_days(19_723), (2024, 1, 1));
        // 2024 is a leap year: 2024-02-29 is day 19782.
        assert_eq!(civil_from_days(19_782), (2024, 2, 29));
        assert_eq!(civil_from_days(-1), (1969, 12, 31));
    }
}
