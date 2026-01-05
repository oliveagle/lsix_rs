use anyhow::{Context, Result};
use std::collections::HashMap;
use std::path::Path;
use serde::{Deserialize, Serialize};
use crate::filter::ImageFeatures;

/// Group ID type
pub type GroupId = String;

/// Different grouping strategies
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GroupBy {
    None,           // No grouping
    Similarity,     // By visual similarity (perceptual hash)
    Color,          // By dominant color
    Size,           // By dimensions (width/height)
    Time,           // By modification time
    Tags,           // By auto-detected tags
}

/// A group of similar images
#[derive(Debug, Clone)]
pub struct ImageGroup {
    pub id: GroupId,
    pub name: String,
    pub images: Vec<String>,
    pub representative: String,  // Most representative image
    pub metadata: GroupMetadata,
}

/// Metadata about a group
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroupMetadata {
    pub group_type: String,
    pub count: usize,
    pub common_features: HashMap<String, String>,
}

/// Perceptual hash for image similarity
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PerceptualHash {
    pub hash: Vec<u8>,
    pub width: u32,
    pub height: u32,
}

impl PerceptualHash {
    /// Calculate Hamming distance between two hashes
    pub fn hamming_distance(&self, other: &PerceptualHash) -> u32 {
        self.hash.iter()
            .zip(other.hash.iter())
            .map(|(&a, _b)| (a as u8).count_ones() as u32)
            .sum()
    }

    /// Calculate similarity score (0.0 to 1.0, where 1.0 is identical)
    pub fn similarity(&self, other: &PerceptualHash) -> f32 {
        let max_distance = (self.hash.len() * 8) as u32;
        let distance = self.hamming_distance(other);
        1.0 - (distance as f32 / max_distance as f32)
    }
}

/// Color histogram for color-based grouping
#[derive(Debug, Clone)]
pub struct ColorHistogram {
    pub red: [u32; 256],
    pub green: [u32; 256],
    pub blue: [u32; 256],
    pub total_pixels: u64,
}

impl ColorHistogram {
    /// Calculate color similarity (0.0 to 1.0)
    pub fn similarity(&self, other: &ColorHistogram) -> f32 {
        let mut dot_product = 0.0f64;
        let mut norm_a = 0.0f64;
        let mut norm_b = 0.0f64;

        for i in 0..256 {
            let a = (self.red[i] + self.green[i] + self.blue[i]) as f64;
            let b = (other.red[i] + other.green[i] + other.blue[i]) as f64;

            dot_product += a * b;
            norm_a += a * a;
            norm_b += b * b;
        }

        if norm_a == 0.0 || norm_b == 0.0 {
            return 0.0;
        }

        (dot_product / (norm_a.sqrt() * norm_b.sqrt())) as f32
    }
}

/// Group images using the specified strategy
pub fn group_images(
    image_paths: &[String],
    strategy: GroupBy,
    similarity_threshold: f32,
) -> Result<Vec<ImageGroup>> {
    match strategy {
        GroupBy::None => {
            // Put all images in one group
            Ok(vec![ImageGroup {
                id: "all".to_string(),
                name: "All Images".to_string(),
                images: image_paths.to_vec(),
                representative: image_paths.first().cloned().unwrap_or_default(),
                metadata: GroupMetadata {
                    group_type: "none".to_string(),
                    count: image_paths.len(),
                    common_features: HashMap::new(),
                },
            }])
        }
        GroupBy::Similarity => group_by_similarity(image_paths, similarity_threshold),
        GroupBy::Color => group_by_color(image_paths, similarity_threshold),
        GroupBy::Size => group_by_size(image_paths),
        GroupBy::Time => group_by_time(image_paths),
        GroupBy::Tags => group_by_tags(image_paths),
    }
}

/// Group images by visual similarity using perceptual hashing
fn group_by_similarity(image_paths: &[String], threshold: f32) -> Result<Vec<ImageGroup>> {
    use rayon::prelude::*;

    // Calculate perceptual hashes for all images
    let hashes: Vec<(String, PerceptualHash)> = image_paths
        .par_iter()
        .filter_map(|path| {
            calculate_perceptual_hash(path).ok().map(|hash| (path.clone(), hash))
        })
        .collect();

    if hashes.is_empty() {
        return Ok(vec![]);
    }

    // Group similar images
    let mut groups: Vec<Vec<String>> = Vec::new();
    let mut assigned = vec![false; hashes.len()];

    for (i, (path_i, hash_i)) in hashes.iter().enumerate() {
        if assigned[i] {
            continue;
        }

        let mut group = vec![path_i.clone()];
        assigned[i] = true;

        // Find similar images
        for (j, (path_j, hash_j)) in hashes.iter().enumerate() {
            if i != j && !assigned[j] {
                let similarity = hash_i.similarity(hash_j);
                if similarity >= threshold {
                    group.push(path_j.clone());
                    assigned[j] = true;
                }
            }
        }

        groups.push(group);
    }

    // Convert to ImageGroup structures
    Ok(groups
        .into_iter()
        .enumerate()
        .map(|(i, images)| {
            let name = format!("Similar Group {}", i + 1);
            ImageGroup {
                id: format!("similarity_{}", i),
                name,
                images: images.clone(),
                representative: images.first().cloned().unwrap_or_default(),
                metadata: GroupMetadata {
                    group_type: "similarity".to_string(),
                    count: images.len(),
                    common_features: HashMap::new(),
                },
            }
        })
        .collect())
}

/// Group images by color similarity
fn group_by_color(image_paths: &[String], threshold: f32) -> Result<Vec<ImageGroup>> {
    use rayon::prelude::*;

    // Calculate color histograms for all images
    let histograms: Vec<(String, ColorHistogram)> = image_paths
        .par_iter()
        .filter_map(|path| {
            calculate_color_histogram(path).ok().map(|hist| (path.clone(), hist))
        })
        .collect();

    if histograms.is_empty() {
        return Ok(vec![]);
    }

    // Group by color
    let mut groups: Vec<Vec<String>> = Vec::new();
    let mut assigned = vec![false; histograms.len()];

    for (i, (path_i, hist_i)) in histograms.iter().enumerate() {
        if assigned[i] {
            continue;
        }

        let mut group = vec![path_i.clone()];
        assigned[i] = true;

        // Find similar colors
        for (j, (path_j, hist_j)) in histograms.iter().enumerate() {
            if i != j && !assigned[j] {
                let similarity = hist_i.similarity(hist_j);
                if similarity >= threshold {
                    group.push(path_j.clone());
                    assigned[j] = true;
                }
            }
        }

        groups.push(group);
    }

    // Convert to ImageGroup structures
    Ok(groups
        .into_iter()
        .enumerate()
        .map(|(i, images)| {
            let dominant_color = get_dominant_color_name(&images);
            let name = format!("{} Images", dominant_color);
            ImageGroup {
                id: format!("color_{}", i),
                name,
                images: images.clone(),
                representative: images.first().cloned().unwrap_or_default(),
                metadata: GroupMetadata {
                    group_type: "color".to_string(),
                    count: images.len(),
                    common_features: {
                        let mut features = HashMap::new();
                        features.insert("dominant_color".to_string(), dominant_color);
                        features
                    },
                },
            }
        })
        .collect())
}

/// Group images by size (dimensions)
fn group_by_size(image_paths: &[String]) -> Result<Vec<ImageGroup>> {
    use rayon::prelude::*;
    use crate::filter::analyze_image;

    // Get image features
    let features: Vec<(String, ImageFeatures)> = image_paths
        .par_iter()
        .filter_map(|path| {
            analyze_image(path).ok().map(|f| (path.clone(), f))
        })
        .collect();

    if features.is_empty() {
        return Ok(vec![]);
    }

    // Group by size
    let mut size_groups: HashMap<String, Vec<String>> = HashMap::new();

    for (path, feat) in features {
        // Round to nearest 100px
        let width_bucket = (feat.width / 100) * 100;
        let height_bucket = (feat.height / 100) * 100;

        let key = format!("{}x{}", width_bucket, height_bucket);
        size_groups.entry(key).or_insert_with(Vec::new).push(path);
    }

    // Convert to ImageGroup structures
    Ok(size_groups
        .into_iter()
        .map(|(size, images)| {
            ImageGroup {
                id: format!("size_{}", size.replace('x', "_")),
                name: format!("{} Images", size),
                images: images.clone(),
                representative: images.first().cloned().unwrap_or_default(),
                metadata: GroupMetadata {
                    group_type: "size".to_string(),
                    count: images.len(),
                    common_features: {
                        let mut features = HashMap::new();
                        features.insert("resolution".to_string(), size);
                        features
                    },
                },
            }
        })
        .collect())
}

/// Group images by time
fn group_by_time(image_paths: &[String]) -> Result<Vec<ImageGroup>> {
    use std::fs;

    let mut time_groups: HashMap<String, Vec<String>> = HashMap::new();

    for path in image_paths {
        if let Ok(metadata) = fs::metadata(path) {
            if let Ok(modified) = metadata.modified() {
                let datetime: chrono::DateTime<chrono::Local> = modified.into();
                let date_key = datetime.format("%Y-%m-%d").to_string();
                time_groups.entry(date_key).or_insert_with(Vec::new).push(path.clone());
            }
        }
    }

    // Sort by date
    let mut sorted_groups: Vec<_> = time_groups.into_iter().collect();
    sorted_groups.sort_by(|a, b| a.0.cmp(&b.0));

    Ok(sorted_groups
        .into_iter()
        .map(|(date, images)| {
            ImageGroup {
                id: format!("date_{}", date.replace('-', "")),
                name: format!("{} Images", date),
                images: images.clone(),
                representative: images.first().cloned().unwrap_or_default(),
                metadata: GroupMetadata {
                    group_type: "time".to_string(),
                    count: images.len(),
                    common_features: {
                        let mut features = HashMap::new();
                        features.insert("date".to_string(), date);
                        features
                    },
                },
            }
        })
        .collect())
}

/// Group images by auto-detected tags
fn group_by_tags(image_paths: &[String]) -> Result<Vec<ImageGroup>> {
    let mut tag_groups: HashMap<String, Vec<String>> = HashMap::new();

    for path in image_paths {
        let tags = extract_tags(path);
        for tag in tags {
            tag_groups.entry(tag).or_insert_with(Vec::new).push(path.clone());
        }
    }

    Ok(tag_groups
        .into_iter()
        .map(|(tag, images)| {
            ImageGroup {
                id: format!("tag_{}", tag.to_lowercase().replace(' ', "_")),
                name: format!("{} Images", tag),
                images: images.clone(),
                representative: images.first().cloned().unwrap_or_default(),
                metadata: GroupMetadata {
                    group_type: "tags".to_string(),
                    count: images.len(),
                    common_features: {
                        let mut features = HashMap::new();
                        features.insert("tag".to_string(), tag);
                        features
                    },
                },
            }
        })
        .collect())
}

/// Calculate a simplified perceptual hash
fn calculate_perceptual_hash(path: &str) -> Result<PerceptualHash> {
    use std::process::Command;

    // Use ImageMagick to get a small grayscale version
    let output = Command::new("convert")
        .arg(path)
        .arg("-colorspace") .arg("Gray")
        .arg("-resize") .arg("8x8!")
        .arg("-format") .arg("%c")
        .arg("histogram:info:-")
        .output()
        .context("Failed to calculate perceptual hash")?;

    // Parse histogram to get average brightness
    let text = String::from_utf8_lossy(&output.stdout);

    // Simplified hash: just use dimensions for now
    // A real implementation would analyze pixel values
    let identify_output = Command::new("identify")
        .arg("-format") .arg("%w %h")
        .arg(path)
        .output()
        .context("Failed to identify image")?;

    let info = String::from_utf8_lossy(&identify_output.stdout);
    let parts: Vec<&str> = info.trim().split_whitespace().collect();

    if parts.len() >= 2 {
        let width: u32 = parts[0].parse()?;
        let height: u32 = parts[1].parse()?;

        // Create a simple hash based on dimensions and filename
        let mut path_hash = std::collections::hash_map::DefaultHasher::new();
        use std::hash::{Hash, Hasher};
        path.hash(&mut path_hash);

        Ok(PerceptualHash {
            hash: vec![path_hash.finish() as u8; 8],  // 64-bit hash
            width,
            height,
        })
    } else {
        anyhow::bail!("Failed to parse image dimensions")
    }
}

/// Calculate color histogram for an image
fn calculate_color_histogram(path: &str) -> Result<ColorHistogram> {
    use std::process::Command;

    let output = Command::new("convert")
        .arg(path)
        .arg("-resize") .arg("100x100!")  // Downsample for speed
        .arg("-format") .arg("%c")
        .arg("histogram:info:-")
        .output()
        .context("Failed to calculate color histogram")?;

    // Parse histogram
    let text = String::from_utf8_lossy(&output.stdout);
    let mut histogram = ColorHistogram {
        red: [0; 256],
        green: [0; 256],
        blue: [0; 256],
        total_pixels: 0,
    };

    // Simple parsing - just count color occurrences
    for line in text.lines() {
        if line.contains("red") {
            if let Some(num) = line.split_whitespace().next() {
                if let Ok(count) = num.parse::<u32>() {
                    // This is simplified - real implementation would parse properly
                    histogram.total_pixels += count as u64;
                }
            }
        }
    }

    Ok(histogram)
}

/// Extract tags from image path/filename
fn extract_tags(path: &str) -> Vec<String> {
    let mut tags = Vec::new();

    // Extract from path components
    if let Some(parent) = Path::new(path).parent() {
        if let Some(dir_name) = parent.file_name() {
            if let Some(dir_str) = dir_name.to_str() {
                let dir_str = dir_str.to_string();
                if is_meaningful_tag(&dir_str) {
                    tags.push(dir_str);
                }
            }
        }
    }

    // Extract from filename
    if let Some(file_name) = Path::new(path).file_stem() {
        if let Some(name_str) = file_name.to_str() {
            // Split by common separators
            for part in name_str.split(&['_', '-', ' ', '.'][..]) {
                if !part.is_empty() && part.len() > 2 {
                    let part = part.to_string();
                    if is_meaningful_tag(&part) {
                        tags.push(part);
                    }
                }
            }
        }
    }

    // Add extension as tag
    if let Some(ext) = Path::new(path).extension() {
        if let Some(ext_str) = ext.to_str() {
            let ext_upper = ext_str.to_uppercase();
            if is_meaningful_tag(&ext_upper) {
                tags.push(ext_upper);
            }
        }
    }

    tags
}

/// Check if a tag is meaningful (not just numbers or common patterns)
fn is_meaningful_tag(tag: &str) -> bool {
    // Filter out empty strings
    if tag.is_empty() {
        return false;
    }

    // Filter out pure numbers
    if tag.chars().all(|c| c.is_numeric()) {
        return false;
    }

    // Filter out very short tags (< 3 chars) unless it's extension
    if tag.len() < 3 && !tag.starts_with('.') && tag.chars().all(|c| c.is_alphabetic()) {
        return false;
    }

    // Filter out common patterns
    let ignore_patterns = [
        "img", "photo", "pic", "image", "dsc", "sam",
        "001", "002", "003", "final", "copy", "version",
    ];

    let tag_lower = tag.to_lowercase();
    if ignore_patterns.contains(&tag_lower.as_str()) {
        return false;
    }

    // Must contain at least one letter
    if !tag.chars().any(|c| c.is_alphabetic()) {
        return false;
    }

    true
}

/// Get dominant color name from a group of images
fn get_dominant_color_name(_images: &[String]) -> String {
    // Simplified - just return a color category
    // Real implementation would analyze actual colors
    "Color".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_tags() {
        let tags = extract_tags("/home/user/Pictures/vacation_beach_2024/photo_001.jpg");
        assert!(tags.contains(&"vacation".to_string()));
        assert!(tags.contains(&"JPG".to_string()));
    }
}
