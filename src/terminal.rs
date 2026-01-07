use anyhow::Result;
use std::io::{self, Write};
use std::time::Duration;

/// Terminal configuration detected via escape sequences
#[derive(Debug, Clone)]
#[allow(dead_code)]
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
/// Uses very short timeout to avoid blocking
fn query_terminal(sequence: &str, timeout_ms: u64) -> Result<Vec<u8>> {
    // Check if we should skip terminal queries
    if std::env::var("LSIX_SKIP_QUERIES").is_ok() {
        return Ok(Vec::new());
    }

    use crossterm::event::{poll, read, Event};
    
    // Enable raw mode to read response without echo
    crossterm::terminal::enable_raw_mode()?;

    // Send the query sequence
    eprint!("{}", sequence);
    io::stderr().flush()?;

    // Read response with short timeout (capped at 200ms)
    let timeout = Duration::from_millis(timeout_ms.min(200)); 
    let response = Vec::new();

    // Use crossterm's event polling instead of direct stdin reading
    // This is more reliable and won't leave junk in the input buffer
    if poll(timeout)? {
        // Try to read the response as raw bytes
        // Terminal responses come as escape sequences
        let start = std::time::Instant::now();
        while start.elapsed() < timeout {
            if poll(Duration::from_millis(1))? {
                match read()? {
                    Event::Key(_key_event) => {
                        // Terminal responses might come as key events
                        // We need to collect them as bytes
                        // For now, just break as we got something
                        break;
                    }
                    _ => break,
                }
            } else {
                break;
            }
        }
    }

    // Disable raw mode immediately
    crossterm::terminal::disable_raw_mode()?;

    Ok(response)
}

/// Detect if terminal supports SIXEL graphics
pub fn detect_sixel() -> Result<bool> {
    // Check for YAFT terminal (vt102 compatible but supports sixel)
    let term = std::env::var("TERM").unwrap_or_default();
    if term.starts_with("yaft") {
        return Ok(true);
    }

    // Check for Ghostty via TERM_PROGRAM
    let term_program = std::env::var("TERM_PROGRAM").unwrap_or_default();
    if term_program.to_lowercase().contains("ghostty") {
        return Ok(true);
    }

    // Check for LSIX_FORCE_SIXEL_SUPPORT environment variable
    if std::env::var("LSIX_FORCE_SIXEL_SUPPORT").is_ok() {
        return Ok(true);
    }

    // Check for common SIXEL-capable terminals by TERM value (fast path)
    let sixel_terminals = [
        "xterm",
        "mlterm",
        "wezterm",
        "foot",
        "contour",
        "kitty",
        "alacritty",
        "mintty",
        "cygwin",
        "ghostty",
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

    // Use smart defaults - skip slow terminal queries
    // Most modern terminals are dark-themed
    let background = "#282a36".to_string(); // Dracula-like dark background
    let foreground = "white".to_string();

    Ok((background, foreground))
}

/// Detect terminal width in pixels
pub fn detect_geometry() -> Result<u32> {
    // Check for environment variable override first
    if let Ok(width_str) = std::env::var("LSIX_WIDTH") {
        if let Ok(width) = width_str.parse::<u32>() {
            return Ok(width);
        }
    }

    // Try to get pixel width via escape sequence CSI 14 t
    // This returns something like \x1b[4;height;widtht
    if let Ok(response) = query_terminal("\x1b[14t", 100) {
        let response_str = String::from_utf8_lossy(&response);
        if let Some(width_part) = response_str.split(';').nth(2) {
            let width_str: String = width_part.chars().take_while(|c| c.is_ascii_digit()).collect();
            if let Ok(width) = width_str.parse::<u32>() {
                if width > 0 {
                    return Ok(width);
                }
            }
        }
    }

    // Fallback: Try to use character width * estimated font width
    if let Ok((cols, _)) = crossterm::terminal::size() {
        // Assume a typical font width of 10-12 pixels
        return Ok(cols as u32 * 12);
    }

    // Use a reasonable default for modern terminals
    Ok(1920)
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
