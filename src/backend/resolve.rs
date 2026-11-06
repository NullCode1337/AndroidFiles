use std::path::{Path, PathBuf};

fn adb_binary_name() -> &'static str {
    if cfg!(target_os = "windows") {
        "adb.exe"
    } else {
        "adb"
    }
}

fn is_executable(path: &Path) -> bool {
    if !path.is_file() {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        path.metadata()
            .map(|m| m.permissions().mode() & 0o111 != 0)
            .unwrap_or(false)
    }
    #[cfg(not(unix))]
    {
        true
    }
}

fn which(name: &str) -> Option<PathBuf> {
    let path_var = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path_var) {
        let candidate = dir.join(name);
        if is_executable(&candidate) {
            return Some(candidate);
        }
    }
    None
}

fn candidate_sdk_roots() -> Vec<PathBuf> {
    let mut roots: Vec<PathBuf> = Vec::new();

    if let Some(home) = dirs::home_dir() {
        #[cfg(target_os = "macos")]
        roots.push(home.join("Library").join("Android").join("sdk"));
        #[cfg(not(target_os = "macos"))]
        roots.push(home.join("Android").join("Sdk"));
    }

    #[cfg(target_os = "windows")]
    if let Some(local) = dirs::data_local_dir() {
        roots.push(local.join("Android").join("Sdk"));
    }

    #[cfg(target_os = "linux")]
    {
        roots.push(PathBuf::from("/usr/lib/android-sdk"));
        roots.push(PathBuf::from("/opt/android-sdk"));
        roots.push(PathBuf::from("/usr/local/android-sdk"));
    }

    roots
}

fn adb_in_sdk_root(root: &Path) -> Option<PathBuf> {
    let platform_tools = root.join("platform-tools").join(adb_binary_name());
    if is_executable(&platform_tools) {
        return Some(platform_tools);
    }
    let bare = root.join(adb_binary_name());
    if is_executable(&bare) {
        return Some(bare);
    }
    None
}

pub fn resolve_adb_binary(user_override: Option<&str>) -> Option<String> {
    if let Some(configured) = user_override {
        let trimmed = configured.trim();
        if !trimmed.is_empty() {
            let path = PathBuf::from(trimmed);
            if is_executable(&path) {
                return Some(trimmed.to_string());
            }
            if let Some(found) = which(trimmed) {
                return Some(found.to_string_lossy().to_string());
            }
        }
    }

    for env_var in ["ANDROID_HOME", "ANDROID_SDK_ROOT"] {
        let Ok(value) = std::env::var(env_var) else {
            continue;
        };
        let value = value.trim();
        if !value.is_empty()
            && let Some(found) = adb_in_sdk_root(Path::new(value))
        {
            return Some(found.to_string_lossy().to_string());
        }
    }

    if let Some(found) = which(adb_binary_name()) {
        return Some(found.to_string_lossy().to_string());
    }

    for root in candidate_sdk_roots() {
        if let Some(found) = adb_in_sdk_root(&root) {
            return Some(found.to_string_lossy().to_string());
        }
    }

    None
}
