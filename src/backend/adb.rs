use std::collections::HashMap;
use std::io::Write;
use std::net::{Ipv4Addr, SocketAddrV4};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use chrono::Utc;

use adb_client::{
    ADBDeviceExt, ADBListItemType, ADBServer, ADBServerDevice, DeviceShort, DeviceState,
};

use crate::backend::progress::{CountingReader, CountingWriter, ProgressSink};
use crate::model::{DeviceInfo, DeviceStateInfo, EntryKind, FileEntry};

// `shell_command` returns Ok at EOF, so mutations recover the exit code here.
const EXIT_SENTINEL: &str = "__ANDROIDFILES_EXIT__";

fn default_socket() -> SocketAddrV4 {
    SocketAddrV4::new(Ipv4Addr::new(127, 0, 0, 1), 5037)
}

pub fn remote_join(parent: &str, name: &str) -> String {
    if parent.ends_with('/') {
        format!("{parent}{name}")
    } else if parent.is_empty() {
        format!("/{name}")
    } else {
        format!("{parent}/{name}")
    }
}

pub fn shell_quote(path: &str) -> String {
    format!("'{}'", path.replace('\'', r"'\''"))
}

fn run_stdout(dev: &mut ADBServerDevice, cmd: &str) -> Result<String, String> {
    // The device runs the raw command through `sh -c` itself, so pass it as a
    // single argument; wrapping it in another `sh -c` would swallow its words.
    let mut out = Vec::new();
    dev.shell_command(&[cmd], &mut out)
        .map_err(|e| e.to_string())?;
    Ok(String::from_utf8_lossy(&out).trim_end().to_string())
}

fn run_status(dev: &mut ADBServerDevice, cmd: &str) -> Result<String, String> {
    let full = format!("{cmd} 2>&1; echo {EXIT_SENTINEL}:$?");
    let mut out = Vec::new();
    dev.shell_command(&[&full], &mut out)
        .map_err(|e| e.to_string())?;
    let text = String::from_utf8_lossy(&out).to_string();

    let marker = format!("{EXIT_SENTINEL}:");
    let Some(idx) = text.rfind(&marker) else {
        return Err(if text.trim().is_empty() {
            "command produced no output".to_string()
        } else {
            text.trim().to_string()
        });
    };

    let code: i32 = text[idx + marker.len()..].trim().parse().unwrap_or(-1);
    let body = text[..idx].trim_end().to_string();

    if code == 0 {
        Ok(body)
    } else if body.is_empty() {
        Err(format!("command exited with status {code}"))
    } else {
        Err(body)
    }
}

pub struct AdbContext {
    adb_path: Option<String>,
    socket: SocketAddrV4,
}

struct Transfer<'a> {
    total: u64,
    base: u64,
    cancel: Option<&'a AtomicBool>,
    sink: Option<Arc<dyn ProgressSink>>,
}

impl Transfer<'_> {
    fn cancelled(&self) -> bool {
        self.cancel
            .map(|c| c.load(Ordering::Relaxed))
            .unwrap_or(false)
    }
}

impl AdbContext {
    pub fn new(adb_path: Option<String>) -> Self {
        Self {
            adb_path,
            socket: default_socket(),
        }
    }

    fn server(&self) -> ADBServer {
        ADBServer::new_from_path(self.socket, self.adb_path.clone())
    }

    pub(crate) fn device(&self, serial: &str) -> ADBServerDevice {
        ADBServerDevice::new(serial.to_string(), Some(self.socket))
    }

    pub fn list_devices(&self) -> Result<Vec<DeviceShort>, String> {
        let mut server = self.server();
        server.devices().map_err(|e| e.to_string())
    }

    pub fn list_device_infos(&self) -> Result<Vec<DeviceInfo>, String> {
        let shorts = self.list_devices()?;
        let mut out = Vec::with_capacity(shorts.len());
        for short in shorts {
            let state = map_state(&short.state);
            let model = self
                .getprop(&short.identifier, "ro.product.model")
                .filter(|m| !m.is_empty())
                .unwrap_or_else(|| short.identifier.clone());
            out.push(DeviceInfo {
                serial: short.identifier,
                model,
                state,
            });
        }
        Ok(out)
    }

    pub fn getprop(&self, serial: &str, prop: &str) -> Option<String> {
        self.shell_stdout(serial, &format!("getprop {prop}")).ok()
    }

    pub fn start_server(&self) {
        ADBServer::start(&Default::default(), &self.adb_path);
    }

    pub fn shell_stdout(&self, serial: &str, cmd: &str) -> Result<String, String> {
        let mut dev = self.device(serial);
        run_stdout(&mut dev, cmd)
    }

    pub fn shell_status(&self, serial: &str, cmd: &str) -> Result<String, String> {
        let mut dev = self.device(serial);
        run_status(&mut dev, cmd)
    }

    pub fn exists(&self, serial: &str, path: &str) -> bool {
        self.shell_stdout(serial, &format!("[ -e {} ] && echo yes", shell_quote(path)))
            .map(|s| s.contains("yes"))
            .unwrap_or(false)
    }

    pub fn is_dir_remote(&self, serial: &str, path: &str) -> bool {
        self.shell_stdout(serial, &format!("[ -d {} ] && echo yes", shell_quote(path)))
            .map(|s| s.contains("yes"))
            .unwrap_or(false)
    }

    pub fn list_dir(&self, serial: &str, path: &str) -> Result<Vec<FileEntry>, String> {
        let mut dev = self.device(serial);
        let items = dev.list(path).map_err(|e| e.to_string())?;

        let symlinks: Vec<(usize, String)> = items
            .iter()
            .enumerate()
            .filter(|(_, item)| item.item_type == ADBListItemType::Symlink)
            .map(|(ix, item)| (ix, remote_join(path, &item.name)))
            .collect();

        // Resolve symlinks in as few shell round trips as the request size
        // allows. Each record is "<index>|<d|f>|<target>", so the target may
        // contain any bytes after the second separator.
        let mut resolved = HashMap::with_capacity(symlinks.len());
        for chunk in symlinks.chunks(64) {
            let script = chunk
                .iter()
                .map(|(ix, link)| {
                    let quoted = shell_quote(link);
                    format!(
                        "printf '%s|%s|' {ix} \"$([ -d {quoted} ] && echo d || echo f)\"; readlink {quoted}; echo"
                    )
                })
                .collect::<Vec<_>>()
                .join("; ");
            let Ok(out) = run_stdout(&mut dev, &script) else {
                continue;
            };
            for line in out.lines() {
                let mut parts = line.splitn(3, '|');
                let (Some(ix), Some(kind)) = (parts.next(), parts.next()) else {
                    continue;
                };
                let Ok(ix) = ix.parse::<usize>() else {
                    continue;
                };
                let target = parts.next().unwrap_or_default().to_string();
                resolved.insert(ix, (target, kind == "d"));
            }
        }

        let mut entries = Vec::with_capacity(items.len());
        for (ix, item) in items.into_iter().enumerate() {
            let full_path = remote_join(path, &item.name);
            let kind = match item.item_type {
                ADBListItemType::File => EntryKind::File,
                ADBListItemType::Directory => EntryKind::Dir,
                ADBListItemType::Symlink => {
                    let (target, resolves_to_dir) = resolved.remove(&ix).unwrap_or_default();
                    EntryKind::Symlink {
                        target,
                        resolves_to_dir,
                    }
                }
            };
            let modified = chrono::DateTime::<Utc>::from_timestamp(item.time as i64, 0);
            entries.push(FileEntry {
                name: item.name.clone(),
                path: full_path,
                kind,
                size: item.size as u64,
                modified,
                mode: item.permissions & 0o777,
                owner: None,
                hidden: item.name.starts_with('.'),
            });
        }

        entries.sort_by_cached_key(|e| (!e.is_dir(), e.name.to_lowercase()));
        Ok(entries)
    }

    pub fn stat_size(&self, serial: &str, path: &str) -> Option<u64> {
        let mut dev = self.device(serial);
        dev.stat(path).ok().map(|s| s.file_size as u64)
    }

    fn guard_delete(&self, path: &str) -> Result<(), String> {
        // Mount roots and shared-storage roots are never deletable.
        let normalized = path.trim();
        let protected = [
            "",
            "/",
            "/.",
            "/..",
            "/sdcard",
            "/storage",
            "/data",
            "/system",
            "/vendor",
            "/dev",
            "/proc",
            "/sys",
            "/mnt",
            "/cache",
            "/persist",
            "/product",
            "/oem",
            "/odm",
            "/system_ext",
            "/firmware",
            "/recovery",
            "/root",
            "/sbin",
            "/init",
            "/etc",
            "/acct",
            "/config",
            "/metadata",
            "/postinstall",
        ];

        if protected.contains(&normalized) {
            return Err(format!("refusing to delete protected path {normalized}"));
        }

        // A top-level path's parent is "/": that path is a mount root.
        if let Some(idx) = normalized.rfind('/') {
            let parent = &normalized[..idx];
            if parent.is_empty() && idx == 0 && !normalized.starts_with("/storage/emulated") {
                return Err(format!(
                    "refusing to delete root-level path {normalized} (root of a mount)"
                ));
            }
        }

        // Shared storage can also appear under its real mount path.
        for root in [
            "/storage/emulated",
            "/storage/self/primary",
            "/storage/emulated/0",
            "/mnt/sdcard",
        ] {
            if normalized == root {
                return Err(format!("refusing to delete protected path {normalized}"));
            }
        }

        Ok(())
    }

    pub fn delete(&self, serial: &str, path: &str) -> Result<(), String> {
        self.guard_delete(path)?;
        self.shell_status(serial, &format!("rm -rf {}", shell_quote(path)))
            .map(|_| ())
    }

    pub fn copy(&self, serial: &str, source: &str, destination: &str) -> Result<(), String> {
        if self.exists(serial, destination) {
            return Err(format!(
                "destination already exists: {}",
                destination.rsplit('/').next().unwrap_or(destination)
            ));
        }
        self.shell_status(
            serial,
            &format!("cp -r {} {}", shell_quote(source), shell_quote(destination)),
        )
        .map(|_| ())
    }

    pub fn move_entry(&self, serial: &str, source: &str, destination: &str) -> Result<(), String> {
        if self.exists(serial, destination) {
            return Err(format!(
                "destination already exists: {}",
                destination.rsplit('/').next().unwrap_or(destination)
            ));
        }
        self.shell_status(
            serial,
            &format!("mv {} {}", shell_quote(source), shell_quote(destination)),
        )
        .map(|_| ())
    }

    pub fn mkdir(&self, serial: &str, path: &str) -> Result<(), String> {
        self.shell_status(serial, &format!("mkdir -p {}", shell_quote(path)))
            .map(|_| ())
    }

    pub fn touch(&self, serial: &str, path: &str) -> Result<(), String> {
        self.shell_status(serial, &format!("touch {}", shell_quote(path)))
            .map(|_| ())
    }

    pub fn pull_file(
        &self,
        serial: &str,
        remote: &str,
        local: &Path,
        base: u64,
        total: u64,
        sink: Option<Arc<dyn ProgressSink>>,
    ) -> Result<(), String> {
        let mut dev = self.device(serial);
        pull_file_with(&mut dev, remote, local, base, total, sink)
    }

    pub fn push_file(
        &self,
        serial: &str,
        local: &Path,
        remote: &str,
        base: u64,
        total: u64,
        sink: Option<Arc<dyn ProgressSink>>,
    ) -> Result<(), String> {
        let mut dev = self.device(serial);
        push_file_with(&mut dev, local, remote, base, total, sink)
    }

    pub fn pull_dir(
        &self,
        serial: &str,
        remote: &str,
        local: &Path,
        cancel: Option<Arc<AtomicBool>>,
        sink: Option<Arc<dyn ProgressSink>>,
    ) -> Result<(), String> {
        let mut dev = self.device(serial);
        // Listing the tree once serves both the progress total and the copy.
        let (tree, total) = scan_remote(&mut dev, remote)?;
        let mut transfer = Transfer {
            total,
            base: 0,
            cancel: cancel.as_deref(),
            sink,
        };
        if let Err(e) = Self::pull_tree(&mut dev, &tree, remote, local, &mut transfer) {
            let _ = std::fs::remove_dir_all(local);
            return Err(e);
        }
        Ok(())
    }

    fn pull_tree(
        dev: &mut ADBServerDevice,
        nodes: &[RemoteNode],
        remote: &str,
        local: &Path,
        transfer: &mut Transfer<'_>,
    ) -> Result<(), String> {
        if transfer.cancelled() {
            return Err("transfer cancelled".to_string());
        }
        std::fs::create_dir_all(local).map_err(|e| e.to_string())?;

        for node in nodes {
            if transfer.cancelled() {
                return Err("transfer cancelled".to_string());
            }
            let remote_path = remote_join(remote, &node.name);
            let local_path = local.join(&node.name);
            match &node.children {
                Some(children) => {
                    Self::pull_tree(dev, children, &remote_path, &local_path, transfer)?;
                }
                None => {
                    pull_file_with(
                        dev,
                        &remote_path,
                        &local_path,
                        transfer.base,
                        transfer.total,
                        transfer.sink.clone(),
                    )?;
                    transfer.base += node.size;
                }
            }
        }
        Ok(())
    }

    pub fn remote_dir_size(&self, serial: &str, remote: &str) -> Result<u64, String> {
        let mut dev = self.device(serial);
        scan_remote(&mut dev, remote).map(|(_, total)| total)
    }

    pub fn push_dir(
        &self,
        serial: &str,
        local: &Path,
        remote: &str,
        cancel: Option<Arc<AtomicBool>>,
        sink: Option<Arc<dyn ProgressSink>>,
    ) -> Result<(), String> {
        let (tree, total) = scan_local(local)?;
        let mut dev = self.device(serial);

        let mut dirs = vec![remote.to_string()];
        collect_local_dirs(&tree, remote, &mut dirs);
        mkdirs_with(&mut dev, &dirs)?;

        let mut transfer = Transfer {
            total,
            base: 0,
            cancel: cancel.as_deref(),
            sink,
        };
        Self::push_tree(&mut dev, &tree, remote, &mut transfer)
    }

    fn push_tree(
        dev: &mut ADBServerDevice,
        nodes: &[LocalNode],
        remote: &str,
        transfer: &mut Transfer<'_>,
    ) -> Result<(), String> {
        if transfer.cancelled() {
            return Err("transfer cancelled".to_string());
        }

        for node in nodes {
            if transfer.cancelled() {
                return Err("transfer cancelled".to_string());
            }
            let remote_path = remote_join(remote, &node.name);
            match &node.children {
                Some(children) => {
                    Self::push_tree(dev, children, &remote_path, transfer)?;
                }
                None => {
                    push_file_with(
                        dev,
                        &node.path,
                        &remote_path,
                        transfer.base,
                        transfer.total,
                        transfer.sink.clone(),
                    )?;
                    transfer.base += node.size;
                }
            }
        }
        Ok(())
    }
}

fn pull_file_with(
    dev: &mut ADBServerDevice,
    remote: &str,
    local: &Path,
    base: u64,
    total: u64,
    sink: Option<Arc<dyn ProgressSink>>,
) -> Result<(), String> {
    if let Some(parent) = local.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let file = std::fs::File::create(local).map_err(|e| e.to_string())?;
    let mut writer = CountingWriter::new(file, base, total, sink);

    let remote_string = remote.to_string();
    if let Err(e) = dev.pull(&remote_string, &mut writer) {
        let _ = std::fs::remove_file(local);
        return Err(e.to_string());
    }
    writer.flush().map_err(|e| {
        let _ = std::fs::remove_file(local);
        e.to_string()
    })?;
    Ok(())
}

fn push_file_with(
    dev: &mut ADBServerDevice,
    local: &Path,
    remote: &str,
    base: u64,
    total: u64,
    sink: Option<Arc<dyn ProgressSink>>,
) -> Result<(), String> {
    let file = std::fs::File::open(local).map_err(|e| e.to_string())?;
    let mut reader = CountingReader::new(file, base, total, sink);
    let remote_string = remote.to_string();
    dev.push(&mut reader, &remote_string)
        .map_err(|e| e.to_string())
}

fn mkdirs_with(dev: &mut ADBServerDevice, dirs: &[String]) -> Result<(), String> {
    // One mkdir per command-line-sized chunk instead of one per directory.
    for chunk in dirs.chunks(64) {
        let quoted = chunk
            .iter()
            .map(|dir| shell_quote(dir))
            .collect::<Vec<_>>()
            .join(" ");
        run_status(dev, &format!("mkdir -p {quoted}"))?;
    }
    Ok(())
}

struct RemoteNode {
    name: String,
    size: u64,
    children: Option<Vec<RemoteNode>>,
}

fn scan_remote(dev: &mut ADBServerDevice, remote: &str) -> Result<(Vec<RemoteNode>, u64), String> {
    let items = dev.list(remote).map_err(|e| e.to_string())?;
    let mut total = 0u64;
    let mut nodes = Vec::with_capacity(items.len());

    for item in items {
        match item.item_type {
            ADBListItemType::File => {
                let size = item.size as u64;
                total += size;
                nodes.push(RemoteNode {
                    name: item.name,
                    size,
                    children: None,
                });
            }
            ADBListItemType::Directory => {
                let path = remote_join(remote, &item.name);
                let (children, size) = scan_remote(dev, &path)?;
                total += size;
                nodes.push(RemoteNode {
                    name: item.name,
                    size,
                    children: Some(children),
                });
            }
            ADBListItemType::Symlink => {}
        }
    }
    Ok((nodes, total))
}

struct LocalNode {
    path: PathBuf,
    name: String,
    size: u64,
    children: Option<Vec<LocalNode>>,
}

fn scan_local(dir: &Path) -> Result<(Vec<LocalNode>, u64), String> {
    let mut entries: Vec<_> = std::fs::read_dir(dir)
        .map_err(|e| e.to_string())?
        .collect::<Result<_, _>>()
        .map_err(|e| e.to_string())?;
    entries.sort_by_key(|e| e.file_name());

    let mut total = 0u64;
    let mut nodes = Vec::with_capacity(entries.len());
    for entry in entries {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        // Do not follow directory symlinks; a loop would recurse forever.
        if entry.file_type().map_err(|e| e.to_string())?.is_dir() {
            let (children, size) = scan_local(&path)?;
            total += size;
            nodes.push(LocalNode {
                path,
                name,
                size,
                children: Some(children),
            });
        } else {
            let size = entry.metadata().map(|m| m.len()).unwrap_or(0);
            total += size;
            nodes.push(LocalNode {
                path,
                name,
                size,
                children: None,
            });
        }
    }
    Ok((nodes, total))
}

fn collect_local_dirs(nodes: &[LocalNode], remote: &str, out: &mut Vec<String>) {
    for node in nodes {
        if let Some(children) = &node.children {
            let path = remote_join(remote, &node.name);
            out.push(path.clone());
            collect_local_dirs(children, &path, out);
        }
    }
}

fn map_state(state: &DeviceState) -> DeviceStateInfo {
    match state {
        DeviceState::Device => DeviceStateInfo::Connected,
        DeviceState::Authorizing => DeviceStateInfo::Authorizing,
        DeviceState::Unauthorized => DeviceStateInfo::Unauthorized,
        DeviceState::Offline => DeviceStateInfo::Offline,
        DeviceState::NoPerm => DeviceStateInfo::NoPermissions,
        DeviceState::Connecting => DeviceStateInfo::Connecting,
        DeviceState::Detached => DeviceStateInfo::Detached,
        DeviceState::Bootloader => DeviceStateInfo::Bootloader,
        DeviceState::Recovery => DeviceStateInfo::Recovery,
        DeviceState::Sideload => DeviceStateInfo::Sideload,
        DeviceState::Rescue => DeviceStateInfo::Rescue,
        DeviceState::NoDevice | DeviceState::Host => DeviceStateInfo::Other(state.to_string()),
    }
}
