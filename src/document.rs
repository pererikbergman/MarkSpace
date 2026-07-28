//! The Document source: the active file's full markdown text held in the
//! editor, the single source of truth for editing (see `CONTEXT.md` and
//! ADR 0004). This slice holds and edits it as plain text; parsing into blocks
//! arrives with the per-block editor.
//!
//! Pure load logic with no dependency on egui, so it stays unit-testable.

use std::path::PathBuf;

/// The active file's editable text and its path.
pub struct Document {
    pub path: PathBuf,
    pub text: String,
}

impl Document {
    /// Load a file into an editable document. `None` if it can't be read as
    /// text (missing, or non-UTF-8 binary).
    pub fn open(path: PathBuf) -> Option<Document> {
        let text = std::fs::read_to_string(&path).ok()?;
        Some(Document { path, text })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn open_reads_the_file_text() {
        let dir = tempdir().unwrap();
        let file = dir.path().join("note.md");
        fs::write(&file, "# Title\n\nhello").unwrap();

        let doc = Document::open(file.clone()).unwrap();

        assert_eq!(doc.path, file);
        assert_eq!(doc.text, "# Title\n\nhello");
    }

    #[test]
    fn open_of_binary_or_missing_is_none() {
        let dir = tempdir().unwrap();

        let bin = dir.path().join("image.png");
        fs::write(&bin, [0xFF, 0xD8, 0xFF, 0x00]).unwrap(); // invalid UTF-8
        assert!(Document::open(bin).is_none(), "binary file");

        assert!(
            Document::open(dir.path().join("nope.md")).is_none(),
            "missing file"
        );
    }

    #[test]
    fn open_of_empty_file_is_empty_text() {
        let dir = tempdir().unwrap();
        let file = dir.path().join("empty.md");
        fs::write(&file, "").unwrap();

        let doc = Document::open(file).unwrap();

        assert_eq!(doc.text, "");
    }
}
