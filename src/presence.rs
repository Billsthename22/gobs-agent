//! Event transitions only: no keystrokes, window titles, or browsing history.
#[derive(Default)]
pub struct Tracker {
    app: Option<String>,
    idle: Option<bool>,
}

impl Tracker {
    pub fn sample(&mut self, app: Option<String>, idle: bool) -> Vec<serde_json::Value> {
        let mut events = Vec::new();
        if self.idle != Some(idle) {
            events.push(serde_json::json!({"type":"user_presence", "state":if idle {"idle"} else {"active"}}));
            self.idle = Some(idle);
            if !idle {
                self.app = None;
            }
        }
        if !idle {
            if let Some(app) = app.filter(|app| !app.is_empty()) {
                if self.app.as_ref() != Some(&app) {
                    events.push(
                        serde_json::json!({"type":"application_activity", "application":app}),
                    );
                    self.app = Some(app);
                }
            }
        }
        events
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn switches_and_idle_transitions_do_not_repeat_or_count_idle_apps() {
        let mut tracker = Tracker::default();
        assert_eq!(tracker.sample(Some("chrome.exe".into()), false).len(), 2);
        assert!(tracker.sample(Some("chrome.exe".into()), false).is_empty());
        assert_eq!(
            tracker.sample(Some("excel.exe".into()), false)[0]["application"],
            "excel.exe"
        );
        assert_eq!(
            tracker.sample(Some("excel.exe".into()), true)[0]["state"],
            "idle"
        );
        assert!(tracker.sample(Some("chrome.exe".into()), true).is_empty());
        assert_eq!(tracker.sample(Some("chrome.exe".into()), false).len(), 2);
        assert!(tracker.sample(None, false).is_empty());
    }
}
