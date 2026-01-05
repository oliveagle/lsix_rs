mod filename;
mod image_proc;
mod terminal;
mod filter;

use anyhow::{Context, Result};
use clap::Parser;
use image_proc::{expand_directories, validate_images_concurrent, process_images_concurrent, ImageConfig};
use filename::FilenameMode;
use filter::{FilterConfig, parse_orientation, parse_file_size};
use std::process::Command;
use std::io::{self, Write};

/// lsix: like ls, but for images.
/// Shows thumbnails of images with titles directly in terminal.
#[derive(Parser, Debug)]
#[command(name = "lsix")]
#[command(author = "hackerb9")]
#[command(version = "2.0.0")]
#[command(about = "Like ls, but for images - displays thumbnails in SIXEL-capable terminals")]
struct Args {
    /// Image files or directories to display
    #[arg(name = "FILES")]
    files: Vec<String>,

    /// Display mode for filenames
    #[arg(short, long, default_value = "short")]
    #[arg(value_parser = clap::builder::PossibleValuesParser::new(["short", "long"]))]
    mode: String,

    // Size filters
    /// Minimum image width in pixels
    #[arg(long)]
    min_width: Option<u32>,

    /// Maximum image width in pixels
    #[arg(long)]
    max_width: Option<u32>,

    /// Minimum image height in pixels
    #[arg(long)]
    min_height: Option<u32>,

    /// Maximum image height in pixels
    #[arg(long)]
    max_height: Option<u32>,

    /// Minimum file size (e.g., 100K, 1M, 1G)
    #[arg(long)]
    min_file_size: Option<String>,

    /// Maximum file size (e.g., 100K, 1M, 1G)
    #[arg(long)]
    max_file_size: Option<String>,

    // Color filters
    /// Minimum brightness (0.0 to 1.0)
    #[arg(long)]
    min_brightness: Option<f32>,

    /// Maximum brightness (0.0 to 1.0)
    #[arg(long)]
    max_brightness: Option<f32>,

    // Orientation filter
    /// Filter by orientation: landscape, portrait, or square
    #[arg(long)]
    orientation: Option<String>,
}

/// Cleanup handler to stop SIXEL and reset terminal
fn cleanup() {
    // Send escape sequence to stop SIXEL
    eprint!("\x1b\\");
    io::stderr().flush().ok();

    // Reset terminal to show characters
    let _ = Command::new("stty").arg("echo").status();
}

/// Setup cleanup handlers
fn setup_cleanup() -> Result<()> {
    // Disable echo at start
    let _ = Command::new("stty").arg("-echo").status();
    Ok(())
}

/// Main function
fn main() -> Result<()> {
    let args = Args::parse();

    // Setup terminal and cleanup
    setup_cleanup()?;

    // Determine filename mode from command line argument
    let filename_mode = match args.mode.as_str() {
        "long" => FilenameMode::Long,
        _ => FilenameMode::Short,
    };

    // Build filter config from command line arguments
    let filter_config = FilterConfig {
        min_width: args.min_width,
        max_width: args.max_width,
        min_height: args.min_height,
        max_height: args.max_height,
        min_file_size: args.min_file_size.and_then(|s| parse_file_size(&s).ok()),
        max_file_size: args.max_file_size.and_then(|s| parse_file_size(&s).ok()),
        min_brightness: args.min_brightness,
        max_brightness: args.max_brightness,
        orientation: args.orientation.and_then(|s| parse_orientation(&s).ok()),
    };

    // Auto-detect terminal capabilities (very fast now)
    let term_config = terminal::autodetect()
        .context("Terminal auto-detection failed")?;

    // Get list of image files
    let image_paths = if args.files.is_empty() {
        // No arguments - find images in current directory
        filename::find_image_files()
    } else {
        // Arguments provided - expand any directories
        let expanded = expand_directories(&args.files);
        expanded
    };

    if image_paths.is_empty() {
        eprintln!("No image files found.");
        cleanup();
        return Ok(());
    }

    // Validate and process images concurrently with filtering
    let images = validate_images_concurrent(&image_paths, !args.files.is_empty(), filename_mode, &filter_config);

    if images.is_empty() {
        eprintln!("No valid images to display.");
        cleanup();
        return Ok(());
    }

    // Create image configuration
    let img_config = ImageConfig::from_terminal_width(
        term_config.width,
        term_config.num_colors,
        &term_config.background,
        &term_config.foreground,
    );

    // Process and display images
    process_images_concurrent(images, &img_config)?;

    // Skip the waiting part - just cleanup and exit
    // The original script waits for terminal response, but it's not strictly necessary

    // Cleanup
    cleanup();

    Ok(())
}
