//! macOS launchd LaunchDaemon installation.
//!
//! The agent installs as a **system daemon** (`/Library/LaunchDaemons`), so it
//! starts at boot and keeps running across logout and across user sessions —
//! which is what "runs in the background like software" means for a managed
//! device.
//!
//! A root daemon has no GUI session, so Screen Recording and Core Location are
//! unavailable to it (both are per-user TCC permissions that cannot be prompted
//! for from a daemon). `video.rs` and `location.rs` therefore gate those
//! features off on macOS — see `SCREEN_STREAM_SUPPORTED` there.
//!
//! Because nothing needs bundle identity or a GUI session, no `.app` bundle is
//! installed: the daemon runs a bare binary at [`INSTALL_PATH`].

use std::fs;
use std::path::Path;
use std::process::{Command, ExitCode, Output};

/// launchd job label, matching the bundle identifier used elsewhere.
const LABEL: &str = "com.gbaja.gbos.agent";

/// Where the daemon binary is installed. `libexec` is the conventional home
/// for a helper binary that users are not meant to run directly.
const INSTALL_PATH: &str = "/usr/local/libexec/gbos-agent";

const PLIST_PATH: &str = "/Library/LaunchDaemons/com.gbaja.gbos.agent.plist";

/// Matches `logging::default_log_path`, so the plist and the in-process logger
/// agree on where output goes.
const LOG_PATH: &str = "/var/log/gbos-agent.log";
const ERROR_LOG_PATH: &str = "/var/log/gbos-agent.err.log";

/// Install (or upgrade) the daemon and start it.
///
/// Idempotent: an existing job is booted out first, so running this again is
/// how an upgrade is applied.
pub fn install() -> Result<(), String> {
    let source = std::env::current_exe()
        .map_err(|error| format!("could not locate the running binary: {}", error))?;

    let install_path = Path::new(INSTALL_PATH);

    install_binary(&source, install_path)?;

    write_plist(Path::new(PLIST_PATH))?;

    // Boot out first so a reinstall replaces the running job instead of
    // failing with "service already loaded".
    let _ = launchctl(&["bootout", "system", PLIST_PATH]);

    launchctl(&["bootstrap", "system", PLIST_PATH])?;
    launchctl(&["enable", &format!("system/{}", LABEL)])?;
    launchctl(&["kickstart", "-k", &format!("system/{}", LABEL)])?;

    crate::log_line!(
        "GBOS agent daemon running from {} — logs at {}",
        INSTALL_PATH,
        LOG_PATH
    );

    Ok(())
}

/// Stop the daemon and remove everything `install` created.
pub fn uninstall() -> Result<(), String> {
    // A failure here usually just means it was not loaded; the removal below is
    // what actually matters, so do not treat it as fatal.
    let _ = launchctl(&["bootout", "system", PLIST_PATH]);

    for path in [PLIST_PATH, INSTALL_PATH] {
        match fs::remove_file(path) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(format!("could not remove {}: {}", path, error)),
        }
    }

    crate::log_line!("GBOS agent daemon removed (logs left at {})", LOG_PATH);

    Ok(())
}

/// Report whether the daemon is installed and running.
pub fn status() -> ExitCode {
    let plist_exists = Path::new(PLIST_PATH).exists();
    let binary_exists = Path::new(INSTALL_PATH).exists();

    println!("GBOS agent service status");
    println!("  label:      {}", LABEL);
    println!(
        "  plist:      {} ({})",
        PLIST_PATH,
        if plist_exists { "present" } else { "missing" }
    );
    println!(
        "  binary:     {} ({})",
        INSTALL_PATH,
        if binary_exists { "present" } else { "missing" }
    );
    println!("  log:        {}", LOG_PATH);

    match Command::new("launchctl")
        .args(["print", &format!("system/{}", LABEL)])
        .output()
    {
        Ok(output) if output.status.success() => {
            let text = String::from_utf8_lossy(&output.stdout);

            // launchctl print emits `state = running` among many other fields.
            let state = text
                .lines()
                .map(str::trim)
                .find_map(|line| line.strip_prefix("state = "))
                .unwrap_or("unknown");

            println!("  loaded:     yes");
            println!("  state:      {}", state);

            // A loaded-but-crashed job is the confusing case worth flagging.
            if state == "running" {
                ExitCode::SUCCESS
            } else {
                println!();
                println!(
                    "The daemon is loaded but not running. Check {} for why it exited.",
                    ERROR_LOG_PATH
                );

                ExitCode::FAILURE
            }
        }

        Ok(_) => {
            println!("  loaded:     no");
            println!();
            println!("The daemon is not registered. Install it with:");
            println!("  sudo {} --install-service", INSTALL_PATH);

            ExitCode::FAILURE
        }

        Err(error) => {
            println!("  loaded:     unknown (launchctl failed: {})", error);

            ExitCode::FAILURE
        }
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Copy the running binary into place and make it executable.
fn install_binary(source: &Path, destination: &Path) -> Result<(), String> {
    if source == destination {
        // Already running from the install path — nothing to copy, and
        // overwriting a running executable is best avoided.
        return Ok(());
    }

    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("could not create {}: {}", parent.display(), error))?;
    }

    fs::copy(source, destination).map_err(|error| {
        format!(
            "could not install {} to {}: {}\n\
             Installing a system daemon requires root — re-run with sudo.",
            source.display(),
            destination.display(),
            error
        )
    })?;

    set_mode(destination, 0o755)
}

/// Write the launchd plist that describes the daemon.
fn write_plist(path: &Path) -> Result<(), String> {
    let plist = plist_contents();

    // Remove any existing plist so the write cannot fail on a read-only or
    // oddly-owned leftover from a previous install.
    let _ = fs::remove_file(path);

    fs::write(path, plist).map_err(|error| {
        format!(
            "could not write {}: {}\n\
             Installing a system daemon requires root — re-run with sudo.",
            path.display(),
            error
        )
    })?;

    set_mode(path, 0o644)?;

    // launchd requires the plist to be owned by root; a leftover file from an
    // unprivileged install would otherwise be silently ignored.
    let _ = Command::new("chown")
        .arg("root:wheel")
        .arg(path)
        .status();

    Ok(())
}

fn plist_contents() -> String {
    // Bake the API URL in so a build pointed at a non-production backend keeps
    // pointing there once it is running as a service. launchd starts the
    // daemon with a near-empty environment, so this cannot come from the shell.
    let api_url = crate::gbos_api_url();

    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>{label}</string>

    <key>ProgramArguments</key>
    <array>
        <string>{binary}</string>
    </array>

    <key>RunAtLoad</key>
    <true/>

    <!-- Restart on an unclean exit, but honour a deliberate stop. A plain
         `true` would fight `launchctl bootout`. -->
    <key>KeepAlive</key>
    <dict>
        <key>SuccessfulExit</key>
        <false/>
    </dict>

    <!-- Cap restart storms if the backend is unreachable for a long time. -->
    <key>ThrottleInterval</key>
    <integer>10</integer>

    <key>ProcessType</key>
    <string>Background</string>

    <key>EnvironmentVariables</key>
    <dict>
        <key>GBOS_API_URL</key>
        <string>{api_url}</string>
    </dict>

    <key>StandardOutPath</key>
    <string>{log}</string>

    <key>StandardErrorPath</key>
    <string>{error_log}</string>
</dict>
</plist>
"#,
        label = LABEL,
        binary = INSTALL_PATH,
        api_url = api_url,
        log = LOG_PATH,
        error_log = ERROR_LOG_PATH,
    )
}

fn set_mode(path: &Path, mode: u32) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;

    fs::set_permissions(path, fs::Permissions::from_mode(mode))
        .map_err(|error| format!("could not set mode {:o} on {}: {}", mode, path.display(), error))
}

/// Run `launchctl` with `args`, mapping a non-zero exit into an error that
/// includes its stderr.
fn launchctl(args: &[&str]) -> Result<Output, String> {
    let output = Command::new("launchctl")
        .args(args)
        .output()
        .map_err(|error| format!("could not run launchctl {}: {}", args.join(" "), error))?;

    if output.status.success() {
        return Ok(output);
    }

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);

    let detail = if stderr.trim().is_empty() {
        stdout.trim().to_string()
    } else {
        stderr.trim().to_string()
    };

    Err(format!(
        "launchctl {} failed ({}): {}",
        args.join(" "),
        output.status,
        detail
    ))
}
