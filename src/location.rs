#![cfg(target_os = "macos")]

/// Whether GPS collection can work in this build.
///
/// Always `false` here: `CLLocationManager` authorization is a per-user, GUI
/// session permission, and `requestWhenInUseAuthorization` cannot present its
/// prompt from a root LaunchDaemon. `startUpdatingLocation` therefore never
/// produces a fix, so the agent refuses to pretend otherwise.
pub const GPS_SUPPORTED: bool = false;

/// Explanation logged when a backend GPS request cannot be honoured.
pub const GPS_UNAVAILABLE_REASON: &str = "Core Location authorization is a per-user \
     permission and cannot be granted to a root LaunchDaemon";

use std::sync::{
    Arc, Mutex,
    mpsc::{self, Sender},
};
use std::thread;
use std::time::Duration;

use objc2::rc::Retained;
use objc2::runtime::{NSObject, NSObjectProtocol, ProtocolObject};
use objc2::{AnyThread, DefinedClass, define_class};
use objc2_core_location::{CLLocation, CLLocationManager, CLLocationManagerDelegate};
use objc2_foundation::{NSArray, NSDate, NSDefaultRunLoopMode, NSRunLoop};

struct LocationDelegateIvars {
    latest_location: Arc<Mutex<Option<(f64, f64)>>>,
}

define_class!(
    #[unsafe(super(NSObject))]
    #[ivars = LocationDelegateIvars]
    struct LocationDelegate;

    unsafe impl NSObjectProtocol for LocationDelegate {}

    unsafe impl CLLocationManagerDelegate for LocationDelegate {
        #[unsafe(method(locationManager:didUpdateLocations:))]
        fn location_manager_did_update_locations(
            &self,
            _manager: &CLLocationManager,
            locations: &NSArray<CLLocation>,
        ) {
            crate::log_line!("Core Location: didUpdateLocations 📍");

            if let Some(location) = locations.lastObject() {
                let coordinate = unsafe { location.coordinate() };

                crate::log_line!(
                    "Core Location fix: {:.6}, {:.6}",
                    coordinate.latitude, coordinate.longitude
                );

                if let Ok(mut latest) = self.ivars().latest_location.lock() {
                    *latest = Some((coordinate.latitude, coordinate.longitude));
                }
            }
        }

        #[unsafe(method(locationManager:didFailWithError:))]
        fn location_manager_did_fail_with_error(
            &self,
            _manager: &CLLocationManager,
            error: &objc2_foundation::NSError,
        ) {
            let domain = error.domain();
            let code = error.code();
            let description = error.localizedDescription();

            crate::log_line!(
                "Core Location ERROR ❌ domain={} code={} description={}",
                domain, code, description
            );
        }

        #[unsafe(method(locationManagerDidChangeAuthorization:))]
        fn location_manager_did_change_authorization(&self, manager: &CLLocationManager) {
            let status = unsafe { manager.authorizationStatus() };

            crate::log_line!("Core Location authorization changed: {:?}", status);
        }
    }
);

impl LocationDelegate {
    fn new(latest_location: Arc<Mutex<Option<(f64, f64)>>>) -> Retained<Self> {
        let this = Self::alloc().set_ivars(LocationDelegateIvars { latest_location });

        unsafe { objc2::msg_send![super(this), init] }
    }
}

enum LocationCommand {
    Start,
    Stop,
}

pub struct LocationManager {
    command_tx: Sender<LocationCommand>,
    latest_location: Arc<Mutex<Option<(f64, f64)>>>,
}

impl LocationManager {
    pub fn new() -> Self {
        let latest_location = Arc::new(Mutex::new(None));
        let (command_tx, command_rx) = mpsc::channel::<LocationCommand>();

        let latest_location_for_thread = Arc::clone(&latest_location);

        thread::spawn(move || {
            let run_loop = NSRunLoop::currentRunLoop();

            // IMPORTANT:
            // CLLocationManager is created on this same thread that owns
            // the run loop. Core Location callbacks are therefore delivered
            // on the correct thread.
            let manager = unsafe { CLLocationManager::new() };

            let authorization = unsafe { manager.authorizationStatus() };

            // There is no non-deprecated replacement for the system-wide
            // Location Services toggle, so keep the check for diagnostics.
            #[allow(deprecated)]
            let services_enabled = unsafe { manager.locationServicesEnabled() };

            crate::log_line!("Core Location initial authorization: {:?}", authorization);

            crate::log_line!("Core Location services enabled: {}", services_enabled);

            let delegate = LocationDelegate::new(latest_location_for_thread);

            let delegate = ProtocolObject::from_retained(delegate);

            unsafe {
                manager.setDelegate(Some(&delegate));
            }

            crate::log_line!("Core Location thread started 🛰️");

            loop {
                while let Ok(command) = command_rx.try_recv() {
                    match command {
                        LocationCommand::Start => {
                            crate::log_line!("Core Location: starting authorization/location updates");

                            unsafe {
                                manager.requestWhenInUseAuthorization();
                                manager.startUpdatingLocation();
                            }
                        }

                        LocationCommand::Stop => {
                            crate::log_line!("Core Location: stopping location updates");

                            unsafe {
                                manager.stopUpdatingLocation();
                            }
                        }
                    }
                }

                let limit_date = NSDate::dateWithTimeIntervalSinceNow(0.5);

                unsafe {
                    run_loop.runMode_beforeDate(NSDefaultRunLoopMode, &limit_date);
                }

                thread::sleep(Duration::from_millis(10));
            }
        });

        Self {
            command_tx,
            latest_location,
        }
    }

    pub fn start(&self) {
        if let Err(error) = self.command_tx.send(LocationCommand::Start) {
            crate::log_line!("Core Location start command failed ❌: {}", error);
        }
    }

    pub fn current_location(&self) -> Option<(f64, f64)> {
        self.latest_location
            .lock()
            .ok()
            .and_then(|location| *location)
    }

    pub fn stop(&self) {
        if let Err(error) = self.command_tx.send(LocationCommand::Stop) {
            crate::log_line!("Core Location stop command failed ❌: {}", error);
        }
    }
}
