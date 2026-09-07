use std::path::PathBuf;

pub struct ProfileLock { _file: std::fs::File }

impl ProfileLock {
    pub fn acquire(root: &std::path::Path) -> anyhow::Result<Self> {
        std::fs::create_dir_all(root)?;
        let file = std::fs::OpenOptions::new().create(true).truncate(false).read(true).write(true)
            .open(root.join(".profile.lock"))?;
        file.try_lock().map_err(|error| anyhow::anyhow!("该资料库已由另一个 Casy 进程打开，无法重复打开：{error}"))?;
        Ok(Self { _file: file })
    }
}

/// Debug 构建可通过环境变量切换到完全隔离的数据目录；release 构建忽略这些覆盖。
fn debug_override(name: &str) -> Option<PathBuf> {
    if !cfg!(debug_assertions) {
        return None;
    }
    std::env::var_os(name)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
}

pub fn isolated_data_root() -> Option<PathBuf> {
    let args: Vec<_> = std::env::args_os().collect();
    args.windows(2)
        .find(|pair| pair[0] == "--profile-dir")
        .map(|pair| PathBuf::from(&pair[1]))
        .filter(|path| path.is_absolute())
        .or_else(|| debug_override("CASY_TEST_DATA_DIR"))
}

pub fn bundled_runtime() -> Option<PathBuf> {
    use tauri::Manager;
    crate::get_app_handle()
        .and_then(|app| app.path().resource_dir().ok())
        .map(|dir| dir.join("runtime"))
        .filter(|dir| dir.is_dir())
        .or_else(|| debug_override("CASY_RUNTIME_DIR").filter(|dir| dir.is_dir()))
}

pub fn runtime_asset(environment: &str, relative: &str) -> Option<PathBuf> {
    std::env::var_os(environment).filter(|v| !v.is_empty()).map(PathBuf::from)
        .filter(|p| p.exists())
        .or_else(|| bundled_runtime().map(|root| root.join(relative)).filter(|p| p.exists()))
}

pub fn pdf_renderer() -> PathBuf {
    runtime_asset("CASY_PDFTOPPM", if cfg!(windows) { "bin/pdftoppm.exe" } else { "bin/pdftoppm" })
        .unwrap_or_else(|| PathBuf::from("pdftoppm"))
}

pub fn data_root() -> PathBuf {
    isolated_data_root().unwrap_or_else(|| {
        dirs::data_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("Casy")
    })
}

pub fn documents_root() -> PathBuf {
    isolated_data_root()
        .map(|root| root.join("documents"))
        .unwrap_or_else(|| {
            dirs::document_dir()
                .unwrap_or_else(|| PathBuf::from("."))
                .join("Casy")
        })
}

pub fn template_root() -> PathBuf {
    debug_override("CASY_TEST_TEMPLATE_DIR").unwrap_or_else(|| documents_root().join("templates"))
}

pub fn export_root() -> PathBuf {
    debug_override("CASY_TEST_EXPORT_DIR").unwrap_or_else(|| documents_root().join("exports"))
}

pub fn log_root() -> PathBuf {
    isolated_data_root()
        .map(|root| root.join("logs"))
        .unwrap_or_else(|| {
            dirs::home_dir()
                .unwrap_or_else(|| PathBuf::from("."))
                .join("Library/Logs/Casy")
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn profile_lock_excludes_other_handles_and_releases_on_drop() {
        let root = tempfile::tempdir().unwrap();
        let first = ProfileLock::acquire(root.path()).unwrap();
        assert!(ProfileLock::acquire(root.path()).is_err());
        drop(first);
        assert!(ProfileLock::acquire(root.path()).is_ok());
    }

    #[test]
    fn default_subdirectories_share_the_same_root_contract() {
        if let Some(root) = isolated_data_root() {
            assert_eq!(documents_root(), root.join("documents"));
            assert_eq!(data_root(), root);
        }
    }
}
