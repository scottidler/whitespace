use crate::config::Config;
use crate::processor::{ProcessingResult, WhitespaceProcessor};
use eyre::Result;
use log::{debug, info, warn};
use rayon::prelude::*;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

pub struct ParallelEngine {
    processor: WhitespaceProcessor,
}

#[derive(Debug)]
pub struct ProcessingSummary {
    pub files_processed: usize,
    pub files_modified: usize,
    pub files_with_errors: usize,
    pub duration: Duration,
}

#[derive(Debug)]
pub struct ProcessingResults {
    pub file_results: Vec<(PathBuf, ProcessingResult)>,
}

impl ParallelEngine {
    pub fn new(config: Arc<Config>, num_threads: usize) -> Result<Self> {
        let thread_count = num_threads;

        debug!("Initializing thread pool with {} threads", thread_count);

        // Only set thread pool if not already initialized (for tests)
        if let Err(_) = rayon::ThreadPoolBuilder::new()
            .num_threads(thread_count)
            .build_global()
        {
            debug!("Thread pool already initialized, using existing configuration");
        }

        let processor = WhitespaceProcessor::new(Arc::clone(&config));

        Ok(Self { processor })
    }

    pub fn process_files_with_results(&self, files: Vec<PathBuf>, dry_run: bool) -> Result<ProcessingResults> {
        let start_time = Instant::now();

        info!("Starting parallel processing of {} files", files.len());
        debug!("Dry run mode: {}", dry_run);

                // Process files in parallel
        let file_results: Vec<(PathBuf, ProcessingResult)> = files
            .par_iter()
            .map(|path| {
                let result = self.processor.process_file(path, dry_run)
                    .unwrap_or_else(|e| {
                        warn!("Failed to process {}: {}", path.display(), e);
                        ProcessingResult {
                            lines_modified: vec![],
                            had_changes: false,
                            error: Some(format!("Processing failed: {}", e)),
                        }
                    });
                (path.clone(), result)
            })
            .collect();

        let duration = start_time.elapsed();

        // Aggregate results
        let results: Vec<ProcessingResult> = file_results.iter().map(|(_, result)| result.clone()).collect();
        let summary = self.aggregate_results(results, duration);

        info!(
            "Processing completed: {} files processed, {} modified, {} errors in {:?}",
            summary.files_processed,
            summary.files_modified,
            summary.files_with_errors,
            summary.duration
        );

        Ok(ProcessingResults {
            file_results,
        })
    }



    fn aggregate_results(&self, results: Vec<ProcessingResult>, duration: Duration) -> ProcessingSummary {
        let mut files_processed = 0;
        let mut files_modified = 0;
        let mut files_with_errors = 0;


        for result in results {
            files_processed += 1;

                        if result.error.is_some() {
                files_with_errors += 1;
            } else if result.had_changes {
                files_modified += 1;
            }
        }

        ProcessingSummary {
            files_processed,
            files_modified,
            files_with_errors,
            duration,
        }
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
    fn test_parallel_processing() {
        let temp_dir = TempDir::new().unwrap();
        let root = temp_dir.path();

        // Create test files with trailing whitespace
        let files = vec![
            root.join("file1.txt"),
            root.join("file2.txt"),
            root.join("file3.txt"),
        ];

        for (i, file) in files.iter().enumerate() {
            fs::write(file, format!("line1   \nline2\t\t\nline{}\n", i + 1)).unwrap();
        }

        let config = create_test_config();
        let engine = ParallelEngine::new(config, 2).unwrap();

        let results = engine.process_files_with_results(files.clone(), false).unwrap();

        let files_modified = results.file_results.iter()
            .filter(|(_, result)| result.had_changes && result.error.is_none())
            .count();

        assert_eq!(results.file_results.len(), 3);
        assert_eq!(files_modified, 3);
        // Files were successfully processed

        // Verify files were actually modified
        for file in &files {
            let content = fs::read_to_string(file).unwrap();
            assert!(!content.contains("   "));
            assert!(!content.contains("\t\t"));
        }
    }

    #[test]
    fn test_dry_run_processing() {
        let temp_dir = TempDir::new().unwrap();
        let root = temp_dir.path();

        let test_file = root.join("test.txt");
        let original_content = "line1   \nline2\t\t\n";
        fs::write(&test_file, original_content).unwrap();

        let config = create_test_config();
        let engine = ParallelEngine::new(config, 1).unwrap();

        let results = engine.process_files_with_results(vec![test_file.clone()], true).unwrap();

        let files_modified = results.file_results.iter()
            .filter(|(_, result)| result.had_changes && result.error.is_none())
            .count();

        assert_eq!(results.file_results.len(), 1);
        assert_eq!(files_modified, 1);
        // File was processed successfully

        // File should not be modified in dry run
        let content = fs::read_to_string(&test_file).unwrap();
        assert_eq!(content, original_content);
    }

    #[test]
    fn test_binary_file_handling() {
        let temp_dir = TempDir::new().unwrap();
        let root = temp_dir.path();

        let binary_file = root.join("binary.dat");
        fs::write(&binary_file, b"binary\0content").unwrap();

        let config = create_test_config();
        let engine = ParallelEngine::new(config, 1).unwrap();

        let results = engine.process_files_with_results(vec![binary_file], false).unwrap();

        let files_modified = results.file_results.iter()
            .filter(|(_, result)| result.had_changes && result.error.is_none())
            .count();

        assert_eq!(results.file_results.len(), 1);
        assert_eq!(files_modified, 0);
        // Binary file was detected and skipped
    }
}