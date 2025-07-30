use eyre::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct Config {
    /// File extensions to process (empty = all text files)
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

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub struct BinaryDetection {
    /// Check for null bytes to detect binary files
    pub check_null_bytes: bool,

    /// Maximum bytes to read for binary detection
    pub sample_size: usize,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub struct ProcessingSettings {
    /// Maximum file size to process (in bytes)
    pub max_file_size: u64,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            file_extensions: vec![],
            exclude_paths: vec![
                // Version control
                ".git/**".to_string(),
                ".svn/**".to_string(),
                ".hg/**".to_string(),
                // Dependencies and virtual environments
                "node_modules/**".to_string(),
                ".venv/**".to_string(),
                "venv/**".to_string(),
                ".env/**".to_string(),
                "env/**".to_string(),
                "__pycache__/**".to_string(),
                ".tox/**".to_string(),
                ".pytest_cache/**".to_string(),
                // Build outputs
                "target/**".to_string(),
                "build/**".to_string(),
                "dist/**".to_string(),
                "out/**".to_string(),
                "bin/**".to_string(),
                "obj/**".to_string(),
                // IDE and editor files
                ".vscode/**".to_string(),
                ".idea/**".to_string(),
                ".vs/**".to_string(),
                "*.tmp/**".to_string(),
                // Package managers
                ".npm/**".to_string(),
                ".yarn/**".to_string(),
                ".pnpm-store/**".to_string(),
                "vendor/**".to_string(),
            ],
            exclude_files: vec![
                "*.min.js".to_string(),
                "*.min.css".to_string(),
                "*.bundle.*".to_string(),
                "*.lock".to_string(),
                "*.log".to_string(),
            ],
            exclude_binary_extensions: vec![
                // Executables and libraries
                "*.exe".to_string(),
                "*.dll".to_string(),
                "*.so".to_string(),
                "*.dylib".to_string(),
                "*.a".to_string(),
                "*.lib".to_string(),
                "*.bin".to_string(),
                "*.out".to_string(),
                // Archives
                "*.zip".to_string(),
                "*.tar".to_string(),
                "*.gz".to_string(),
                "*.bz2".to_string(),
                "*.xz".to_string(),
                "*.7z".to_string(),
                "*.rar".to_string(),
                // Images
                "*.jpg".to_string(),
                "*.jpeg".to_string(),
                "*.png".to_string(),
                "*.gif".to_string(),
                "*.bmp".to_string(),
                "*.ico".to_string(),
                "*.svg".to_string(),
                "*.webp".to_string(),
                // Audio/Video
                "*.mp3".to_string(),
                "*.mp4".to_string(),
                "*.avi".to_string(),
                "*.mov".to_string(),
                "*.wav".to_string(),
                "*.flac".to_string(),
                // Documents
                "*.pdf".to_string(),
                "*.doc".to_string(),
                "*.docx".to_string(),
                "*.xls".to_string(),
                "*.xlsx".to_string(),
                "*.ppt".to_string(),
                "*.pptx".to_string(),
                // Other binary formats
                "*.sqlite".to_string(),
                "*.db".to_string(),
                "*.dat".to_string(),
                "*.pyc".to_string(),
                "*.class".to_string(),
                "*.jar".to_string(),
            ],
            binary_detection: BinaryDetection::default(),
            processing: ProcessingSettings::default(),
        }
    }
}

impl Default for BinaryDetection {
    fn default() -> Self {
        Self {
            check_null_bytes: true,
            sample_size: 8192,
        }
    }
}

impl Default for ProcessingSettings {
    fn default() -> Self {
        Self {
            max_file_size: 104857600, // 100MB
        }
    }
}

impl Config {
    /// Load configuration with fallback chain
    pub fn load(config_path: Option<&PathBuf>) -> Result<Self> {
        // If explicit config path provided, try to load it
        if let Some(path) = config_path {
            return Self::load_from_file(path)
                .context(format!("Failed to load config from {}", path.display()));
        }

        // Try primary location: ~/.config/whitespace/whitespace.yml
        if let Some(config_dir) = dirs::config_dir() {
            let project_name = env!("CARGO_PKG_NAME");
            let primary_config = config_dir.join(project_name).join(format!("{}.yml", project_name));
            if primary_config.exists() {
                match Self::load_from_file(&primary_config) {
                    Ok(config) => return Ok(config),
                    Err(e) => {
                        log::warn!("Failed to load config from {}: {}", primary_config.display(), e);
                    }
                }
            }
        }

        // No config file found, use defaults
        log::info!("No config file found, using defaults");
        Ok(Self::default())
    }

    fn load_from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let content = fs::read_to_string(&path)
            .context("Failed to read config file")?;

        let config: Self = serde_yaml::from_str(&content)
            .context("Failed to parse config file")?;

        log::info!("Loaded config from: {}", path.as_ref().display());
        Ok(config)
    }
}
