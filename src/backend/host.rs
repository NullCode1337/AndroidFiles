use std::fs;
use std::path::Path;
use std::time::UNIX_EPOCH;

use chrono::{DateTime, Utc};

use crate::model::{EntryKind, FileEntry};

fn modified_of(meta: &fs::Metadata) -> Option<DateTime<Utc>> {
    meta.modified().ok().and_then(|t| {
        let dur = t.duration_since(UNIX_EPOCH).ok()?;
        DateTime::<Utc>::from_timestamp(dur.as_secs() as i64, dur.subsec_nanos())
    })
}

#[cfg(unix)]
fn mode_of(meta: &fs::Metadata) -> u32 {
    use std::os::unix::fs::MetadataExt;
    meta.mode() & 0o7777
}

#[cfg(not(unix))]
fn mode_of(_meta: &fs::Metadata) -> u32 {
    0
}

fn entry_from_path(path: &Path) -> Option<FileEntry> {
    let meta = fs::metadata(path).ok()?;
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| path.to_string_lossy().to_string());

    let kind = if meta.is_dir() {
        EntryKind::Dir
    } else if meta.is_file() {
        EntryKind::File
    } else {
        EntryKind::Other
    };

    Some(FileEntry {
        name: name.clone(),
        path: path.to_string_lossy().to_string(),
        kind,
        size: meta.len(),
        modified: modified_of(&meta),
        mode: mode_of(&meta),
        owner: None,
        hidden: name.starts_with('.'),
    })
}

pub fn list_directory(path: &Path) -> anyhow::Result<Vec<FileEntry>> {
    let mut entries = Vec::new();
    for entry in fs::read_dir(path)? {
        let entry = entry?;
        if let Some(file_entry) = entry_from_path(&entry.path()) {
            entries.push(file_entry);
        }
    }
    entries.sort_by(|a, b| {
        let a_dir = a.is_dir();
        let b_dir = b.is_dir();
        b_dir
            .cmp(&a_dir)
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
    Ok(entries)
}

pub fn is_directory(path: &Path) -> bool {
    fs::metadata(path).map(|m| m.is_dir()).unwrap_or(false)
}

pub fn file_size(path: &Path) -> u64 {
    fs::metadata(path).map(|m| m.len()).unwrap_or(0)
}

pub fn delete(path: &Path) -> anyhow::Result<()> {
    let meta = fs::symlink_metadata(path)?;
    if meta.is_dir() {
        fs::remove_dir_all(path)?;
    } else {
        fs::remove_file(path)?;
    }
    Ok(())
}

pub fn copy(source: &Path, destination: &Path) -> anyhow::Result<()> {
    let meta = fs::metadata(source)?;
    if meta.is_dir() {
        copy_dir_recursive(source, destination)
    } else {
        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::copy(source, destination)?;
        Ok(())
    }
}

fn copy_dir_recursive(source: &Path, destination: &Path) -> anyhow::Result<()> {
    fs::create_dir_all(destination)?;
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let target = destination.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_dir_recursive(&entry.path(), &target)?;
        } else {
            fs::copy(entry.path(), target)?;
        }
    }
    Ok(())
}

pub fn move_entry(source: &Path, destination: &Path) -> anyhow::Result<()> {
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::rename(source, destination)?;
    Ok(())
}

pub fn mkdir(path: &Path) -> anyhow::Result<()> {
    fs::create_dir_all(path)?;
    Ok(())
}

pub fn touch(path: &Path) -> anyhow::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let _ = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)?;
    Ok(())
}
