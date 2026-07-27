//! Quick Info: the active file's stats shown in the bottom Context Column
//! sub-panel — size, last-modified, and workspace-relative path (word/char/
//! line counts arrive in a later slice).
//!
//! Reading is small filesystem I/O; the formatters are pure. No egui here, so
//! it all stays unit-testable. See `CONTEXT.md` (Quick Info, active file).

use std::path::Path;
use std::time::SystemTime;

/// The active file's stats for the Quick Info sub-panel.
pub struct QuickInfo {
    pub size: u64,
    pub modified: Option<SystemTime>,
    pub relative_path: String,
}

impl QuickInfo {
    /// Read the file's metadata, with its path shown relative to
    /// `workspace_root`. `None` if the file can't be read.
    pub fn read(file: &Path, workspace_root: &Path) -> Option<QuickInfo> {
        let meta = std::fs::metadata(file).ok()?;
        Some(QuickInfo {
            size: meta.len(),
            modified: meta.modified().ok(),
            relative_path: relative_path(file, workspace_root),
        })
    }
}

/// Format a byte count for display, e.g. `512 B`, `1.5 KB`, `2.0 MB`.
pub fn format_size(bytes: u64) -> String {
    const KB: f64 = 1024.0;
    let bytes = bytes as f64;
    if bytes < KB {
        format!("{bytes:.0} B")
    } else if bytes < KB * KB {
        format!("{:.1} KB", bytes / KB)
    } else if bytes < KB * KB * KB {
        format!("{:.1} MB", bytes / (KB * KB))
    } else {
        format!("{:.1} GB", bytes / (KB * KB * KB))
    }
}

/// The file's path relative to the workspace root, falling back to the full
/// path if it isn't under the root.
fn relative_path(file: &std::path::Path, root: &std::path::Path) -> String {
    file.strip_prefix(root)
        .unwrap_or(file)
        .to_string_lossy()
        .into_owned()
}

/// Format an elapsed time as a compact relative age, e.g. `just now`, `2m`,
/// `3h`, `5d`.
pub fn format_age(elapsed: std::time::Duration) -> String {
    let secs = elapsed.as_secs();
    if secs < 60 {
        "just now".to_string()
    } else if secs < 3600 {
        format!("{}m", secs / 60)
    } else if secs < 86400 {
        format!("{}h", secs / 3600)
    } else {
        format!("{}d", secs / 86400)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn format_size_scales_by_unit() {
        assert_eq!(format_size(512), "512 B");
        assert_eq!(format_size(1536), "1.5 KB");
        assert_eq!(format_size(1024 * 1024), "1.0 MB");
        assert_eq!(format_size(3 * 1024 * 1024 * 1024), "3.0 GB");
    }

    #[test]
    fn read_gathers_size_relative_path_and_modified() {
        use std::fs;
        use tempfile::tempdir;

        let root = tempdir().unwrap();
        let sub = root.path().join("notes");
        fs::create_dir(&sub).unwrap();
        let file = sub.join("a.md");
        fs::write(&file, "hello").unwrap(); // 5 bytes

        let info = QuickInfo::read(&file, root.path()).unwrap();

        assert_eq!(info.size, 5);
        assert_eq!(info.relative_path, "notes/a.md");
        assert!(info.modified.is_some());
    }

    #[test]
    fn read_of_a_missing_file_is_none() {
        use tempfile::tempdir;
        let root = tempdir().unwrap();
        assert!(QuickInfo::read(&root.path().join("nope.md"), root.path()).is_none());
    }

    #[test]
    fn relative_path_is_relative_to_the_workspace_root() {
        use std::path::Path;
        assert_eq!(
            relative_path(Path::new("/w/docs/a.md"), Path::new("/w")),
            "docs/a.md"
        );
        assert_eq!(
            relative_path(Path::new("/other/x.md"), Path::new("/w")),
            "/other/x.md",
            "a path outside the workspace falls back to absolute"
        );
    }

    #[test]
    fn format_age_reads_as_relative_time() {
        assert_eq!(format_age(Duration::from_secs(5)), "just now");
        assert_eq!(format_age(Duration::from_secs(120)), "2m");
        assert_eq!(format_age(Duration::from_secs(3 * 3600)), "3h");
        assert_eq!(format_age(Duration::from_secs(5 * 86400)), "5d");
    }
}
