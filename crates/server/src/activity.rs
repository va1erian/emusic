//! In-memory tracking of what paired devices are doing right now.
//!
//! Fed by the authentication extractor (every authenticated request), the
//! WebSocket handler and the stream/render routes. Only the admin listener
//! reads it; nothing here is exposed on the public API. State is lost on
//! restart by design: it describes the live process, not history (that is the
//! audit log's job).

use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard};

use serde::Serialize;

use crate::util::unix_now;

/// A device counts as active if it made a request this recently, in seconds.
pub const ACTIVE_WINDOW_SECS: i64 = 5 * 60;

/// Entries idle for longer than this are forgotten, in seconds.
const FORGET_AFTER_SECS: i64 = 24 * 3600;

/// Longest user agent kept, in characters.
const MAX_USER_AGENT_CHARS: usize = 200;

/// Live activity of one device.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct DeviceActivity {
    /// Device id.
    pub device_id: String,
    /// Device name at the time of the last request.
    pub device_name: String,
    /// Last authenticated request (Unix seconds).
    pub last_request_at: i64,
    /// Real client address of the last request.
    pub client_ip: String,
    /// `User-Agent` of the last request, truncated.
    pub user_agent: String,
    /// Currently open WebSocket connections.
    pub open_websockets: u32,
    /// Last track streamed or rendered.
    pub last_track_id: Option<String>,
    /// When `last_track_id` was requested (Unix seconds).
    pub last_track_at: Option<i64>,
}

impl DeviceActivity {
    /// Whether the device made a request within [`ACTIVE_WINDOW_SECS`] of
    /// `now` or holds an open WebSocket.
    pub fn is_active(&self, now: i64) -> bool {
        self.open_websockets > 0 || now.saturating_sub(self.last_request_at) <= ACTIVE_WINDOW_SECS
    }
}

/// Thread-safe map of device id to [`DeviceActivity`].
#[derive(Debug, Default)]
pub struct ActivityTracker {
    devices: Mutex<HashMap<String, DeviceActivity>>,
}

impl ActivityTracker {
    /// An empty tracker.
    pub fn new() -> Self {
        Self::default()
    }

    /// Records an authenticated request.
    pub fn touch(&self, device_id: &str, device_name: &str, client_ip: &str, user_agent: &str) {
        let now = unix_now();
        let mut devices = self.lock();
        if !devices.contains_key(device_id) {
            devices.retain(|_, entry| {
                entry.open_websockets > 0
                    || now.saturating_sub(entry.last_request_at) < FORGET_AFTER_SECS
            });
        }
        let entry = devices.entry(device_id.to_string()).or_default();
        entry.device_id = device_id.to_string();
        entry.device_name = device_name.to_string();
        entry.last_request_at = now;
        entry.client_ip = client_ip.to_string();
        entry.user_agent = user_agent.chars().take(MAX_USER_AGENT_CHARS).collect();
    }

    /// Records that `device_id` requested `track_id` for playback.
    pub fn track_requested(&self, device_id: &str, track_id: &str) {
        if let Some(entry) = self.lock().get_mut(device_id) {
            entry.last_track_id = Some(track_id.to_string());
            entry.last_track_at = Some(unix_now());
        }
    }

    /// Counts an open WebSocket until the returned guard is dropped.
    pub fn websocket_opened(self: &Arc<Self>, device_id: &str) -> WebSocketGuard {
        if let Some(entry) = self.lock().get_mut(device_id) {
            entry.open_websockets = entry.open_websockets.saturating_add(1);
        }
        WebSocketGuard {
            tracker: Arc::clone(self),
            device_id: device_id.to_string(),
        }
    }

    /// Active devices at `now`, most recent first.
    pub fn active(&self, now: i64) -> Vec<DeviceActivity> {
        let mut active: Vec<_> = self
            .lock()
            .values()
            .filter(|entry| entry.is_active(now))
            .cloned()
            .collect();
        active.sort_by_key(|entry| std::cmp::Reverse(entry.last_request_at));
        active
    }

    fn lock(&self) -> MutexGuard<'_, HashMap<String, DeviceActivity>> {
        self.devices
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

/// Decrements a device's open-WebSocket count when dropped.
pub struct WebSocketGuard {
    tracker: Arc<ActivityTracker>,
    device_id: String,
}

impl Drop for WebSocketGuard {
    fn drop(&mut self) {
        if let Some(entry) = self.tracker.lock().get_mut(&self.device_id) {
            entry.open_websockets = entry.open_websockets.saturating_sub(1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn touched_devices_are_active_until_the_window_passes() {
        let tracker = ActivityTracker::new();
        tracker.touch("a", "Phone", "203.0.113.4", "emusic-android/1.0");
        let now = unix_now();
        let active = tracker.active(now);
        assert_eq!(active.len(), 1);
        assert_eq!(active[0].device_name, "Phone");
        assert_eq!(active[0].client_ip, "203.0.113.4");
        assert!(tracker.active(now + ACTIVE_WINDOW_SECS + 1).is_empty());
    }

    #[test]
    fn an_open_websocket_keeps_a_device_active() {
        let tracker = Arc::new(ActivityTracker::new());
        tracker.touch("a", "Desktop", "127.0.0.1", "");
        let guard = tracker.websocket_opened("a");
        let later = unix_now() + ACTIVE_WINDOW_SECS * 10;
        assert_eq!(tracker.active(later)[0].open_websockets, 1);
        drop(guard);
        assert!(tracker.active(later).is_empty());
    }

    #[test]
    fn track_requests_and_user_agents_are_recorded() {
        let tracker = ActivityTracker::new();
        tracker.touch("a", "Desktop", "127.0.0.1", &"x".repeat(500));
        tracker.track_requested("a", "track-1");
        tracker.track_requested("unknown", "track-2");
        let entry = &tracker.active(unix_now())[0];
        assert_eq!(entry.last_track_id.as_deref(), Some("track-1"));
        assert_eq!(entry.user_agent.len(), MAX_USER_AGENT_CHARS);
        assert_eq!(tracker.active(unix_now()).len(), 1);
    }
}
