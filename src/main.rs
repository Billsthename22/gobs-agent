mod service;

#[cfg(target_os = "macos")]
mod location;

#[cfg(target_os = "windows")]
mod location_windows;

#[cfg(target_os = "windows")]
use location_windows as location;

#[cfg(target_os = "windows")]
mod session;

// Mounts the inert stand-in so that `session::SessionBridge` can be named on
// every platform and the event loop needs no platform branches. See the module's
// own comment.
#[cfg(not(target_os = "windows"))]
mod session_noop;

#[cfg(not(target_os = "windows"))]
use session_noop as session;

// `logging` and `video` live in the library (see `lib.rs`) so the diagnostics
// under `src/bin/` can exercise the real pipeline. Importing them rather than
// declaring `mod` here keeps a single copy of each — redeclaring would compile
// the module twice and give the binary a second, separate implementation.
use agent::{log_line, logging, video};

use std::env;
use std::process::ExitCode;
use std::time::Duration;

/// Backend the agent talks to, overridable per install.
///
/// Also baked into the macOS launchd plist and read from the service
/// environment on Windows, because a service starts with a near-empty
/// environment and cannot inherit this from an interactive shell.
pub(crate) fn gbos_api_url() -> String {
    env::var("GBOS_API_URL")
        .unwrap_or_else(|_| "https://gbos-backend-production.up.railway.app".to_string())
        .trim_end_matches('/')
        .to_string()
}

fn gbos_ws_url() -> String {
    let api_url = gbos_api_url();

    if let Some(rest) = api_url.strip_prefix("https://") {
        format!("wss://{}", rest)
    } else if let Some(rest) = api_url.strip_prefix("http://") {
        format!("ws://{}", rest)
    } else {
        format!("ws://{}", api_url)
    }
}

use futures_util::{SinkExt, StreamExt};
use serde_json::json;
use sysinfo::{Disks, Networks, System};
use tokio::signal;
use tokio::sync::mpsc;
use tokio::time::interval;

#[derive(serde::Deserialize)]
struct AgentRegisterResponse {
    device_id: i32,
    device_name: String,
    hostname: String,
    status: String,
    websocket_url: String,
}

async fn register_agent() -> Result<i32, Box<dyn std::error::Error>> {
    let client = reqwest::Client::new();

    let hostname = sysinfo::System::host_name().unwrap_or_else(|| "GBOS-UNKNOWN".to_string());

    let operating_system =
        sysinfo::System::long_os_version().unwrap_or_else(|| std::env::consts::OS.to_string());

    let payload = serde_json::json!({
        "device_name": hostname,
        "hostname": hostname,
        "operating_system": operating_system,
        "ip_address": null
    });

    crate::log_line!("Registering agent with GBOS backend...");
    crate::log_line!("Hostname: {}", hostname);
    crate::log_line!("OS: {}", operating_system);

    let response = client
        .post(format!("{}/agents/register", gbos_api_url()))
        .json(&payload)
        .send()
        .await?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();

        return Err(format!("Agent registration failed: {} - {}", status, body).into());
    }

    let registration: AgentRegisterResponse = response.json().await?;

    crate::log_line!("Agent registered successfully!");
    crate::log_line!("Device ID: {}", registration.device_id);
    crate::log_line!("Device name: {}", registration.device_name);
    crate::log_line!("Hostname: {}", registration.hostname);
    crate::log_line!("Status: {}", registration.status);
    crate::log_line!("WebSocket: {}", registration.websocket_url);

    Ok(registration.device_id)
}

use tokio_tungstenite::{connect_async, tungstenite::Message};

fn current_username() -> String {
    env::var("USER")
        .or_else(|_| env::var("USERNAME"))
        .unwrap_or_else(|_| "Unknown user".to_string())
}

#[cfg(target_os = "macos")]
fn detect_console_user() -> Option<String> {
    use std::io::Write;
    use std::process::{Command, Stdio};

    let mut child = Command::new("scutil")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .ok()?;

    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(b"show State:/Users/ConsoleUser\n").ok()?;
    }

    let output = child.wait_with_output().ok()?;
    let output = String::from_utf8_lossy(&output.stdout);

    for line in output.lines() {
        let trimmed = line.trim();

        if trimmed.starts_with("Name :") {
            let username = trimmed.strip_prefix("Name :")?.trim();

            if !username.is_empty() && username != "loginwindow" {
                return Some(username.to_string());
            }
        }
    }

    None
}

#[cfg(target_os = "windows")]
fn detect_console_user() -> Option<String> {
    // Deliberately not `USERNAME`: a service runs in Session 0 as LocalSystem,
    // so its environment says "SYSTEM" no matter who is logged in, and session
    // tracking would record that instead of the real user. Ask the session
    // instead — see `session::active_console_user`.
    session::active_console_user()
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
fn detect_console_user() -> Option<String> {
    std::env::var("USER")
        .or_else(|_| std::env::var("USERNAME"))
        .ok()
        .filter(|username| !username.trim().is_empty())
}

// ---------------------------------------------------------------------------
// OS shutdown requests
// ---------------------------------------------------------------------------

/// Shutdown signals from the operating system.
///
/// Built **once** and reused across loop iterations on purpose: a signal
/// stream recreated inside `tokio::select!` would drop anything that arrived
/// between iterations.
///
/// launchd stops a daemon with SIGTERM, and `signal::ctrl_c()` only covers
/// SIGINT. Without this the agent would die before sending the WebSocket
/// `Close` frame, leaving the device marked "online" in the backend until its
/// ping cycle expires.
#[cfg(unix)]
struct StopSignals {
    sigterm: Option<tokio::signal::unix::Signal>,
    sighup: Option<tokio::signal::unix::Signal>,
}

#[cfg(unix)]
impl StopSignals {
    fn new() -> Self {
        use tokio::signal::unix::{signal, SignalKind};

        Self {
            sigterm: signal(SignalKind::terminate()).ok(),
            sighup: signal(SignalKind::hangup()).ok(),
        }
    }

    /// Resolves when SIGTERM or SIGHUP arrives.
    async fn recv(&mut self) {
        match (self.sigterm.as_mut(), self.sighup.as_mut()) {
            (Some(sigterm), Some(sighup)) => {
                tokio::select! {
                    _ = sigterm.recv() => {}
                    _ = sighup.recv() => {}
                }
            }

            (Some(sigterm), None) => {
                sigterm.recv().await;
            }

            (None, Some(sighup)) => {
                sighup.recv().await;
            }

            // Never resolves rather than panicking, so a platform that refuses
            // to install the handler still runs normally.
            (None, None) => std::future::pending::<()>().await,
        }
    }
}

/// Windows stops the service through the SCM control handler, which the
/// `shutdown_rx` watch channel already reports, so there is nothing extra to
/// listen for.
#[cfg(not(unix))]
struct StopSignals;

#[cfg(not(unix))]
impl StopSignals {
    fn new() -> Self {
        Self
    }

    async fn recv(&mut self) {
        std::future::pending::<()>().await
    }
}

async fn run_agent(mut shutdown_rx: Option<tokio::sync::watch::Receiver<bool>>) {
    let device_id = loop {
        if shutdown_rx
            .as_ref()
            .map(|receiver| *receiver.borrow())
            .unwrap_or(false)
        {
            crate::log_line!("GBOS service shutdown requested before registration 🛑");
            return;
        }

        match register_agent().await {
            Ok(id) => break id,
            Err(error) => {
                crate::log_line!("Agent registration failed!");
                crate::log_line!("Error: {}", error);
                crate::log_line!("Retrying registration in 5 seconds...");
            }
        }

        if let Some(receiver) = shutdown_rx.as_mut() {
            tokio::select! {
                _ = tokio::time::sleep(Duration::from_secs(5)) => {}

                result = receiver.changed() => {
                    if result.is_err() || *receiver.borrow() {
                        crate::log_line!("GBOS service shutdown requested 🛑");
                        return;
                    }
                }
            }
        } else {
            tokio::time::sleep(Duration::from_secs(5)).await;
        }
    };

    crate::log_line!("GBOS agent registered with device ID {}", device_id);

    loop {
        if shutdown_rx
            .as_ref()
            .map(|receiver| *receiver.borrow())
            .unwrap_or(false)
        {
            crate::log_line!("GBOS service shutdown requested 🛑");
            return;
        }

        crate::log_line!("Starting GBOS monitoring session...");

        run_session(device_id, &mut shutdown_rx).await;

        if shutdown_rx
            .as_ref()
            .map(|receiver| *receiver.borrow())
            .unwrap_or(false)
        {
            crate::log_line!("GBOS service shutdown completed 🛑");
            return;
        }

        crate::log_line!("GBOS connection lost.");
        crate::log_line!("Reconnecting in 5 seconds...");

        if let Some(receiver) = shutdown_rx.as_mut() {
            tokio::select! {
                _ = tokio::time::sleep(Duration::from_secs(5)) => {}

                result = receiver.changed() => {
                    if result.is_err() || *receiver.borrow() {
                        crate::log_line!("GBOS service shutdown requested 🛑");
                        return;
                    }
                }
            }
        } else {
            tokio::time::sleep(Duration::from_secs(5)).await;
        }
    }
}

async fn run_session(device_id: i32, shutdown_rx: &mut Option<tokio::sync::watch::Receiver<bool>>) {
    let initial_console_user = detect_console_user();

    crate::log_line!("Current user: {}", current_username());
    crate::log_line!("Console user: {:?}", initial_console_user);

    let mut last_console_user = initial_console_user;

    let url = format!("{}/ws/devices/{}", gbos_ws_url(), device_id);

    crate::log_line!("GBOS Device Agent");
    crate::log_line!("Connecting to {}", url);

    let (mut websocket, _) = match connect_async(&url).await {
        Ok(connection) => {
            crate::log_line!("Connected to GBOS backend ✅");
            connection
        }
        Err(error) => {
            crate::log_line!("Connection failed ❌");
            crate::log_line!("Error: {}", error);
            return;
        }
    };

    // Send LOGIN for the currently active console session.
    if let Some(username) = &last_console_user {
        let login_message = json!({
            "type": "session",
            "event": "login",
            "username": username
        });

        if let Err(error) = websocket
            .send(Message::Text(login_message.to_string().into()))
            .await
        {
            crate::log_line!("Login event failed ❌");
            crate::log_line!("Error: {}", error);
            return;
        }

        crate::log_line!("Login event sent 🔐 for {}", username);
    } else {
        crate::log_line!("No active console user detected.");
    }

    let mut system = System::new();

    // Network tracking.
    let mut networks = Networks::new_with_refreshed_list();
    let mut previous_network_bytes: u64 = 0;

    // Establish CPU measurement baseline.
    system.refresh_cpu_usage();

    tokio::time::sleep(Duration::from_secs(1)).await;

    system.refresh_cpu_usage();
    system.refresh_memory();

    // Establish initial network byte baseline.
    networks.refresh(true);

    for (_, network) in &networks {
        previous_network_bytes += network.received();
        previous_network_bytes += network.transmitted();
    }

    let mut heartbeat = interval(Duration::from_secs(5));
    let mut telemetry_timer = interval(Duration::from_secs(5));
    let mut session_timer = interval(Duration::from_secs(5));

    // Screen streaming is disabled by default.
    // The backend explicitly enables it when an authorized
    // remote monitoring session is started. The agent then runs a
    // dedicated H.264 capture/encode thread that pushes packets onto a
    // channel the websocket loop forwards as binary messages.
    let (screen_tx, mut screen_rx) = mpsc::unbounded_channel::<video::StreamPacket>();

    let mut screen_stream: Option<video::VideoStream> = None;

    // On Windows the agent normally runs as a service, which lives in Session 0
    // and therefore cannot see the user's desktop or obtain a per-user location
    // permission. There a helper runs inside the user's session and hands its
    // results back over a named pipe; everywhere else this is inert and the
    // agent captures for itself. See `session.rs`.
    let mut session_bridge = session::SessionBridge::new(screen_tx.clone());

    let helper_owns_capture = session_bridge.delegates();

    // GPS collection is disabled by default.
    // The backend explicitly enables it through an authorized
    // GPS_COLLECTION command.
    //
    // Only constructed where GPS can actually work. That is false in two cases:
    // on macOS the daemon has no GUI session and Core Location authorization
    // cannot be granted to it, and on Windows the service's own `Geolocator`
    // would be asking on behalf of Session 0, where there is no user to grant
    // anything. In both, building the manager would spawn a thread that can
    // never produce a fix.
    let location_manager = if location::GPS_SUPPORTED && !helper_owns_capture {
        Some(location::LocationManager::new())
    } else {
        None
    };

    let mut gps_enabled = false;
    let mut gps_timer = interval(Duration::from_secs(10));

    // Created before the loop so no shutdown signal is dropped between
    // iterations.
    let mut stop_signals = StopSignals::new();

    loop {
        tokio::select! {
            // WINDOWS SERVICE SHUTDOWN
            _ = async {
                if let Some(receiver) = shutdown_rx.as_mut() {
                    let _ = receiver.changed().await;
                } else {
                    std::future::pending::<()>().await;
                }
            } => {
                if shutdown_rx
                    .as_ref()
                    .map(|receiver| *receiver.borrow())
                    .unwrap_or(false)
                {
                    crate::log_line!("GBOS service shutdown requested 🛑");

                    if let Err(error) = websocket
                        .send(Message::Close(None))
                        .await
                    {
                        crate::log_line!("Close frame failed ❌");
                        crate::log_line!("Error: {}", error);
                    }

                    break;
                }
            }

            // OS SHUTDOWN (SIGTERM from launchd, SIGHUP)
            _ = stop_signals.recv() => {
                crate::log_line!("Shutdown signal received — disconnecting gracefully 🛑");

                // Send a proper close handshake so the backend marks the device
                // offline immediately instead of waiting out its ping cycle.
                if let Err(error) = websocket
                    .send(Message::Close(None))
                    .await
                {
                    crate::log_line!("Close frame failed ❌");
                    crate::log_line!("Error: {}", error);
                }

                break;
            }

            // HEARTBEAT
            _ = heartbeat.tick() => {
                let message = json!({
                    "type": "heartbeat"
                });

                if let Err(error) = websocket
                    .send(Message::Text(message.to_string().into()))
                    .await
                {
                    crate::log_line!("Heartbeat failed ❌");
                    crate::log_line!("Error: {}", error);
                    break;
                }

                crate::log_line!("Heartbeat sent 💓");
            }

            // TELEMETRY
            _ = telemetry_timer.tick() => {
                system.refresh_cpu_usage();
                system.refresh_memory();

                let cpu_usage = system.global_cpu_usage();

                let total_memory = system.total_memory();
                let used_memory = system.used_memory();

                let memory_usage = if total_memory > 0 {
                    (used_memory as f64 / total_memory as f64) * 100.0
                } else {
                    0.0
                };

                let disks = Disks::new_with_refreshed_list();

                let mut storage_usage = 0.0;
                let mut largest_disk_size = 0;

                for disk in disks.list() {
                    let total_space = disk.total_space();
                    let available_space = disk.available_space();

                    if total_space == 0 {
                        continue;
                    }

                    if total_space > largest_disk_size {
                        largest_disk_size = total_space;

                        let used_space = total_space - available_space;

                        storage_usage =
                            (used_space as f64 / total_space as f64) * 100.0;
                    }
                }

                networks.refresh(true);

                let mut total_network_bytes: u64 = 0;

                for (_, network) in &networks {
                    total_network_bytes += network.received();
                    total_network_bytes += network.transmitted();
                }

                let bytes_transferred =
                    total_network_bytes.saturating_sub(previous_network_bytes);

                let bytes_per_second =
                    bytes_transferred as f64 / 5.0;

                let network_speed_mbps =
                    (bytes_per_second * 8.0) / 1_000_000.0;

                previous_network_bytes = total_network_bytes;

                crate::log_line!("CPU usage: {:.1}%", cpu_usage);
                crate::log_line!("Memory usage: {:.1}%", memory_usage);
                crate::log_line!("Storage usage: {:.1}%", storage_usage);
                crate::log_line!("Network speed: {:.2} Mbps", network_speed_mbps);

                let telemetry = json!({
                    "type": "telemetry",
                    "cpu_usage": cpu_usage,
                    "memory_usage": memory_usage,
                    "storage_usage": storage_usage,
                    "network_speed_mbps": network_speed_mbps
                });

                if let Err(error) = websocket
                    .send(Message::Text(telemetry.to_string().into()))
                    .await
                {
                    crate::log_line!("Telemetry failed ❌");
                    crate::log_line!("Error: {}", error);
                    break;
                }

                crate::log_line!("CPU + memory + storage + network telemetry sent 📊");
            }

            // GPS LOCATION
            // Only runs while the backend has GPS collection enabled; whichever
            // source is in play reports nothing until it has a fix.
            _ = gps_timer.tick() => {
                // `gps_enabled` can only become true where GPS is supported
                // (see the GPS_COLLECTION handler), so this also skips the
                // unsupported case.
                if gps_enabled {
                    // A Windows service has no location of its own: the fix, if
                    // there is one, was measured by the session helper.
                    let fix = if helper_owns_capture {
                        session_bridge.take_fix()
                    } else {
                        location_manager
                            .as_ref()
                            .and_then(|manager| manager.current_location())
                    };

                    match fix {
                        Some((latitude, longitude)) => {
                            let message = json!({
                                "type": "gps_location",
                                "latitude": latitude,
                                "longitude": longitude
                            });

                            if let Err(error) = websocket
                                .send(Message::Text(message.to_string().into()))
                                .await
                            {
                                crate::log_line!("Location send failed ❌");
                                crate::log_line!("Error: {}", error);
                                break;
                            }

                            crate::log_line!(
                                "Location sent 📍 ({:.5}, {:.5})",
                                latitude,
                                longitude
                            );
                        }

                        None => {
                            crate::log_line!(
                                "GPS enabled but no fix yet 📍"
                            );
                        }
                    }
                }
            }

            // SCREEN STREAM
            // Encoded H.264 frames arrive from the capture thread's channel.
            Some(packet) = screen_rx.recv() => {
                let is_config =
                    packet.kind == video::STREAM_KIND_CONFIG;

                let binary = packet.into_binary();

                let binary_len = binary.len();

                if let Err(error) = websocket
                    .send(Message::Binary(binary.into()))
                    .await
                {
                    crate::log_line!("Screen stream send failed ❌");
                    crate::log_line!("Error: {}", error);
                    break;
                }

                if is_config {
                    crate::log_line!(
                        "Screen stream config forwarded ⏩ ({:.1} KB)",
                        binary_len as f64 / 1024.0
                    );
                }
            }

            // AUTOMATIC SESSION MONITORING
            _ = session_timer.tick() => {
                // Follow the console session: start a helper when a user logs
                // in, and end it when they log out or switch away. Inert unless
                // this process is the Windows service.
                session_bridge.tick();

                let current_console_user = detect_console_user();

                if current_console_user != last_console_user {
                    crate::log_line!(
                        "Console session changed: {:?} -> {:?}",
                        last_console_user,
                        current_console_user
                    );

                    // User logged out.
                    if let Some(previous_user) = &last_console_user {
                        let logout_message = json!({
                            "type": "session",
                            "event": "logout",
                            "username": previous_user
                        });

                        if let Err(error) = websocket
                            .send(Message::Text(logout_message.to_string().into()))
                            .await
                        {
                            crate::log_line!("Logout event failed ❌");
                            crate::log_line!("Error: {}", error);
                            break;
                        }

                        crate::log_line!(
                            "Logout event sent 🔓 for {}",
                            previous_user
                        );
                    }

                    // New user logged in.
                    if let Some(new_user) = &current_console_user {
                        let login_message = json!({
                            "type": "session",
                            "event": "login",
                            "username": new_user
                        });

                        if let Err(error) = websocket
                            .send(Message::Text(login_message.to_string().into()))
                            .await
                        {
                            crate::log_line!("Login event failed ❌");
                            crate::log_line!("Error: {}", error);
                            break;
                        }

                        crate::log_line!(
                            "Login event sent 🔐 for {}",
                            new_user
                        );
                    }

                    last_console_user = current_console_user;
                }
            }

            // SHUTDOWN SIGNAL (Ctrl-C / SIGTERM)
            _ = signal::ctrl_c() => {
                crate::log_line!("Shutdown signal received — disconnecting gracefully 🛑");

                // Send a proper close handshake so the backend marks the
                // device offline immediately instead of waiting out its
                // websocket ping cycle.
                if let Err(error) = websocket
                    .send(Message::Close(None))
                    .await
                {
                    crate::log_line!("Close frame failed ❌");
                    crate::log_line!("Error: {}", error);
                }

                break;
            }

            // BACKEND MESSAGES
            message = websocket.next() => {
                match message {
                    Some(Ok(Message::Text(text))) => {
                        crate::log_line!("Backend: {}", text);

                        match serde_json::from_str::<serde_json::Value>(&text) {
                            Ok(message) => {
                                match message.get("type").and_then(|value| value.as_str()) {
                                    Some("START_SCREEN_STREAM") => {
                                        if !video::SCREEN_STREAM_SUPPORTED {
                                            // Refusing loudly beats silently
                                            // ignoring the backend, which would
                                            // look like a hung monitoring session.
                                            crate::log_line!(
                                                "Backend requested screen streaming, but this \
                                                 build cannot capture the screen: {}. \
                                                 Ignoring the request.",
                                                video::SCREEN_STREAM_UNAVAILABLE_REASON
                                            );
                                        } else if helper_owns_capture {
                                            // The service itself would capture
                                            // Session 0's blank desktop. The
                                            // helper is the only thing that can
                                            // see the user's screen, so the
                                            // request is passed to it; its frames
                                            // arrive on the same channel the
                                            // in-process capture thread would
                                            // have used.
                                            crate::log_line!(
                                                "Screen streaming requested; delegating to the \
                                                 session helper 🖥️ (H.264)"
                                            );

                                            session_bridge.set_screen(true);
                                        } else if screen_stream.is_none() {
                                            crate::log_line!(
                                                "Screen streaming ENABLED 🖥️ (H.264)"
                                            );

                                            match video::VideoStream::start(
                                                screen_tx.clone(),
                                            ) {
                                                Ok(handle) => {
                                                    screen_stream =
                                                        Some(handle);
                                                }
                                                Err(error) => {
                                                    crate::log_line!(
                                                        "Screen stream start failed ❌"
                                                    );
                                                    crate::log_line!(
                                                        "Error: {}",
                                                        error
                                                    );
                                                }
                                            }
                                        }
                                    }

                                    Some("STOP_SCREEN_STREAM") => {
                                        if helper_owns_capture {
                                            session_bridge.set_screen(false);

                                            crate::log_line!(
                                                "Screen streaming DISABLED 🛑"
                                            );
                                        } else if let Some(mut handle) =
                                            screen_stream.take()
                                        {
                                            handle.stop();

                                            crate::log_line!(
                                                "Screen streaming DISABLED 🛑"
                                            );
                                        }
                                    }

                                    Some("GPS_COLLECTION") => {
                                        let enabled = message
                                            .get("enabled")
                                            .and_then(|value| value.as_bool())
                                            .unwrap_or(false);

                                        if !location::GPS_SUPPORTED {
                                            // Only worth reporting when the
                                            // backend asked to turn it *on*.
                                            if enabled {
                                                crate::log_line!(
                                                    "Backend enabled GPS collection, but this \
                                                     build cannot read location: {}. \
                                                     Ignoring the request.",
                                                    location::GPS_UNAVAILABLE_REASON
                                                );
                                            }
                                        } else if helper_owns_capture {
                                            // The service has no location of
                                            // its own to offer — Session 0 has
                                            // no user to grant the permission —
                                            // so this is the helper's to answer.
                                            // It is also told when collection is
                                            // turned *off*, so a helper that
                                            // connects later is not left
                                            // measuring.
                                            session_bridge.set_gps(enabled);

                                            gps_enabled = enabled;

                                            if enabled {
                                                crate::log_line!(
                                                    "GPS collection ENABLED; delegating to the \
                                                     session helper 📍"
                                                );
                                            } else {
                                                crate::log_line!(
                                                    "GPS collection DISABLED 🛑"
                                                );
                                            }
                                        } else if enabled && !gps_enabled {
                                            gps_enabled = true;

                                            if let Some(manager) =
                                                location_manager.as_ref()
                                            {
                                                manager.start();
                                            }

                                            crate::log_line!(
                                                "GPS collection ENABLED 📍"
                                            );
                                        } else if !enabled && gps_enabled {
                                            gps_enabled = false;

                                            if let Some(manager) =
                                                location_manager.as_ref()
                                            {
                                                manager.stop();
                                            }

                                            crate::log_line!(
                                                "GPS collection DISABLED 🛑"
                                            );
                                        }
                                    }

                                    _ => {}
                                }
                            }

                            Err(error) => {
                                crate::log_line!(
                                    "Backend message is not JSON: {}",
                                    error
                                );
                            }
                        }
                    }

                    Some(Ok(Message::Close(_))) => {
                        crate::log_line!("Backend closed connection 🔌");
                        break;
                    }

                    Some(Err(error)) => {
                        crate::log_line!("WebSocket error ❌");
                        crate::log_line!("Error: {}", error);
                        break;
                    }

                    None => {
                        crate::log_line!("Connection ended 🔌");
                        break;
                    }

                    _ => {}
                }
            }
        }
    }

    // Stop the screen capture/encode thread once the websocket closes so no
    // encode thread outlives the connection.
    if let Some(mut handle) = screen_stream.take() {
        handle.stop();
    }

    // Stop GPS collection so the delegate thread halts updates before exit.
    if let Some(manager) = location_manager.as_ref() {
        manager.stop();
    }

    // End the session helper too. The helper holds no backend connection of its
    // own, so nothing else would ever tell it to stop — and leaving it running
    // would mean a capture thread that survives both the connection and, on a
    // reconnect, would double up with the next helper.
    session_bridge.stop();
}

// ---------------------------------------------------------------------------
// Entry points
// ---------------------------------------------------------------------------

/// Entry point shared by every platform.
///
/// Service lifecycle commands (`--install-service` and friends) are handled
/// before anything else, so the same binary both runs the agent and manages
/// its own installation.
fn main() -> ExitCode {
    if let Some(command) = service::command_from_args() {
        return handle_service_command(command);
    }

    run_agent_entry()
}

fn handle_service_command(command: service::Command) -> ExitCode {
    // The Windows per-session helper is dispatched separately: it runs inside a
    // user's session and talks to the service, never to the backend directly.
    #[cfg(target_os = "windows")]
    if command == service::Command::SessionHelper {
        return session::run_helper();
    }

    // Service commands run from a terminal, so echo to stdout as well as the
    // log file.
    logging::init(logging::default_log_path());

    service::run(command)
}

/// Run the agent itself — under the service manager where there is one.
#[cfg(target_os = "windows")]
fn run_agent_entry() -> ExitCode {
    match windows_service::service_dispatcher::start(service::windows::SERVICE_NAME, ffi_service_main)
    {
        Ok(()) => ExitCode::SUCCESS,

        Err(error) if is_not_started_by_service_manager(&error) => {
            // Launched from a console (double-click, or `GBOS-Agent.exe` typed
            // at a prompt) rather than by the SCM. Run in the foreground so the
            // binary is still useful for diagnosis.
            logging::init(logging::default_log_path());
            logging::set_stdout_echo(true);

            crate::log_line!(
                "Not started by the service manager — running in the foreground. \
                 Install the background service with --install-service."
            );

            run_foreground();

            ExitCode::SUCCESS
        }

        Err(error) => {
            eprintln!("GBOS agent service dispatcher failed: {}", error);

            ExitCode::FAILURE
        }
    }
}

/// Windows error raised when the process was not launched by the SCM.
#[cfg(target_os = "windows")]
const ERROR_FAILED_SERVICE_CONTROLLER_CONNECT: i32 = 1063;

#[cfg(target_os = "windows")]
fn is_not_started_by_service_manager(error: &windows_service::Error) -> bool {
    match error {
        windows_service::Error::Winapi(io_error) => {
            io_error.raw_os_error() == Some(ERROR_FAILED_SERVICE_CONTROLLER_CONNECT)
        }

        _ => false,
    }
}

#[cfg(not(target_os = "windows"))]
fn run_agent_entry() -> ExitCode {
    // As a launchd daemon stdout already lands in the plist's StandardOutPath,
    // so echoing is harmless; run from a terminal it is what you want to see.
    logging::init(logging::default_log_path());

    run_foreground();

    ExitCode::SUCCESS
}

/// Build a runtime and run the agent until it is asked to stop.
fn run_foreground() {
    let runtime = match tokio::runtime::Runtime::new() {
        Ok(runtime) => runtime,

        Err(error) => {
            crate::log_line!("Could not start the Tokio runtime: {}", error);

            return;
        }
    };

    runtime.block_on(run_agent(None));
}

#[cfg(target_os = "windows")]
extern "system" fn ffi_service_main(_argc: u32, _argv: *mut *mut u16) {
    // A service has no console, so stdout must never be written to: route
    // everything to the log file. Done here rather than in `main` because the
    // SCM jumps straight into the dispatcher.
    logging::init(logging::default_log_path());
    logging::set_stdout_echo(false);

    // From here on the agent is Session 0, so screen and location have to come
    // from a helper in the user's session rather than from this process.
    session::note_running_as_service();

    if let Err(error) = service_main() {
        crate::log_line!("GBOS service failed: {:?}", error);
    }
}

#[cfg(target_os = "windows")]
fn service_main() -> windows_service::Result<()> {
    use std::time::Duration;

    use windows_service::{
        service::{
            ServiceControl, ServiceControlAccept, ServiceExitCode, ServiceState, ServiceStatus,
            ServiceType,
        },
        service_control_handler::{self, ServiceControlHandlerResult},
    };

    let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);

    let event_handler = move |event| -> ServiceControlHandlerResult {
        match event {
            ServiceControl::Stop | ServiceControl::Shutdown => {
                crate::log_line!("GBOS service stop requested");

                let _ = shutdown_tx.send(true);

                ServiceControlHandlerResult::NoError
            }

            ServiceControl::Interrogate => ServiceControlHandlerResult::NoError,

            _ => ServiceControlHandlerResult::NotImplemented,
        }
    };

    let status_handle = service_control_handler::register("GBOSAgent", event_handler)?;

    status_handle.set_service_status(ServiceStatus {
        service_type: ServiceType::OWN_PROCESS,
        current_state: ServiceState::Running,
        controls_accepted: ServiceControlAccept::STOP | ServiceControlAccept::SHUTDOWN,
        exit_code: ServiceExitCode::Win32(0),
        checkpoint: 0,
        wait_hint: Duration::default(),
        process_id: None,
    })?;

    let runtime = tokio::runtime::Runtime::new().map_err(|error| {
        windows_service::Error::Winapi(std::io::Error::other(format!(
            "Failed to create Tokio runtime: {}",
            error
        )))
    })?;

    runtime.block_on(run_agent(Some(shutdown_rx)));

    status_handle.set_service_status(ServiceStatus {
        service_type: ServiceType::OWN_PROCESS,
        current_state: ServiceState::Stopped,
        controls_accepted: ServiceControlAccept::empty(),
        exit_code: ServiceExitCode::Win32(0),
        checkpoint: 0,
        wait_hint: Duration::default(),
        process_id: None,
    })?;

    Ok(())
}
