mod filename;
mod image_proc;
mod terminal;
mod filter;
mod grouping;
mod ai_tagging;

use anyhow::{Context, Result};
use clap::Parser;
use image_proc::{expand_directories, expand_directories_recursive, validate_images_concurrent, process_images_concurrent, process_images_grouped, ImageConfig};
use filename::FilenameMode;
use filter::{FilterConfig, parse_orientation, parse_file_size};
use grouping::{GroupBy, group_images};
use ai_tagging::{AITaggingConfig, tag_images_parallel, clear_ai_cache};
use std::path::Path as StdPath;
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

    /// Filter by specific tag (can be used multiple times)
    #[arg(long)]
    tag: Vec<String>,

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
        ai_config.debug = args.debug;  // Set debug flag from command line

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

        eprintln!("\n╔════════════════════════════════════════════════════════════════════════════╗");
        eprintln!("║                    AI Auto-Tagging Images                                    ║");
        eprintln!("╚════════════════════════════════════════════════════════════════════════════╝\n");

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
        eprintln!("\n╔════════════════════════════════════════════════════════════════════════════╗");
        eprintln!("║                    Generated Tags Preview                                   ║");
        eprintln!("╚════════════════════════════════════════════════════════════════════════════╝\n");

        for (path, tags) in ai_tags_map.iter() {
            if let Some(name) = StdPath::new(path).file_name() {
                eprintln!("{}:", name.to_string_lossy());
                eprintln!("  Tags: {}\n", tags.tags.join(", "));
            }
        }

        eprintln!("💡 Tips:");
        eprintln!("  - Tags are cached for 30 days");
        eprintln!("  - Use --tag <TAG> to filter by AI-generated tag");
        eprintln!("  - Use --clear-ai-cache to clear cache and regenerate");
        eprintln!("  - API costs vary by provider (gpt-4o-mini is cost-effective)\n");

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

    // Handle --list-tags option
    if args.list_tags {
        use grouping::list_tag_statistics;
        let image_paths: Vec<String> = images.iter().map(|img| img.path.clone()).collect();

        eprintln!("\n╔════════════════════════════════════════════════════════════════════════════╗");
        eprintln!("║                        Tag Statistics                                       ║");
        eprintln!("╚════════════════════════════════════════════════════════════════════════════╝\n");

        list_tag_statistics(&image_paths, &args.sort_tags_by)?;

        eprintln!("\n💡 Tips:");
        eprintln!("  --tag <TAG>              Show only images with specific tag");
        eprintln!("  --group-by tags          Group images by tag (one group at a time)");
        eprintln!("  --tag <TAG> --tag <TAG> Show images with multiple tags (any match)\n");

        cleanup();
        return Ok(());
    }

    // Filter by specific tags if --tag is provided
    let images = if !args.tag.is_empty() {
        use grouping::filter_by_tags;
        filter_by_tags(images, &args.tag)?
    } else {
        images
    };

    if images.is_empty() {
        eprintln!("No images match the specified tag filters.");
        cleanup();
        return Ok(());
    }

    // Determine grouping strategy
    let group_strategy = match args.group_by.as_str() {
        "similarity" => GroupBy::Similarity,
        "color" => GroupBy::Color,
        "size" => GroupBy::Size,
        "time" => GroupBy::Time,
        "tags" => GroupBy::Tags,
        _ => GroupBy::None,
    };

    // Create image configuration
    let img_config = ImageConfig::from_terminal_width(
        term_config.width,
        term_config.num_colors,
        &term_config.background,
        &term_config.foreground,
    );

    // Process and display images (with or without grouping)
    if group_strategy != GroupBy::None {
        // Extract image paths
        let image_paths: Vec<String> = images.iter().map(|img| img.path.clone()).collect();

        eprintln!("Grouping images by {:?}...", args.group_by);
        eprintln!("This may take a moment for analysis...");

        // Group images
        let groups = group_images(&image_paths, group_strategy, args.similarity_threshold)
            .context("Image grouping failed")?;

        if groups.is_empty() {
            eprintln!("No groups found.");
            cleanup();
            return Ok(());
        }

        eprintln!("Found {} group(s)", groups.len());

        // Display grouped images
        process_images_grouped(groups, images, &img_config)?;
    } else {
        // Process and display images without grouping
        process_images_concurrent(images, &img_config)?;
    }

    // Skip the waiting part - just cleanup and exit
    // The original script waits for terminal response, but it's not strictly necessary

    // Cleanup
    cleanup();

    Ok(())
}
