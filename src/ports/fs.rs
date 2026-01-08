use eyre::{Context, Result};
use std::cell::RefCell;
use std::collections::HashMap;
use std::fs::Metadata;
use std::path::{Path, PathBuf};

/// Trait for filesystem operations, enabling dependency injection for testing.
pub trait FileSystem: Send + Sync {
    fn read(&self, path: &Path) -> Result<Vec<u8>>;
    fn write(&self, path: &Path, content: &[u8]) -> Result<()>;
    fn metadata(&self, path: &Path) -> Result<FsMetadata>;
    fn is_dir(&self, path: &Path) -> bool;
    fn is_file(&self, path: &Path) -> bool;
    fn is_symlink(&self, path: &Path) -> bool;
    fn exists(&self, path: &Path) -> bool;
}

/// Simplified metadata struct for our needs.
#[derive(Debug, Clone)]
pub struct FsMetadata {
    pub len: u64,
    pub is_file: bool,
    pub is_dir: bool,
}

impl From<Metadata> for FsMetadata {
    fn from(m: Metadata) -> Self {
        Self {
            len: m.len(),
            is_file: m.is_file(),
            is_dir: m.is_dir(),
        }
    }
}

/// Real filesystem implementation.
#[derive(Debug, Clone, Default)]
pub struct RealFs;

impl FileSystem for RealFs {
    fn read(&self, path: &Path) -> Result<Vec<u8>> {
        std::fs::read(path).with_context(|| format!("Failed to read file: {}", path.display()))
    }

    fn write(&self, path: &Path, content: &[u8]) -> Result<()> {
        std::fs::write(path, content).with_context(|| format!("Failed to write file: {}", path.display()))
    }

    fn metadata(&self, path: &Path) -> Result<FsMetadata> {
        std::fs::metadata(path)
            .map(FsMetadata::from)
            .with_context(|| format!("Failed to read metadata: {}", path.display()))
    }

    fn is_dir(&self, path: &Path) -> bool {
        path.is_dir()
    }

    fn is_file(&self, path: &Path) -> bool {
        path.is_file()
    }

    fn is_symlink(&self, path: &Path) -> bool {
        path.is_symlink()
    }

    fn exists(&self, path: &Path) -> bool {
        path.exists()
    }
}

/// In-memory filesystem for testing.
#[derive(Debug, Default)]
pub struct MemFs {
    files: RefCell<HashMap<PathBuf, Vec<u8>>>,
}

impl MemFs {
    pub fn new() -> Self {
        Self {
            files: RefCell::new(HashMap::new()),
        }
    }

    pub fn with_file<P: Into<PathBuf>>(self, path: P, content: &[u8]) -> Self {
        self.files.borrow_mut().insert(path.into(), content.to_vec());
        self
    }

    pub fn get_content(&self, path: &Path) -> Option<Vec<u8>> {
        self.files.borrow().get(path).cloned()
    }
}

impl FileSystem for MemFs {
    fn read(&self, path: &Path) -> Result<Vec<u8>> {
        self.files
            .borrow()
            .get(path)
            .cloned()
            .ok_or_else(|| eyre::eyre!("File not found: {}", path.display()))
    }

    fn write(&self, path: &Path, content: &[u8]) -> Result<()> {
        self.files.borrow_mut().insert(path.to_path_buf(), content.to_vec());
        Ok(())
    }

    fn metadata(&self, path: &Path) -> Result<FsMetadata> {
        let files = self.files.borrow();
        if let Some(content) = files.get(path) {
            Ok(FsMetadata {
                len: content.len() as u64,
                is_file: true,
                is_dir: false,
            })
        } else {
            Err(eyre::eyre!("File not found: {}", path.display()))
        }
    }

    fn is_dir(&self, _path: &Path) -> bool {
        false
    }

    fn is_file(&self, path: &Path) -> bool {
        self.files.borrow().contains_key(path)
    }

    fn is_symlink(&self, _path: &Path) -> bool {
        false
    }

    fn exists(&self, path: &Path) -> bool {
        self.files.borrow().contains_key(path)
    }
}

// MemFs is not Sync due to RefCell, but we can make it work for single-threaded tests
unsafe impl Sync for MemFs {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_memfs_read_write() {
        let fs = MemFs::new().with_file("test.txt", b"hello world");

        let content = fs.read(Path::new("test.txt")).unwrap();
        assert_eq!(content, b"hello world");

        fs.write(Path::new("test.txt"), b"updated").unwrap();
        let content = fs.read(Path::new("test.txt")).unwrap();
        assert_eq!(content, b"updated");
    }

    #[test]
    fn test_memfs_metadata() {
        let fs = MemFs::new().with_file("test.txt", b"hello");

        let meta = fs.metadata(Path::new("test.txt")).unwrap();
        assert_eq!(meta.len, 5);
        assert!(meta.is_file);
        assert!(!meta.is_dir);
    }

    #[test]
    fn test_memfs_not_found() {
        let fs = MemFs::new();
        assert!(fs.read(Path::new("missing.txt")).is_err());
    }

    #[test]
    fn test_realfs_exists() {
        let fs = RealFs;
        assert!(fs.exists(Path::new("Cargo.toml")));
        assert!(!fs.exists(Path::new("nonexistent-file-12345.txt")));
    }
}
