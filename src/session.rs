//! Windows per-session helper: screen capture and location.
//!
//! # Why this exists
//!
//! A Windows service runs in **Session 0**, which has been isolated from the
//! interactive desktop since Vista. From Session 0:
//!
//! * GDI screen capture returns Session 0's own desktop, not the user's, so the
//!   stream is blank rather than merely failing;
//! * WinRT `Geolocator` has no user context to grant the per-user location
//!   permission against, so it fails with access denied.
//!
//! Both therefore run in a helper process that the service launches *into the
//! active console session*, where a real desktop and a real user token exist.
//!
//! # Why the helper does not just open its own WebSocket
//!
//! The backend keeps exactly one connection per device:
//! `DeviceConnectionManager.connections` is a `Dict[int, WebSocket]` keyed by
//! device id, and `connect()` overwrites the entry. A second connection for the
//! same device would evict the service's socket and flap the device
//! offline/online in the frontend. So the **service stays the only thing that
//! talks to the backend**, and the helper talks only to the service.
//!
//! # Protocol
//!
//! Over a named pipe, one direction each so there is never any framing
//! ambiguity:
//!
//! * helper → service: length-prefixed binary frames
//!   `[u8 kind][u32 big-endian length][payload]`
//!   * kind 1 — video, payload is the `video.rs` wire format verbatim, so the
//!     service can forward it to the backend without re-encoding and the
//!     frontend's existing decoder is unchanged;
//!   * kind 2 — GPS, payload is UTF-8 JSON `{"latitude":..,"longitude":..}`;
//!   * kind 3 — log text, so helper output lands in the same log file.
//! * service → helper: newline-delimited JSON control messages, forwarded
//!   verbatim from the backend (`START_SCREEN_STREAM`, `STOP_SCREEN_STREAM`,
//!   `GPS_COLLECTION`).

use std::io;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tokio::sync::mpsc;
use windows::core::{PCWSTR, PWSTR};
use windows::Win32::Foundation::{CloseHandle, HANDLE, INVALID_HANDLE_VALUE, WAIT_OBJECT_0};
use windows::Win32::Security::{
    DuplicateTokenEx, InitializeSecurityDescriptor, PSECURITY_DESCRIPTOR,
    SECURITY_ATTRIBUTES, SECURITY_DESCRIPTOR, SecurityImpersonation, SetSecurityDescriptorDacl,
    TOKEN_ALL_ACCESS, TokenPrimary,
};
use windows::Win32::Storage::FileSystem::{ReadFile, WriteFile};
use windows::Win32::System::Environment::{CreateEnvironmentBlock, DestroyEnvironmentBlock};
use windows::Win32::System::Pipes::{
    ConnectNamedPipe, CreateNamedPipeW, GetNamedPipeClientProcessId,
    GetNamedPipeClientSessionId, PIPE_READMODE_BYTE, PIPE_TYPE_BYTE, PIPE_WAIT,
};
use windows::Win32::System::RemoteDesktop::{WTSGetActiveConsoleSessionId, WTSQueryUserToken};
use windows::Win32::System::Threading::{
    CREATE_NO_WINDOW, CREATE_UNICODE_ENVIRONMENT, CreateProcessAsUserW, CreateProcessWithTokenW,
    LOGON_WITH_PROFILE, PROCESS_INFORMATION, STARTUPINFOW, TerminateProcess,
    WaitForSingleObject,
};

use crate::logging;
use crate::video;

/// Revision of the security descriptor layout `InitializeSecurityDescriptor`
/// expects. win32metadata does not export a name for it.
const SECURITY_DESCRIPTOR_REVISION: u32 = 1;

// ---------------------------------------------------------------------------
// Frame protocol
// ---------------------------------------------------------------------------

/// Video frame in the `video.rs` wire format.
const FRAME_VIDEO: u8 = 1;

/// GPS reading as UTF-8 JSON.
const FRAME_GPS: u8 = 2;

/// Log line as UTF-8 text.
const FRAME_LOG: u8 = 3;

/// `WTSGetActiveConsoleSessionId` returns this when no session is attached
/// (for example, at boot before anyone logs in, or on a locked headless host).
const NO_ACTIVE_SESSION: u32 = 0xFFFF_FFFF;

// ---------------------------------------------------------------------------
// Pipe endpoint
// ---------------------------------------------------------------------------

/// A connected named-pipe endpoint.
///
/// Shared between the reader thread and the writer, which is safe because the
/// protocol gives each direction a single reader and a single writer, and a
/// pipe handle supports concurrent reads and writes.
struct Pipe {
    handle: HANDLE,
}

// SAFETY: a pipe HANDLE is an opaque kernel handle. Reads and writes on a
// byte-mode pipe are independent, and this module only ever reads from one
// thread and writes from one thread.
unsafe impl Send for Pipe {}
unsafe impl Sync for Pipe {}

impl Drop for Pipe {
    fn drop(&mut self) {
        if self.handle != INVALID_HANDLE_VALUE && !self.handle.is_invalid() {
            // SAFETY: the handle came from CreateNamedPipeW or CreateFileW and
            // is closed exactly once, here.
            unsafe {
                let _ = CloseHandle(self.handle);
            }
        }
    }
}

impl Pipe {
    /// Read exactly `buffer.len()` bytes, or fail at end of pipe.
    fn read_exact(&self, buffer: &mut [u8]) -> io::Result<()> {
        let mut filled = 0usize;

        while filled < buffer.len() {
            let mut read = 0u32;

            // SAFETY: `buffer[filled..]` is valid for the duration of the call
            // and `read` is a live out-parameter.
            unsafe {
                ReadFile(
                    self.handle,
                    Some(&mut buffer[filled..]),
                    Some(&mut read),
                    None,
                )
            }
            .map_err(|error| io::Error::other(format!("ReadFile failed: {}", error)))?;

            if read == 0 {
                return Err(io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    "pipe closed",
                ));
            }

            filled += read as usize;
        }

        Ok(())
    }

    /// Write one protocol frame.
    fn write_frame(&self, kind: u8, payload: &[u8]) -> io::Result<()> {
        let mut framed = Vec::with_capacity(5 + payload.len());

        framed.push(kind);
        framed.extend_from_slice(&(payload.len() as u32).to_be_bytes());
        framed.extend_from_slice(payload);

        self.write_all(&framed)
    }

    /// Write one newline-terminated control message.
    ///
    /// Control messages are not framed: the helper reads them with
    /// [`Pipe::read_control_line`], and keeping the two directions in different
    /// formats means neither side can misread the other's bytes as its own.
    fn write_line(&self, message: &str) -> io::Result<()> {
        let mut line = Vec::with_capacity(message.len() + 1);

        line.extend_from_slice(message.as_bytes());
        line.push(b'\n');

        self.write_all(&line)
    }

    /// Write every byte or fail.
    fn write_all(&self, bytes: &[u8]) -> io::Result<()> {
        let mut written = 0usize;

        while written < bytes.len() {
            let mut wrote = 0u32;

            // SAFETY: `bytes[written..]` is valid for the duration of the call
            // and `wrote` is a live out-parameter.
            unsafe {
                WriteFile(
                    self.handle,
                    Some(&bytes[written..]),
                    Some(&mut wrote),
                    None,
                )
            }
            .map_err(|error| io::Error::other(format!("WriteFile failed: {}", error)))?;

            if wrote == 0 {
                return Err(io::Error::new(io::ErrorKind::WriteZero, "pipe closed"));
            }

            written += wrote as usize;
        }

        Ok(())
    }

    /// Read one protocol frame: `kind`, then a big-endian length, then payload.
    fn read_frame(&self) -> io::Result<(u8, Vec<u8>)> {
        let mut header = [0u8; 5];
        self.read_exact(&mut header)?;

        let length = u32::from_be_bytes([header[1], header[2], header[3], header[4]]) as usize;

        // Bound the allocation: a corrupt or hostile length must not be able to
        // make the service allocate arbitrary memory.
        const MAX_FRAME_BYTES: usize = 8 * 1024 * 1024;

        if length > MAX_FRAME_BYTES {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("frame length {} exceeds the {} byte limit", length, MAX_FRAME_BYTES),
            ));
        }

        let mut payload = vec![0u8; length];
        self.read_exact(&mut payload)?;

        Ok((header[0], payload))
    }

    /// Read one newline-terminated control message.
    ///
    /// Byte-at-a-time on purpose: control messages are short, rare, and this
    /// keeps the framing obviously correct without a buffer to get wrong.
    fn read_control_line(&self) -> io::Result<String> {
        let mut line = Vec::new();
        let mut byte = [0u8; 1];

        loop {
            self.read_exact(&mut byte)?;

            if byte[0] == b'\n' {
                return Ok(String::from_utf8_lossy(&line).into_owned());
            }

            // Tolerate CRLF without treating the CR as content.
            if byte[0] != b'\r' {
                line.push(byte[0]);
            }

            if line.len() > 64 * 1024 {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "control message exceeded 64 KiB",
                ));
            }
        }
    }

    /// Connect to the service's pipe as the helper.
    fn connect(pipe_name: &str) -> io::Result<Self> {
        use windows::Win32::Storage::FileSystem::{
            CreateFileW, FILE_ATTRIBUTE_NORMAL, FILE_GENERIC_READ, FILE_GENERIC_WRITE,
            FILE_SHARE_NONE, OPEN_EXISTING,
        };

        let wide = to_wide(pipe_name);

        // SAFETY: `wide` is NUL-terminated and lives until the call returns.
        let handle = unsafe {
            CreateFileW(
                windows::core::PCWSTR(wide.as_ptr()),
                (FILE_GENERIC_READ | FILE_GENERIC_WRITE).0,
                FILE_SHARE_NONE,
                None,
                OPEN_EXISTING,
                FILE_ATTRIBUTE_NORMAL,
                None,
            )
        }
        .map_err(|error| io::Error::other(format!("could not open {}: {}", pipe_name, error)))?;

        Ok(Self { handle })
    }

    /// Create the service's pipe server.
    ///
    /// Returns immediately, without waiting for the helper: the pipe has to
    /// exist *before* the helper is launched, or the helper's `CreateFileW`
    /// would fail. Call [`Pipe::accept_connection`] to wait for it.
    fn create_server(pipe_name: &str) -> io::Result<Self> {
        let wide = to_wide(pipe_name);

        // A pipe created without a security descriptor gets the creating
        // process's *default* DACL. For the service that is LocalSystem plus
        // Administrators — which would deny the helper, running as an ordinary
        // logged-in user, any access at all. So the DACL has to be set
        // explicitly, and here it is set to nothing (a NULL DACL), granting
        // everyone access.
        //
        // That is not the protection boundary. The boundary is the session check
        // in `SessionBridge::start_helper`, which rejects any client that is not
        // in the session the helper was launched into. Pipe names are
        // enumerable, so keeping the name secret would not have been a boundary
        // anyway; the alternative here would be to build an ACL around the
        // user's SID, which is a great deal more unsafe code for the same
        // outcome.
        let mut descriptor = SECURITY_DESCRIPTOR::default();

        // `PSECURITY_DESCRIPTOR` is a newtype over a raw pointer rather than an
        // alias, so it has to be constructed rather than cast to.
        let descriptor_pointer = PSECURITY_DESCRIPTOR(
            &mut descriptor as *mut SECURITY_DESCRIPTOR as *mut core::ffi::c_void,
        );

        // SAFETY: `descriptor` outlives both calls, and a null DACL is the
        // documented way to express "no protection".
        unsafe {
            InitializeSecurityDescriptor(descriptor_pointer, SECURITY_DESCRIPTOR_REVISION)
                .map_err(|error| {
                    io::Error::other(format!(
                        "could not initialize the pipe's descriptor: {}",
                        error
                    ))
                })?;

            SetSecurityDescriptorDacl(descriptor_pointer, true, None, false).map_err(|error| {
                io::Error::other(format!("could not set the pipe's DACL: {}", error))
            })?;
        }

        let attributes = SECURITY_ATTRIBUTES {
            nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
            lpSecurityDescriptor: &mut descriptor as *mut SECURITY_DESCRIPTOR
                as *mut core::ffi::c_void,
            bInheritHandle: false.into(),
        };

        // SAFETY: `wide` is NUL-terminated and `attributes` — along with the
        // descriptor it points at — outlives the call.
        let handle = unsafe {
            CreateNamedPipeW(
                windows::core::PCWSTR(wide.as_ptr()),
                windows::Win32::Storage::FileSystem::PIPE_ACCESS_DUPLEX,
                PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_WAIT,
                1,
                64 * 1024,
                64 * 1024,
                0,
                Some(&attributes),
            )
        };

        if handle == INVALID_HANDLE_VALUE || handle.is_invalid() {
            return Err(io::Error::other(format!(
                "could not create pipe {}",
                pipe_name
            )));
        }

        Ok(Self { handle })
    }

    /// Wait for the helper to connect. Blocking — call on a dedicated thread.
    fn accept_connection(&self) -> io::Result<()> {
        // SAFETY: the handle is a live pipe handle with no pending operations.
        unsafe { ConnectNamedPipe(self.handle, None) }
            .map_err(|error| io::Error::other(format!("ConnectNamedPipe failed: {}", error)))
    }
}

/// NUL-terminated UTF-16 for the Win32 W APIs.
fn to_wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}

// ---------------------------------------------------------------------------
// Helper entry point
// ---------------------------------------------------------------------------

/// Run as the per-session helper. Invoked by the service, never directly.
pub fn run_helper() -> std::process::ExitCode {
    let Some(pipe_name) = argument_value("--pipe") else {
        eprintln!("--session-helper requires --pipe <name>");
        return std::process::ExitCode::FAILURE;
    };

    logging::init(logging::default_log_path());

    let runtime = match tokio::runtime::Runtime::new() {
        Ok(runtime) => runtime,
        Err(error) => {
            crate::log_line!("Helper could not start a Tokio runtime: {}", error);
            return std::process::ExitCode::FAILURE;
        }
    };

    runtime.block_on(helper_main(pipe_name))
}

async fn helper_main(pipe_name: String) -> std::process::ExitCode {
    crate::log_line!(
        "Session helper starting for user {} (pipe {})",
        std::env::var("USERNAME").unwrap_or_else(|_| "unknown".to_string()),
        pipe_name
    );

    let pipe = match Pipe::connect(&pipe_name) {
        Ok(pipe) => Arc::new(pipe),
        Err(error) => {
            crate::log_line!("Session helper could not connect to the service: {}", error);
            return std::process::ExitCode::FAILURE;
        }
    };

    // Control messages arrive on a reader thread, because reading a blocking
    // pipe must not stall the frame-forwarding loop below.
    let (control_tx, mut control_rx) = mpsc::unbounded_channel::<String>();

    {
        let pipe = Arc::clone(&pipe);

        std::thread::spawn(move || {
            loop {
                match pipe.read_control_line() {
                    Ok(line) => {
                        if control_tx.send(line).is_err() {
                            break;
                        }
                    }

                    // The service closing the pipe is the normal shutdown
                    // path: dropping the sender makes the select arm below see
                    // the channel close and exit.
                    Err(_) => break,
                }
            }
        });
    }

    let (screen_tx, mut screen_rx) = mpsc::unbounded_channel::<video::StreamPacket>();

    let mut screen_stream: Option<video::VideoStream> = None;
    let mut gps_manager: Option<crate::location_windows::LocationManager> = None;
    let mut gps_timer = tokio::time::interval(Duration::from_secs(10));

    loop {
        tokio::select! {
            // CONTROL FROM THE SERVICE
            message = control_rx.recv() => {
                let Some(message) = message else {
                    crate::log_line!("Service disconnected; session helper exiting");
                    break;
                };

                handle_control(
                    &message,
                    &pipe,
                    &screen_tx,
                    &mut screen_stream,
                    &mut gps_manager,
                );
            }

            // SCREEN FRAMES → SERVICE
            Some(packet) = screen_rx.recv() => {
                let kind = packet.kind;
                let binary = packet.into_binary();

                let is_config = kind == video::STREAM_KIND_CONFIG;

                if let Err(error) = pipe.write_frame(FRAME_VIDEO, &binary) {
                    crate::log_line!("Could not forward a screen frame to the service: {}", error);
                    break;
                }

                if is_config {
                    crate::log_line!(
                        "Session helper captured stream config ⏩ ({:.1} KB)",
                        binary.len() as f64 / 1024.0
                    );
                }
            }

            // GPS → SERVICE
            _ = gps_timer.tick() => {
                let Some(manager) = gps_manager.as_ref() else {
                    continue;
                };

                let Some((latitude, longitude)) = manager.current_location() else {
                    continue;
                };

                let payload = format!(
                    "{{\"latitude\":{},\"longitude\":{}}}",
                    latitude, longitude
                );

                if let Err(error) = pipe.write_frame(FRAME_GPS, payload.as_bytes()) {
                    crate::log_line!("Could not forward a location fix to the service: {}", error);
                    break;
                }
            }
        }
    }

    if let Some(mut handle) = screen_stream.take() {
        handle.stop();
    }

    std::process::ExitCode::SUCCESS
}

fn handle_control(
    message: &str,
    pipe: &Arc<Pipe>,
    screen_tx: &mpsc::UnboundedSender<video::StreamPacket>,
    screen_stream: &mut Option<video::VideoStream>,
    gps_manager: &mut Option<crate::location_windows::LocationManager>,
) {
    let Ok(message) = serde_json::from_str::<serde_json::Value>(message) else {
        return;
    };

    match message.get("type").and_then(|value| value.as_str()) {
        Some("START_SCREEN_STREAM") => {
            if screen_stream.is_none() {
                match video::VideoStream::start(screen_tx.clone()) {
                    Ok(handle) => {
                        *screen_stream = Some(handle);
                        crate::log_line!("Session helper screen streaming ENABLED 🖥️");
                    }

                    Err(error) => {
                        let _ = pipe.write_frame(
                            FRAME_LOG,
                            format!("Screen stream start failed: {}", error).as_bytes(),
                        );
                    }
                }
            }
        }

        Some("STOP_SCREEN_STREAM") => {
            if let Some(mut handle) = screen_stream.take() {
                handle.stop();
                crate::log_line!("Session helper screen streaming DISABLED 🛑");
            }
        }

        Some("GPS_COLLECTION") => {
            let enabled = message
                .get("enabled")
                .and_then(|value| value.as_bool())
                .unwrap_or(false);

            match (enabled, gps_manager.as_ref()) {
                (true, None) => {
                    let manager = crate::location_windows::LocationManager::new();
                    manager.start();

                    *gps_manager = Some(manager);

                    crate::log_line!("Session helper GPS collection ENABLED 📍");
                }

                (false, Some(manager)) => {
                    manager.stop();

                    *gps_manager = None;

                    crate::log_line!("Session helper GPS collection DISABLED 🛑");
                }

                _ => {}
            }
        }

        _ => {}
    }
}

/// Value of `--flag value` in the process arguments.
fn argument_value(flag: &str) -> Option<String> {
    let mut arguments = std::env::args().skip(1);

    while let Some(argument) = arguments.next() {
        if argument == flag {
            return arguments.next();
        }
    }

    None
}

// ---------------------------------------------------------------------------
// Service side
// ---------------------------------------------------------------------------

/// Set once the SCM has started this process as a service.
///
/// The same binary also runs in the foreground when launched from a console —
/// there it sits in the user's own session and captures directly, so it must not
/// delegate.
static RUNNING_AS_SERVICE: AtomicBool = AtomicBool::new(false);

/// Record that the SCM started this process as a service.
pub fn note_running_as_service() {
    RUNNING_AS_SERVICE.store(true, Ordering::Relaxed);
}

/// What the backend currently wants the helper to be doing.
///
/// Held separately from the connection because a command can arrive before the
/// helper has attached, or while no user is logged in. Rather than dropping it,
/// the service remembers it and replays it to each helper as it connects — which
/// is also what makes a mid-session helper restart pick up where the last one
/// left off.
#[derive(Default)]
struct Wanted {
    screen: AtomicBool,
    gps: AtomicBool,
}

impl Wanted {
    /// The control messages that bring a freshly connected helper in line.
    fn control_lines(&self) -> [String; 2] {
        let screen = if self.screen.load(Ordering::Relaxed) {
            "START_SCREEN_STREAM"
        } else {
            "STOP_SCREEN_STREAM"
        };

        [
            format!(r#"{{"type":"{}"}}"#, screen),
            format!(
                r#"{{"type":"GPS_COLLECTION","enabled":{}}}"#,
                self.gps.load(Ordering::Relaxed)
            ),
        ]
    }
}

/// An owned kernel handle, closed on drop.
struct Owned {
    handle: HANDLE,
}

impl Drop for Owned {
    fn drop(&mut self) {
        if self.handle != INVALID_HANDLE_VALUE && !self.handle.is_invalid() {
            // SAFETY: the handle was produced by CreateProcessAsUserW or
            // CreateProcessWithTokenW and is closed exactly once, here.
            unsafe {
                let _ = CloseHandle(self.handle);
            }
        }
    }
}

/// A running helper and the pipe it is connected to.
struct Link {
    /// The service's end of the pipe.
    pipe: Arc<Pipe>,

    /// The helper process, kept so a wedged helper can be terminated and so its
    /// exit can be noticed.
    process: Owned,

    /// Set by the reader thread once the helper has connected. Control messages
    /// are only written when this is true: writing to a pipe nobody has opened
    /// would block until a reader appears.
    connected: Arc<AtomicBool>,

    /// Cleared by the reader thread when the pipe breaks.
    alive: Arc<AtomicBool>,
}

/// The service's view of the per-session helper.
///
/// Everything the helper produces is fed into the same machinery the in-process
/// capture path uses, so the event loop forwards it to the backend without
/// knowing which of the two produced it:
///
/// * video frames are pushed into the screen channel as the [`video::StreamPacket`]
///   they already are, and go out as WebSocket binary messages verbatim;
/// * location fixes are held until the loop's GPS tick collects them.
pub struct SessionBridge {
    /// The channel the in-process capture thread would push into.
    screen_tx: mpsc::UnboundedSender<video::StreamPacket>,

    /// Whether this process must delegate at all — see [`RUNNING_AS_SERVICE`].
    delegating: bool,

    /// The session a helper was launched into, if any.
    session_id: Option<u32>,

    /// What the backend wants, kept so a new helper can be told.
    wanted: Arc<Wanted>,

    /// The connected helper, if one is running.
    link: Option<Link>,

    /// The most recent fix reported by the helper, waiting to be sent.
    fix: Arc<Mutex<Option<(f64, f64)>>>,
}

impl SessionBridge {
    /// Build a bridge for this process.
    ///
    /// Cheap and side-effect free: nothing is launched until [`Self::tick`].
    pub fn new(screen_tx: mpsc::UnboundedSender<video::StreamPacket>) -> Self {
        Self {
            screen_tx,
            delegating: RUNNING_AS_SERVICE.load(Ordering::Relaxed),
            session_id: None,
            wanted: Arc::new(Wanted::default()),
            link: None,
            fix: Arc::new(Mutex::new(None)),
        }
    }

    /// Whether screen and location have to be captured by a helper rather than
    /// by this process.
    ///
    /// True only for the SCM service. It is deliberately about *this process's
    /// situation*, not about whether a helper is currently connected, because it
    /// also decides whether building an in-process location manager is worth
    /// anything — inside Session 0 it is not.
    pub fn delegates(&self) -> bool {
        self.delegating
    }

    /// Start, stop, or restart the helper to match the active console session.
    ///
    /// Called on the loop's session tick, which is the same cadence the
    /// in-process build uses to notice logins — reusing it avoids a second
    /// notification mechanism for the same event.
    pub fn tick(&mut self) {
        if !self.delegating {
            return;
        }

        if self.link.is_some() && !self.link_is_live() {
            crate::log_line!("Session helper is gone; it will be restarted if a user is logged in");

            self.link = None;
        }

        // SAFETY: no arguments and no invariants beyond being called from
        // anywhere; it reads a value the kernel maintains.
        let current = unsafe { WTSGetActiveConsoleSessionId() };

        let no_session = current == NO_ACTIVE_SESSION;

        match (no_session, self.session_id) {
            // Nobody is logged in and nobody was: nothing to do, and this is the
            // common case at boot.
            (true, None) => {}

            // The console session went away — a logout, or a switch to the lock
            // screen.
            (true, Some(_)) => {
                self.stop_helper("no console session is active");

                self.session_id = None;
            }

            // A user logged in.
            (false, None) => {
                self.session_id = Some(current);
                self.start_helper(current);
            }

            // Fast user switching: the helper belongs to a session that is no
            // longer on the console.
            (false, Some(previous)) if previous != current => {
                self.stop_helper("the console session changed");

                self.session_id = Some(current);
                self.start_helper(current);
            }

            // Same session, so a helper should be running. Restart it if it is
            // not: it can be killed by a crash, by Task Manager, or by the
            // session's own teardown, and the service is the only thing that
            // will notice.
            (false, Some(_)) => {
                if self.link.is_none() {
                    self.start_helper(current);
                }
            }
        }
    }

    /// Tell the helper whether to stream the screen.
    pub fn set_screen(&mut self, enabled: bool) {
        self.wanted.screen.store(enabled, Ordering::Relaxed);

        let message = if enabled {
            r#"{"type":"START_SCREEN_STREAM"}"#
        } else {
            r#"{"type":"STOP_SCREEN_STREAM"}"#
        };

        self.send_control(message);
    }

    /// Tell the helper whether to collect location.
    pub fn set_gps(&mut self, enabled: bool) {
        self.wanted.gps.store(enabled, Ordering::Relaxed);

        self.send_control(&format!(
            r#"{{"type":"GPS_COLLECTION","enabled":{}}}"#,
            enabled
        ));
    }

    /// Take the most recent fix the helper reported, if there is a new one.
    ///
    /// Taking rather than peeking keeps the loop's "enabled but no fix yet"
    /// diagnostic meaningful: it appears once per fix rather than on every tick.
    pub fn take_fix(&self) -> Option<(f64, f64)> {
        self.fix.lock().ok().and_then(|mut slot| slot.take())
    }

    /// Shut the helper down. Called on the way out so no helper outlives the
    /// service's connection to the backend.
    pub fn stop(&mut self) {
        self.stop_helper("the agent is shutting down");
    }

    /// Whether the helper is connected and still running.
    fn link_is_live(&self) -> bool {
        let Some(link) = &self.link else {
            return false;
        };

        if !link.alive.load(Ordering::Relaxed) {
            return false;
        }

        // A helper that is still running has not signalled its handle, and a
        // zero timeout is how you ask without blocking.
        //
        // SAFETY: `process.handle` is a live process handle owned by `link`.
        unsafe { WaitForSingleObject(link.process.handle, 0) != WAIT_OBJECT_0 }
    }

    /// Write one control message to the connected helper, if there is one.
    fn send_control(&self, message: &str) {
        let Some(link) = &self.link else {
            // No helper yet. `wanted` already records the intent, and the
            // reader thread replays it when one connects.
            return;
        };

        if !link.connected.load(Ordering::Relaxed) {
            return;
        }

        // One `WriteFile` of a short message, and the pipe's buffer was created
        // at 64 KiB, so the write lands whole and two control messages cannot
        // interleave.
        if let Err(error) = link.pipe.write_line(message) {
            crate::log_line!("Could not send a command to the session helper: {}", error);
        }
    }

    /// Create the pipe, launch the helper into `session_id`, and start reading.
    fn start_helper(&mut self, session_id: u32) {
        // The name carries a nonce so that a stale helper from an earlier launch
        // — one the service has already given up on — cannot attach to the new
        // pipe and be mistaken for the current one.
        let pipe_name = format!(r"\\.\pipe\gbos-agent-session-{:016x}", nonce());

        let server = match Pipe::create_server(&pipe_name) {
            Ok(server) => Arc::new(server),

            Err(error) => {
                crate::log_line!("Could not create the session helper's pipe: {}", error);

                return;
            }
        };

        let process = match launch_helper(session_id, &pipe_name) {
            Ok(process) => process,

            Err(error) => {
                crate::log_line!(
                    "Could not start a session helper in session {}: {}",
                    session_id,
                    error
                );

                return;
            }
        };

        let connected = Arc::new(AtomicBool::new(false));
        let alive = Arc::new(AtomicBool::new(true));

        {
            let pipe = Arc::clone(&server);
            let connected = Arc::clone(&connected);
            let alive = Arc::clone(&alive);

            let screen_tx = self.screen_tx.clone();
            let wanted = Arc::clone(&self.wanted);
            let fix = Arc::clone(&self.fix);

            std::thread::spawn(move || {
                // Blocking, so it cannot happen on the async loop.
                if let Err(error) = pipe.accept_connection() {
                    crate::log_line!("The session helper never connected: {}", error);

                    alive.store(false, Ordering::Relaxed);

                    return;
                }

                // `ConnectNamedPipe` succeeding is not by itself proof that the
                // right process connected — the pipe's DACL admits everyone — so
                // the client is checked against the session the helper was
                // launched into.
                if !client_is_in_session(&pipe, session_id) {
                    crate::log_line!(
                        "Refused a pipe client that is not the session {} helper",
                        session_id
                    );

                    alive.store(false, Ordering::Relaxed);

                    return;
                }

                connected.store(true, Ordering::Relaxed);

                crate::log_line!("Session helper connected (session {})", session_id);

                // Commands issued before the helper attached were stored rather
                // than sent, so replay what the backend currently wants.
                for line in wanted.control_lines() {
                    if let Err(error) = pipe.write_line(&line) {
                        crate::log_line!("Could not brief the session helper: {}", error);

                        break;
                    }
                }

                loop {
                    match pipe.read_frame() {
                        // Video. The payload is already in the format the
                        // backend expects, so it is not decoded and re-encoded —
                        // it is forwarded as the packet it is.
                        Ok((FRAME_VIDEO, payload)) => match video::StreamPacket::from_binary(&payload)
                        {
                            Some(packet) => {
                                if screen_tx.send(packet).is_err() {
                                    break;
                                }
                            }

                            None => crate::log_line!(
                                "Discarded a malformed screen frame from the session helper ({} bytes)",
                                payload.len()
                            ),
                        },

                        Ok((FRAME_GPS, payload)) => {
                            match serde_json::from_slice::<serde_json::Value>(&payload) {
                                Ok(reading) => {
                                    let latitude =
                                        reading.get("latitude").and_then(|value| value.as_f64());
                                    let longitude =
                                        reading.get("longitude").and_then(|value| value.as_f64());

                                    if let (Some(latitude), Some(longitude)) = (latitude, longitude)
                                    {
                                        if let Ok(mut slot) = fix.lock() {
                                            *slot = Some((latitude, longitude));
                                        }
                                    }
                                }

                                Err(error) => crate::log_line!(
                                    "Discarded a malformed location reading from the session helper: {}",
                                    error
                                ),
                            }
                        }

                        // The helper's own log, so both halves of the agent end
                        // up in one file.
                        Ok((FRAME_LOG, payload)) => crate::log_line!(
                            "Session helper: {}",
                            String::from_utf8_lossy(&payload).trim_end()
                        ),

                        Ok((kind, _)) => crate::log_line!(
                            "Discarded an unknown frame (kind {}) from the session helper",
                            kind
                        ),

                        // The helper exited or was killed.
                        Err(_) => break,
                    }
                }

                connected.store(false, Ordering::Relaxed);
                alive.store(false, Ordering::Relaxed);

                crate::log_line!("Session helper disconnected");
            });
        }

        self.link = Some(Link {
            pipe: server,
            process,
            connected,
            alive,
        });

        crate::log_line!("Started a session helper in session {}", session_id);
    }

    /// Drop the current link and end the helper process.
    fn stop_helper(&mut self, reason: &str) {
        let Some(link) = self.link.take() else {
            return;
        };

        crate::log_line!("Stopping the session helper ({})", reason);

        // Terminating rather than asking nicely, because the cases this runs in
        // are the ones where the helper cannot be relied on to answer: a logged
        // out session, a wedged capture thread, or the service shutting down.
        // Nothing is lost by it — the log file is written unbuffered on every
        // line, and the helper holds no other state.
        //
        // SAFETY: `process.handle` is a live process handle owned by `link`, and
        // a zero exit code is arbitrary because nothing reads it.
        unsafe {
            let _ = TerminateProcess(link.process.handle, 0);
        }

        link.connected.store(false, Ordering::Relaxed);
        link.alive.store(false, Ordering::Relaxed);
    }
}

/// Whether the process on the other end of `pipe` is in `session_id`.
fn client_is_in_session(pipe: &Pipe, session_id: u32) -> bool {
    let mut client_session = u32::MAX;

    // SAFETY: `pipe.handle` is a connected pipe handle and `client_session` is a
    // live out-parameter.
    if unsafe { GetNamedPipeClientSessionId(pipe.handle, &mut client_session) }.is_err() {
        return false;
    }

    if client_session != session_id {
        // Distinguishing the failure matters: a helper that landed in session 0
        // captures a blank desktop rather than failing, so "the ids differ" is
        // the single most useful thing to have in the log.
        crate::log_line!(
            "Pipe client is in session {} but the helper was launched into session {}",
            client_session,
            session_id
        );

        return false;
    }

    let mut client_process = 0u32;

    // SAFETY: as above.
    if unsafe { GetNamedPipeClientProcessId(pipe.handle, &mut client_process) }.is_ok() {
        crate::log_line!("Session helper is process {}", client_process);
    }

    true
}

/// Launch this same binary as a helper inside `session_id`.
///
/// Returns the process handle, or the reason it could not be started.
fn launch_helper(session_id: u32, pipe_name: &str) -> Result<Owned, String> {
    let executable = std::env::current_exe()
        .map_err(|error| format!("could not locate this executable: {}", error))?;

    let executable = executable.to_string_lossy().into_owned();

    // The helper is this binary with a flag, so there is no second artefact to
    // install, ship, or keep in step with the service.
    let mut command_line = to_wide(&format!(
        "\"{}\" --session-helper --pipe {}",
        executable, pipe_name
    ));

    let application = to_wide(&executable);
    let desktop = to_wide("winsta0\\default");

    let startup = STARTUPINFOW {
        cb: std::mem::size_of::<STARTUPINFOW>() as u32,

        // Without this the helper would be started on Session 0's desktop, where
        // it would capture a blank screen — the exact failure this whole design
        // exists to avoid.
        lpDesktop: PWSTR(desktop.as_ptr() as *mut u16),
        ..Default::default()
    };

    let mut process_info = PROCESS_INFORMATION::default();

    // The user's token. `WTSQueryUserToken` requires LocalSystem with
    // `SeTcbPrivilege`, which is why the installer must not offer a lesser
    // service account.
    let mut user_token = HANDLE::default();

    // SAFETY: `user_token` is a live out-parameter.
    unsafe { WTSQueryUserToken(session_id, &mut user_token) }
        .map_err(|error| format!("WTSQueryUserToken failed: {}", error))?;

    // Closed on every path below, including the error ones.
    let user_token = Owned { handle: user_token };

    let mut primary_token = HANDLE::default();

    // The token comes back primary already, but not with the access rights
    // `CreateProcessAsUserW` needs, so it is duplicated with a full mask.
    //
    // SAFETY: `user_token` is live and `primary_token` is a live out-parameter.
    let duplicated = unsafe {
        DuplicateTokenEx(
            user_token.handle,
            TOKEN_ALL_ACCESS,
            None,
            SecurityImpersonation,
            TokenPrimary,
            &mut primary_token,
        )
    };

    if let Err(error) = duplicated {
        return Err(format!("DuplicateTokenEx failed: {}", error));
    }

    let primary_token = Owned { handle: primary_token };

    // So the helper inherits the user's environment rather than LocalSystem's.
    let mut environment = std::ptr::null_mut();

    // SAFETY: `primary_token` is live and `environment` is a live out-parameter.
    let environment = match unsafe {
        CreateEnvironmentBlock(&mut environment, Some(primary_token.handle), false)
    } {
        Ok(()) => Some(environment),

        Err(error) => {
            // Not fatal: without a block the helper inherits the service's
            // environment, which is enough for it to run.
            crate::log_line!(
                "Could not build an environment for the session helper ({}); \
                 it will inherit the service's",
                error
            );

            None
        }
    };

    let creation_flags = CREATE_UNICODE_ENVIRONMENT | CREATE_NO_WINDOW;

    let environment_pointer = environment.map(|block| block as *const core::ffi::c_void);

    let mut attempt = |label: &str| -> Result<(), windows::core::Error> {
        // SAFETY: every pointer handed over outlives the call — the two wide
        // strings, `startup`, and `process_info` are all locals here — and the
        // token is a live primary token.
        unsafe {
            match label {
                "CreateProcessAsUserW" => CreateProcessAsUserW(
                    Some(primary_token.handle),
                    PCWSTR(application.as_ptr()),
                    Some(PWSTR(command_line.as_mut_ptr())),
                    None,
                    None,
                    false,
                    creation_flags,
                    environment_pointer,
                    PCWSTR::null(),
                    &startup,
                    &mut process_info,
                ),

                _ => CreateProcessWithTokenW(
                    primary_token.handle,
                    LOGON_WITH_PROFILE,
                    PCWSTR(application.as_ptr()),
                    Some(PWSTR(command_line.as_mut_ptr())),
                    creation_flags,
                    environment_pointer,
                    PCWSTR::null(),
                    &startup,
                    &mut process_info,
                ),
            }
        }
    };

    let started = attempt("CreateProcessAsUserW").or_else(|first| {
        // The two differ in which privileges they need — `CreateProcessAsUserW`
        // wants `SeAssignPrimaryTokenPrivilege` and `SeIncreaseQuotaPrivilege`,
        // `CreateProcessWithTokenW` wants `SeImpersonatePrivilege` — so a policy
        // that removes one can leave the other working.
        crate::log_line!(
            "CreateProcessAsUserW failed ({}); trying CreateProcessWithTokenW",
            first
        );

        attempt("CreateProcessWithTokenW").inspect_err(|second| {
            crate::log_line!("CreateProcessWithTokenW failed too: {}", second);
        })
    });

    if let Some(block) = environment {
        // SAFETY: the block came from `CreateEnvironmentBlock` and has not been
        // freed yet.
        unsafe {
            let _ = DestroyEnvironmentBlock(block as *const core::ffi::c_void);
        }
    }

    started.map_err(|error| format!("could not start the session helper: {}", error))?;

    // The helper never needs the thread handle.
    if !process_info.hThread.is_invalid() {
        // SAFETY: the handle came from the process creation above.
        unsafe {
            let _ = CloseHandle(process_info.hThread);
        }
    }

    Ok(Owned {
        handle: process_info.hProcess,
    })
}

/// The name of the logged-in user on the active console session.
///
/// A Windows service runs in Session 0 and its own `USERNAME` is `SYSTEM`, so
/// the environment cannot answer this — the session has to be asked.
pub fn active_console_user() -> Option<String> {
    use windows::Win32::System::RemoteDesktop::{
        WTSFreeMemory, WTSQuerySessionInformationW, WTSUserName,
    };

    // SAFETY: no arguments; reads a value the kernel maintains.
    let session_id = unsafe { WTSGetActiveConsoleSessionId() };

    if session_id == NO_ACTIVE_SESSION {
        return None;
    }

    let mut buffer = PWSTR::null();
    let mut length = 0u32;

    // SAFETY: the out-parameters are live, and `None` is the handle for the
    // local machine.
    unsafe {
        WTSQuerySessionInformationW(None, session_id, WTSUserName, &mut buffer, &mut length).ok()?
    };

    if buffer.is_null() {
        return None;
    }

    // SAFETY: the call above succeeded, so `buffer` points at `length` bytes of
    // UTF-16 that stay valid until `WTSFreeMemory`.
    let username = unsafe {
        let slice = std::slice::from_raw_parts(buffer.0, length as usize / 2);

        let end = slice.iter().position(|unit| *unit == 0).unwrap_or(slice.len());

        let username = String::from_utf16_lossy(&slice[..end]);

        WTSFreeMemory(buffer.0 as *mut core::ffi::c_void);

        username
    };

    let username = username.trim().to_string();

    // The logon screen reports itself as a session with an empty name.
    if username.is_empty() {
        None
    } else {
        Some(username)
    }
}

/// A value that differs between helper launches within one boot.
///
/// Not a secret — pipe names are enumerable, and the session check is what
/// actually gates the pipe — only a way to keep a superseded helper from
/// attaching to its successor's pipe. The clock and the process id are enough
/// for that, and cost nothing.
fn nonce() -> u64 {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_nanos() as u64)
        .unwrap_or_default();

    nanos ^ ((std::process::id() as u64) << 32)
}
