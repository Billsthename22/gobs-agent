#[cfg(target_os = "macos")]
mod location;

#[cfg(target_os = "windows")]
mod location_windows;

#[cfg(target_os = "windows")]
use location_windows as location;

mod video;

use std::env;
use std::time::Duration;

fn gbos_api_url() -> String {
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

    let hostname = sysinfo::System::host_name()
        .unwrap_or_else(|| "GBOS-UNKNOWN".to_string());

    let operating_system = sysinfo::System::long_os_version()
        .unwrap_or_else(|| std::env::consts::OS.to_string());

    let payload = serde_json::json!({
        "device_name": hostname,
        "hostname": hostname,
        "operating_system": operating_system,
        "ip_address": null
    });

    println!("Registering agent with GBOS backend...");
    println!("Hostname: {}", hostname);
    println!("OS: {}", operating_system);

    let response = client
        .post(format!("{}/agents/register", gbos_api_url()))
        .json(&payload)
        .send()
        .await?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();

        return Err(format!(
            "Agent registration failed: {} - {}",
            status, body
        )
        .into());
    }

    let registration: AgentRegisterResponse = response.json().await?;

    println!("Agent registered successfully!");
    println!("Device ID: {}", registration.device_id);
    println!("Device name: {}", registration.device_name);
    println!("Hostname: {}", registration.hostname);
    println!("Status: {}", registration.status);
    println!("WebSocket: {}", registration.websocket_url);

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
        stdin
            .write_all(b"show State:/Users/ConsoleUser\n")
            .ok()?;
    }

    let output = child.wait_with_output().ok()?;
    let output = String::from_utf8_lossy(&output.stdout);

    for line in output.lines() {
        let trimmed = line.trim();

        if trimmed.starts_with("Name :") {
            let username = trimmed
                .strip_prefix("Name :")?
                .trim();

            if !username.is_empty() && username != "loginwindow" {
                return Some(username.to_string());
            }
        }
    }

    None
}

#[cfg(target_os = "windows")]
fn detect_console_user() -> Option<String> {
    std::env::var("USERNAME")
        .ok()
        .filter(|username| !username.trim().is_empty())
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
fn detect_console_user() -> Option<String> {
    std::env::var("USER")
        .or_else(|_| std::env::var("USERNAME"))
        .ok()
        .filter(|username| !username.trim().is_empty())
}
#[tokio::main]
async fn main() {
    let device_id = match register_agent().await {
        Ok(id) => id,
        Err(error) => {
            println!("Agent registration failed!");
            println!("Error: {}", error);
            return;
        }
    };

    let initial_console_user = detect_console_user();

    println!("Current user: {}", current_username());
    println!("Console user: {:?}", initial_console_user);

    let mut last_console_user = initial_console_user;

    let url = format!("{}/ws/devices/{}", gbos_ws_url(), device_id);

    println!("GBOS Device Agent");
    println!("Connecting to {}", url);

    let (mut websocket, _) = match connect_async(&url).await {
        Ok(connection) => {
            println!("Connected to GBOS backend ✅");
            connection
        }
        Err(error) => {
            println!("Connection failed ❌");
            println!("Error: {}", error);
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
            println!("Login event failed ❌");
            println!("Error: {}", error);
            return;
        }

        println!("Login event sent 🔐 for {}", username);
    } else {
        println!("No active console user detected.");
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
    let (screen_tx, mut screen_rx) =
        mpsc::unbounded_channel::<video::StreamPacket>();

    let mut screen_stream: Option<video::VideoStream> = None;

    // GPS collection is disabled by default.
    // The backend explicitly enables it through an authorized
    // GPS_COLLECTION command.
    let location_manager = location::LocationManager::new();
    let mut gps_enabled = false;
    let mut gps_timer = interval(Duration::from_secs(10));

    loop {
        tokio::select! {
            // HEARTBEAT
            _ = heartbeat.tick() => {
                let message = json!({
                    "type": "heartbeat"
                });

                if let Err(error) = websocket
                    .send(Message::Text(message.to_string().into()))
                    .await
                {
                    println!("Heartbeat failed ❌");
                    println!("Error: {}", error);
                    break;
                }

                println!("Heartbeat sent 💓");
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

                println!("CPU usage: {:.1}%", cpu_usage);
                println!("Memory usage: {:.1}%", memory_usage);
                println!("Storage usage: {:.1}%", storage_usage);
                println!("Network speed: {:.2} Mbps", network_speed_mbps);

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
                    println!("Telemetry failed ❌");
                    println!("Error: {}", error);
                    break;
                }

                println!("CPU + memory + storage + network telemetry sent 📊");
            }

            // GPS LOCATION
            // Only runs while the backend has GPS collection enabled; the
            // delegate thread reports nothing until it has a fix.
            _ = gps_timer.tick() => {
                if gps_enabled {
                    match location_manager.current_location() {
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
                                println!("Location send failed ❌");
                                println!("Error: {}", error);
                                break;
                            }

                            println!(
                                "Location sent 📍 ({:.5}, {:.5})",
                                latitude,
                                longitude
                            );
                        }

                        None => {
                            println!(
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
                    println!("Screen stream send failed ❌");
                    println!("Error: {}", error);
                    break;
                }

                if is_config {
                    println!(
                        "Screen stream config forwarded ⏩ ({:.1} KB)",
                        binary_len as f64 / 1024.0
                    );
                }
            }

            // AUTOMATIC SESSION MONITORING
            _ = session_timer.tick() => {
                let current_console_user = detect_console_user();

                if current_console_user != last_console_user {
                    println!(
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
                            println!("Logout event failed ❌");
                            println!("Error: {}", error);
                            break;
                        }

                        println!(
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
                            println!("Login event failed ❌");
                            println!("Error: {}", error);
                            break;
                        }

                        println!(
                            "Login event sent 🔐 for {}",
                            new_user
                        );
                    }

                    last_console_user = current_console_user;
                }
            }

            // SHUTDOWN SIGNAL (Ctrl-C / SIGTERM)
            _ = signal::ctrl_c() => {
                println!("Shutdown signal received — disconnecting gracefully 🛑");

                // Send a proper close handshake so the backend marks the
                // device offline immediately instead of waiting out its
                // websocket ping cycle.
                if let Err(error) = websocket
                    .send(Message::Close(None))
                    .await
                {
                    println!("Close frame failed ❌");
                    println!("Error: {}", error);
                }

                break;
            }

            // BACKEND MESSAGES
            message = websocket.next() => {
                match message {
                    Some(Ok(Message::Text(text))) => {
                        println!("Backend: {}", text);

                        match serde_json::from_str::<serde_json::Value>(&text) {
                            Ok(message) => {
                                match message.get("type").and_then(|value| value.as_str()) {
                                    Some("START_SCREEN_STREAM") => {
                                        if screen_stream.is_none() {
                                            println!(
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
                                                    println!(
                                                        "Screen stream start failed ❌"
                                                    );
                                                    println!(
                                                        "Error: {}",
                                                        error
                                                    );
                                                }
                                            }
                                        }
                                    }

                                    Some("STOP_SCREEN_STREAM") => {
                                        if let Some(mut handle) =
                                            screen_stream.take()
                                        {
                                            handle.stop();

                                            println!(
                                                "Screen streaming DISABLED 🛑"
                                            );
                                        }
                                    }

                                    Some("GPS_COLLECTION") => {
                                        let enabled = message
                                            .get("enabled")
                                            .and_then(|value| value.as_bool())
                                            .unwrap_or(false);

                                        if enabled && !gps_enabled {
                                            gps_enabled = true;
                                            location_manager.start();

                                            println!(
                                                "GPS collection ENABLED 📍"
                                            );
                                        } else if !enabled && gps_enabled {
                                            gps_enabled = false;
                                            location_manager.stop();

                                            println!(
                                                "GPS collection DISABLED 🛑"
                                            );
                                        }
                                    }

                                    _ => {}
                                }
                            }

                            Err(error) => {
                                println!(
                                    "Backend message is not JSON: {}",
                                    error
                                );
                            }
                        }
                    }

                    Some(Ok(Message::Close(_))) => {
                        println!("Backend closed connection 🔌");
                        break;
                    }

                    Some(Err(error)) => {
                        println!("WebSocket error ❌");
                        println!("Error: {}", error);
                        break;
                    }

                    None => {
                        println!("Connection ended 🔌");
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
    location_manager.stop();
}