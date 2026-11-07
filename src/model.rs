//! Domain types shared by the backend and the UI.

use std::time::Instant;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// The kind of a filesystem entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum EntryKind {
    File,
    Dir,
    Symlink {
        target: String,
        resolves_to_dir: bool,
    },
    Other,
}

/// A single filesystem entry, either on the host or on a device.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileEntry {
    pub name: String,
    pub path: String,
    pub kind: EntryKind,
    pub size: u64,
    pub modified: Option<DateTime<Utc>>,
    /// Unix mode bits (lower 9 permission bits), 0 when unknown.
    pub mode: u32,
    pub owner: Option<String>,
    pub hidden: bool,
}

impl FileEntry {
    pub fn is_dir(&self) -> bool {
        matches!(self.kind, EntryKind::Dir)
            || matches!(
                self.kind,
                EntryKind::Symlink {
                    resolves_to_dir: true,
                    ..
                }
            )
    }
}

/// A connected device, as surfaced to the UI.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeviceInfo {
    pub serial: String,
    pub model: String,
    pub state: DeviceStateInfo,
    /// Wireless transport for this hardware, when it is hidden behind a USB row.
    pub wireless_serial: Option<String>,
}

/// High-level device state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DeviceStateInfo {
    Connected,
    Authorizing,
    Unauthorized,
    Offline,
    NoPermissions,
    Connecting,
    Detached,
    Bootloader,
    Recovery,
    Sideload,
    Rescue,
    Other(String),
}

impl DeviceStateInfo {
    pub fn is_ready(&self) -> bool {
        matches!(self, DeviceStateInfo::Connected)
    }

    pub fn label(&self) -> &'static str {
        match self {
            DeviceStateInfo::Connected => "Connected",
            DeviceStateInfo::Authorizing => "Authorizing",
            DeviceStateInfo::Unauthorized => "Unauthorized",
            DeviceStateInfo::Offline => "Offline",
            DeviceStateInfo::NoPermissions => "No permissions",
            DeviceStateInfo::Connecting => "Connecting",
            DeviceStateInfo::Detached => "Detached",
            DeviceStateInfo::Bootloader => "Bootloader",
            DeviceStateInfo::Recovery => "Recovery",
            DeviceStateInfo::Sideload => "Sideload",
            DeviceStateInfo::Rescue => "Rescue",
            DeviceStateInfo::Other(_) => "Unknown",
        }
    }

    pub fn help(&self) -> Option<&'static str> {
        match self {
            DeviceStateInfo::Unauthorized => {
                Some("Tap \"Allow USB debugging\" on the device, then reconnect.")
            }
            DeviceStateInfo::NoPermissions => Some(
                "adb cannot access the device. Check your udev rules on Linux or re-authorize the device.",
            ),
            DeviceStateInfo::Offline => {
                Some("Device is offline. Reconnect the cable and try again.")
            }
            DeviceStateInfo::Authorizing => Some("Waiting for authorization on the device."),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TransferDirection {
    Push,
    Pull,
}

/// Progress of one transfer, as stored in the UI model.
#[derive(Debug, Clone)]
pub struct TransferProgress {
    pub id: u64,
    pub name: String,
    pub direction: TransferDirection,
    pub bytes_done: u64,
    pub bytes_total: u64,
    pub done: bool,
    pub failed: bool,
    pub error: Option<String>,
    pub started: DateTime<Utc>,
    pub finished_at: Option<Instant>,
}

impl TransferProgress {
    pub fn fraction(&self) -> f32 {
        if self.bytes_total == 0 {
            0.0
        } else {
            (self.bytes_done as f64 / self.bytes_total as f64) as f32
        }
    }
}

/// A cut/copy entry shared across the host and device panes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClipboardEntry {
    pub op: ClipboardOp,
    pub source: ClipboardSource,
    pub items: Vec<ClipboardItem>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClipboardItem {
    pub path: String,
    pub name: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ClipboardOp {
    Cut,
    Copy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ClipboardSource {
    Host,
    Device,
}

/// A persisted quick-location bookmark.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Favorite {
    pub device_serial: String,
    pub path: String,
    pub name: String,
}

pub fn reorder_favorites(favorites: &mut Vec<Favorite>, from: usize, to: usize) -> bool {
    let len = favorites.len();
    if from >= len || to >= len || from == to {
        return false;
    }
    let favorite = favorites.remove(from);
    favorites.insert(to, favorite);
    true
}

/// A pending backup entry shown in the sidebar.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BackupEntry {
    pub device_serial: String,
    pub source: String,
    pub destination: String,
}

/// Where a path lives.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    Host,
    Device,
}
