use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use crate::backend::adb::AdbContext;
use crate::backend::resolve::resolve_adb_binary;
use crate::model::DeviceInfo;

#[derive(Debug, Clone)]
pub enum PollEvent {
    AdbMissing,
    ServerNotRunning,
    Devices(Vec<DeviceInfo>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Tag {
    AdbMissing,
    ServerNotRunning,
    Devices,
}

impl PollEvent {
    fn tag(&self) -> Tag {
        match self {
            PollEvent::AdbMissing => Tag::AdbMissing,
            PollEvent::ServerNotRunning => Tag::ServerNotRunning,
            PollEvent::Devices(_) => Tag::Devices,
        }
    }
}

fn poll_once(adb: &AdbContext, resolved: &mut Option<bool>) -> PollEvent {
    // Re-resolve only while missing; once found, the binary does not move.
    if !resolved.unwrap_or(false) {
        *resolved = Some(resolve_adb_binary(None).is_some());
    }
    if *resolved != Some(true) {
        return PollEvent::AdbMissing;
    }
    match adb.list_device_infos() {
        Ok(devices) => PollEvent::Devices(devices),
        Err(_) => PollEvent::ServerNotRunning,
    }
}

pub fn spawn_poller(
    adb_path: Option<String>,
    on_event: impl Fn(PollEvent) + Send + Sync + 'static,
) -> Arc<AtomicBool> {
    let shutdown = Arc::new(AtomicBool::new(false));
    let flag = shutdown.clone();

    std::thread::spawn(move || {
        let adb = AdbContext::new(adb_path);
        let mut resolved: Option<bool> = None;
        let mut backoff = Duration::from_secs(1);
        let mut last_tag: Option<Tag> = None;
        let mut last_devices: Option<Vec<DeviceInfo>> = None;

        loop {
            if flag.load(Ordering::Relaxed) {
                break;
            }

            let event = poll_once(&adb, &mut resolved);
            let tag = event.tag();

            // Emit only on a change; repeats rebuild for nothing.
            let tag_changed = Some(tag) != last_tag;
            if tag_changed {
                backoff = Duration::from_secs(1);
                last_tag = Some(tag);
                last_devices = None;
            }

            let devices_changed = match &event {
                PollEvent::Devices(devices) => last_devices.as_ref() != Some(devices),
                _ => false,
            };
            let devices_present =
                matches!(&event, PollEvent::Devices(devices) if !devices.is_empty());
            if let PollEvent::Devices(devices) = &event {
                last_devices = Some(devices.clone());
            }
            if tag_changed || devices_changed {
                on_event(event);
            }

            let deadline = Instant::now() + backoff;
            while Instant::now() < deadline {
                if flag.load(Ordering::Relaxed) {
                    return;
                }
                std::thread::sleep(Duration::from_millis(100));
            }

            backoff = if devices_present {
                // A disconnect must reach the UI quickly while a phone is up.
                Duration::from_secs(2)
            } else {
                (backoff * 2).min(Duration::from_secs(30))
            };
        }
    });

    shutdown
}
