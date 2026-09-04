use std::path::PathBuf;

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
    debug_override("CASY_TEST_DATA_DIR")
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
    fn default_subdirectories_share_the_same_root_contract() {
        if let Some(root) = isolated_data_root() {
            assert_eq!(documents_root(), root.join("documents"));
            assert_eq!(data_root(), root);
        }
    }
}
