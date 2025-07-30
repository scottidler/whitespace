use crate::config::Config;
use eyre::Result;
use log::{debug, warn};
use std::fs;
use std::path::Path;
use std::sync::Arc;

pub struct WhitespaceProcessor {
    config: Arc<Config>,
}

#[derive(Debug, Clone)]
pub struct ProcessingResult {
    pub lines_modified: Vec<usize>,
    pub had_changes: bool,
    pub error: Option<String>,
}

impl WhitespaceProcessor {
    pub fn new(config: Arc<Config>) -> Self {
        Self { config }
    }

    pub fn process_file(&self, path: &Path, dry_run: bool) -> Result<ProcessingResult> {
        debug!("Processing file: {}", path.display());

        // Read file content
        let content = match fs::read(path) {
            Ok(bytes) => bytes,
            Err(e) => {
                let error_msg = format!("Failed to read file: {}", e);
                warn!("{}: {}", error_msg, path.display());
                            return Ok(ProcessingResult {
                lines_modified: vec![],
                had_changes: false,
                error: Some(error_msg),
            });
            }
        };

        // Check if file is binary
        if self.is_binary_content(&content) {
            debug!("Skipping binary file: {}", path.display());
            return Ok(ProcessingResult {
                lines_modified: vec![],
                had_changes: false,
                error: Some("Binary file detected".to_string()),
            });
        }

        // Convert to string
        let content_str = match String::from_utf8(content) {
            Ok(s) => s,
            Err(_) => {
                debug!("Skipping file with invalid UTF-8: {}", path.display());
                return Ok(ProcessingResult {
                    lines_modified: vec![],
                    had_changes: false,
                    error: Some("Invalid UTF-8 encoding".to_string()),
                });
            }
        };

        // Process content
        let (processed_content, modified_lines, _) = self.process_content(&content_str);
        let had_changes = !modified_lines.is_empty();

        // Write back if not dry run and there are changes
        if !dry_run && had_changes {
            if let Err(e) = fs::write(path, &processed_content) {
                let error_msg = format!("Failed to write file: {}", e);
                warn!("{}: {}", error_msg, path.display());
                return Ok(ProcessingResult {
                    lines_modified: modified_lines,
                    had_changes,
                    error: Some(error_msg),
                });
            }
            debug!("Wrote cleaned file: {}", path.display());
        }

                if had_changes {
            debug!(
                "File processed: {} lines modified",
                modified_lines.len()
            );
        }

        Ok(ProcessingResult {
            lines_modified: modified_lines,
            had_changes,
            error: None,
        })
    }

    fn process_content(&self, content: &str) -> (String, Vec<usize>, usize) {
        let mut processed_lines = Vec::new();
        let mut modified_line_numbers = Vec::new();
        let mut total_bytes_saved = 0;

        for (line_num, line) in content.lines().enumerate() {
            let original_len = line.len();
            let trimmed_line = line.trim_end();
            let trimmed_len = trimmed_line.len();

            if trimmed_len < original_len {
                // Line had trailing whitespace
                modified_line_numbers.push(line_num + 1); // 1-based line numbers
                total_bytes_saved += original_len - trimmed_len;
                debug!("Line {}: removed {} trailing bytes", line_num + 1, original_len - trimmed_len);
            }

            processed_lines.push(trimmed_line);
        }

        // Reconstruct content preserving original line endings
        let processed_content = if content.ends_with('\n') {
            format!("{}\n", processed_lines.join("\n"))
        } else {
            processed_lines.join("\n")
        };

        (processed_content, modified_line_numbers, total_bytes_saved)
    }

    fn is_binary_content(&self, content: &[u8]) -> bool {
        if !self.config.binary_detection.check_null_bytes {
            return false;
        }

        let sample_size = self.config.binary_detection.sample_size.min(content.len());
        let sample = &content[..sample_size];

        // Check for null bytes
        sample.contains(&0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn create_test_config() -> Arc<Config> {
        Arc::new(Config::default())
    }

    #[test]
    fn test_process_content_trailing_spaces() {
        let config = create_test_config();
        let processor = WhitespaceProcessor::new(config);

        let content = "line1   \nline2\t\t\nline3\n";
        let (processed, modified_lines, bytes_saved) = processor.process_content(content);

        assert_eq!(processed, "line1\nline2\nline3\n");
        assert_eq!(modified_lines, vec![1, 2]);
        assert_eq!(bytes_saved, 5); // 3 spaces + 2 tabs
    }

    #[test]
    fn test_process_content_no_trailing_newline() {
        let config = create_test_config();
        let processor = WhitespaceProcessor::new(config);

        let content = "line1   \nline2\t\t";
        let (processed, modified_lines, bytes_saved) = processor.process_content(content);

        assert_eq!(processed, "line1\nline2");
        assert_eq!(modified_lines, vec![1, 2]);
        assert_eq!(bytes_saved, 5);
    }

    #[test]
    fn test_process_content_no_changes() {
        let config = create_test_config();
        let processor = WhitespaceProcessor::new(config);

        let content = "line1\nline2\nline3\n";
        let (processed, modified_lines, bytes_saved) = processor.process_content(content);

        assert_eq!(processed, content);
        assert_eq!(modified_lines.len(), 0);
        assert_eq!(bytes_saved, 0);
    }

    #[test]
    fn test_binary_detection() {
        let config = create_test_config();
        let processor = WhitespaceProcessor::new(config);

        let text_content = b"Hello, world!\n";
        let binary_content = b"Hello\0world\n";

        assert!(!processor.is_binary_content(text_content));
        assert!(processor.is_binary_content(binary_content));
    }

    #[test]
    fn test_process_file_dry_run() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("test.txt");

        let original_content = "line1   \nline2\t\t\n";
        fs::write(&test_file, original_content).unwrap();

        let config = create_test_config();
        let processor = WhitespaceProcessor::new(config);

        let result = processor.process_file(&test_file, true).unwrap();

        assert!(result.had_changes);
        assert_eq!(result.lines_modified, vec![1, 2]);
        assert!(result.error.is_none());

        // File should not be modified in dry run
        let file_content = fs::read_to_string(&test_file).unwrap();
        assert_eq!(file_content, original_content);
    }

    #[test]
    fn test_process_file_actual_modification() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("test.txt");

        let original_content = "line1   \nline2\t\t\n";
        fs::write(&test_file, original_content).unwrap();

        let config = create_test_config();
        let processor = WhitespaceProcessor::new(config);

        let result = processor.process_file(&test_file, false).unwrap();

        assert!(result.had_changes);
        assert_eq!(result.lines_modified, vec![1, 2]);
        assert!(result.error.is_none());

        // File should be modified
        let file_content = fs::read_to_string(&test_file).unwrap();
        assert_eq!(file_content, "line1\nline2\n");
    }
}