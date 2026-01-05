use anyhow::{Context, Result};
use rayon::prelude::*;
use std::process::{Command, Stdio};
use std::io::{self, Write};
use std::sync::OnceLock;

// Import filename types
use crate::filename::FilenameMode;

/// ImageMagick command detection result
static IMAGEMAGICK_MODE: OnceLock<ImageMagickMode> = OnceLock::new();

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ImageMagickMode {
    /// ImageMagick 7.x - use "magick montage", "magick convert"
    V7,
    /// ImageMagick 6.x - use "montage", "convert"
    V6,
}

/// Detect ImageMagick version and command style
fn detect_imagemagick() -> ImageMagickMode {
    // Check for ImageMagick 7.x first (magick command)
    if Command::new("magick")
        .arg("-version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
    {
        return ImageMagickMode::V7;
    }

    // Check for ImageMagick 6.x (montage command)
    if Command::new("montage")
        .arg("-version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
    {
        return ImageMagickMode::V6;
    }

    // Default to V7 (will fail with clear error message)
    ImageMagickMode::V7
}

/// Get the detected ImageMagick mode
fn get_imagemagick_mode() -> ImageMagickMode {
    *IMAGEMAGICK_MODE.get_or_init(|| detect_imagemagick())
}

/// Configuration for image processing
#[derive(Debug, Clone)]
pub struct ImageConfig {
    pub tile_width: u32,
    pub tile_height: u32,
    pub tile_xspace: u32,
    pub tile_yspace: u32,
    pub num_tiles_per_row: u32,
    pub num_colors: u32,
    pub background: String,
    pub foreground: String,
    pub font_family: Option<String>,
    pub font_size: u32,
    pub shadow: bool,
}

impl ImageConfig {
    /// Create a new ImageConfig based on terminal width
    /// Follows the original lsix script logic
    pub fn from_terminal_width(width: u32, num_colors: u32, bg: &str, fg: &str) -> Self {
        // Original lsix uses fixed 360px tile size
        // Check for environment variable override
        let tilesize = if let Ok(size_str) = std::env::var("LSIX_TILESIZE") {
            size_str.parse().unwrap_or(360)
        } else {
            360  // Fixed size, same as original script
        };

        let tile_width = tilesize;
        let tile_height = tilesize;

        // Space on either side of each tile is less than 0.5% of total screen width
        let tile_xspace = width / 201;
        let tile_yspace = tile_xspace / 2;

        // Figure out how many tiles we can fit per row
        // Original formula: width / (tilewidth + 2*tilexspace + 1)
        let num_tiles_per_row = (width / (tile_width + 2 * tile_xspace + 1)).max(1);

        // Font size is based on width of each tile
        let font_size = (tile_width / 10).max(10);

        Self {
            tile_width,
            tile_height,
            tile_xspace,
            tile_yspace,
            num_tiles_per_row,
            num_colors,
            background: bg.to_string(),
            foreground: fg.to_string(),
            font_family: None,
            font_size,
            shadow: num_colors > 16,
        }
    }

    /// Get ImageMagick montage options
    fn get_montage_options(&self) -> Vec<String> {
        let mut opts = Vec::new();

        // Tile layout
        opts.push("-tile".to_string());
        opts.push(format!("{}x1", self.num_tiles_per_row));

        // Geometry - IMPORTANT: use ">" to only shrink, never enlarge!
        // This matches the original script behavior
        opts.push("-geometry".to_string());
        opts.push(format!(
            "{}x{}>+{}+{}",
            self.tile_width, self.tile_height, self.tile_xspace, self.tile_yspace
        ));

        // Background and foreground colors
        opts.push("-background".to_string());
        opts.push(self.background.clone());
        opts.push("-fill".to_string());
        opts.push(self.foreground.clone());

        // Auto-orient for proper JPEG rotation
        opts.push("-auto-orient".to_string());

        // Shadow for higher color depths (same as original script)
        if self.shadow {
            opts.push("-shadow".to_string());
        }

        // Font settings
        if let Some(ref family) = self.font_family {
            opts.push("-font".to_string());
            opts.push(family.clone());
        }

        opts.push("-pointsize".to_string());
        opts.push(format!("{}", self.font_size));

        opts
    }

    /// Get the montage command based on ImageMagick version
    fn get_montage_command(&self) -> Command {
        match get_imagemagick_mode() {
            ImageMagickMode::V7 => {
                let mut cmd = Command::new("magick");
                cmd.arg("montage");
                cmd
            }
            ImageMagickMode::V6 => Command::new("montage"),
        }
    }

    /// Get the convert command based on ImageMagick version
    fn get_convert_command(&self) -> Command {
        match get_imagemagick_mode() {
            ImageMagickMode::V7 => {
                let mut cmd = Command::new("magick");
                cmd.arg("-");
                cmd
            }
            ImageMagickMode::V6 => {
                // For ImageMagick 6.x, we need to use '-' as the first argument
                // to indicate stdin input
                let mut cmd = Command::new("convert");
                cmd.arg("-");
                cmd
            }
        }
    }
}

/// A single image entry with its label
#[derive(Debug, Clone)]
pub struct ImageEntry {
    pub path: String,
    pub label: String,
}

/// Process and display images in chunks, with concurrent loading
pub fn process_images_concurrent(
    images: Vec<ImageEntry>,
    config: &ImageConfig,
) -> Result<()> {
    // Process images in chunks (rows)
    let chunk_size = config.num_tiles_per_row as usize;

    for chunk in images.chunks(chunk_size) {
        // Process the chunk (one row of images)
        process_chunk(chunk, config)?;
        // Flush output to ensure immediate display
        io::stdout().flush()?;
        io::stderr().flush()?;
    }

    Ok(())
}

/// Process a chunk of images (one row) using streaming output
/// This matches the original script behavior: render each row immediately
fn process_chunk(images: &[ImageEntry], config: &ImageConfig) -> Result<()> {
    // Build montage arguments for this row
    let mut montage_args = config.get_montage_options();

    // Add labels and file paths for each image
    for img in images {
        montage_args.push("-label".to_string());
        montage_args.push(img.label.clone());
        montage_args.push(img.path.clone());
    }

    // Output to stdout in GIF format (for piping)
    montage_args.push("gif:-".to_string());

    // Start montage process
    let mut montage_cmd = config.get_montage_command();
    let mut montage_child = montage_cmd
        .args(&montage_args)
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .context("Failed to execute montage command")?;

    // Start convert process, taking stdin from montage stdout
    let mut convert_cmd = config.get_convert_command();
    let mut convert_child = convert_cmd
        .arg("-colors")
        .arg(format!("{}", config.num_colors))
        .arg("sixel:-")
        .stdin(Stdio::piped())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()
        .context("Failed to execute convert command")?;

    // Pipe montage output to convert input
    if let Some(mut montage_stdout) = montage_child.stdout.take() {
        if let Some(mut convert_stdin) = convert_child.stdin.take() {
            // Copy data from montage to convert in streaming fashion
            std::io::copy(&mut montage_stdout, &mut convert_stdin)?;
        }
    }

    // Wait for both processes to complete
    let montage_status = montage_child.wait()?;
    if !montage_status.success() {
        anyhow::bail!("Montage command failed with exit code: {:?}", montage_status.code());
    }

    let convert_status = convert_child.wait()?;
    if !convert_status.success() {
        anyhow::bail!("Convert command failed with exit code: {:?}", convert_status.code());
    }

    Ok(())
}

/// Pre-load and validate image files concurrently
/// Returns only valid image entries
pub fn validate_images_concurrent(paths: &[String], explicit: bool, mode: FilenameMode) -> Vec<ImageEntry> {
    use crate::filename::{process_image_path, process_label_with_mode};

    paths
        .par_iter() // Parallel iteration
        .filter_map(|path| {
            // Check if file exists and is readable
            let path_obj = std::path::Path::new(path);

            if !path_obj.exists() {
                eprintln!("Warning: File not found: {}", path);
                return None;
            }

            // Process the path (add [0] for animated formats if needed)
            let processed_path = process_image_path(path, explicit);

            // Create image entry
            Some(ImageEntry {
                path: processed_path,
                label: process_label_with_mode(path, mode),
            })
        })
        .collect()
}

/// Find and process directories recursively
pub fn expand_directories(paths: &[String]) -> Vec<String> {
    let mut result = Vec::new();

    for path in paths {
        let path_obj = std::path::Path::new(path);

        if path_obj.is_dir() {
            // Recursively process directory
            eprintln!("Recursing on {}", path);

            if let Ok(entries) = std::fs::read_dir(path) {
                for entry in entries.filter_map(|e| e.ok()) {
                    if let Some(path_str) = entry.path().to_str() {
                        result.push(path_str.to_string());
                    }
                }
            }
        } else {
            // Regular file, keep as is
            result.push(path.clone());
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_image_config_from_width() {
        let config = ImageConfig::from_terminal_width(1024, 256, "white", "black");
        assert_eq!(config.tile_width, 360);
        assert_eq!(config.tile_height, 360);
        assert_eq!(config.font_size, 36);
        assert_eq!(config.shadow, true); // 256 > 16
    }

    #[test]
    fn test_image_config_low_color() {
        let config = ImageConfig::from_terminal_width(800, 16, "white", "black");
        assert_eq!(config.shadow, false); // 16 is not > 16
    }
}
