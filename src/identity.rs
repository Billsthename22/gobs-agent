//! Stable machine identity, created locally before the first enrollment request.
//!
//! The backend stores only a hash of this secret. Keeping the same file across
//! retries and upgrades proves that a reconnecting agent is the same machine.

use ring::rand::{SecureRandom, SystemRandom};
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

pub fn device_token() -> io::Result<String> {
    if let Ok(token) = std::env::var("GBOS_DEVICE_TOKEN") {
        if !token.trim().is_empty() {
            return Ok(token.trim().to_owned());
        }
    }

    match std::env::var_os("GBOS_DEVICE_TOKEN_FILE") {
        // A configured file remains authoritative. A typo or lost credential
        // must not silently replace an existing machine's identity.
        Some(path) => read_token(Path::new(&path), false),
        None => load_or_create(&default_path(), true),
    }
}

fn default_path() -> PathBuf {
    if cfg!(target_os = "windows") {
        PathBuf::from(std::env::var_os("ProgramData").unwrap_or_else(|| "C:\\ProgramData".into()))
            .join("GBOS")
            .join("device-token")
    } else {
        PathBuf::from("/Library/Application Support/GBOS/device-token")
    }
}

fn load_or_create(path: &Path, require_root_owner: bool) -> io::Result<String> {
    let directory = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "Device identity needs a parent directory",
            )
        })?;
    secure_directory(directory, require_root_owner)?;

    match read_token(path, require_root_owner) {
        Ok(token) => return Ok(token),
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(error),
    }

    // 256 bits from the operating system's secure random source. Generate it
    // once per saved identity, never per HTTP request or reconnect.
    let token = random_hex()?;
    let temporary_path = directory.join(format!(".device-token-{}.tmp", random_hex()?));
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut temporary_file = options.open(&temporary_path)?;
    let result = (|| {
        secure_file(&temporary_path, require_root_owner)?;
        temporary_file.write_all(token.as_bytes())?;
        temporary_file.sync_all()?;
        drop(temporary_file);

        // A hard link publishes a complete file without replacing an identity
        // another starting process has just saved. Both NTFS and the supported
        // macOS filesystems support this operation.
        match fs::hard_link(&temporary_path, path) {
            Ok(()) => {
                #[cfg(unix)]
                fs::File::open(directory)?.sync_all()?;
                Ok(token)
            }
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
                read_token(path, require_root_owner)
            }
            Err(error) => Err(error),
        }
    })();
    // Only this process owns this randomly named, create_new temporary file.
    let _ = fs::remove_file(&temporary_path);
    result
}

fn random_hex() -> io::Result<String> {
    let mut bytes = [0u8; 32];
    SystemRandom::new()
        .fill(&mut bytes)
        .map_err(|_| io::Error::other("Could not generate a secure device identity"))?;
    use std::fmt::Write as _;
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        let _ = write!(encoded, "{byte:02x}");
    }
    Ok(encoded)
}

fn read_token(path: &Path, require_root_owner: bool) -> io::Result<String> {
    let metadata = checked_metadata(path)?;
    if !metadata.is_file() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Device identity must be a regular file",
        ));
    }
    secure_file(path, require_root_owner)?;
    let token = fs::read_to_string(path)?.trim().to_owned();
    if token.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Device identity file is empty; refusing to replace it",
        ));
    }
    Ok(token)
}

fn checked_metadata(path: &Path) -> io::Result<fs::Metadata> {
    let metadata = fs::symlink_metadata(path)?;
    let linked = metadata.file_type().is_symlink();
    #[cfg(target_os = "windows")]
    let linked = {
        use std::os::windows::fs::MetadataExt;
        use windows::Win32::Storage::FileSystem::FILE_ATTRIBUTE_REPARSE_POINT;
        linked || metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT.0 != 0
    };
    if linked {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "Device identity storage cannot be a link",
        ));
    }
    Ok(metadata)
}

fn secure_directory(path: &Path, require_root_owner: bool) -> io::Result<()> {
    let mut builder = fs::DirBuilder::new();
    builder.recursive(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        builder.mode(0o700);
    }
    builder.create(path)?;
    if !checked_metadata(path)?.is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Device identity storage must be a directory",
        ));
    }
    protect(path, true, require_root_owner)
}

fn secure_file(path: &Path, require_root_owner: bool) -> io::Result<()> {
    checked_metadata(path)?;
    protect(path, false, require_root_owner)
}

#[cfg(unix)]
fn protect(path: &Path, directory: bool, require_root_owner: bool) -> io::Result<()> {
    use std::os::unix::fs::{MetadataExt, PermissionsExt};
    if require_root_owner && fs::metadata(path)?.uid() != 0 {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "Machine identity storage must be owned by root; run the installed agent service",
        ));
    }
    fs::set_permissions(
        path,
        fs::Permissions::from_mode(if directory { 0o700 } else { 0o600 }),
    )
}

#[cfg(target_os = "windows")]
fn protect(path: &Path, directory: bool, _require_root_owner: bool) -> io::Result<()> {
    use std::os::windows::ffi::OsStrExt;
    use windows::Win32::Foundation::{HLOCAL, LocalFree};
    use windows::Win32::Security::Authorization::{
        ConvertStringSecurityDescriptorToSecurityDescriptorW, SDDL_REVISION_1,
    };
    use windows::Win32::Security::{
        DACL_SECURITY_INFORMATION, PROTECTED_DACL_SECURITY_INFORMATION, PSECURITY_DESCRIPTOR,
        SetFileSecurityW,
    };
    use windows::core::PCWSTR;

    // Protected DACLs replace inherited permissions. SID aliases work on every
    // Windows language: SY = LocalSystem, BA = local Administrators.
    let descriptor_text = if directory {
        "D:P(A;OICI;FA;;;SY)(A;OICI;FA;;;BA)"
    } else {
        "D:P(A;;FA;;;SY)(A;;FA;;;BA)"
    };
    let descriptor_wide: Vec<u16> = descriptor_text.encode_utf16().chain(Some(0)).collect();
    let path_wide: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
    let mut descriptor = PSECURITY_DESCRIPTOR::default();
    unsafe {
        ConvertStringSecurityDescriptorToSecurityDescriptorW(
            PCWSTR(descriptor_wide.as_ptr()),
            SDDL_REVISION_1,
            &mut descriptor,
            None,
        )
        .map_err(|error| {
            io::Error::other(format!(
                "Could not prepare device identity permissions: {error}"
            ))
        })?;
        let result = SetFileSecurityW(
            PCWSTR(path_wide.as_ptr()),
            DACL_SECURITY_INFORMATION | PROTECTED_DACL_SECURITY_INFORMATION,
            descriptor,
        )
        .ok();
        let _ = LocalFree(Some(HLOCAL(descriptor.0)));
        result.map_err(|error| {
            io::Error::other(format!(
                "Could not protect device identity; run the installed agent service: {error}"
            ))
        })
    }
}

#[cfg(not(any(unix, target_os = "windows")))]
fn protect(_path: &Path, _directory: bool, _require_root_owner: bool) -> io::Result<()> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "Secure device identity storage is unavailable on this platform",
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestDirectory(PathBuf);

    impl TestDirectory {
        fn new() -> Self {
            let path =
                std::env::temp_dir().join(format!("gbos-identity-test-{}", random_hex().unwrap()));
            fs::create_dir(&path).unwrap();
            Self(path)
        }

        fn token_path(&self) -> PathBuf {
            self.0.join("identity").join("device-token")
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn first_start_saves_identity_and_restarts_reuse_it() {
        let directory = TestDirectory::new();
        let path = directory.token_path();
        let first = load_or_create(&path, false).unwrap();
        assert_eq!(first.len(), 64);
        assert!(first.bytes().all(|b| b.is_ascii_hexdigit()));
        assert_eq!(first, fs::read_to_string(&path).unwrap());
        assert_eq!(first, load_or_create(&path, false).unwrap());
        assert_eq!(fs::read_dir(path.parent().unwrap()).unwrap().count(), 1);
    }

    #[test]
    fn existing_credentials_are_reused_without_changing_their_contents() {
        let directory = TestDirectory::new();
        let path = directory.token_path();
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, " existing-device-credential\n").unwrap();
        assert_eq!(
            load_or_create(&path, false).unwrap(),
            "existing-device-credential"
        );
        assert_eq!(
            fs::read_to_string(&path).unwrap(),
            " existing-device-credential\n"
        );
    }

    #[test]
    fn empty_or_invalid_credentials_are_never_replaced() {
        let directory = TestDirectory::new();
        let path = directory.token_path();
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        for contents in [b" \n".as_slice(), b"\xff\xfe".as_slice()] {
            fs::write(&path, contents).unwrap();
            assert_eq!(
                load_or_create(&path, false).unwrap_err().kind(),
                io::ErrorKind::InvalidData
            );
            assert_eq!(fs::read(&path).unwrap(), contents);
        }
    }

    #[test]
    fn concurrent_starts_publish_only_one_identity() {
        let directory = TestDirectory::new();
        let path = directory.token_path();
        let workers: Vec<_> = (0..8)
            .map(|_| {
                let path = path.clone();
                std::thread::spawn(move || load_or_create(&path, false).unwrap())
            })
            .collect();
        let tokens: Vec<_> = workers
            .into_iter()
            .map(|worker| worker.join().unwrap())
            .collect();
        assert!(tokens.iter().all(|token| token == &tokens[0]));
        assert_eq!(fs::read_to_string(&path).unwrap(), tokens[0]);
        assert_eq!(fs::read_dir(path.parent().unwrap()).unwrap().count(), 1);
    }

    #[test]
    fn path_errors_are_returned_instead_of_changing_identity() {
        let directory = TestDirectory::new();
        let path = directory.token_path();
        fs::create_dir_all(&path).unwrap();
        assert_eq!(
            load_or_create(&path, false).unwrap_err().kind(),
            io::ErrorKind::InvalidData
        );
    }

    #[cfg(unix)]
    #[test]
    fn machine_store_has_private_directory_and_file_modes() {
        use std::os::unix::fs::PermissionsExt;
        let directory = TestDirectory::new();
        let path = directory.token_path();
        load_or_create(&path, false).unwrap();
        assert_eq!(
            fs::metadata(path.parent().unwrap())
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o700
        );
        assert_eq!(
            fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
        fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();
        load_or_create(&path, false).unwrap();
        assert_eq!(
            fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn machine_store_allows_only_system_and_administrators() {
        use std::os::windows::ffi::OsStrExt;
        use windows::Win32::Foundation::{HLOCAL, LocalFree};
        use windows::Win32::Security::Authorization::{
            ConvertSecurityDescriptorToStringSecurityDescriptorW, GetNamedSecurityInfoW,
            SDDL_REVISION_1, SE_FILE_OBJECT,
        };
        use windows::Win32::Security::{DACL_SECURITY_INFORMATION, PSECURITY_DESCRIPTOR};
        use windows::core::{PCWSTR, PWSTR};

        let directory = TestDirectory::new();
        let path = directory.token_path();
        load_or_create(&path, false).unwrap();
        for stored_path in [path.parent().unwrap(), path.as_path()] {
            let wide: Vec<u16> = stored_path
                .as_os_str()
                .encode_wide()
                .chain(Some(0))
                .collect();
            let mut descriptor = PSECURITY_DESCRIPTOR::default();
            let mut text = PWSTR::null();
            let sddl = unsafe {
                GetNamedSecurityInfoW(
                    PCWSTR(wide.as_ptr()),
                    SE_FILE_OBJECT,
                    DACL_SECURITY_INFORMATION,
                    None,
                    None,
                    None,
                    None,
                    &mut descriptor,
                )
                .ok()
                .unwrap();
                let converted = ConvertSecurityDescriptorToStringSecurityDescriptorW(
                    descriptor,
                    SDDL_REVISION_1,
                    DACL_SECURITY_INFORMATION,
                    &mut text,
                    None,
                );
                let _ = LocalFree(Some(HLOCAL(descriptor.0)));
                converted.unwrap();
                let sddl = text.to_string().unwrap();
                let _ = LocalFree(Some(HLOCAL(text.0.cast())));
                sddl
            };
            assert!(
                sddl.starts_with("D:P"),
                "DACL must disable inherited access"
            );
            assert_eq!(sddl.matches("(A;").count(), 2);
            assert!(sddl.contains(";;;SY)"));
            assert!(sddl.contains(";;;BA)"));
        }
    }

    #[cfg(unix)]
    #[test]
    fn linked_identity_is_rejected_without_touching_its_target() {
        use std::os::unix::fs::symlink;
        let directory = TestDirectory::new();
        let path = directory.token_path();
        let target = directory.0.join("other-secret");
        fs::write(&target, "unchanged").unwrap();
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        symlink(&target, &path).unwrap();
        assert_eq!(
            load_or_create(&path, false).unwrap_err().kind(),
            io::ErrorKind::PermissionDenied
        );
        assert_eq!(fs::read_to_string(target).unwrap(), "unchanged");
    }

    #[cfg(unix)]
    #[test]
    fn machine_store_rejects_non_root_owner() {
        use std::os::unix::fs::MetadataExt;
        let directory = TestDirectory::new();
        if fs::metadata(&directory.0).unwrap().uid() == 0 {
            return;
        }
        let path = directory.token_path();
        assert_eq!(
            load_or_create(&path, true).unwrap_err().kind(),
            io::ErrorKind::PermissionDenied
        );
        assert!(!path.exists());
    }
}
