use crate::config::Config;
use crate::ports::fs::FileSystem;
use eyre::Result;
use log::{debug, warn};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use walkdir::WalkDir;

pub struct FileWalker<F: FileSystem> {
    config: Arc<Config>,
    fs: Arc<F>,
}

impl<F: FileSystem> FileWalker<F> {
    pub fn new(config: Arc<Config>, fs: Arc<F>) -> Self {
        Self { config, fs }
    }

    pub fn collect_files(&self, root: &Path, recursive: bool) -> Result<Vec<PathBuf>> {
        debug!("Starting file collection from: {}", root.display());

        let mut files = Vec::new();
        let walker = if recursive { WalkDir::new(root) } else { WalkDir::new(root).max_depth(1) };

        for entry in walker.into_iter() {
            match entry {
                Ok(entry) => {
                    let path = entry.path();

                    // Skip directories
                    if self.fs.is_dir(path) {
                        continue;
                    }

                    // Skip symlinks
                    if self.fs.is_symlink(path) {
                        debug!("Skipping symlink: {}", path.display());
                        continue;
                    }

                    if self.should_process_file(path) {
                        debug!("Adding file for processing: {}", path.display());
                        files.push(path.to_path_buf());
                    } else {
                        debug!("Filtering out file: {}", path.display());
                    }
                }
                Err(e) => {
                    warn!("Error accessing path during walk: {}", e);
                }
            }
        }

        debug!("Collected {} files for processing", files.len());
        Ok(files)
    }

    fn should_process_file(&self, path: &Path) -> bool {
        // Check if path matches exclusion patterns
        if self.is_excluded_path(path) {
            debug!("Path excluded by exclude-paths pattern: {}", path.display());
            return false;
        }

        // Check if filename matches exclusion patterns
        if self.is_excluded_file(path) {
            debug!("File excluded by exclude-files pattern: {}", path.display());
            return false;
        }

        // Check if file has binary extension
        if self.has_binary_extension(path) {
            debug!("File excluded by binary extension: {}", path.display());
            return false;
        }

        // Check file size using FileSystem trait
        match self.fs.metadata(path) {
            Ok(metadata) => {
                if metadata.len > self.config.processing.max_file_size {
                    debug!("File too large ({}): {}", metadata.len, path.display());
                    return false;
                }
            }
            Err(_) => {
                warn!("Could not read metadata for: {}", path.display());
                return false;
            }
        }

        true
    }

    fn is_excluded_path(&self, path: &Path) -> bool {
        let path_str = path.to_string_lossy();

        for pattern in &self.config.exclude_paths {
            if let Ok(glob_pattern) = glob::Pattern::new(pattern)
                && glob_pattern.matches(&path_str)
            {
                return true;
            }

            // Also check if any parent directory matches the pattern
            for ancestor in path.ancestors() {
                let ancestor_str = ancestor.to_string_lossy();
                if let Ok(glob_pattern) = glob::Pattern::new(pattern)
                    && glob_pattern.matches(&ancestor_str)
                {
                    return true;
                }

                // Special handling for patterns like ".git/**"
                if pattern.ends_with("/**") {
                    let base_pattern = &pattern[..pattern.len() - 3]; // Remove "/**"
                    if ancestor.file_name().is_some_and(|name| {
                        glob::Pattern::new(base_pattern)
                            .is_ok_and(|base_glob| base_glob.matches(&name.to_string_lossy()))
                    }) {
                        return true;
                    }
                }
            }
        }

        false
    }

    fn is_excluded_file(&self, path: &Path) -> bool {
        if let Some(filename) = path.file_name() {
            let filename_str = filename.to_string_lossy();

            for pattern in &self.config.exclude_files {
                if let Ok(glob_pattern) = glob::Pattern::new(pattern)
                    && glob_pattern.matches(&filename_str)
                {
                    return true;
                }
            }
        }

        false
    }

    fn has_binary_extension(&self, path: &Path) -> bool {
        if let Some(filename) = path.file_name() {
            let filename_str = filename.to_string_lossy();

            for pattern in &self.config.exclude_binary_extensions {
                if let Ok(glob_pattern) = glob::Pattern::new(pattern)
                    && glob_pattern.matches(&filename_str)
                {
                    return true;
                }
            }
        }

        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ports::fs::RealFs;
    use std::fs;
    use tempfile::TempDir;

    fn create_test_config() -> Arc<Config> {
        Arc::new(Config::default())
    }

    #[test]
    fn test_collect_files_recursive() {
        let temp_dir = TempDir::new().unwrap();
        let root = temp_dir.path();

        // Create test files
        fs::write(root.join("test.txt"), "content").unwrap();
        fs::create_dir(root.join("subdir")).unwrap();
        fs::write(root.join("subdir").join("nested.rs"), "content").unwrap();

        let config = create_test_config();
        let real_fs = Arc::new(RealFs);
        let walker = FileWalker::new(config, real_fs);

        let files = walker.collect_files(root, true).unwrap();
        assert_eq!(files.len(), 2);

        let filenames: Vec<String> = files
            .iter()
            .map(|p| p.file_name().unwrap().to_string_lossy().to_string())
            .collect();

        assert!(filenames.contains(&"test.txt".to_string()));
        assert!(filenames.contains(&"nested.rs".to_string()));
    }

    #[test]
    fn test_collect_files_non_recursive() {
        let temp_dir = TempDir::new().unwrap();
        let root = temp_dir.path();

        // Create test files
        fs::write(root.join("test.txt"), "content").unwrap();
        fs::create_dir(root.join("subdir")).unwrap();
        fs::write(root.join("subdir").join("nested.rs"), "content").unwrap();

        let config = create_test_config();
        let real_fs = Arc::new(RealFs);
        let walker = FileWalker::new(config, real_fs);

        let files = walker.collect_files(root, false).unwrap();
        assert_eq!(files.len(), 1);

        let filename = files[0].file_name().unwrap().to_string_lossy();
        assert_eq!(filename, "test.txt");
    }

    #[test]
    fn test_binary_extension_filtering() {
        let temp_dir = TempDir::new().unwrap();
        let root = temp_dir.path();

        // Create test files
        fs::write(root.join("test.txt"), "content").unwrap();
        fs::write(root.join("binary.exe"), "binary").unwrap();
        fs::write(root.join("library.so"), "binary").unwrap();

        let config = create_test_config();
        let real_fs = Arc::new(RealFs);
        let walker = FileWalker::new(config, real_fs);

        let files = walker.collect_files(root, false).unwrap();
        assert_eq!(files.len(), 1);

        let filename = files[0].file_name().unwrap().to_string_lossy();
        assert_eq!(filename, "test.txt");
    }

    #[test]
    fn test_exclude_paths_filtering() {
        let temp_dir = TempDir::new().unwrap();
        let root = temp_dir.path();

        // Create test files
        fs::write(root.join("test.txt"), "content").unwrap();
        fs::create_dir_all(root.join(".git")).unwrap();
        fs::write(root.join(".git").join("config"), "git config").unwrap();

        let config = create_test_config();
        let real_fs = Arc::new(RealFs);
        let walker = FileWalker::new(config, real_fs);

        let files = walker.collect_files(root, true).unwrap();
        assert_eq!(files.len(), 1);

        let filename = files[0].file_name().unwrap().to_string_lossy();
        assert_eq!(filename, "test.txt");
    }
}
