mod ai_tagging;
mod filename;
mod filter;
mod grouping;
mod image_proc;
mod term_image;
mod terminal;
mod tui_browser;

use ai_tagging::{clear_ai_cache, tag_images_parallel, AITaggingConfig};
use anyhow::{Context, Result};

const BUILD_TIME: &str = include_str!(concat!(env!("OUT_DIR"), "/build_time.txt"));

use clap::Parser;
use filename::FilenameMode;
use filter::{parse_file_size, parse_orientation, FilterConfig};
use image_proc::{
    expand_directories, expand_directories_recursive,
};
use std::io::{self, Write};
use std::path::Path as StdPath;

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

    // Grouping options
    /// Group images by: similarity, color, size, time, tags, none
    #[arg(long, default_value = "none")]
    #[arg(value_parser = clap::builder::PossibleValuesParser::new(["none", "similarity", "color", "size", "time", "tags"]))]
    group_by: String,

    /// Similarity threshold for grouping (0.0 to 1.0, default: 0.85)
    #[arg(long, default_value = "0.85")]
    similarity_threshold: f32,

    // Tag management
    /// List all tags with image counts (does not display images)
    #[arg(long)]
    list_tags: bool,

    /// Sort tags by: count, name (default: count)
    #[arg(long, default_value = "count")]
    #[arg(value_parser = clap::builder::PossibleValuesParser::new(["count", "name"]))]
    sort_tags_by: String,

    /// Filter by specific tag (OR logic - match any tag)
    #[arg(long)]
    tag: Vec<String>,

    /// Filter by specific tag (AND logic - must match all tags)
    #[arg(long)]
    tag_and: Vec<String>,

    /// Filter by specific tag to exclude (NOT logic)
    #[arg(long)]
    tag_not: Vec<String>,

    // Directory options
    /// Recursive directory search
    #[arg(short, long)]
    recursive: bool,

    // AI tagging options
    /// Generate AI tags for images (requires LSIX_AI_API_KEY)
    #[arg(long)]
    ai_tag: bool,

    /// Clear AI tag cache
    #[arg(long)]
    clear_ai_cache: bool,

    /// Force regenerate AI tags, ignoring cache
    #[arg(long)]
    force: bool,

    /// Enable debug output for AI API calls
    #[arg(long)]
    debug: bool,

    /// Start TUI browser mode for image navigation
    #[arg(long)]
    tui: bool,
}

/// Cleanup handler to stop SIXEL and reset terminal
fn cleanup() {
    // Send escape sequence to stop SIXEL
    eprint!("\x1b\\");
    io::stderr().flush().ok();
}

/// Main function
fn main() -> Result<()> {
    let args = Args::parse();

    // Determine filename mode from command line argument
    let _filename_mode = match args.mode.as_str() {
        "long" => FilenameMode::Long,
        _ => FilenameMode::Short,
    };

    // Build filter config from command line arguments
    let _filter_config = FilterConfig {
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

    // Skip terminal auto-detection for TUI mode - it's not needed and can cause input issues
    // Set environment variable to skip terminal queries
    std::env::set_var("LSIX_SKIP_QUERIES", "1");
    
    // Auto-detect terminal capabilities (very fast now)
    let _term_config = terminal::autodetect().context("Terminal auto-detection failed")?;

    // Handle --clear-ai-cache
    if args.clear_ai_cache {
        let ai_config = AITaggingConfig::default();
        clear_ai_cache(&ai_config)?;
        cleanup();
        return Ok(());
    }

    // Get list of image files
    let image_paths = if args.files.is_empty() {
        // No arguments - find images in current directory
        filename::find_image_files()
    } else {
        // Arguments provided - expand any directories
        if args.recursive {
            expand_directories_recursive(&args.files)
        } else {
            expand_directories(&args.files)
        }
    };

    if image_paths.is_empty() {
        eprintln!("No image files found.");
        cleanup();
        return Ok(());
    }

    // Handle --ai-tag option
    if args.ai_tag {
        let mut ai_config = AITaggingConfig::default();
        ai_config.debug = args.debug; // Set debug flag from command line

        // Only check API key if not using localhost
        if !ai_config.api_endpoint.contains("localhost") && ai_config.api_key.is_empty() {
            eprintln!("Error: LSIX_AI_API_KEY environment variable not set!");
            eprintln!("\nTo use AI tagging, set your API key:");
            eprintln!("  export LSIX_AI_API_KEY='your-api-key-here'");
            eprintln!("\nFor local LLM (no API key required):");
            eprintln!("  export LSIX_AI_ENDPOINT='http://localhost:8000/v1/chat/completions'");
            eprintln!("  export LSIX_AI_MODEL='Qwen3VL-8B-Instruct-Q8_0.gguf'");
            eprintln!("\nSupported: OpenAI (GPT-4, GPT-4o), Anthropic (Claude), local LLMs");
            cleanup();
            return Ok(());
        }

        eprintln!(
            "\n╔════════════════════════════════════════════════════════════════════════════╗"
        );
        eprintln!(
            "║                    AI Auto-Tagging Images                                    ║"
        );
        eprintln!(
            "╚════════════════════════════════════════════════════════════════════════════╝\n"
        );

        eprintln!("Model: {}", ai_config.model);
        eprintln!("API Endpoint: {}", ai_config.api_endpoint);
        eprintln!("Max tags per image: {}", ai_config.max_tags);
        eprintln!("Images to process: {}", image_paths.len());

        if ai_config.custom_prompt.is_some() {
            eprintln!("Prompt: Custom (from ~/.lsix/tag_prompt.md)");
        } else {
            eprintln!("Prompt: Default (create ~/.lsix/tag_prompt.md to customize)");
        }
        eprintln!();

        if ai_config.api_endpoint.contains("localhost") {
            eprintln!("💡 Using local LLM - first run will be slower, subsequent runs use cache\n");
        } else {
            eprintln!("💡 Tip: Run once to cache tags, then filtering is instant!\n");
        }

        if args.force {
            eprintln!("⚠️  Force mode enabled - ignoring cache and regenerating all tags\n");
        }

        // Tag all images with AI
        let ai_tags_map = tag_images_parallel(&image_paths, &ai_config, args.force)
            .context("AI tagging failed")?;

        eprintln!("\n✓ AI tagging complete!");
        eprintln!("  Total images tagged: {}", ai_tags_map.len());
        eprintln!("  Cache location: {:?}", ai_config.cache_dir);

        // Display all generated tags
        eprintln!(
            "\n╔════════════════════════════════════════════════════════════════════════════╗"
        );
        eprintln!(
            "║                    Generated Tags Preview                                   ║"
        );
        eprintln!(
            "╚════════════════════════════════════════════════════════════════════════════╝\n"
        );

        for (path, tags) in ai_tags_map.iter() {
            if let Some(name) = StdPath::new(path).file_name() {
                eprintln!("{}:", name.to_string_lossy());
                eprintln!("  Tags: {}\n", tags.tags.join(", "));
                if let Some(rating) = &tags.content_rating {
                    eprintln!("  Content Rating: {}", rating.to_uppercase());
                }
            }
        }

        eprintln!("💡 Tips:");
        eprintln!("  - Tags are cached for 30 days");
        eprintln!("  - Use --tag <TAG> to filter by AI-generated tag (OR logic)");
        eprintln!("  - Use --tag-and <TAG> for AND logic (must match all)");
        eprintln!("  - Use --tag-not <TAG> to exclude tags (NOT logic)");
        eprintln!("  - Comma-separated tags: --tag \"beach,sunset\"");
        eprintln!("  - Use --clear-ai-cache to clear cache and regenerate");
        eprintln!("  - API costs vary by provider (gpt-4o-mini is cost-effective)\n");

        cleanup();
        return Ok(());
    }

    // Always use TUI browser mode for displaying images
    eprintln!("Starting TUI browser mode...");
    eprintln!("Found {} images to browse.", image_paths.len());
    eprintln!("Build time: {}", BUILD_TIME.trim());
    eprintln!("Use Arrow keys to navigate, Enter to view full size, q to quit");

    // Run the TUI browser
    if let Err(e) = tui_browser::run_tui_browser(image_paths) {
        eprintln!("TUI browser error: {}", e);
        cleanup();
        return Err(anyhow::anyhow!("TUI browser failed: {}", e));
    }

    cleanup();
    Ok(())
}
