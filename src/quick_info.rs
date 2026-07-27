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
    /// Content counts, or `None` if the file couldn't be read as text.
    pub counts: Option<Counts>,
}

impl QuickInfo {
    /// Read the file's metadata and content counts, with its path shown
    /// relative to `workspace_root`. `None` if the file's metadata can't be
    /// read; `counts` is `None` if the content isn't readable as text.
    pub fn read(file: &Path, workspace_root: &Path) -> Option<QuickInfo> {
        let meta = std::fs::metadata(file).ok()?;
        Some(QuickInfo {
            size: meta.len(),
            modified: meta.modified().ok(),
            relative_path: relative_path(file, workspace_root),
            counts: std::fs::read_to_string(file).ok().map(|text| count_text(&text)),
        })
    }
}

/// Whether the file on disk differs from a cached [`QuickInfo`] (size or
/// modified time), warranting a recompute. `true` if nothing is cached or the
/// file can't be read. A cheap stat — no content read.
pub fn file_changed(cached: Option<&QuickInfo>, file: &Path) -> bool {
    let Some(cached) = cached else {
        return true;
    };
    match std::fs::metadata(file) {
        Ok(meta) => meta.len() != cached.size || meta.modified().ok() != cached.modified,
        Err(_) => true,
    }
}

/// Word, character, and line counts of a file's text.
#[derive(Debug, PartialEq, Clone, Copy)]
pub struct Counts {
    pub words: usize,
    pub chars: usize,
    pub lines: usize,
}

/// Count words (whitespace-separated), characters (Unicode scalar values), and
/// lines in `text`. A trailing newline does not add a phantom line, and an
/// empty string is all zeros.
pub fn count_text(text: &str) -> Counts {
    Counts {
        words: text.split_whitespace().count(),
        chars: text.chars().count(),
        lines: text.lines().count(),
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
    fn count_text_counts_words_chars_and_lines() {
        let counts = count_text("hello world\nfoo bar baz\n");
        assert_eq!(counts.words, 5);
        assert_eq!(counts.lines, 2);
        assert_eq!(counts.chars, 24);
    }

    #[test]
    fn count_text_of_empty_is_all_zeros() {
        assert_eq!(
            count_text(""),
            Counts { words: 0, chars: 0, lines: 0 }
        );
    }

    #[test]
    fn count_text_trailing_newline_adds_no_phantom_line() {
        assert_eq!(count_text("a\nb\n").lines, 2);
        assert_eq!(count_text("a\nb").lines, 2);
    }

    #[test]
    fn count_text_counts_unicode_scalar_chars() {
        let counts = count_text("café");
        assert_eq!(counts.chars, 4);
        assert_eq!(counts.words, 1);
    }

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
    fn read_includes_counts_for_text_and_none_for_binary() {
        use std::fs;
        use tempfile::tempdir;

        let root = tempdir().unwrap();

        let text = root.path().join("a.md");
        fs::write(&text, "one two\n").unwrap(); // 2 words, 8 chars, 1 line
        let info = QuickInfo::read(&text, root.path()).unwrap();
        assert_eq!(info.counts, Some(Counts { words: 2, chars: 8, lines: 1 }));

        let bin = root.path().join("b.bin");
        fs::write(&bin, [0xFF, 0xFE, 0x00]).unwrap(); // invalid UTF-8
        let bin_info = QuickInfo::read(&bin, root.path()).unwrap();
        assert_eq!(bin_info.size, 3, "metadata still available");
        assert_eq!(bin_info.counts, None, "unreadable-as-text → counts unknown");
    }

    #[test]
    fn file_changed_detects_size_and_missing_cache() {
        use std::fs;
        use tempfile::tempdir;

        let root = tempdir().unwrap();
        let f = root.path().join("a.md");
        fs::write(&f, "hi").unwrap();
        let cached = QuickInfo::read(&f, root.path()).unwrap();

        assert!(!file_changed(Some(&cached), &f), "unchanged file");

        fs::write(&f, "hello world").unwrap(); // size 2 -> 11
        assert!(file_changed(Some(&cached), &f), "size changed");

        assert!(file_changed(None, &f), "nothing cached yet");
        assert!(
            file_changed(Some(&cached), &root.path().join("gone.md")),
            "unreadable file"
        );
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
