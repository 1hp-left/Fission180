//! Stage on the destination filesystem, flush, then atomically replace the name.
use crate::{Document, Result};
use std::{io::Write, path::Path};

pub fn save(document: &Document, path: &Path) -> Result<()> {
    staged_write(path, |file| {
        file.write_all(document.to_json()?.as_bytes())?;
        Ok(())
    })
}

pub fn staged_write(
    path: &Path,
    write: impl FnOnce(&mut std::fs::File) -> Result<()>,
) -> Result<()> {
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let mut staged = tempfile::NamedTempFile::new_in(parent)?;
    write(staged.as_file_mut())?;
    staged.as_file_mut().sync_all()?;
    // tempfile uses platform replacement primitives (MoveFileExW on Windows).
    staged.persist(path).map_err(|e| e.error)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn failed_stage_and_invalid_document_preserve_original_bytes() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("document.ofrust");
        std::fs::write(&path, b"original").unwrap();
        assert!(
            staged_write(&path, |f| {
                f.write_all(b"partial")?;
                Err(crate::Error::Invalid("injected write failure".into()))
            })
            .is_err()
        );
        assert_eq!(std::fs::read(&path).unwrap(), b"original");
        let invalid = Document {
            version: 99,
            ..Default::default()
        };
        assert!(save(&invalid, &path).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), b"original");
        save(&Document::default(), &path).unwrap();
        assert_eq!(
            Document::from_json(&std::fs::read_to_string(&path).unwrap()).unwrap(),
            Document::default()
        );
    }
    #[cfg(windows)]
    #[test]
    fn failed_windows_replacement_preserves_locked_file() {
        use std::os::windows::fs::OpenOptionsExt;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("locked.ofrust");
        std::fs::write(&path, b"original").unwrap();
        let _lock = std::fs::OpenOptions::new()
            .read(true)
            .share_mode(1)
            .open(&path)
            .unwrap();
        assert!(save(&Document::default(), &path).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), b"original");
    }
}
