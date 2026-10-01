//! Fixed native arguments, independently testable without running a power action.
pub fn invocation(platform: &str, command: &str) -> Result<(&'static str, &'static [&'static str]), String> {
    match (platform, command) {
        ("windows", "restart") => Ok(("shutdown.exe", &["/r", "/t", "0", "/f"])),
        ("windows", "shutdown") => Ok(("shutdown.exe", &["/s", "/t", "0", "/f"])),
        ("macos", "restart") => Ok(("/sbin/shutdown", &["-r", "now"])),
        ("macos", "shutdown") => Ok(("/sbin/shutdown", &["-h", "now"])),
        _ => Err("Unsupported platform or power command".into()),
    }
}

pub fn valid_request(id: &str, expires_at: Option<u64>, now: u64) -> bool {
    !id.is_empty() && id.len() <= 128 && expires_at.is_some_and(|expiry| expiry >= now && expiry <= now + 30)
}

pub fn execute(command: &str, dry_run: bool) -> Result<(), String> {
    let (program, args) = invocation(std::env::consts::OS, command)?;
    if dry_run { return Ok(()); }
    let status = std::process::Command::new(program).args(args).status()
        .map_err(|error| format!("Could not execute native power request: {error}"))?;
    if status.success() { Ok(()) }
    else { Err(format!("Native power request failed ({status}); verify service privileges")) }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn native_arguments_are_fixed() {
        assert_eq!(invocation("windows", "restart").unwrap(), ("shutdown.exe", &["/r", "/t", "0", "/f"][..]));
        assert_eq!(invocation("windows", "shutdown").unwrap(), ("shutdown.exe", &["/s", "/t", "0", "/f"][..]));
        assert_eq!(invocation("macos", "restart").unwrap(), ("/sbin/shutdown", &["-r", "now"][..]));
        assert_eq!(invocation("macos", "shutdown").unwrap(), ("/sbin/shutdown", &["-h", "now"][..]));
        assert!(invocation("macos", "restart; echo injected").is_err());
    }
    #[test]
    fn dry_run_and_validation() {
        assert!(execute("restart", true).is_ok());
        assert!(execute("shutdown", true).is_ok());
        assert!(execute("arbitrary", true).is_err());
        assert!(valid_request("id", Some(110), 100));
        assert!(!valid_request("id", Some(90), 100));
        assert!(!valid_request("id", None, 100));
        assert!(!valid_request("", Some(110), 100));
    }
}
