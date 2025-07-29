use clap::Parser;
use colored::*;
use eyre::{Context, Result};
use log::info;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

mod cli;
mod config;
mod engine;
mod processor;
mod walker;

use cli::Cli;
use config::Config;
use engine::ParallelEngine;
use walker::FileWalker;

fn setup_logging() -> Result<()> {
    // Create log directory
    let log_dir = dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("whitespace")
        .join("logs");

    fs::create_dir_all(&log_dir)
        .context("Failed to create log directory")?;

    let log_file = log_dir.join("whitespace.log");

    // Setup env_logger with file output
    let target = Box::new(fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_file)
        .context("Failed to open log file")?);

    // Check for RUST_LOG environment variable, default to INFO
    let log_level = env::var("RUST_LOG").unwrap_or_else(|_| "info".to_string());

    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or(&log_level))
        .target(env_logger::Target::Pipe(target))
        .init();

    info!("Logging initialized, writing to: {}", log_file.display());
    Ok(())
}

fn format_line_numbers(lines: &[usize]) -> String {
    if lines.is_empty() {
        return String::new();
    }

    let mut ranges = Vec::new();
    let mut start = lines[0];
    let mut end = lines[0];

    for &line in &lines[1..] {
        if line == end + 1 {
            end = line;
        } else {
            if start == end {
                ranges.push(start.to_string());
            } else {
                ranges.push(format!("{}-{}", start, end));
            }
            start = line;
            end = line;
        }
    }

    // Add the last range
    if start == end {
        ranges.push(start.to_string());
    } else {
        ranges.push(format!("{}-{}", start, end));
    }

    format!(" ({})", ranges.join(","))
}

fn process_directory(
    target_dir: &Path,
    config: &Arc<Config>,
    cli: &Cli,
    engine: &ParallelEngine,
    processor: &processor::WhitespaceProcessor,
) -> Result<(usize, usize)> {
    info!("Processing directory: {}", target_dir.display());

    // Initialize file walker
    let walker = FileWalker::new(Arc::clone(config));

    // Collect files
    let files = walker.collect_files(target_dir, cli.recursive)
        .with_context(|| format!("Failed to collect files from {}", target_dir.display()))?;

    if files.is_empty() {
        return Ok((0, 0));
    }

    info!("Found {} files to process in {}", files.len(), target_dir.display());

    // Process files
    let summary = engine.process_files(files, cli.dry_run)
        .with_context(|| format!("Failed to process files in {}", target_dir.display()))?;

    // Display results to console for this directory
    let mut files_with_changes = 0;

    // Re-collect and process for display (always dry run for console output)
    let files = walker.collect_files(target_dir, cli.recursive)?;

    for file in &files {
        if let Ok(result) = processor.process_file(file, true) { // Always dry run for display
            if result.had_changes && result.error.is_none() {
                let line_info = format_line_numbers(&result.lines_modified);
                println!("{}{}",
                    file.display().to_string().blue(),
                    line_info.dimmed()
                );
                files_with_changes += 1;
            }
        }
    }

    Ok((files_with_changes, summary.files_modified))
}

fn run_application(cli: &Cli, config: &Config) -> Result<()> {
    info!("Starting whitespace removal application");

    let config = Arc::new((*config).clone());

    // Determine target directories
    let target_dirs: Vec<PathBuf> = if cli.directories.is_empty() {
        vec![PathBuf::from(".")]
    } else {
        cli.directories.clone()
    };

    info!("Target directories: {:?}", target_dirs);
    info!("Recursive: {}", cli.recursive);
    info!("Dry run: {}", cli.dry_run);
    info!("Threads: {}", cli.threads);

    // Initialize components
    let engine = ParallelEngine::new(Arc::clone(&config), cli.threads)
        .context("Failed to initialize parallel engine")?;
    let processor = processor::WhitespaceProcessor::new(Arc::clone(&config));

    let mut total_files_with_changes = 0;
    let mut total_files_modified = 0;
    let mut processed_dirs = 0;

    // Process each directory
    for target_dir in &target_dirs {
                if !target_dir.exists() {
            eprintln!("{} {} {}", "❌".red(), "Directory does not exist:".red(), target_dir.display().to_string().yellow());
            continue;
        }

        if !target_dir.is_dir() {
            eprintln!("{} {} {}", "❌".red(), "Not a directory:".red(), target_dir.display().to_string().yellow());
            continue;
        }

        match process_directory(target_dir, &config, cli, &engine, &processor) {
            Ok((files_with_changes, files_modified)) => {
                total_files_with_changes += files_with_changes;
                total_files_modified += files_modified;
                processed_dirs += 1;
            }
            Err(e) => {
                eprintln!("{} {} {}: {}", "⚠️".yellow(), "Error processing".red(), target_dir.display().to_string().yellow(), e);
            }
        }
    }

    if processed_dirs == 0 {
        println!("{}", "No valid directories found to process".yellow());
        return Ok(());
    }

        // Display summary with colors and icons
    if cli.dry_run {
        if total_files_with_changes == 0 {
            println!("\n{}", "✅ No trailing whitespace found".green().bold());
        } else {
            println!("\n{} {} {}",
                "📋".cyan(),
                format!("{}", total_files_with_changes).cyan().bold(),
                "files NOT cleaned".yellow()
            );
        }
    } else {
        if total_files_modified == 0 {
            println!("\n{}", "✅ No files needed cleaning".green().bold());
        } else {
            println!("\n{} {} {}",
                "🧹".green(),
                format!("{}", total_files_modified).cyan().bold(),
                "files cleaned".green().bold()
            );
        }
    }

    // Log summary information
    info!("Processing completed:");
    info!("  Directories processed: {}", processed_dirs);
    info!("  Files with changes: {}", total_files_with_changes);
    info!("  Files modified: {}", total_files_modified);

    Ok(())
}

fn main() -> Result<()> {
    // Setup logging first
    setup_logging()
        .context("Failed to setup logging")?;

    // Parse CLI arguments
    let cli = Cli::parse();

    // Load configuration
    let config = Config::load(cli.config.as_ref())
        .context("Failed to load configuration")?;

    info!("Starting with config from: {:?}", cli.config.as_ref().map(|p| p.display().to_string()).unwrap_or_else(|| "defaults".to_string()));

    // Run the main application logic
    run_application(&cli, &config)
        .context("Application failed")?;

    Ok(())
}
