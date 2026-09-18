//! Background-service lifecycle: install, uninstall, and status.
//!
//! The agent installs itself as the platform-native service — a launchd
//! LaunchDaemon on macOS, an SCM service on Windows — so the same binary that
//! runs in the foreground can also register and control its own service. This
//! mirrors how the Windows build already relied on `windows-service`, and
//! removes the need for hand-written `sc create` or `launchctl load` commands.
//!
//! On Windows the binary additionally runs as a *per-session helper*, launched
//! by the service into the interactive user's session. See `windows.rs`.

use std::process::ExitCode;

#[cfg(target_os = "macos")]
mod macos;

#[cfg(target_os = "windows")]
pub(crate) mod windows;

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
mod unsupported;

/// A command-line request that should be handled before the agent runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Command {
    /// Register the background service and start it.
    InstallService,
    /// Stop and deregister the background service, removing its files.
    UninstallService,
    /// Report whether the service is installed and running.
    ServiceStatus,
    /// Windows only: run as the per-session helper inside a user's session.
    SessionHelper,
}

/// Parse the mode from the process arguments.
///
/// Hand-rolled rather than pulling in `clap`: four flags do not justify a
/// dependency, and the agent's argument surface is deliberately tiny.
pub fn command_from_args() -> Option<Command> {
    std::env::args().skip(1).find_map(|argument| match argument.as_str() {
        "--install-service" => Some(Command::InstallService),
        "--uninstall-service" => Some(Command::UninstallService),
        "--status" => Some(Command::ServiceStatus),
        "--session-helper" => Some(Command::SessionHelper),
        _ => None,
    })
}

/// Handle a service command, printing human-readable output.
///
/// `SessionHelper` is only meaningful on Windows, where it is dispatched
/// separately by the caller — reaching here means the platform cannot support
/// it, which is reported rather than silently ignored.
pub fn run(command: Command) -> ExitCode {
    match command {
        Command::InstallService => report(install_service(), "installed"),
        Command::UninstallService => report(uninstall_service(), "uninstalled"),
        Command::ServiceStatus => status(),

        Command::SessionHelper => {
            crate::log_line!(
                "--session-helper is only supported on Windows; this build cannot \
                 capture the screen or location from a background session"
            );

            ExitCode::FAILURE
        }
    }
}

fn report(result: Result<(), String>, past_tense: &str) -> ExitCode {
    match result {
        Ok(()) => {
            crate::log_line!("GBOS agent service {}", past_tense);

            ExitCode::SUCCESS
        }

        Err(error) => {
            crate::log_line!("GBOS agent service could not be {}: {}", past_tense, error);

            ExitCode::FAILURE
        }
    }
}

// ---------------------------------------------------------------------------
// Platform dispatch
// ---------------------------------------------------------------------------

#[cfg(target_os = "macos")]
fn install_service() -> Result<(), String> {
    macos::install()
}

#[cfg(target_os = "macos")]
fn uninstall_service() -> Result<(), String> {
    macos::uninstall()
}

#[cfg(target_os = "macos")]
fn status() -> ExitCode {
    macos::status()
}

#[cfg(target_os = "windows")]
fn install_service() -> Result<(), String> {
    windows::install()
}

#[cfg(target_os = "windows")]
fn uninstall_service() -> Result<(), String> {
    windows::uninstall()
}

#[cfg(target_os = "windows")]
fn status() -> ExitCode {
    windows::status()
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
fn install_service() -> Result<(), String> {
    Err(unsupported::MESSAGE.to_string())
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
fn uninstall_service() -> Result<(), String> {
    Err(unsupported::MESSAGE.to_string())
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
fn status() -> ExitCode {
    crate::log_line!("{}", unsupported::MESSAGE);

    ExitCode::FAILURE
}
