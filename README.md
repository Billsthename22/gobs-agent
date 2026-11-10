# Admin-approved updates

Version 0.1.4 adds an optional signed updater. Builds require a pinned `GBOS_UPDATE_PUBLIC_KEY` to enable remote updates. Existing installations need one manual bootstrap upgrade. See the adjacent [backend update guide](../backend/AGENT_UPDATES.md) before building or distributing updater-enabled releases.

# GBOS device agent

Version 0.1.5 connects automatically after installation. It defaults to `https://gbos-backend.onrender.com`, creates its own machine identity, and registers the computer in Devices. There is no manual credential, hostname entry, or PowerShell setup. Existing installations retain their saved identity.

Version 0.1.3 changes the Windows helper pipe to overlapped I/O, with a separate completion event for each read/write/connect. A pending control read can no longer hold up application or video uploads on the same pipe. The native Windows regression test uploads app data and a video-sized payload while the control reader waits. The service logs its version at startup; `GBOS-Agent.exe --version` prints the binary version without starting the agent. Notice startup failures are forwarded to the service log even when the employee account cannot write to the protected log folder.

## Windows app activity and employee notice

The logged-in session helper samples the foreground application's executable name every five seconds and reports changes, such as `chrome.exe` to `EXCEL.EXE`. It reports active/idle transitions after five minutes without keyboard or mouse input. Sampling does not record keystrokes, mouse coordinates, window titles, document contents, or browser URLs. Very brief app switches between samples can be missed. These are events, not measurements of employee productivity or exact time spent per app.

Employees are informed during company onboarding. Routine app/idle reporting produces no pop-ups. A GBOS icon in the Windows notification area opens a window explaining what is collected; Windows may place it in the overflow area. Live screen viewing displays a separate always-on-top indicator. If the notice UI cannot start, the session helper refuses app sampling and screen capture. The Windows foreground/debug mode uses the same notice and indicator. This functionality does not add app tracking on macOS.

The helper sends events through the existing authenticated service connection. Its bounded event queue is held in memory, not a durable offline history. No additional database columns are required; the backend stores `APPLICATION_ACTIVE`, `USER_ACTIVE`, and `USER_IDLE` activity records.

## Installing a machine

1. Download the latest installer from GBOS. On Windows, open `GBOS-Agent-Windows.exe` and approve the administrator prompt. The installer uses the existing installation folder or `C:\Program Files\GBOS` automatically.
2. Finish the installer. The background service starts automatically, including after a restart.
3. Open Devices in GBOS. The computer appears online when it connects. An offline computer retries automatically when its connection returns.

The macOS installer starts its LaunchDaemon automatically too. Install the updated package; no credential provisioning is needed.

The agent generates and securely saves a random machine identity before contacting the backend. A new hostname is enrolled automatically, or an active precreated record without an identity is linked. Registration and WebSocket connections then authenticate using that identity. An existing enrolled hostname cannot be claimed with a different identity, and inactive records remain blocked. A lost identity or hostname conflict can be handled by an administrator through advanced device recovery.

## Advanced configuration

Normal installation needs no configuration. Existing deployments and development environments can still supply `GBOS_DEVICE_TOKEN` or use `GBOS_DEVICE_TOKEN_FILE`. Otherwise the agent creates and reuses its identity at:

- Windows: `C:\ProgramData\GBOS\device-token` (uses the system's `ProgramData` value).
- macOS: `/Library/Application Support/GBOS/device-token`.
- Custom location: set `GBOS_DEVICE_TOKEN_FILE` in the actual service environment.

The agent protects the default identity directory and file for the service account and administrators. Do not distribute the saved file between machines or include it in an installer. Explicit environment credentials and unreadable or invalid existing files are never silently replaced. A shell variable is not automatically inherited by installed services.

For a foreground development run, point `GBOS_DEVICE_TOKEN_FILE` at an existing development identity file and run `cargo run --bin agent`. The `.env.example` file documents optional variables; the agent does not load `.env` automatically.

`GBOS_API_URL` overrides the default backend. Ensure the credential belongs to the backend being used. HTTP registration sends a bearer header; WebSocket connections also authenticate with a bearer header. Tokens are not placed in connection URLs.

## Builds

```sh
cargo check
cargo test --bin agent
cargo build --release --bin agent
```

The GitHub Actions **Windows Agent Build** workflow builds the Windows executable and NSIS installer. Download its `gbos-agent-windows-installer` artifact and copy the contained installer to the frontend's `public/downloads/GBOS-Agent-Windows.exe` before publishing downloads.

On macOS, run `bash packaging/macos/build-dmg.sh`. It builds the installed Apple targets, creates a package and disk image, and copies the DMG to the adjacent frontend's `public/downloads/GBOS-Agent-macOS.dmg` when that directory exists. It does not install or restart the agent on this machine.

See [the backend README](../backend/README.md) in the shared workspace for roles, invitations, audit records, and deployment details.
