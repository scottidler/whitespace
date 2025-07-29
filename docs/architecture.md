# Whitespace Removal Tool - Architecture & Design

## Overview

The `whitespace` tool is a high-performance CLI application designed to recursively remove trailing whitespace from files in a directory tree. It prioritizes speed, accuracy, and safety through comprehensive configuration options and dry-run capabilities.

## Core Requirements

### Functional Requirements
- **Recursive Processing**: Process all files in a directory tree starting from CWD (or specified directory)
- **Trailing Whitespace Removal**: Remove only insignificant trailing whitespace from code files
- **High Performance**: Utilize parallel processing (Rayon) for maximum speed
- **Dry Run Mode**: Show which files would be modified without making changes
- **Configurable Filtering**: Support path/filename exclusion patterns via configuration
- **Safe Operation**: Never modify files that shouldn't be touched

### Non-Functional Requirements
- **Performance**: Lightning-fast processing using `rayon::par_iter()`
- **Reliability**: Comprehensive error handling with `eyre`, no `unwrap()` calls
- **Testability**: Full unit test coverage proving correctness
- **Configurability**: YAML-based configuration with sensible defaults
- **Logging**: Structured logging to `~/.local/share/whitespace/logs/`

## Architecture Overview

```
┌─────────────────┐    ┌─────────────────┐    ┌─────────────────┐
│   CLI Parser    │────│  Configuration  │────│   File Walker   │
│   (clap)        │    │   (serde_yaml)  │    │   (walkdir)     │
└─────────────────┘    └─────────────────┘    └─────────────────┘
         │                       │                       │
         │                       │                       │
         ▼                       ▼                       ▼
┌─────────────────┐    ┌─────────────────┐    ┌─────────────────┐
│   Main Engine   │────│   Filter Chain  │────│ Parallel Proc.  │
│                 │    │                 │    │   (rayon)       │
└─────────────────┘    └─────────────────┘    └─────────────────┘
         │                       │                       │
         │                       │                       │
         ▼                       ▼                       ▼
┌─────────────────┐    ┌─────────────────┐    ┌─────────────────┐
│ Whitespace Proc │────│   File Writer   │────│    Logging      │
│                 │    │                 │    │   (env_logger)  │
└─────────────────┘    └─────────────────┘    └─────────────────┘
```

## Module Structure

### 1. CLI Module (`src/cli.rs`)
**Current State**: Basic structure exists with config and verbose flags.

**Enhancements Needed**:
```rust
pub struct Cli {
    /// Target directory (defaults to current working directory)
    #[arg(short, long, help = "Target directory to process")]
    pub directory: Option<PathBuf>,

    /// Path to config file
    #[arg(short, long, help = "Path to config file")]
    pub config: Option<PathBuf>,

    /// Perform dry run (show what would be changed)
    #[arg(short = 'n', long, help = "Dry run - show files that would be modified")]
    pub dry_run: bool,

    /// Enable verbose output
    #[arg(short, long, help = "Enable verbose output")]
    pub verbose: bool,

    /// Process files recursively
    #[arg(short, long, help = "Recurse into subdirectories", default_value = "true")]
    pub recursive: bool,

    /// Number of parallel threads (0 = auto-detect)
    #[arg(short = 'j', long, help = "Number of parallel threads", default_value = "0")]
    pub threads: usize,
}
```

### 2. Configuration Module (`src/config.rs`)
**Current State**: Basic YAML loading with fallback chain.

**Enhanced Configuration Structure**:
```rust
#[derive(Debug, Deserialize, Serialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct Config {
    /// File extensions to process
    pub file_extensions: Vec<String>,

    /// Path patterns to exclude (glob patterns)
    pub exclude_paths: Vec<String>,

    /// Filename patterns to exclude (glob patterns)
    pub exclude_files: Vec<String>,

    /// Binary file extensions to exclude (fast pre-filter)
    pub exclude_binary_extensions: Vec<String>,

    /// Binary file detection settings
    pub binary_detection: BinaryDetection,

    /// Processing settings
    pub processing: ProcessingSettings,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub struct BinaryDetection {
    /// Check for null bytes to detect binary files
    pub check_null_bytes: bool,

    /// Maximum bytes to read for binary detection
    pub sample_size: usize,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub struct ProcessingSettings {
    /// Maximum file size to process (in bytes)
    pub max_file_size: u64,
}
```

**Default Configuration** (`~/.config/whitespace/whitespace.yml`):
```yaml
# File extensions to process (empty = all text files)
file-extensions: []

# Path patterns to exclude (glob patterns)
exclude-paths:
  - ".git/**"
  - ".svn/**"
  - ".hg/**"
  - "node_modules/**"
  - "target/**"
  - "build/**"
  - "dist/**"
  - ".vscode/**"
  - ".idea/**"
  - "*.tmp/**"

# Filename patterns to exclude
exclude-files:
  - "*.min.js"
  - "*.min.css"
  - "*.bundle.*"
  - "*.lock"
  - "*.log"

# Binary file extensions to exclude (fast pre-filter)
exclude-binary-extensions:
  # Executables and libraries
  - "*.exe"
  - "*.dll"
  - "*.so"
  - "*.dylib"
  - "*.a"
  - "*.lib"
  - "*.bin"
  - "*.out"
  # Archives
  - "*.zip"
  - "*.tar"
  - "*.gz"
  - "*.bz2"
  - "*.xz"
  - "*.7z"
  - "*.rar"
  # Images
  - "*.jpg"
  - "*.jpeg"
  - "*.png"
  - "*.gif"
  - "*.bmp"
  - "*.ico"
  - "*.svg"
  - "*.webp"
  # Audio/Video
  - "*.mp3"
  - "*.mp4"
  - "*.avi"
  - "*.mov"
  - "*.wav"
  - "*.flac"
  # Documents
  - "*.pdf"
  - "*.doc"
  - "*.docx"
  - "*.xls"
  - "*.xlsx"
  - "*.ppt"
  - "*.pptx"
  # Other binary formats
  - "*.sqlite"
  - "*.db"
  - "*.dat"
  - "*.pyc"
  - "*.class"
  - "*.jar"

# Binary file detection
binary-detection:
  check-null-bytes: true
  sample-size: 8192

# Processing settings
processing:
  max-file-size: 104857600  # 100MB
```

### 3. File Walker Module (`src/walker.rs`)
**Purpose**: Efficiently traverse directory trees and collect files for processing.

**Key Components**:
- Use `walkdir` crate for directory traversal
- Apply filtering based on configuration
- Collect files into processing queue
- Handle symlinks and permissions gracefully

```rust
pub struct FileWalker {
    config: Arc<Config>,
}

impl FileWalker {
    pub fn new(config: Arc<Config>) -> Self { /* ... */ }

    pub fn collect_files(&self, root: &Path) -> Result<Vec<PathBuf>> { /* ... */ }

    fn should_process_file(&self, path: &Path) -> bool { /* ... */ }

    fn is_excluded_path(&self, path: &Path) -> bool { /* ... */ }

    fn is_excluded_file(&self, path: &Path) -> bool { /* ... */ }
}
```

### 4. File Filter Module (`src/filter.rs`)
**Purpose**: Determine which files should be processed based on various criteria.

**Filter Chain**:
1. **Symlink Filter**: Skip all symbolic links
2. **Path Exclusion Filter**: Check against `exclude_paths` patterns
3. **Filename Exclusion Filter**: Check against `exclude_files` patterns
4. **Binary Extension Filter**: Fast pre-filter to exclude known binary file extensions
5. **Binary Detection Filter**: Check for null bytes in file content - **SKIP** binary files completely
6. **Size Filter**: Skip files exceeding `max_file_size` (configurable, log when skipped)
7. **Process ALL remaining file types** (no inclusion filtering - only exclusion-based)

```rust
pub trait FileFilter {
    fn should_process(&self, path: &Path, metadata: &Metadata) -> Result<bool>;
}

pub struct FilterChain {
    filters: Vec<Box<dyn FileFilter>>,
}

pub struct PathExclusionFilter { /* ... */ }
pub struct FilenameExclusionFilter { /* ... */ }
pub struct BinaryExtensionFilter { /* ... */ }
pub struct BinaryDetectionFilter { /* ... */ }
pub struct SizeFilter { /* ... */ }
```

### 5. Whitespace Processor Module (`src/processor.rs`)
**Purpose**: Core logic for detecting and removing trailing whitespace.

**Key Features**:
- **WHITESPACE ONLY**: Remove trailing spaces and tabs, preserve all newlines exactly as-is
- Memory-efficient line-by-line processing
- Preserve file encoding and line endings (no modification of newline characters)
- Track changes for reporting
- Skip binary files completely

```rust
pub struct WhitespaceProcessor {
    config: Arc<Config>,
}

pub struct ProcessingResult {
    pub file_path: PathBuf,
    pub lines_modified: usize,
    pub bytes_saved: usize,
    pub had_changes: bool,
    pub error: Option<String>,
}

impl WhitespaceProcessor {
    pub fn process_file(&self, path: &Path, dry_run: bool) -> Result<ProcessingResult> { /* ... */ }

    /// Process content by removing only trailing whitespace (spaces/tabs), preserving newlines
    fn process_content(&self, content: &str) -> (String, usize, usize) { /* ... */ }

    fn is_binary_content(&self, content: &[u8]) -> bool { /* ... */ }
}
```

### 6. Parallel Engine Module (`src/engine.rs`)
**Purpose**: Orchestrate parallel processing using Rayon.

**Key Components**:
- Thread pool management
- Work distribution
- Progress reporting
- Result aggregation
- Error handling and recovery

```rust
pub struct ParallelEngine {
    config: Arc<Config>,
    thread_pool: ThreadPool,
}

pub struct ProcessingSummary {
    pub files_processed: usize,
    pub files_modified: usize,
    pub total_lines_modified: usize,
    pub total_bytes_saved: usize,
    pub errors: Vec<String>,
    pub duration: Duration,
}

impl ParallelEngine {
    pub fn new(config: Arc<Config>, num_threads: usize) -> Result<Self> { /* ... */ }

    pub fn process_files(&self, files: Vec<PathBuf>, dry_run: bool) -> Result<ProcessingSummary> { /* ... */ }
}
```

### 7. Enhanced Main Module (`src/main.rs`)
**Current State**: Basic scaffold with logging setup.

**Enhanced Flow**:
1. Parse CLI arguments
2. Load and validate configuration
3. Setup logging (INFO level default, RUST_LOG override, file-only)
4. Initialize parallel engine
5. Walk directory tree and collect files
6. Process files in parallel (skip binary files completely)
7. Console output: show only processed files or dry-run results
8. Generate and display summary
9. Exit with appropriate code

## Performance Optimizations

### 1. Parallel Processing Strategy
- Use `rayon::par_iter()` for file processing
- Optimal thread count: `num_cpus::get()` or user-specified
- Chunk files into batches for better load balancing
- Memory-mapped file I/O for large files

### 2. I/O Optimizations
- Read files in chunks to minimize memory usage
- Use buffered I/O for better performance
- Batch file system operations where possible
- Avoid unnecessary file metadata lookups

### 3. Memory Management
- Process files line-by-line to minimize memory footprint
- Use string slices where possible to avoid allocations
- Implement proper backpressure to prevent memory exhaustion
- Clean up resources promptly

## Error Handling Strategy

### 1. Error Handling with eyre
We'll use `eyre::Result<T>` throughout the codebase and `eyre::Context` for adding context to errors:

```rust
use eyre::{Context, Result, WrapErr};

// Example usage patterns:
fn process_file(path: &Path) -> Result<()> {
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("Failed to read file: {}", path.display()))?;

    // Processing logic...

    Ok(())
}

fn validate_config(config: &Config) -> Result<()> {
    if config.processing.max_file_size == 0 {
        eyre::bail!("max_file_size cannot be zero");
    }
    Ok(())
}
```

### 2. Error Recovery
- **Permission Issues**: Log and continue processing other files
- **File Changes During Processing**: Retry once, then skip if still changing
- **Symlinks**: Skip completely (don't follow)
- **Files Too Large**: Skip and log based on configurable max size
- Collect errors for final summary report
- Provide detailed error context using `eyre`

## Testing Strategy

### 1. Unit Tests
- **Configuration Loading**: Test YAML parsing, defaults, fallbacks
- **File Filtering**: Test all filter types with various inputs
- **Whitespace Processing**: Test edge cases, different line endings
- **Path Handling**: Test glob patterns, exclusions, symlinks

### 2. Integration Tests
- **End-to-End Workflows**: Full directory processing scenarios
- **Performance Tests**: Benchmark with large directory trees
- **Error Scenarios**: Test error handling and recovery
- **Configuration Variants**: Test different config combinations

### 3. Test Data Structure
```
tests/
├── fixtures/
│   ├── sample_project/          # Test directory tree
│   ├── config_variants/         # Different config files
│   └── edge_cases/              # Special test cases
├── unit/
│   ├── config_tests.rs
│   ├── filter_tests.rs
│   ├── processor_tests.rs
│   └── walker_tests.rs
├── integration/
│   ├── end_to_end_tests.rs
│   ├── performance_tests.rs
│   └── error_handling_tests.rs
└── common/
    └── test_utils.rs
```

### 4. Test Coverage Requirements
- Minimum 90% line coverage
- 100% coverage for critical paths (file modification logic)
- Edge case coverage for all public APIs
- Property-based testing for complex algorithms

## Security Considerations

### 1. File System Safety
- Validate all file paths to prevent directory traversal
- Check file permissions before modification
- Create backup files for critical operations (optional feature)
- Respect symlink boundaries

### 2. Resource Limits
- Implement maximum file size limits
- Prevent excessive memory usage
- Limit parallel thread count
- Timeout protection for long-running operations

### 3. Configuration Security
- Validate configuration file permissions
- Sanitize user-provided patterns
- Prevent code injection through glob patterns
- Secure temporary file handling

## Monitoring and Observability

### 1. Logging Strategy
- **Default Level**: INFO (overrideable via RUST_LOG environment variable)
- **Log Destination**: File only (`~/.local/share/whitespace/logs/whitespace.log`)
- **Console Output**: Only show files processed/would be processed (not log statements)
- **Log Levels Used**:
  - **DEBUG**: Copious detailed information throughout processing
  - **INFO**: File processing results, summary statistics
  - **WARN**: Skipped files, permission issues, minor problems
  - **ERROR**: Critical failures, unrecoverable errors
- **Performance Metrics**: Track processing times and throughput
- **Error Tracking**: Detailed error context and stack traces

### 2. Console Output Format
**File Processing Output**:
```
./src/main.rs (13,40-44, 55)
./src/config.rs (23, 67-69)
./README.md (12, 45)

67 files cleaned
```

**Dry-run Output**:
```
./src/main.rs (13,40-44, 55)
./src/config.rs (23, 67-69)
./README.md (12, 45)

67 files NOT cleaned
```

### 3. Summary Reports
```
Processing Summary:
  Files scanned: 1,234
  Files processed: 567
  Files modified: 89
  Binary files skipped: 45
  Symlinks skipped: 12
  Files too large: 3
  Lines cleaned: 456
  Bytes saved: 2.3 KB
  Duration: 1.2s
  Throughput: 1,028 files/sec

Errors encountered: 2
  - Permission denied: /root/secret.txt
  - File too large: /data/huge.log (500MB)
```

## Configuration Management

### 1. Configuration Precedence
**Order of precedence (highest to lowest)**:
1. **CLI arguments** (highest priority)
2. **Environment variables** (RUST_LOG for log level)
3. **`~/.config/whitespace/whitespace.yml`** (user config)
4. **Built-in defaults** (lowest priority)

**Note**: Local `./whitespace.yml` should be used as example/documentation only, not in precedence chain to avoid test contamination.

### 2. Thread Pool Configuration
- **Default**: Use all available CPU cores (`num_cpus::get()`)
- **Rationale**: Maximize parallelization for performance
- **Override**: CLI argument `--threads` allows user specification
- **Thread Pool**: Initialize Rayon thread pool with specified count

## Deployment and Distribution

### 1. Binary Distribution
- Single-file executable with no external dependencies
- Cross-platform support (Linux, macOS, Windows)
- Optimized release builds with LTO and panic=abort
- Minimal binary size through careful dependency selection

### 2. Installation Methods
- Direct binary download from releases
- Package manager integration (apt, brew, etc.)
- Cargo install from crates.io
- Docker container for isolated execution

### 3. Configuration Management
- Automatic config directory creation
- Sample configuration file generation
- Configuration validation and migration
- Environment variable overrides

## Future Enhancements

### 1. Advanced Features
- **Backup Mode**: Create backups before modification
- **Undo Functionality**: Reverse previous operations
- **Watch Mode**: Monitor directories for changes
- **Integration**: Git hooks, CI/CD pipeline integration

### 2. Performance Improvements
- **Memory Mapping**: Use mmap for very large files
- **Async I/O**: Non-blocking file operations
- **Caching**: Cache file metadata and binary detection results
- **SIMD**: Vectorized whitespace detection

### 3. User Experience
- **Interactive Mode**: Prompt for confirmation on sensitive operations
- **GUI Interface**: Optional graphical interface
- **Shell Completion**: Tab completion for CLI arguments
- **Configuration Wizard**: Interactive config file generation

## Implementation Phases

### Phase 1: Core Functionality
1. Enhanced CLI argument parsing
2. Comprehensive configuration system
3. File walker with basic filtering
4. Sequential whitespace processor
5. Basic logging and error handling

### Phase 2: Performance & Parallelization
1. Rayon-based parallel processing
2. Advanced file filtering
3. Memory optimizations
4. Performance benchmarking
5. Thread pool tuning

### Phase 3: Production Readiness
1. Comprehensive test suite
2. Error handling refinement
3. Documentation and examples
4. Security audit
5. Performance profiling

### Phase 4: Advanced Features
1. Dry-run mode enhancements
2. Progress reporting
3. Summary statistics
4. Configuration validation
5. Binary distribution

This architecture provides a solid foundation for building a high-performance, reliable, and maintainable whitespace removal tool that meets all specified requirements while remaining extensible for future enhancements.