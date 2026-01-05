use anyhow::Result;
use std::io::{self, Read, Write};
use std::time::Duration;

/// Terminal configuration detected via escape sequences
#[derive(Debug, Clone)]
pub struct TerminalConfig {
    pub has_sixel: bool,
    pub num_colors: u32,
    pub width: u32,
    pub background: String,
    pub foreground: String,
}

impl Default for TerminalConfig {
    fn default() -> Self {
        Self {
            has_sixel: false,
            num_colors: 16,
            width: 1024,
            background: "white".to_string(),
            foreground: "black".to_string(),
        }
    }
}

/// Send an escape sequence and read the response from the terminal
fn query_terminal(sequence: &str, timeout_ms: u64) -> Result<Vec<u8>> {
    // Disable echo
    let _ = std::process::Command::new("stty").arg("-echo").status();

    // Send the query sequence
    eprint!("{}", sequence);
    io::stderr().flush()?;

    // Read response with timeout
    let start = std::time::Instant::now();
    let mut response = Vec::new();
    let stdin = io::stdin();
    let timeout = Duration::from_millis(timeout_ms);

    while start.elapsed() < timeout {
        let mut byte = [0u8; 1];
        match stdin.lock().read(&mut byte) {
            Ok(1) => {
                response.push(byte[0]);

                // Check for termination sequences
                if response.ends_with(b"c") || response.ends_with(b"S") || response.ends_with(b"\\") {
                    break;
                }
            }
            Ok(0) | Err(_) => {
                // No data available, sleep briefly
                std::thread::sleep(Duration::from_millis(1));
            }
            _ => {}
        }
    }

    // Re-enable echo
    let _ = std::process::Command::new("stty").arg("echo").status();

    Ok(response)
}

/// Detect if terminal supports SIXEL graphics
pub fn detect_sixel() -> Result<bool> {
    // Check for YAFT terminal (vt102 compatible but supports sixel)
    let term = std::env::var("TERM").unwrap_or_default();
    if term.starts_with("yaft") {
        return Ok(true);
    }

    // Check for LSIX_FORCE_SIXEL_SUPPORT environment variable
    if std::env::var("LSIX_FORCE_SIXEL_SUPPORT").is_ok() {
        return Ok(true);
    }

    // Check for common SIXEL-capable terminals by TERM value (fast path)
    let sixel_terminals = [
        "xterm", "mlterm", "wezterm", "foot", "contour",
        "kitty", "alacritty", "mintty", "cygwin",
    ];

    let term_lower = term.to_lowercase();
    for sixel_term in &sixel_terminals {
        if term_lower.contains(sixel_term) {
            // Known SIXEL terminal, skip slow query
            return Ok(true);
        }
    }

    // Unknown terminal, try quick query (50ms timeout)
    let response = query_terminal("\x1b[c", 50)?;

    // Parse response for SIXEL support (code 4)
    let response_str = String::from_utf8_lossy(&response);
    let codes: Vec<&str> = response_str.split([';', '?', 'c', '\x1b']).collect();

    let has_sixel = codes.iter().any(|&c| c == "4");

    if !has_sixel {
        anyhow::bail!(
            "Your terminal does not report having sixel graphics support.\n\
             Please use a sixel capable terminal, such as xterm -ti vt340.\n\
             Or set LSIX_FORCE_SIXEL_SUPPORT=1 to force enable."
        );
    }

    Ok(has_sixel)
}

/// Detect the number of color registers the terminal supports
pub fn detect_colors() -> Result<u32> {
    let term = std::env::var("TERM").unwrap_or_default();

    // YAFT doesn't respond to VT220 escape sequences
    if term.starts_with("yaft") {
        return Ok(256);
    }

    // For modern terminals, default to 256 colors
    Ok(256)
}

/// Detect terminal background and foreground colors
pub fn detect_colorscheme() -> Result<(String, String)> {
    let term = std::env::var("TERM").unwrap_or_default();

    // YAFT defaults
    if term.starts_with("yaft") {
        return Ok(("black".to_string(), "white".to_string()));
    }

    // Check for environment variable override (highest priority)
    if let Ok(bg) = std::env::var("LSIX_BACKGROUND") {
        let fg = std::env::var("LSIX_FOREGROUND").unwrap_or_else(|_| "white".to_string());
        return Ok((bg, fg));
    }

    let timeout = Duration::from_millis(250);
    let mut background = "white".to_string();
    let mut foreground = "black".to_string();

    // Query background color: ESC]11;?ESC\
    let bg_response = query_terminal("\x1b]11;?\x1b\\", timeout.as_millis() as u64)?;
    let bg_str = String::from_utf8_lossy(&bg_response);

    // Parse rgb:rrrr/gggg/bbbb format
    if bg_str.contains("rgb:") {
        let parts: Vec<&str> = bg_str.split([':', '/', '\\']).collect();
        if parts.len() >= 5 {
            // Convert to #rrrrggggbbbb format for ImageMagick
            background = format!("#{}{}{}",
                parts.get(2).unwrap_or(&"ffff"),
                parts.get(3).unwrap_or(&"ffff"),
                parts.get(4).unwrap_or(&"ffff")
            );
            // Clean up any escape sequences
            background = background.replace('\x1b', "").trim().to_string();
        }

        // Query foreground color: ESC]10;?ESC\
        let fg_response = query_terminal("\x1b]10;?\x1b\\", timeout.as_millis() as u64)?;
        let fg_str = String::from_utf8_lossy(&fg_response);

        if fg_str.contains("rgb:") {
            let parts: Vec<&str> = fg_str.split([':', '/', '\\']).collect();
            if parts.len() >= 5 {
                foreground = format!("#{}{}{}",
                    parts.get(2).unwrap_or(&"0000"),
                    parts.get(3).unwrap_or(&"0000"),
                    parts.get(4).unwrap_or(&"0000")
                );
                foreground = foreground.replace('\x1b', "").trim().to_string();
            }
        }

        // Check for reverse video mode: ESC[?5$p
        let rv_response = query_terminal("\x1b[?5$p", timeout.as_millis() as u64)?;
        let rv_str = String::from_utf8_lossy(&rv_response);
        let parts: Vec<&str> = rv_str.split([';', '?', '$', 'p']).collect();

        if parts.len() >= 3 && (parts[2] == "1" || parts[2] == "3") {
            std::mem::swap(&mut background, &mut foreground);
        }
    } else {
        // Terminal didn't respond with color information
        // Most modern terminals are dark-themed, so use a reasonable dark default
        // instead of blinding white
        background = "#282a36".to_string();  // Dracula-like dark background
        foreground = "white".to_string();
    }

    Ok((background, foreground))
}

/// Detect terminal width in pixels
pub fn detect_geometry() -> Result<u32> {
    let timeout = Duration::from_millis(250);

    // Check for environment variable override first
    if let Ok(width_str) = std::env::var("LSIX_WIDTH") {
        if let Ok(width) = width_str.parse::<u32>() {
            return Ok(width);
        }
    }

    // Method 1: Query SIXEL graphics geometry (preferred)
    // This is the same method the original script uses
    let response = query_terminal("\x1b[?2;1;0S", timeout.as_millis() as u64)?;
    let response_str = String::from_utf8_lossy(&response);

    // Parse response: format is ESC[?2;1;widthS
    let parts: Vec<&str> = response_str.split(';').collect();
    if parts.len() >= 3 {
        if let Ok(w) = parts[2].trim_end_matches('S').parse::<u32>() {
            if w > 0 {
                return Ok(w);
            }
        }
    }

    // Method 2: Fallback to dtterm WindowOps to approximate SIXEL geometry
    let response = query_terminal("\x1b[14t", timeout.as_millis() as u64)?;
    let response_str = String::from_utf8_lossy(&response);

    let parts: Vec<&str> = response_str.split(';').collect();
    if parts.len() >= 3 {
        if let Ok(w) = parts[2].trim_end_matches('t').parse::<u32>() {
            if w > 0 {
                return Ok(w);
            }
        }
    }

    // Method 3: Last resort - use COLUMNS with fallback
    if let Ok(cols) = std::env::var("COLUMNS") {
        if let Ok(col_count) = cols.trim().parse::<u32>() {
            if col_count > 0 {
                // Estimate pixel width (this is a rough fallback)
                let estimated_width = col_count * 9;
                if estimated_width >= 400 {
                    return Ok(estimated_width);
                }
            }
        }
    }

    // Default fallback (same as original script)
    Ok(1024)
}

/// Auto-detect terminal capabilities and configuration
/// Optimized for speed - uses smart defaults instead of slow queries
pub fn autodetect() -> Result<TerminalConfig> {
    // Fast detection based on TERM and environment variables
    let has_sixel = detect_sixel()?;

    if !has_sixel {
        anyhow::bail!(
            "Your terminal does not report having sixel graphics support.\n\
             Please use a sixel capable terminal, such as xterm -ti vt340.\n\
             Or set LSIX_FORCE_SIXEL_SUPPORT=1 to force enable."
        );
    }

    // Use smart defaults - no slow queries
    let num_colors = detect_colors()?;
    let (background, foreground) = detect_colorscheme()?;
    let width = detect_geometry()?;

    Ok(TerminalConfig {
        has_sixel,
        num_colors,
        width,
        background,
        foreground,
    })
}
