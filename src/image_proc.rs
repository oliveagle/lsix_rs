use anyhow::{Context, Result};
use rayon::prelude::*;
use std::collections::hash_map::DefaultHasher;
use std::fs;
use std::hash::{Hash, Hasher};
use std::io::{self, Write};
use std::process::{Command, Stdio};
use std::sync::OnceLock;

// Import filename types
use crate::filename::FilenameMode;
use crate::filter::{analyze_image, FilterConfig};
use crate::grouping::ImageGroup;

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
            360 // Fixed size, same as original script
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

        // Optimize color count for performance
        // Use fewer colors for faster processing
        let optimized_colors = if let Ok(colors_str) = std::env::var("LSIX_COLORS") {
            colors_str.parse().unwrap_or(num_colors)
        } else {
            // Default to 128 colors for better performance (vs 256)
            num_colors.min(128)
        };

        // Disable shadow by default for better performance
        let shadow = if let Ok(shadow_str) = std::env::var("LSIX_SHADOW") {
            shadow_str != "0"
        } else {
            // No shadow by default - much faster
            false
        };

        Self {
            tile_width,
            tile_height,
            tile_xspace,
            tile_yspace,
            num_tiles_per_row,
            num_colors: optimized_colors,
            background: bg.to_string(),
            foreground: fg.to_string(),
            font_family: None,
            font_size,
            shadow,
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
/// Processes multiple rows in parallel for better performance
pub fn process_images_concurrent(images: Vec<ImageEntry>, config: &ImageConfig) -> Result<()> {
    use rayon::prelude::*;

    // Process images in chunks (rows)
    let chunk_size = config.num_tiles_per_row as usize;
    let chunks: Vec<_> = images.chunks(chunk_size).collect();

    // Process rows in parallel, but maintain order for display
    let results: Vec<Result<Vec<u8>>> = chunks
        .par_iter() // Parallel iteration over rows
        .map(|chunk| generate_sixel_output_cached(chunk, config))
        .collect();

    // Output in order
    for result in results {
        let data = result?;
        io::stdout().write_all(&data)?;
        io::stdout().flush()?;
    }

    Ok(())
}

/// Process and display images grouped by criteria
/// Shows group headers and processes each group separately
pub fn process_images_grouped(
    groups: Vec<ImageGroup>,
    all_images: Vec<ImageEntry>,
    config: &ImageConfig,
) -> Result<()> {
    use std::io::Write;

    for (group_idx, group) in groups.iter().enumerate() {
        // Print group header
        eprintln!("\n╔═══════════════════════════════════════════════════════════════");
        eprintln!(
            "║ Group {}: {} ({} images)",
            group_idx + 1,
            group.name,
            group.images.len()
        );

        // Show group metadata
        if !group.metadata.common_features.is_empty() {
            let features: Vec<String> = group
                .metadata
                .common_features
                .iter()
                .map(|(k, v)| format!("{}: {}", k, v))
                .collect();
            eprintln!("║ {}", features.join(", "));
        }

        eprintln!("╚═══════════════════════════════════════════════════════════════");
        io::stderr().flush()?;

        // Find ImageEntry objects for images in this group
        let group_images: Vec<ImageEntry> = all_images
            .iter()
            .filter(|img| group.images.contains(&img.path))
            .cloned()
            .collect();

        if group_images.is_empty() {
            eprintln!("(No images in this group)");
            continue;
        }

        // Process images in this group
        process_images_concurrent(group_images, config)?;

        // Add separator between groups
        if group_idx < groups.len() - 1 {
            eprintln!("\n"); // Extra newline between groups
        }
    }

    Ok(())
}

/// Generate SIXEL output with caching support
fn generate_sixel_output_cached(images: &[ImageEntry], config: &ImageConfig) -> Result<Vec<u8>> {
    // Try to use cache
    if let Ok(cache_dir) = get_cache_dir() {
        let cache_key = generate_cache_key(images, config);
        let cache_path = cache_dir.join(&cache_key);

        // Check if cache is valid
        if is_cache_valid(&cache_path, images) {
            // Try to read from cache
            match fs::read(&cache_path) {
                Ok(data) => return Ok(data),
                Err(_) => {}
            }
        }

        // Cache miss or invalid, generate new output
        let sixel_output = generate_sixel_output(images, config)?;

        // Write to cache for next time
        let _ = write_to_cache(&cache_path, &sixel_output);

        return Ok(sixel_output);
    }

    // Fallback: generate output without caching
    generate_sixel_output(images, config)
}

/// Generate cache key based on images and config
fn generate_cache_key(images: &[ImageEntry], config: &ImageConfig) -> String {
    let mut hasher = DefaultHasher::new();

    // Hash configuration parameters
    config.tile_width.hash(&mut hasher);
    config.tile_height.hash(&mut hasher);
    config.num_colors.hash(&mut hasher);
    config.background.hash(&mut hasher);
    config.foreground.hash(&mut hasher);
    config.shadow.hash(&mut hasher);

    // Hash image paths and modification times
    for img in images {
        img.path.hash(&mut hasher);
        // Include file modification time in hash
        if let Ok(metadata) = fs::metadata(&img.path) {
            if let Ok(modified) = metadata.modified() {
                modified
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs()
                    .hash(&mut hasher);
            }
        }
    }

    format!("{:x}", hasher.finish())
}

/// Get cache directory path
fn get_cache_dir() -> Result<std::path::PathBuf> {
    let cache_dir = if let Ok(home) = std::env::var("HOME") {
        std::path::PathBuf::from(home).join(".cache").join("lsix")
    } else {
        std::path::PathBuf::from("/tmp/lsix")
    };

    // Create cache directory if it doesn't exist
    if !cache_dir.exists() {
        fs::create_dir_all(&cache_dir)?;
    }

    Ok(cache_dir)
}

/// Check if cached data is valid for the given images
fn is_cache_valid(cache_path: &std::path::Path, images: &[ImageEntry]) -> bool {
    if !cache_path.exists() {
        return false;
    }

    // Check if all source images still exist and haven't been modified
    for img in images {
        if let Ok(metadata) = fs::metadata(&img.path) {
            if let Ok(modified) = metadata.modified() {
                if let Ok(cache_metadata) = fs::metadata(cache_path) {
                    if let Ok(cache_modified) = cache_metadata.modified() {
                        // Cache should be newer than source images
                        if modified > cache_modified {
                            return false;
                        }
                    }
                }
            }
        } else {
            // Source image doesn't exist
            return false;
        }
    }

    true
}

/// Write to cache
fn write_to_cache(cache_path: &std::path::Path, data: &[u8]) -> Result<()> {
    fs::write(cache_path, data)?;
    Ok(())
}

/// Generate SIXEL output for a chunk of images
fn generate_sixel_output(images: &[ImageEntry], config: &ImageConfig) -> Result<Vec<u8>> {
    // Build montage arguments for this row
    let mut montage_args = config.get_montage_options();

    // Track valid images
    let mut valid_images = Vec::new();

    // Add labels and file paths for each image
    for img in images {
        if img.path.is_empty() {
            eprintln!("Warning: Skipping image with empty path");
            continue;
        }

        // Check if file exists
        if !std::path::Path::new(&img.path).exists() {
            eprintln!("Warning: File not found: {}", img.path);
            continue;
        }

        valid_images.push(img);
        montage_args.push("-label".to_string());
        montage_args.push(img.label.clone());
        montage_args.push(img.path.clone());
    }

    // If no valid images, return empty output
    if valid_images.is_empty() {
        eprintln!("Warning: No valid images in this chunk");
        return Ok(Vec::new());
    }

    // Output to stdout in GIF format (for piping)
    montage_args.push("gif:-".to_string());

    // Debug: print arguments if LSIX_DEBUG is set
    if std::env::var("LSIX_DEBUG").is_ok() {
        eprintln!("Montage args: {:?}", montage_args);
    }

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
        .stdout(Stdio::piped())
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

    // Read output from convert
    let sixel_data = if let Some(mut convert_stdout) = convert_child.stdout.take() {
        let mut buffer = Vec::new();
        std::io::copy(&mut convert_stdout, &mut buffer)?;
        buffer
    } else {
        Vec::new()
    };

    // Wait for both processes to complete
    let montage_status = montage_child.wait()?;
    if !montage_status.success() {
        anyhow::bail!(
            "Montage command failed with exit code: {:?}",
            montage_status.code()
        );
    }

    let convert_status = convert_child.wait()?;
    if !convert_status.success() {
        anyhow::bail!(
            "Convert command failed with exit code: {:?}",
            convert_status.code()
        );
    }

    Ok(sixel_data)
}

/// Pre-load and validate image files concurrently
/// Returns only valid image entries that match the filter criteria
pub fn validate_images_concurrent(
    paths: &[String],
    explicit: bool,
    mode: FilenameMode,
    filter_config: &FilterConfig,
) -> Vec<ImageEntry> {
    use crate::filename::{process_image_path, process_label_with_mode};

    // Check if any filter is active
    let has_filters = filter_config.min_width.is_some()
        || filter_config.max_width.is_some()
        || filter_config.min_height.is_some()
        || filter_config.max_height.is_some()
        || filter_config.min_file_size.is_some()
        || filter_config.max_file_size.is_some()
        || filter_config.min_brightness.is_some()
        || filter_config.max_brightness.is_some()
        || filter_config.orientation.is_some();

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

            // If filters are active, analyze and check
            if has_filters {
                match analyze_image(&processed_path) {
                    Ok(features) => {
                        if !filter_config.matches(&features) {
                            // Image doesn't match filter, skip it
                            return None;
                        }
                    }
                    Err(e) => {
                        eprintln!("Warning: Failed to analyze {}: {}", path, e);
                        // Include image anyway if analysis fails
                    }
                }
            }

            // Create image entry
            Some(ImageEntry {
                path: processed_path,
                label: process_label_with_mode(path, mode),
            })
        })
        .collect()
}

/// Find and process directories recursively
/// Filters to only include image files
pub fn expand_directories(paths: &[String]) -> Vec<String> {
    // Supported image extensions
    let image_extensions = [
        "jpg", "jpeg", "png", "gif", "webp", "tiff", "tif", "pnm", "ppm", "pgm", "pbm", "pam",
        "xbm", "xpm", "bmp", "ico", "svg", "eps",
    ];

    let mut result = Vec::new();

    for path in paths {
        let path_obj = std::path::Path::new(path);

        if path_obj.is_dir() {
            // Process directory (non-recursive unless -r flag is used)
            eprintln!("Scanning directory: {}", path);

            if let Ok(entries) = std::fs::read_dir(path) {
                for entry in entries.filter_map(|e| e.ok()) {
                    let entry_path = entry.path();
                    // Only add if it's a file with image extension
                    if entry_path.is_file() {
                        if let Some(ext) = entry_path.extension() {
                            if image_extensions.contains(&ext.to_string_lossy().as_ref()) {
                                if let Some(path_str) = entry_path.to_str() {
                                    result.push(path_str.to_string());
                                }
                            }
                        }
                    }
                }
            }
        } else {
            // Regular file - check if it has image extension
            if let Some(ext) = path_obj.extension() {
                if image_extensions.contains(&ext.to_string_lossy().as_ref()) {
                    result.push(path.clone());
                }
            }
        }
    }

    result.sort();
    result
}

/// Recursively find all images in directory tree
pub fn expand_directories_recursive(paths: &[String]) -> Vec<String> {
    let image_extensions = [
        "jpg", "jpeg", "png", "gif", "webp", "tiff", "tif", "pnm", "ppm", "pgm", "pbm", "pam",
        "xbm", "xpm", "bmp", "ico", "svg", "eps",
    ];

    let mut result = Vec::new();

    for path in paths {
        let path_obj = std::path::Path::new(path);

        if path_obj.is_dir() {
            // Recursively process directory and all subdirectories
            eprintln!("Recursively scanning: {}", path);

            if let Ok(entries) = std::fs::read_dir(path) {
                for entry in entries.filter_map(|e| e.ok()) {
                    let entry_path = entry.path();

                    if entry_path.is_dir() {
                        // Recurse into subdirectory
                        let subdir_path = entry_path.to_string_lossy().to_string();
                        let sub_result = expand_directories_recursive(&[subdir_path]);
                        result.extend(sub_result);
                    } else if entry_path.is_file() {
                        // Check if it's an image file
                        if let Some(ext) = entry_path.extension() {
                            if image_extensions.contains(&ext.to_string_lossy().as_ref()) {
                                if let Some(path_str) = entry_path.to_str() {
                                    result.push(path_str.to_string());
                                }
                            }
                        }
                    }
                }
            }
        } else {
            // Regular file - check if it has image extension
            if let Some(ext) = path_obj.extension() {
                if image_extensions.contains(&ext.to_string_lossy().as_ref()) {
                    result.push(path.clone());
                }
            }
        }
    }

    result.sort();
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
