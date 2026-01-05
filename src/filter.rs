use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::process::{Command, Stdio};

/// Image analysis results
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageFeatures {
    pub width: u32,
    pub height: u32,
    pub file_size: u64,
    pub brightness: f32,        // 0.0 (dark) to 1.0 (bright)
    pub dominant_color: String, // Hex color
    pub orientation: ImageOrientation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ImageOrientation {
    Landscape, // width > height
    Portrait,  // height > width
    Square,    // width == height (within tolerance)
}

/// Filter criteria for images
#[derive(Debug, Clone)]
pub struct FilterConfig {
    // Size filters
    pub min_width: Option<u32>,
    pub max_width: Option<u32>,
    pub min_height: Option<u32>,
    pub max_height: Option<u32>,
    pub min_file_size: Option<u64>,
    pub max_file_size: Option<u64>,

    // Color filters
    pub min_brightness: Option<f32>,
    pub max_brightness: Option<f32>,

    // Orientation filter
    pub orientation: Option<ImageOrientation>,
}

impl Default for FilterConfig {
    fn default() -> Self {
        Self {
            min_width: None,
            max_width: None,
            min_height: None,
            max_height: None,
            min_file_size: None,
            max_file_size: None,
            min_brightness: None,
            max_brightness: None,
            orientation: None,
        }
    }
}

impl FilterConfig {
    /// Check if an image matches all filter criteria
    pub fn matches(&self, features: &ImageFeatures) -> bool {
        // Width filter
        if let Some(min_w) = self.min_width {
            if features.width < min_w {
                return false;
            }
        }
        if let Some(max_w) = self.max_width {
            if features.width > max_w {
                return false;
            }
        }

        // Height filter
        if let Some(min_h) = self.min_height {
            if features.height < min_h {
                return false;
            }
        }
        if let Some(max_h) = self.max_height {
            if features.height > max_h {
                return false;
            }
        }

        // File size filter
        if let Some(min_size) = self.min_file_size {
            if features.file_size < min_size {
                return false;
            }
        }
        if let Some(max_size) = self.max_file_size {
            if features.file_size > max_size {
                return false;
            }
        }

        // Brightness filter
        if let Some(min_bright) = self.min_brightness {
            if features.brightness < min_bright {
                return false;
            }
        }
        if let Some(max_bright) = self.max_brightness {
            if features.brightness > max_bright {
                return false;
            }
        }

        // Orientation filter
        if let Some(orient) = self.orientation {
            if features.orientation != orient {
                return false;
            }
        }

        true
    }
}

/// Analyze an image file to extract features
pub fn analyze_image(path: &str) -> Result<ImageFeatures> {
    let path_obj = Path::new(path);

    // Get file size
    let metadata = std::fs::metadata(path_obj).context("Failed to get file metadata")?;
    let file_size = metadata.len();

    // Use ImageMagick identify to get image info
    let identify_cmd = if Command::new("magick")
        .arg("identify")
        .arg("-version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
    {
        "magick"
    } else {
        "identify"
    };

    // Get image dimensions and format
    let output = Command::new(identify_cmd)
        .arg("-format")
        .arg("%w %h") // width height
        .arg(path)
        .output()
        .context("Failed to run identify command")?;

    let info = String::from_utf8_lossy(&output.stdout);
    let parts: Vec<&str> = info.trim().split_whitespace().collect();

    if parts.len() < 2 {
        anyhow::bail!("Failed to parse image info from identify");
    }

    let width: u32 = parts[0].parse().context("Failed to parse width")?;
    let height: u32 = parts[1].parse().context("Failed to parse height")?;

    // Determine orientation
    let aspect_ratio = width as f32 / height as f32;
    let orientation = if aspect_ratio > 1.1 {
        ImageOrientation::Landscape
    } else if aspect_ratio < 0.9 {
        ImageOrientation::Portrait
    } else {
        ImageOrientation::Square
    };

    // Get brightness (using ImageMagick to analyze)
    let brightness_output = Command::new(identify_cmd)
        .arg("-format")
        .arg("%[mean]") // mean brightness
        .arg(path)
        .output()
        .context("Failed to get brightness")?;

    let brightness_str = String::from_utf8_lossy(&brightness_output.stdout);
    let brightness: f32 = brightness_str.trim().parse().unwrap_or(0.5) / 65535.0; // ImageMagick returns 16-bit value

    // Get dominant color (simplified - just take center pixel)
    let color_output = Command::new(identify_cmd)
        .arg("-format")
        .arg("%[pixel:p{50%,50%}]") // center pixel color
        .arg(path)
        .output()
        .context("Failed to get dominant color")?;

    let dominant_color = String::from_utf8_lossy(&color_output.stdout)
        .trim()
        .to_string();

    Ok(ImageFeatures {
        width,
        height,
        file_size,
        brightness: brightness.min(1.0).max(0.0),
        dominant_color,
        orientation,
    })
}

/// Parse orientation from string
pub fn parse_orientation(s: &str) -> Result<ImageOrientation> {
    match s.to_lowercase().as_str() {
        "landscape" | "horizontal" | "h" => Ok(ImageOrientation::Landscape),
        "portrait" | "vertical" | "v" => Ok(ImageOrientation::Portrait),
        "square" | "s" => Ok(ImageOrientation::Square),
        _ => anyhow::bail!(
            "Invalid orientation: {}. Use: landscape, portrait, or square",
            s
        ),
    }
}

/// Parse human-readable file size (e.g., "100K", "2M", "1G")
pub fn parse_file_size(s: &str) -> Result<u64> {
    let s = s.trim().to_uppercase();
    let (num_str, _unit) = if s.ends_with('B') {
        (&s[..s.len() - 1], &s[s.len() - 1..])
    } else {
        (s.as_str(), "")
    };

    let num: f64 = num_str
        .trim_end_matches('K')
        .trim_end_matches('M')
        .trim_end_matches('G')
        .trim_end_matches('T')
        .parse()
        .context("Invalid file size format")?;

    let multiplier = if num_str.ends_with('K') || num_str.ends_with("KB") {
        1024.0
    } else if num_str.ends_with('M') || num_str.ends_with("MB") {
        1024.0 * 1024.0
    } else if num_str.ends_with('G') || num_str.ends_with("GB") {
        1024.0 * 1024.0 * 1024.0
    } else if num_str.ends_with('T') || num_str.ends_with("TB") {
        1024.0 * 1024.0 * 1024.0 * 1024.0
    } else {
        1.0
    };

    Ok((num * multiplier) as u64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_file_size() {
        assert_eq!(parse_file_size("100").unwrap(), 100);
        assert_eq!(parse_file_size("1K").unwrap(), 1024);
        assert_eq!(parse_file_size("1M").unwrap(), 1024 * 1024);
        assert_eq!(
            parse_file_size("1.5M").unwrap(),
            (1.5 * 1024.0 * 1024.0) as u64
        );
    }

    #[test]
    fn test_parse_orientation() {
        assert_eq!(
            parse_orientation("landscape").unwrap(),
            ImageOrientation::Landscape
        );
        assert_eq!(
            parse_orientation("portrait").unwrap(),
            ImageOrientation::Portrait
        );
        assert_eq!(
            parse_orientation("square").unwrap(),
            ImageOrientation::Square
        );
        assert_eq!(parse_orientation("h").unwrap(), ImageOrientation::Landscape);
        assert_eq!(parse_orientation("v").unwrap(), ImageOrientation::Portrait);
    }

    #[test]
    fn test_filter_matches() {
        let filter = FilterConfig {
            min_width: Some(100),
            max_width: Some(1000),
            orientation: Some(ImageOrientation::Landscape),
            ..Default::default()
        };

        let features = ImageFeatures {
            width: 500,
            height: 300,
            file_size: 1024,
            brightness: 0.5,
            dominant_color: "#ffffff".to_string(),
            orientation: ImageOrientation::Landscape,
        };

        assert!(filter.matches(&features));

        // Test mismatch
        let features_portrait = ImageFeatures {
            orientation: ImageOrientation::Portrait,
            ..features
        };
        assert!(!filter.matches(&features_portrait));
    }
}
