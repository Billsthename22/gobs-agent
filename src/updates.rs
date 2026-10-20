//! Admin-approved, signed binary replacement. The updater runs independently
//! of the service it stops, keeps the old binary, and requires a fresh heartbeat
//! from the new agent before declaring success. No arbitrary install commands.
use base64::{Engine, engine::general_purpose::STANDARD};
use ring::{digest, signature};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::sync::atomic::{AtomicBool, Ordering};
use std::{
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
    process::{Command, ExitCode},
    time::Duration,
};

type UpdateResult<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;
const PUBLIC_KEY: &str = match option_env!("GBOS_UPDATE_PUBLIC_KEY") {
    Some(key) => key,
    None => "",
};
const MAX_SIZE: u64 = 250_000_000;
static UPDATER_ACCEPTED: AtomicBool = AtomicBool::new(false);

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Manifest {
    version: String,
    platform: String,
    architecture: String,
    url: String,
    sha256: String,
    size: u64,
    signature: String,
}

impl Manifest {
    fn payload(&self) -> Vec<u8> {
        // serde_json's default Map sorts keys, matching the release signing tool.
        serde_json::to_vec(
            &json!({"architecture": self.architecture, "platform": self.platform,
            "sha256": self.sha256, "size": self.size, "url": self.url, "version": self.version}),
        )
        .unwrap()
    }

    fn verify(
        &self,
        key: &str,
        current: &str,
        platform: &str,
        architecture: &str,
    ) -> UpdateResult<()> {
        if self.platform != platform
            || self.architecture != architecture
            || version(&self.version)? <= version(current)?
        {
            return Err("Release is not a newer version for this platform and architecture".into());
        }
        if self.size == 0
            || self.size > MAX_SIZE
            || self.sha256.len() != 64
            || !self
                .sha256
                .bytes()
                .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
        {
            return Err("Invalid release size or checksum".into());
        }
        let url = reqwest::Url::parse(&self.url)?;
        if url.scheme() != "https"
            || url.host_str().is_none()
            || !url.username().is_empty()
            || url.password().is_some()
            || url.fragment().is_some()
            || url.port().is_some_and(|p| p != 443)
        {
            return Err("Update requires an HTTPS URL without credentials or fragments".into());
        }
        signature::UnparsedPublicKey::new(&signature::ED25519, STANDARD.decode(key)?)
            .verify(&self.payload(), &STANDARD.decode(&self.signature)?)
            .map_err(|_| "Release signature verification failed")?;
        Ok(())
    }
}

fn version(value: &str) -> UpdateResult<[u32; 3]> {
    let parts: Vec<_> = value.split('.').collect();
    if parts.len() != 3
        || parts.iter().any(|p| {
            p.is_empty()
                || p.len() > 6
                || (p.len() > 1 && p.starts_with('0'))
                || !p.bytes().all(|b| b.is_ascii_digit())
        })
    {
        return Err("Expected a stable major.minor.patch version".into());
    }
    Ok([parts[0].parse()?, parts[1].parse()?, parts[2].parse()?])
}

#[derive(Deserialize)]
struct CheckResponse {
    update: Option<ApprovedUpdate>,
    updater_accepted: bool,
}
#[derive(Deserialize)]
struct ApprovedUpdate {
    job_id: String,
    manifest: Manifest,
}
#[derive(Deserialize, Serialize)]
struct Transaction {
    job_id: String,
    manifest: Manifest,
    phase: String,
    approved_at: u64,
}
#[derive(Deserialize, Serialize)]
struct Receipt {
    job_id: String,
    status: String,
    detail: String,
}

fn directory() -> PathBuf {
    if cfg!(target_os = "windows") {
        PathBuf::from(std::env::var("ProgramData").unwrap_or_else(|_| "C:\\ProgramData".into()))
            .join("GBOS")
            .join("updates")
    } else {
        PathBuf::from("/Library/Application Support/GBOS/updates")
    }
}

fn installed_binary() -> PathBuf {
    if cfg!(target_os = "windows") {
        PathBuf::from(std::env::var("ProgramFiles").unwrap_or_else(|_| "C:\\Program Files".into()))
            .join("GBOS")
            .join("GBOS-Agent.exe")
    } else {
        PathBuf::from("/usr/local/libexec/gbos-agent")
    }
}

fn capable() -> bool {
    cfg!(any(target_os = "windows", target_os = "macos"))
        && STANDARD.decode(PUBLIC_KEY).is_ok_and(|k| k.len() == 32)
        && std::env::current_exe()
            .ok()
            .and_then(|p| p.canonicalize().ok())
            == installed_binary().canonicalize().ok()
        && installed_binary().exists()
}

fn secure_directory() -> UpdateResult<PathBuf> {
    let dir = directory();
    fs::create_dir_all(&dir)?;
    if fs::symlink_metadata(&dir)?.file_type().is_symlink() {
        return Err("Update directory cannot be a symlink".into());
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::{MetadataExt, PermissionsExt};
        if fs::metadata(&dir)?.uid() != 0 {
            return Err("Update directory must be owned by root".into());
        }
        fs::set_permissions(&dir, fs::Permissions::from_mode(0o700))?;
    }
    #[cfg(target_os = "windows")]
    {
        // Replace the entire ACL, including inherited entries. SID names work
        // on localized Windows installations. No path enters shell source.
        command(Command::new(powershell()).args(["-NoProfile", "-NonInteractive", "-Command",
            "& { $ErrorActionPreference='Stop'; $p=Join-Path $env:ProgramData 'GBOS\\updates'; $a=New-Object System.Security.AccessControl.DirectorySecurity; $a.SetAccessRuleProtection($true,$false); foreach($s in @('S-1-5-18','S-1-5-32-544')) { $id=New-Object System.Security.Principal.SecurityIdentifier($s); $r=New-Object System.Security.AccessControl.FileSystemAccessRule($id,'FullControl','ContainerInherit,ObjectInherit','None','Allow'); $a.AddAccessRule($r) }; Set-Acl -LiteralPath $p -AclObject $a }"]))?;
    }
    Ok(dir)
}

fn write_json<T: Serialize>(path: &Path, value: &T) -> UpdateResult<()> {
    let temp = path.with_extension("tmp");
    let mut file = fs::File::create(&temp)?;
    file.write_all(&serde_json::to_vec(value)?)?;
    file.sync_all()?;
    drop(file);
    fs::rename(temp, path)?;
    #[cfg(unix)]
    fs::File::open(path.parent().ok_or("Missing update directory")?)?.sync_all()?;
    Ok(())
}

fn read_json<T: for<'a> Deserialize<'a>>(path: &Path) -> UpdateResult<T> {
    Ok(serde_json::from_slice(&fs::read(path)?)?)
}

fn hash_file(path: &Path) -> UpdateResult<(String, u64)> {
    let mut file = fs::File::open(path)?;
    let mut context = digest::Context::new(&digest::SHA256);
    let mut total = 0;
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let size = file.read(&mut buffer)?;
        if size == 0 {
            break;
        }
        total += size as u64;
        if total > MAX_SIZE {
            return Err("Update exceeds maximum size".into());
        }
        context.update(&buffer[..size]);
    }
    Ok((
        context
            .finish()
            .as_ref()
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect(),
        total,
    ))
}

/// A successful authenticated WebSocket heartbeat proves the replacement can
/// run and reconnect. The independent helper observes this marker.
pub fn confirm_connection() {
    if !UPDATER_ACCEPTED.load(Ordering::Relaxed) || !capable() {
        return;
    }
    if let Ok(tx) = read_json::<Transaction>(&directory().join("transaction.json")) {
        if tx.phase == "installing" && tx.manifest.version == env!("CARGO_PKG_VERSION") {
            let _ = fs::write(directory().join("ready"), tx.job_id);
        }
    }
}

pub fn start(device_id: i32) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let client = match reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .connect_timeout(Duration::from_secs(15))
            .timeout(Duration::from_secs(300))
            .build()
        {
            Ok(c) => c,
            Err(_) => return,
        };
        loop {
            if let Err(error) = poll(&client, device_id).await {
                crate::log_line!("[UPDATE] Check failed: {}", error);
            }
            tokio::time::sleep(Duration::from_secs(30)).await;
        }
    })
}

async fn poll(client: &reqwest::Client, device_id: i32) -> UpdateResult<()> {
    let token = crate::device_token().map_err(|_| "Device credential unavailable")?;
    let receipt = read_json::<Receipt>(&directory().join("result.json")).ok();
    let response = client.post(format!("{}/agents/updates/check", crate::gbos_api_url())).bearer_auth(&token)
        .json(&json!({"device_id": device_id, "version": env!("CARGO_PKG_VERSION"), "platform": std::env::consts::OS,
            "architecture": std::env::consts::ARCH, "protocol": 1, "updater_ready": capable(), "public_key": PUBLIC_KEY,
            "result": receipt})).send().await?.error_for_status()?.json::<CheckResponse>().await?;
    UPDATER_ACCEPTED.store(response.updater_accepted, Ordering::Relaxed);
    if capable() && resume_helper()? {
        return Ok(());
    }
    if let Some(update) = response.update {
        if !capable() {
            return Err("Agent is not running from its supported service installation".into());
        }
        if let Err(error) = prepare(client, device_id, &token, &update).await {
            let _ = report(
                client,
                device_id,
                &token,
                &update.job_id,
                "failed",
                "Update preparation or verification failed; existing agent retained",
            )
            .await;
            return Err(error);
        }
    }
    Ok(())
}

async fn report(
    client: &reqwest::Client,
    device_id: i32,
    token: &str,
    job_id: &str,
    status: &str,
    detail: &str,
) -> UpdateResult<()> {
    client
        .post(format!(
            "{}/agents/updates/{job_id}/status",
            crate::gbos_api_url()
        ))
        .bearer_auth(token)
        .json(&json!({"device_id": device_id, "status": status, "detail": detail}))
        .send()
        .await?
        .error_for_status()?;
    Ok(())
}

async fn prepare(
    client: &reqwest::Client,
    device_id: i32,
    token: &str,
    update: &ApprovedUpdate,
) -> UpdateResult<()> {
    if update.job_id.len() != 36
        || !update
            .job_id
            .bytes()
            .all(|b| b.is_ascii_hexdigit() || b == b'-')
    {
        return Err("Malformed update identifier".into());
    }
    update.manifest.verify(
        PUBLIC_KEY,
        env!("CARGO_PKG_VERSION"),
        std::env::consts::OS,
        std::env::consts::ARCH,
    )?;
    let dir = secure_directory()?;
    let lock = fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(dir.join("update.lock"))?;
    if lock.try_lock().is_err() {
        return Ok(());
    }
    if let Ok(tx) = read_json::<Transaction>(&dir.join("transaction.json")) {
        if tx.phase != "finished" {
            return Ok(());
        }
    }
    let mut response = client
        .get(&update.manifest.url)
        .send()
        .await?
        .error_for_status()?;
    if response
        .content_length()
        .is_some_and(|size| size > update.manifest.size)
    {
        return Err("Download is larger than the signed release".into());
    }
    let payload = dir.join("payload.new");
    let mut file = fs::File::create(&payload)?;
    let mut size = 0;
    while let Some(chunk) = response.chunk().await? {
        size += chunk.len() as u64;
        if size > update.manifest.size {
            return Err("Download exceeded its signed size".into());
        }
        file.write_all(&chunk)?;
    }
    file.sync_all()?;
    drop(file);
    if hash_file(&payload)? != (update.manifest.sha256.clone(), update.manifest.size) {
        return Err("Release checksum or size mismatch".into());
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&payload, fs::Permissions::from_mode(0o700))?;
    }
    let helper = dir.join(if cfg!(target_os = "windows") {
        "updater-helper.exe"
    } else {
        "updater-helper"
    });
    fs::copy(std::env::current_exe()?, &helper)?;
    for path in ["ready", "result.json"] {
        let _ = fs::remove_file(dir.join(path));
    }
    report(
        client,
        device_id,
        token,
        &update.job_id,
        "installing",
        "Signed binary verified; starting independent updater",
    )
    .await?;
    write_json(
        &dir.join("transaction.json"),
        &Transaction {
            job_id: update.job_id.clone(),
            manifest: update.manifest.clone(),
            phase: "staged".into(),
            approved_at: unix_time(),
        },
    )?;
    if let Err(error) = launch_helper(&helper) {
        let _ = fs::remove_file(dir.join("transaction.json"));
        return Err(error);
    }
    Ok(())
}

fn command(command: &mut Command) -> UpdateResult<()> {
    let output = command.output()?;
    if !output.status.success() {
        return Err(format!(
            "Service updater command failed: {}",
            String::from_utf8_lossy(&output.stderr)
        )
        .into());
    }
    Ok(())
}

fn unix_time() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn resume_helper() -> UpdateResult<bool> {
    let dir = directory();
    let Ok(tx) = read_json::<Transaction>(&dir.join("transaction.json")) else {
        return Ok(false);
    };
    if tx.phase == "finished" {
        return Ok(false);
    }
    let lock = fs::OpenOptions::new()
        .write(true)
        .open(dir.join("update.lock"))?;
    if lock.try_lock().is_ok() {
        let helper = dir.join(if cfg!(target_os = "windows") {
            "updater-helper.exe"
        } else {
            "updater-helper"
        });
        launch_helper(&helper)?;
    }
    Ok(true)
}

#[cfg(target_os = "windows")]
fn powershell() -> PathBuf {
    PathBuf::from(std::env::var("SystemRoot").unwrap_or_else(|_| "C:\\Windows".into()))
        .join("System32/WindowsPowerShell/v1.0/powershell.exe")
}

fn launch_helper(helper: &Path) -> UpdateResult<()> {
    #[cfg(target_os = "windows")]
    {
        let task = format!("\"{}\" --apply-update", helper.display());
        command(Command::new("schtasks.exe").args([
            "/Create",
            "/TN",
            "GBOSAgentUpdate",
            "/SC",
            "ONSTART",
            "/RU",
            "SYSTEM",
            "/RL",
            "HIGHEST",
            "/TR",
            &task,
            "/F",
        ]))?;
        command(Command::new("schtasks.exe").args(["/Run", "/TN", "GBOSAgentUpdate"]))?;
        return Ok(());
    }
    #[cfg(target_os = "macos")]
    {
        // A separate launchd job survives stopping the main daemon. Keep the
        // plist until completion so an interrupted transaction recovers at boot.
        let escaped = helper
            .to_string_lossy()
            .replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;");
        let plist = format!(
            r#"<?xml version="1.0" encoding="UTF-8"?><!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd"><plist version="1.0"><dict><key>Label</key><string>com.gbaja.gbos.updater</string><key>ProgramArguments</key><array><string>{escaped}</string><string>--apply-update</string></array><key>RunAtLoad</key><true/></dict></plist>"#
        );
        let path = "/Library/LaunchDaemons/com.gbaja.gbos.updater.plist";
        fs::write(path, plist)?;
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o600))?;
        let _ = Command::new("/bin/launchctl")
            .args(["bootout", "system/com.gbaja.gbos.updater"])
            .output();
        command(Command::new("/bin/launchctl").args(["bootstrap", "system", path]))?;
        return Ok(());
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        let _ = helper;
        Err("Automatic updates are supported on Windows and macOS services only".into())
    }
}

fn stop_service() -> UpdateResult<()> {
    #[cfg(target_os = "windows")]
    command(Command::new(powershell()).args(["-NoProfile", "-NonInteractive", "-Command", "$ErrorActionPreference='Stop'; $s=Get-Service -Name GBOSAgent; if($s.Status -ne 'Stopped') { Stop-Service -Name GBOSAgent -Force; $s.WaitForStatus('Stopped',[TimeSpan]::FromSeconds(45)) }"]))?;
    #[cfg(target_os = "macos")]
    command(Command::new("/bin/launchctl").args(["bootout", "system/com.gbaja.gbos.agent"]))?;
    Ok(())
}

fn start_service() -> UpdateResult<()> {
    #[cfg(target_os = "windows")]
    command(Command::new("sc.exe").args(["start", "GBOSAgent"]))?;
    #[cfg(target_os = "macos")]
    command(Command::new("/bin/launchctl").args([
        "bootstrap",
        "system",
        "/Library/LaunchDaemons/com.gbaja.gbos.agent.plist",
    ]))?;
    Ok(())
}

fn cleanup_helper_job() {
    #[cfg(target_os = "windows")]
    {
        let _ = Command::new("schtasks.exe")
            .args(["/Delete", "/TN", "GBOSAgentUpdate", "/F"])
            .output();
    }
    #[cfg(target_os = "macos")]
    {
        let _ = fs::remove_file("/Library/LaunchDaemons/com.gbaja.gbos.updater.plist");
    }
}

fn replace_binary(source: &Path) -> UpdateResult<()> {
    let destination = installed_binary();
    let staged = destination.with_extension("update");
    fs::copy(source, &staged)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&staged, fs::Permissions::from_mode(0o755))?;
    }
    fs::OpenOptions::new()
        .write(true)
        .open(&staged)?
        .sync_all()?;
    fs::rename(staged, destination)?;
    Ok(())
}

trait ServiceControl {
    fn stop(&mut self) -> UpdateResult<()>;
    fn replace(&mut self, source: &Path) -> UpdateResult<()>;
    fn start(&mut self) -> UpdateResult<()>;
}

struct NativeService;
impl ServiceControl for NativeService {
    fn stop(&mut self) -> UpdateResult<()> {
        stop_service()
    }
    fn replace(&mut self, source: &Path) -> UpdateResult<()> {
        replace_binary(source)
    }
    fn start(&mut self) -> UpdateResult<()> {
        start_service()
    }
}

fn activate_candidate(
    service: &mut impl ServiceControl,
    payload: &Path,
    mut healthy: impl FnMut() -> bool,
) -> UpdateResult<()> {
    service.stop()?;
    service.replace(payload)?;
    service.start()?;
    if healthy() {
        Ok(())
    } else {
        Err("Updated agent did not reconnect within two minutes".into())
    }
}

fn restore_previous(service: &mut impl ServiceControl, backup: &Path) -> UpdateResult<()> {
    // The failed candidate may never have started, so a stop error alone must
    // not prevent attempting to restore the known working image.
    let _ = service.stop();
    service.replace(backup)?;
    service.start()
}

fn run_transaction(dir: &Path, tx: &mut Transaction) -> UpdateResult<()> {
    if unix_time().saturating_sub(tx.approved_at) > 600 || tx.approved_at > unix_time() + 60 {
        return Err("Staged update approval is too old; request approval again".into());
    }
    tx.manifest.verify(
        PUBLIC_KEY,
        env!("CARGO_PKG_VERSION"),
        std::env::consts::OS,
        std::env::consts::ARCH,
    )?;
    let payload = dir.join("payload.new");
    if hash_file(&payload)? != (tx.manifest.sha256.clone(), tx.manifest.size) {
        return Err("Staged release failed verification".into());
    }
    // Persist the backup before making any change to the service.
    fs::copy(installed_binary(), dir.join("previous.bin"))?;
    fs::OpenOptions::new()
        .write(true)
        .open(dir.join("previous.bin"))?
        .sync_all()?;
    tx.phase = "installing".into();
    write_json(&dir.join("transaction.json"), tx)?;
    activate_candidate(&mut NativeService, &payload, || {
        for _ in 0..60 {
            if fs::read_to_string(dir.join("ready")).ok().as_deref() == Some(&tx.job_id) {
                return true;
            }
            std::thread::sleep(Duration::from_secs(2));
        }
        false
    })
}

fn apply() -> UpdateResult<()> {
    let dir = secure_directory()?;
    let lock = fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(dir.join("update.lock"))?;
    // The service holds this briefly while launching this independent helper.
    let mut locked = false;
    for _ in 0..30 {
        if lock.try_lock().is_ok() {
            locked = true;
            break;
        }
        std::thread::sleep(Duration::from_secs(1));
    }
    if !locked {
        return Err("Another updater is active".into());
    }
    let mut tx: Transaction = read_json(&dir.join("transaction.json"))?;
    if tx.phase == "finished" {
        cleanup_helper_job();
        return Ok(());
    }
    let result = if tx.phase == "staged" {
        run_transaction(&dir, &mut tx)
    } else {
        Err("Recovered an interrupted installation; restoring previous binary".into())
    };
    let receipt = match result {
        Ok(()) => Receipt {
            job_id: tx.job_id.clone(),
            status: "succeeded".into(),
            detail: "New agent reconnected".into(),
        },
        Err(error) => {
            let mut detail = format!("Update failed: {error}");
            if tx.phase == "installing" {
                match restore_previous(&mut NativeService, &dir.join("previous.bin")) {
                    Ok(()) => detail.push_str("; previous agent restored"),
                    Err(rollback) => detail.push_str(&format!(
                        "; recovery requires administrator attention: {rollback}"
                    )),
                }
            }
            Receipt {
                job_id: tx.job_id.clone(),
                status: "failed".into(),
                detail: detail.chars().take(500).collect(),
            }
        }
    };
    write_json(&dir.join("result.json"), &receipt)?;
    tx.phase = "finished".into();
    write_json(&dir.join("transaction.json"), &tx)?;
    cleanup_helper_job();
    crate::log_line!("[UPDATE] {}: {}", receipt.status, receipt.detail);
    Ok(())
}

pub fn apply_update() -> ExitCode {
    match apply() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("GBOS updater: {error}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn signed_manifest() -> (Manifest, String) {
        let pair = signature::Ed25519KeyPair::from_seed_unchecked(&[42; 32]).unwrap();
        use signature::KeyPair;
        let mut manifest = Manifest {
            version: "0.1.5".into(),
            platform: "windows".into(),
            architecture: "x86_64".into(),
            url: "https://downloads.example.com/agent.exe".into(),
            sha256: "a".repeat(64),
            size: 1024,
            signature: String::new(),
        };
        manifest.signature = STANDARD.encode(pair.sign(&manifest.payload()).as_ref());
        (manifest, STANDARD.encode(pair.public_key().as_ref()))
    }
    #[test]
    fn signed_releases_require_matching_platform_and_newer_version() {
        let (m, key) = signed_manifest();
        assert!(m.verify(&key, "0.1.4", "windows", "x86_64").is_ok());
        assert!(m.verify(&key, "0.1.5", "windows", "x86_64").is_err());
        assert!(m.verify(&key, "0.1.6", "windows", "x86_64").is_err());
        assert!(m.verify(&key, "0.1.4", "macos", "x86_64").is_err());
        assert!(m.verify(&key, "0.1.4", "windows", "aarch64").is_err());
    }
    #[test]
    fn tampered_manifests_and_insecure_urls_are_rejected() {
        let (mut m, key) = signed_manifest();
        m.sha256 = "b".repeat(64);
        assert!(m.verify(&key, "0.1.4", "windows", "x86_64").is_err());
        m.url = "http://downloads.example.com/agent.exe".into();
        assert!(m.verify(&key, "0.1.4", "windows", "x86_64").is_err());
    }
    #[test]
    fn versions_compare_numerically() {
        assert!(version("0.1.10").unwrap() > version("0.1.9").unwrap());
        for bad in ["1.2", "01.2.3", "1.2.3-beta", "1.2.3.4", "1.2.-1"] {
            assert!(version(bad).is_err());
        }
    }

    #[derive(Default)]
    struct FakeService {
        events: Vec<String>,
        fail_once: Option<&'static str>,
    }
    impl FakeService {
        fn event(&mut self, name: &str) -> UpdateResult<()> {
            self.events.push(name.into());
            if self.fail_once == Some(name) {
                self.fail_once = None;
                return Err("Simulated service failure".into());
            }
            Ok(())
        }
    }
    impl ServiceControl for FakeService {
        fn stop(&mut self) -> UpdateResult<()> {
            self.event("stop")
        }
        fn replace(&mut self, source: &Path) -> UpdateResult<()> {
            self.event(source.to_str().unwrap())
        }
        fn start(&mut self) -> UpdateResult<()> {
            self.event("start")
        }
    }
    #[test]
    fn replacement_requires_a_healthy_reconnection() {
        let mut service = FakeService::default();
        assert!(activate_candidate(&mut service, Path::new("candidate"), || true).is_ok());
        assert_eq!(service.events, ["stop", "candidate", "start"]);
        let mut service = FakeService::default();
        assert!(activate_candidate(&mut service, Path::new("candidate"), || false).is_err());
        assert!(restore_previous(&mut service, Path::new("backup")).is_ok());
        assert_eq!(
            service.events,
            ["stop", "candidate", "start", "stop", "backup", "start"]
        );
    }
    #[test]
    fn service_failures_still_allow_recovery() {
        for stage in ["stop", "candidate", "start"] {
            let mut service = FakeService {
                fail_once: Some(stage),
                ..Default::default()
            };
            assert!(activate_candidate(&mut service, Path::new("candidate"), || true).is_err());
            assert!(restore_previous(&mut service, Path::new("backup")).is_ok());
            assert!(
                service
                    .events
                    .ends_with(&["stop".into(), "backup".into(), "start".into()])
            );
        }
        let mut service = FakeService {
            fail_once: Some("stop"),
            ..Default::default()
        };
        assert!(restore_previous(&mut service, Path::new("backup")).is_ok());
    }
}
