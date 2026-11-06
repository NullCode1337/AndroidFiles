use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use adb_client::ADBListItemType;

use crate::backend::adb::{AdbContext, remote_join};
use crate::backend::progress::ProgressSink;

pub fn default_options() -> BackupOptions {
    BackupOptions {
        exclude: vec!["Android".to_string()],
        include: vec!["Android/media/com.whatsapp".to_string()],
    }
}

#[derive(Debug, Clone)]
pub struct BackupOptions {
    pub exclude: Vec<String>,
    pub include: Vec<String>,
}

#[derive(Debug, Default)]
pub struct BackupSummary {
    pub files: u64,
    pub dirs: u64,
    pub bytes: u64,
}

fn is_excluded(rel: &str, options: &BackupOptions) -> bool {
    for inc in &options.include {
        if path_is_under(rel, inc) {
            return false;
        }
    }
    for exc in &options.exclude {
        if path_is_under(rel, exc) {
            return true;
        }
    }
    false
}

fn path_is_under(rel: &str, prefix: &str) -> bool {
    let rel = rel.trim_start_matches('/');
    let prefix = prefix.trim_start_matches('/').trim_end_matches('/');
    rel == prefix || rel.starts_with(&format!("{prefix}/"))
}

pub fn backup_device(
    adb: &AdbContext,
    serial: &str,
    source_remote: &str,
    local_dest: &Path,
    options: &BackupOptions,
    cancel: Option<Arc<AtomicBool>>,
    sink: Option<Arc<dyn ProgressSink>>,
) -> Result<BackupSummary, String> {
    let total = adb.remote_dir_size(serial, source_remote)?;
    let mut summary = BackupSummary::default();
    let mut base = 0u64;

    if let Err(e) = backup_inner(
        adb,
        serial,
        source_remote,
        local_dest,
        "",
        options,
        total,
        &mut base,
        &mut summary,
        cancel.as_deref(),
        sink.clone(),
    ) {
        let _ = std::fs::remove_dir_all(local_dest);
        return Err(e);
    }
    Ok(summary)
}

#[allow(clippy::too_many_arguments)]
fn backup_inner(
    adb: &AdbContext,
    serial: &str,
    remote: &str,
    local: &Path,
    rel: &str,
    options: &BackupOptions,
    total: u64,
    base: &mut u64,
    summary: &mut BackupSummary,
    cancel: Option<&AtomicBool>,
    sink: Option<Arc<dyn ProgressSink>>,
) -> Result<(), String> {
    if cancel.map(|c| c.load(Ordering::Relaxed)).unwrap_or(false) {
        return Err("backup cancelled".to_string());
    }
    std::fs::create_dir_all(local).map_err(|e| e.to_string())?;

    let mut dev = adb.device(serial);
    let items = dev.list(remote).map_err(|e| e.to_string())?;

    for item in items {
        if cancel.map(|c| c.load(Ordering::Relaxed)).unwrap_or(false) {
            return Err("backup cancelled".to_string());
        }
        let child_rel = if rel.is_empty() {
            item.name.clone()
        } else {
            format!("{rel}/{}", item.name)
        };
        if is_excluded(&child_rel, options) {
            continue;
        }

        let remote_path = remote_join(remote, &item.name);
        let local_path = local.join(&item.name);
        match item.item_type {
            ADBListItemType::Directory => {
                backup_inner(
                    adb,
                    serial,
                    &remote_path,
                    &local_path,
                    &child_rel,
                    options,
                    total,
                    base,
                    summary,
                    cancel,
                    sink.clone(),
                )?;
                summary.dirs += 1;
            }
            ADBListItemType::File => {
                adb.pull_file(
                    serial,
                    &remote_path,
                    &local_path,
                    *base,
                    total,
                    sink.clone(),
                )?;
                *base += item.size as u64;
                summary.files += 1;
                summary.bytes += item.size as u64;
            }
            ADBListItemType::Symlink => {}
        }
    }
    Ok(())
}
