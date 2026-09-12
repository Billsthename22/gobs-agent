use std::sync::{Arc, Mutex};

use windows::Devices::Geolocation::{Geolocator, Geoposition};

pub struct LocationManager {
    latest_location: Arc<Mutex<Option<(f64, f64)>>>,
}

impl LocationManager {
    pub fn new() -> Self {
        println!("Windows Location Manager initialized");

        Self {
            latest_location: Arc::new(Mutex::new(None)),
        }
    }

    pub fn start(&self) {
        println!("Windows location collection started");

        let latest_location = Arc::clone(&self.latest_location);

        std::thread::spawn(move || {
            let result = (|| -> windows::core::Result<()> {
                let locator = Geolocator::new()?;

                println!("Windows Geolocator initialized");

                let operation = locator.GetGeopositionAsync()?;
                let position: Geoposition = operation.get()?;

                let coordinate = position.Coordinate()?;

                let latitude = coordinate.Latitude()?;
                let longitude = coordinate.Longitude()?;

                println!(
                    "Windows location fix: {:.6}, {:.6}",
                    latitude, longitude
                );

                if let Ok(mut location) = latest_location.lock() {
                    *location = Some((latitude, longitude));
                }

                Ok(())
            })();

            if let Err(error) = result {
                eprintln!("Windows location error: {error}");
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
        println!("Windows location collection stopped");
    }
}
