//! Windows SCM service installation and control.
//!
//! The agent runs as a `LocalSystem` service so it survives logout, starts at
//! boot, and can launch the per-session helper into a user's interactive
//! session (`WTSQueryUserToken` requires `SeTcbPrivilege`, which only
//! `LocalSystem` holds — see `session.rs`).
//!
//! The service itself lives in Session 0 and therefore cannot capture the
//! screen or read location; that work is done by the helper.

use std::ffi::OsString;
use std::process::ExitCode;
use std::time::Duration;

use windows_service::service::{
    ServiceAccess, ServiceAction, ServiceActionType, ServiceErrorControl, ServiceFailureActions,
    ServiceFailureResetPeriod, ServiceInfo, ServiceStartType, ServiceState, ServiceType,
};
use windows_service::service_manager::{ServiceManager, ServiceManagerAccess};

/// Internal service name. Also the name shown in `services.msc` and used by
/// `sc.exe`, so it is part of the product's operational surface.
pub const SERVICE_NAME: &str = "GBOSAgent";

const DISPLAY_NAME: &str = "GBOS Device Agent";

const DESCRIPTION: &str =
    "Connects this computer to GBOS for device management, telemetry and remote monitoring.";

/// Access rights needed for the install path: create-or-open, reconfigure an
/// existing registration on upgrade, and start it.
const INSTALL_ACCESS: ServiceAccess = ServiceAccess::from_bits_truncate(
    ServiceAccess::QUERY_STATUS.bits()
        | ServiceAccess::QUERY_CONFIG.bits()
        | ServiceAccess::CHANGE_CONFIG.bits()
        | ServiceAccess::START.bits()
        | ServiceAccess::STOP.bits()
        | ServiceAccess::DELETE.bits(),
);

/// Windows error raised when the service name is already registered.
const ERROR_SERVICE_EXISTS: i32 = 1073;

/// Windows error raised when starting a service that is already running.
const ERROR_SERVICE_ALREADY_RUNNING: i32 = 1056;

/// Install (or upgrade) the service and start it.
pub fn install() -> Result<(), String> {
    let executable = std::env::current_exe()
        .map_err(|error| format!("could not locate the running binary: {}", error))?;

    let manager = ServiceManager::local_computer(
        None::<&str>,
        ServiceManagerAccess::CONNECT | ServiceManagerAccess::CREATE_SERVICE,
    )
    .map_err(|error| {
        format!(
            "could not open the service manager ({}). \
             Installing a service requires an elevated (Administrator) prompt.",
            error
        )
    })?;

    let info = service_info(&executable);

    let service = match manager.create_service(&info, INSTALL_ACCESS) {
        Ok(service) => service,

        Err(error) if is_win32_error(&error, ERROR_SERVICE_EXISTS) => {
            // Already registered — this is the upgrade path, so update the
            // existing registration rather than failing.
            let service = manager
                .open_service(SERVICE_NAME, INSTALL_ACCESS)
                .map_err(|error| format!("could not open the existing service: {}", error))?;

            service
                .change_config(&info)
                .map_err(|error| format!("could not update the service configuration: {}", error))?;

            service
        }

        Err(error) => return Err(format!("could not create the service: {}", error)),
    };

    let _ = service.set_description(DESCRIPTION);

    apply_recovery_actions(&service);

    match service.start(&[] as &[&std::ffi::OsStr]) {
        Ok(()) => {}

        // Restarting on upgrade: the service is already up, which is fine.
        Err(error) if is_win32_error(&error, ERROR_SERVICE_ALREADY_RUNNING) => {}

        Err(error) => return Err(format!("could not start the service: {}", error)),
    }

    crate::log_line!(
        "GBOS agent service '{}' installed and running",
        SERVICE_NAME
    );

    Ok(())
}

/// Stop the service and deregister it. Idempotent.
pub fn uninstall() -> Result<(), String> {
    let manager = ServiceManager::local_computer(None::<&str>, ServiceManagerAccess::CONNECT)
        .map_err(|error| {
            format!(
                "could not open the service manager ({}). \
                 Removing a service requires an elevated (Administrator) prompt.",
                error
            )
        })?;

    let service = match manager.open_service(SERVICE_NAME, INSTALL_ACCESS) {
        Ok(service) => service,

        // Not installed: nothing to do.
        Err(_) => {
            crate::log_line!("GBOS agent service '{}' is not installed", SERVICE_NAME);

            return Ok(());
        }
    };

    if let Ok(status) = service.query_status() {
        if status.current_state != ServiceState::Stopped {
            // A stop failure should not block removal, so report and continue.
            if let Err(error) = service.stop() {
                crate::log_line!("Could not stop the service cleanly: {}", error);
            } else if let Err(error) = wait_for_stopped(&service, Duration::from_secs(20)) {
                crate::log_line!("Service did not report Stopped: {}", error);
            }
        }
    }

    service
        .delete()
        .map_err(|error| format!("could not delete the service: {}", error))?;

    crate::log_line!("GBOS agent service '{}' removed", SERVICE_NAME);

    Ok(())
}

/// Report whether the service is installed, how it starts, and whether it runs.
pub fn status() -> ExitCode {
    println!("GBOS agent service status");
    println!("  service:    {}", SERVICE_NAME);

    let manager = match ServiceManager::local_computer(None::<&str>, ServiceManagerAccess::CONNECT) {
        Ok(manager) => manager,
        Err(error) => {
            println!("  installed:  unknown (could not open the service manager: {})", error);

            return ExitCode::FAILURE;
        }
    };

    let service = match manager.open_service(SERVICE_NAME, ServiceAccess::QUERY_STATUS | ServiceAccess::QUERY_CONFIG) {
        Ok(service) => service,

        Err(_) => {
            println!("  installed:  no");
            println!();
            println!("The service is not registered. Install it from an elevated prompt with:");
            println!("  GBOS-Agent.exe --install-service");

            return ExitCode::FAILURE;
        }
    };

    println!("  installed:  yes");

    if let Ok(config) = service.query_config() {
        println!("  start:      {:?}", config.start_type);
    }

    match service.query_status() {
        Ok(status) => {
            println!("  state:      {:?}", status.current_state);

            if status.current_state == ServiceState::Running {
                ExitCode::SUCCESS
            } else {
                println!();
                println!(
                    "The service is registered but not running. Check the GBOS agent log for why it exited."
                );

                ExitCode::FAILURE
            }
        }

        Err(error) => {
            println!("  state:      unknown ({})", error);

            ExitCode::FAILURE
        }
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn service_info(executable: &std::path::Path) -> ServiceInfo {
    ServiceInfo {
        name: OsString::from(SERVICE_NAME),
        display_name: OsString::from(DISPLAY_NAME),
        service_type: ServiceType::OWN_PROCESS,

        // Start at boot, and keep running when a user logs out.
        start_type: ServiceStartType::AutoStart,
        error_control: ServiceErrorControl::Normal,
        executable_path: executable.to_path_buf(),

        // Empty: the SCM launches the binary with no arguments, which selects
        // the service dispatcher.
        launch_arguments: vec![],
        dependencies: vec![],

        // `None` means LocalSystem. This is required, not incidental:
        // launching the per-session helper needs `WTSQueryUserToken`, which
        // only LocalSystem is permitted to call.
        account_name: None,
        account_password: None,
    }
}

/// Restart the service if it crashes, backing off between attempts.
fn apply_recovery_actions(service: &windows_service::service::Service) {
    let actions = ServiceFailureActions {
        // Reset the failure count after a day without failures.
        reset_period: ServiceFailureResetPeriod::After(Duration::from_secs(86_400)),
        reboot_msg: None,
        command: None,
        actions: Some(vec![
            ServiceAction {
                action_type: ServiceActionType::Restart,
                delay: Duration::from_secs(5),
            },
            ServiceAction {
                action_type: ServiceActionType::Restart,
                delay: Duration::from_secs(10),
            },
            ServiceAction {
                action_type: ServiceActionType::Restart,
                delay: Duration::from_secs(30),
            },
        ]),
    };

    if let Err(error) = service.update_failure_actions(actions) {
        // Not fatal: the service still works, Windows just will not restart it
        // automatically. Worth recording rather than hiding.
        crate::log_line!("Could not set service recovery actions: {}", error);
    }
}

fn wait_for_stopped(
    service: &windows_service::service::Service,
    timeout: Duration,
) -> Result<(), String> {
    let deadline = std::time::Instant::now() + timeout;

    while std::time::Instant::now() < deadline {
        match service.query_status() {
            Ok(status) if status.current_state == ServiceState::Stopped => return Ok(()),

            Ok(_) => std::thread::sleep(Duration::from_millis(250)),

            Err(error) => return Err(format!("could not query service status: {}", error)),
        }
    }

    Err(format!("timed out after {:?}", timeout))
}

/// True when a `windows-service` error wraps the given Win32 error code.
fn is_win32_error(error: &windows_service::Error, code: i32) -> bool {
    match error {
        windows_service::Error::Winapi(io_error) => {
            io_error.raw_os_error() == Some(code)
        }

        _ => false,
    }
}
