/// Whether GPS collection can work in this build.
///
/// `true` here because the Windows agent reads location from a per-session
/// helper running inside the interactive user's session, where the WinRT
/// `Geolocator` has a user context to grant the location permission against.
/// The Session 0 service itself could not — see `session.rs`.
pub const GPS_SUPPORTED: bool = true;

/// Explanation logged when a backend GPS request cannot be honoured.
///
/// Unused on Windows, but `main.rs` references it in the same branch on every
/// platform so the refusal message stays identical.
#[allow(dead_code)]
pub const GPS_UNAVAILABLE_REASON: &str = "location collection is unavailable in this build";

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use windows::Devices::Geolocation::{
    GeolocationAccessStatus, Geolocator, PositionAccuracy, PositionStatus,
};
use windows::Foundation::TimeSpan;

/// How long to wait between fixes.
///
/// The service's own GPS tick is 10 s, so asking any faster would only produce
/// readings that overwrite each other unread.
const REFRESH_INTERVAL: Duration = Duration::from_secs(10);

/// How old a cached reading may be before `GetGeopositionAsync` fetches a new
/// one, and how long to wait for it.
///
/// The timeout matters: the parameterless `GetGeopositionAsync` can wait
/// indefinitely on a machine with no location hardware, and this runs on a
/// thread that `stop()` expects to wind down.
const MAX_AGE: TimeSpan = TimeSpan {
    Duration: 5 * 10_000_000,
};

const TIMEOUT: TimeSpan = TimeSpan {
    Duration: 30 * 10_000_000,
};

/// `E_FAIL`, used to report a position that came back without coordinates.
const E_FAIL: i32 = 0x8000_4005u32 as i32;

/// Reads location from the WinRT `Geolocator`.
///
/// Only usable inside an interactive session — the helper's, on Windows. From
/// Session 0 the permission cannot be granted at all, so the service never
/// builds one of these.
pub struct LocationManager {
    latest_location: Arc<Mutex<Option<(f64, f64)>>>,

    /// Cleared by [`LocationManager::stop`], which is what ends the thread's
    /// loop. Without it `stop` would be a log line that stopped nothing.
    running: Arc<AtomicBool>,
}

impl LocationManager {
    pub fn new() -> Self {
        crate::log_line!("Windows Location Manager initialized");

        Self {
            latest_location: Arc::new(Mutex::new(None)),
            running: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn start(&self) {
        // Guard against a second start, which would leave two threads racing to
        // write the same slot and only one flag to stop them.
        if self.running.swap(true, Ordering::SeqCst) {
            return;
        }

        crate::log_line!("Windows location collection started");

        let latest_location = Arc::clone(&self.latest_location);
        let running = Arc::clone(&self.running);

        std::thread::spawn(move || {
            let result = (|| -> windows::core::Result<()> {
                let runtime = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .map_err(|error| {
                        windows::core::Error::new(
                            windows::core::HRESULT(0x80004005u32 as i32),
                            format!("Failed to create Tokio runtime: {error}"),
                        )
                    })?;

                runtime.block_on(async {
                    // Unpackaged Win32 apps have no package identity, so nothing
                    // has ever put the location permission to the user, and
                    // `GetGeopositionAsync` fails with access denied until
                    // something asks. Asking is the app's job, and this is it —
                    // its absence is why location never worked here.
                    match Geolocator::RequestAccessAsync()?.await? {
                        GeolocationAccessStatus::Allowed => {}

                        other => {
                            crate::log_line!(
                                "Windows location permission was not granted (status {}). \
                                 Location will not be reported. The user can allow it under \
                                 Settings → Privacy & security → Location.",
                                other.0
                            );

                            running.store(false, Ordering::SeqCst);

                            return Ok::<(), windows::core::Error>(());
                        }
                    }

                    let locator = Geolocator::new()?;

                    // Without this the default accuracy can be coarse enough to
                    // be useless for locating a machine.
                    locator.SetDesiredAccuracy(PositionAccuracy::High)?;

                    crate::log_line!("Windows Geolocator initialized");

                    // Only the first failure of a run is logged: a machine with
                    // the location service switched off would otherwise write a
                    // line every ten seconds for as long as it is on.
                    let mut reported_failure = false;

                    while running.load(Ordering::SeqCst) {
                        // `Disabled` is a setting rather than a fault, and is
                        // worth naming — it is not something retrying will fix.
                        let disabled = locator
                            .LocationStatus()
                            .map(|status| status == PositionStatus::Disabled)
                            .unwrap_or(false);

                        if disabled {
                            if !reported_failure {
                                reported_failure = true;

                                crate::log_line!(
                                    "Windows location is switched off in system settings; \
                                     not retrying until it is enabled"
                                );
                            }

                            tokio::time::sleep(REFRESH_INTERVAL).await;

                            continue;
                        }

                        match fetch(&locator).await {
                            Ok((latitude, longitude)) => {
                                if reported_failure {
                                    crate::log_line!("Windows location recovered");
                                }

                                reported_failure = false;

                                if let Ok(mut location) = latest_location.lock() {
                                    *location = Some((latitude, longitude));
                                }

                                crate::log_line!(
                                    "Windows location fix: {:.6}, {:.6}",
                                    latitude,
                                    longitude
                                );
                            }

                            Err(error) => {
                                if !reported_failure {
                                    reported_failure = true;

                                    crate::log_line!(
                                        "Windows location error: {error} (further failures will \
                                         be silent until it recovers)"
                                    );
                                }
                            }
                        }

                        tokio::time::sleep(REFRESH_INTERVAL).await;
                    }

                    crate::log_line!("Windows location collection stopped");

                    Ok(())
                })
            })();

            if let Err(error) = result {
                crate::log_line!("Windows location error: {error}");

                // So a later `start` is not refused as "already running" by a
                // thread that has died.
                running.store(false, Ordering::SeqCst);
            }
        });
    }

    pub fn current_location(&self) -> Option<(f64, f64)> {
        self.latest_location
            .lock()
            .ok()
            .and_then(|location| *location)
    }

    pub fn stop(&self) {
        self.running.store(false, Ordering::SeqCst);

        crate::log_line!("Windows location collection stopped");
    }
}

/// One position, or the reason there isn't one.
async fn fetch(locator: &Geolocator) -> windows::core::Result<(f64, f64)> {
    let operation = locator.GetGeopositionAsyncWithAgeAndTimeout(MAX_AGE, TIMEOUT)?;

    let position = operation.await?;

    let coordinate = position.Coordinate()?;

    let latitude = coordinate.Latitude()?;
    let longitude = coordinate.Longitude()?;

    // A successful call can still carry no position — the sensor can be
    // initialising, or have no data. Reporting (0, 0) as a fix would put the
    // device in the Gulf of Guinea, so this is treated as a failure instead.
    if latitude == 0.0 && longitude == 0.0 {
        return Err(windows::core::Error::new(
            windows::core::HRESULT(E_FAIL),
            "the position contained no coordinates",
        ));
    }

    Ok((latitude, longitude))
}
