# GBOS device agent

Version 0.1.2 connects to `https://gbos-backend.onrender.com` by default and authenticates registration and WebSocket connections with a **per-device credential**. Upgrading from 0.1.1 preserves the existing credential file.

## Windows app activity and employee notice

The logged-in session helper samples the foreground application's executable name every five seconds and reports changes, such as `chrome.exe` to `EXCEL.EXE`. It reports active/idle transitions after five minutes without keyboard or mouse input. Sampling does not record keystrokes, mouse coordinates, window titles, document contents, or browser URLs. Very brief app switches between samples can be missed. These are events, not measurements of employee productivity or exact time spent per app.

Employees are informed during company onboarding. Routine app/idle reporting produces no pop-ups. A GBOS icon in the Windows notification area opens a window explaining what is collected; Windows may place it in the overflow area. Live screen viewing displays a separate always-on-top indicator. If the notice UI cannot start, the session helper refuses app sampling and screen capture. The Windows foreground/debug mode uses the same notice and indicator. This functionality does not add app tracking on macOS.

The helper sends events through the existing authenticated service connection. Its bounded event queue is held in memory, not a durable offline history. No additional database columns are required; the backend stores `APPLICATION_ACTIVE`, `USER_ACTIVE`, and `USER_IDLE` activity records.

## Enrolling a machine

1. Sign in to the frontend as an administrator and open **Settings → Workspace access and audit history**.
2. Create a device record if needed. Its hostname must match the actual machine (`hostname` on macOS; `$env:COMPUTERNAME` in Windows PowerShell).
3. Choose **Issue credential** for that device. Copy the credential immediately; only its SHA-256 hash is stored in the backend. Issuing another credential invalidates the previous one and disconnects its agent.
4. Install the updated agent and provision the credential on that machine as described below. Restart its service after changing the credential.

Existing database device records can be reused. Agents no longer create or claim device records anonymously.

## Credential configuration

The agent first checks the process/service environment variable `GBOS_DEVICE_TOKEN`. If unset, it reads a file containing only the credential:

- Windows: `C:\ProgramData\GBOS\device-token` (uses the system's `ProgramData` value).
- macOS: `/Library/Application Support/GBOS/device-token`.
- Custom location: set `GBOS_DEVICE_TOKEN_FILE` in the actual service environment.

Restrict the file to the service account and machine administrators. Do not save it in this repository, include it in an installer, or share it between devices. A shell environment variable is not automatically inherited by installed services. The standard file locations work without passing shell variables to a service.

On macOS, create `/Library/Application Support/GBOS` as root, put the credential in its `device-token` file, and set the directory to mode `700` and the file to `600`, owned by root. Restart the installed LaunchDaemon with:

```sh
sudo launchctl kickstart -k system/com.gbaja.gbos.agent
```

On Windows, create the credential file from an Administrator PowerShell session and restrict its ACL to `SYSTEM` and the local Administrators group. The Windows service runs as LocalSystem. Restart it after provisioning:

```powershell
Restart-Service GBOSAgent
```

For a foreground development run, set `GBOS_DEVICE_TOKEN` in that terminal and run `cargo run --bin agent`. The `.env.example` file documents variables; the agent does not load `.env` automatically.

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
