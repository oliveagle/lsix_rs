#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilenameMode {
    Short, // Only basename (default)
    Long,  // Full path with processing
}

/// Process a filename for display in ImageMagick labels
/// This replicates the processlabel function from the original bash script
pub fn process_label(filename: &str) -> String {
    process_label_with_mode(filename, FilenameMode::Short)
}

/// Process a filename with specified mode
pub fn process_label_with_mode(filename: &str, mode: FilenameMode) -> String {
    const SPAN: usize = 15;

    // Step 0: For short mode, extract just the basename
    let processed = if mode == FilenameMode::Short {
        // Get basename (remove directory path)
        if let Some(name) = std::path::Path::new(filename).file_name() {
            name.to_string_lossy().to_string()
        } else {
            filename.to_string()
        }
    } else {
        // Long mode: use full path
        filename.to_string()
    };

    // Step 1: Remove silly prefixes like "file://"
    // Step 2: Remove [0] suffix (used for animated GIFs)
    // Step 3: Replace control characters with question marks
    let cleaned = processed
        .trim_start_matches(':')
        .trim_start_matches("file://")
        .trim_end_matches("[0]")
        .chars()
        .map(|c| if c.is_ascii_control() { '?' } else { c })
        .collect::<String>();

    // Step 4: If filename is too long, remove extension (.jpg).
    // Step 5: Split long filenames with newlines (recursively)
    let split = halve_string(&cleaned, SPAN);

    // Step 6: Escape special characters for ImageMagick
    // % -> %%, \ -> \\, @ -> \@
    split
        .replace('%', "%%")
        .replace('\\', "\\\\")
        .replace('@', "\\@")
}

/// Recursively split a string into chunks of at most span characters
/// This replicates the awk halve function from the original script
fn halve_string(s: &str, span: usize) -> String {
    if s.len() <= span {
        return s.to_string();
    }

    let mid = s.len() / 2;
    let left = &s[..mid];
    let right = &s[mid..];

    format!(
        "{}\n{}",
        halve_string(left, span),
        halve_string(right, span)
    )
}

/// Process image paths to handle animated GIFs and other multi-frame formats
/// When no arguments are specified, only show first frame of animated formats
pub fn process_image_path(path: &str, explicit: bool) -> String {
    if !explicit {
        let lower = path.to_lowercase();
        if lower.ends_with(".gif") || lower.ends_with(".webp") {
            return format!("{}[0]", path);
        }
    }
    path.to_string()
}

/// Find image files in the current directory
/// Returns a sorted list of image file paths
pub fn find_image_files() -> Vec<String> {
    let extensions = [
        "jpg", "jpeg", "png", "gif", "webp", "tiff", "tif", "pnm", "ppm", "pgm", "pbm", "pam",
        "xbm", "xpm", "bmp", "ico", "svg", "eps",
    ];

    let mut files = Vec::new();

    for ext in &extensions {
        let pattern = format!("*.{}", ext);
        if let Ok(entries) = glob::glob(&pattern) {
            for entry in entries.filter_map(|e| e.ok()) {
                if entry.is_file() {
                    if let Some(path_str) = entry.to_str() {
                        files.push(path_str.to_string());
                    }
                }
            }
        }
    }

    files.sort();
    files
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_process_label_simple() {
        assert_eq!(process_label("test.jpg"), "test.jpg");
    }

    #[test]
    fn test_process_label_with_prefix() {
        assert_eq!(process_label("file://test.jpg"), "test.jpg");
    }

    #[test]
    fn test_process_label_with_frame_suffix() {
        assert_eq!(process_label("animated.gif[0]"), "animated.gif");
    }

    #[test]
    fn test_process_label_escape_chars() {
        assert_eq!(process_label("test%file.jpg"), "test%%file.jpg");
        assert_eq!(process_label("test\\file.jpg"), "test\\\\file.jpg");
        assert_eq!(process_label("test@file.jpg"), "test\\@file.jpg");
    }

    #[test]
    fn test_halve_string() {
        assert_eq!(halve_string("short", 10), "short");
        // 16 chars, span=5 -> splits into 4,4,4,4
        assert_eq!(
            halve_string("verylongfilename", 5),
            "very\nlong\nfile\nname"
        );
    }

    #[test]
    fn test_process_image_path() {
        // Explicit argument - keep as is
        assert_eq!(process_image_path("test.gif", true), "test.gif");

        // Non-explicit - add [0] for animated formats
        assert_eq!(process_image_path("test.gif", false), "test.gif[0]");
        assert_eq!(process_image_path("test.webp", false), "test.webp[0]");
        assert_eq!(process_image_path("test.jpg", false), "test.jpg");
    }

    #[test]
    fn test_short_mode() {
        // Short mode should only show basename
        assert_eq!(
            process_label_with_mode("/path/to/image.jpg", FilenameMode::Short),
            "image.jpg"
        );
    }

    #[test]
    fn test_long_mode() {
        // Long mode should show full path
        assert_eq!(
            process_label_with_mode("/path/to/image.jpg", FilenameMode::Long),
            "/path/to/image.jpg"
        );
    }
}
