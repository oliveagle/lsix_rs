use anyhow::{Context, Result};
use std::collections::HashMap;
use std::io::Read;
use std::path::Path;
use std::fs;
use std::sync::{Arc, Mutex};
use serde::{Deserialize, Serialize};
use serde_json::json;

/// AI tagging configuration
#[derive(Debug, Clone)]
pub struct AITaggingConfig {
    pub api_endpoint: String,
    pub api_key: String,
    pub model: String,
    pub max_tags: usize,
    pub confidence_threshold: f32,
    pub cache_dir: Option<std::path::PathBuf>,
    pub custom_prompt: Option<String>,
    pub debug: bool,
}

impl Default for AITaggingConfig {
    fn default() -> Self {
        let api_key = std::env::var("LSIX_AI_API_KEY")
            .unwrap_or_default();

        // Detect if using local LLM (localhost or no API key)
        let is_local = std::env::var("LSIX_AI_ENDPOINT")
            .unwrap_or_default()
            .contains("localhost") || api_key.is_empty();

        // Load custom prompt from config file
        let custom_prompt = load_custom_prompt();

        Self {
            api_endpoint: std::env::var("LSIX_AI_ENDPOINT")
                .unwrap_or_else(|_| "https://api.openai.com/v1/chat/completions".to_string()),
            api_key,
            model: std::env::var("LSIX_AI_MODEL")
                .unwrap_or_else(|_| {
                    if is_local {
                        "Qwen3VL-8B-Instruct-Q8_0.gguf".to_string()
                    } else {
                        "gpt-4o-mini".to_string()
                    }
                }),
            max_tags: 10,
            confidence_threshold: 0.5,
            cache_dir: Some(
                std::path::PathBuf::from(std::env::var("HOME").unwrap_or_default())
                    .join(".cache")
                    .join("lsix")
                    .join("ai_tags")
            ),
            custom_prompt,
            debug: false,  // Default to no debug output
        }
    }
}

/// Load custom prompt from $HOME/.lsix/tag_prompt.md
fn load_custom_prompt() -> Option<String> {
    let home = std::env::var("HOME").ok()?;
    let prompt_path = std::path::PathBuf::from(home)
        .join(".lsix")
        .join("tag_prompt.md");

    if !prompt_path.exists() {
        return None;
    }

    match fs::read_to_string(&prompt_path) {
        Ok(content) => {
            // Remove leading/trailing whitespace and empty lines
            let trimmed = content.trim().to_string();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed)
            }
        }
        Err(e) => {
            eprintln!("Warning: Failed to read prompt file {:?}: {}", prompt_path, e);
            None
        }
    }
}

/// AI-generated tags for an image
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AITags {
    pub tags: Vec<String>,
    pub content_rating: Option<String>,  // Content rating: "sfw" or "nsfw"
    pub confidence: f32,
    pub model: String,
    pub timestamp: i64,
    pub cache_hit: bool,
}

/// Tag a single image using AI
pub fn tag_image_ai(image_path: &str, config: &AITaggingConfig, force: bool) -> Result<AITags> {
    // Check cache first (unless force is enabled)
    if !force {
        if let Some(cache_dir) = &config.cache_dir {
            if let Ok(cached) = load_cached_tags(cache_dir, image_path) {
                // Verify cache is not too old (30 days)
                let now = chrono::Utc::now().timestamp();
                if now - cached.timestamp < 30 * 24 * 3600 {
                    return Ok(AITags {
                        cache_hit: true,
                        ..cached
                    });
                }
            }
        }
    }

    // Encode image to base64
    let image_base64 = encode_image_to_base64(image_path)?;

    // Prepare API request - use custom prompt if available, otherwise use default
    let prompt = if let Some(custom) = &config.custom_prompt {
        // Custom prompt may contain {} placeholder for max_tags
        if custom.contains("{}") {
            custom.replace("{}", &config.max_tags.to_string())
        } else {
            custom.clone()
        }
    } else {
        // Default prompt
        format!(
            "You are an expert image tagging and content rating system. Identify the MAIN SUBJECTS and SPECIFIC OBJECTS in this image, and provide content classification.\n\
            \n\
            Focus on:\n\
            1. PRIMARY OBJECTS (clothing, products, items, people)\n\
            2. SPECIFIC DETAILS (patterns, accessories, features)\n\
            3. STYLE/GENRE (business, casual, cartoon, realistic)\n\
            4. KEY ATTRIBUTES (colors, materials, mood)\n\
            5. CONTENT CLASSIFICATION: Determine if content is appropriate for general audiences (SFW) or contains adult content (NSFW)\n\
            \n\
            CONTENT CLASSIFICATION GUIDELINES:\n\
            - SFW (Safe For Work): Family-friendly content, no nudity, minimal skin exposure, no sexual content\n\
            - NSFW (Not Safe For Work): Nudity, sexual content, excessive skin exposure, suggestive poses, adult themes\n\
            - For anime/manga: Consider typical cultural norms, but flag explicit sexual content as NSFW\n\
            - For artistic nudes: Generally NSFW unless clearly in educational/cultural context\n\
            - For clothing: Bikinis/swimwear is context-dependent (beach/sport = often SFW, intimate setting = potentially NSFW)\n\
            \n\
            IGNORE background and minor details. Tag what the image is ABOUT.\n\
            \n\
            MANDATORY: You MUST provide exactly {} tags followed by content classification. Return ONLY comma-separated tags and classification in format: 'tag1, tag2, tag3, ... , sfw|nsfw'.\n\
            Tags should be: lowercase English, 1-2 words each, very specific.\n\
            Content classification is MANDATORY and must be either 'sfw' (safe for work) or 'nsfw' (not safe for work) at the end.\n\
            DO NOT provide any other text or explanation - ONLY the comma-separated tags and classification.\n\
            \n\
            Examples:\n\
            - Photo of business suit: 'suit, formal, business, professional, office attire, sfw'\n\
            - Cartoon rabbit with watch: 'cartoon, rabbit, watch, character, minimalist, sfw'\n\
            - Beach photo: 'beach, ocean, sunset, sand, waves, horizon, tropical, sfw'\n\
            - Portrait: 'portrait, person, face, smiling, casual, indoor, sfw'",
            config.max_tags
        )
    };

    // Debug output
    if config.debug {
        eprintln!("\n╔════════════════════════════════════════════════════════════════════════════╗");
        eprintln!("║                    API Request Debug                                           ║");
        eprintln!("╚════════════════════════════════════════════════════════════════════════════╝");
        eprintln!("\n📤 Sending request to: {}", config.api_endpoint);
        eprintln!("📝 Model: {}", config.model);
        eprintln!("📄 Image: {}", image_path);
        eprintln!("📊 Image size: {} bytes (base64 encoded)", image_base64.len());
        eprintln!("\n📜 Prompt ({} characters):", prompt.len());
        eprintln!("────────────────────────────────────────────────────────────────");
        eprintln!("{}", prompt);
        eprintln!("────────────────────────────────────────────────────────────────");
    }

    let request_body = if config.api_endpoint.contains("openai") || config.api_endpoint.contains("localhost") || config.api_endpoint.contains("v1/chat/completions") {
        // OpenAI-compatible format (used by most local LLM servers too)
        json!({
            "model": config.model,
            "messages": [
                {
                    "role": "user",
                    "content": [
                        {
                            "type": "text",
                            "text": prompt
                        },
                        {
                            "type": "image_url",
                            "image_url": {
                                "url": format!("data:image/png;base64,{}", image_base64)
                            }
                        }
                    ]
                }
            ],
            "max_tokens": 200,
            "temperature": 0.8,
            "stream": false
        })
    } else {
        // Generic format (Claude, etc.)
        json!({
            "model": config.model,
            "messages": [
                {
                    "role": "user",
                    "content": prompt
                }
            ],
            "images": [image_base64],
            "max_tokens": 200
        })
    };

    // Debug output for request body
    if config.debug {
        eprintln!("\n📦 Request body (JSON):");
        eprintln!("────────────────────────────────────────────────────────────────");
        // Pretty print JSON, but truncate the base64 image data
        let debug_json = request_body.to_string();
        if debug_json.len() > 2000 {
            eprintln!("{} ... (truncated, total {} chars)", &debug_json[..2000], debug_json.len());
        } else {
            eprintln!("{}", debug_json);
        }
        eprintln!("────────────────────────────────────────────────────────────────");
    }

    // Call API
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(60))  // Longer timeout for local LLM
        .build()?;

    let mut request_builder = client
        .post(&config.api_endpoint)
        .header("Content-Type", "application/json");

    // Only add Authorization header if we have an API key
    if !config.api_key.is_empty() {
        request_builder = request_builder.header("Authorization", format!("Bearer {}", config.api_key));
    }

    let response = request_builder
        .json(&request_body)
        .send()
        .context("Failed to call AI API")?;

    let status = response.status();
    if !status.is_success() {
        let error_text = response.text().unwrap_or_default();
        anyhow::bail!("AI API error ({}): {}", status, error_text);
    }

    // Parse response
    let response_json: serde_json::Value = response.json().context("Failed to parse AI response")?;

    // Debug output for response
    if config.debug {
        eprintln!("\n╔════════════════════════════════════════════════════════════════════════════╗");
        eprintln!("║                    API Response Debug                                          ║");
        eprintln!("╚════════════════════════════════════════════════════════════════════════════╝");
        eprintln!("\n📥 Status: {}", status);
        eprintln!("\n📦 Full response JSON:");
        eprintln!("────────────────────────────────────────────────────────────────");
        eprintln!("{}", serde_json::to_string_pretty(&response_json).unwrap_or_else(|_| "Failed to pretty print".to_string()));
        eprintln!("────────────────────────────────────────────────────────────────");
    }

    // Extract tags based on response format
    let tags_text = extract_tags_from_response(&response_json)?;

    // Debug output for extracted tags text
    if config.debug {
        eprintln!("\n🔍 Extracted tags text: \"{}\"", tags_text);
    }

    // Parse tags - split by comma and process
    let all_parts: Vec<String> = tags_text
        .split(',')
        .map(|s| s.trim().to_lowercase())
        .filter(|s| !s.is_empty() && s.len() > 2)
        .collect();

    // Separate content classification from regular tags
    let mut regular_tags = Vec::new();
    let mut content_classification = None;

    for part in all_parts {
        if part == "sfw" || part == "nsfw" {
            content_classification = Some(part);
        } else if regular_tags.len() < config.max_tags {
            regular_tags.push(part);
        }
    }

    // Add content classification as a tag if it exists
    let mut tags = regular_tags;
    if let Some(classification) = content_classification {
        tags.push(classification);
    }

    // Extract content rating from tags if present
    let mut content_rating = None;
    let final_tags: Vec<String> = tags.into_iter().filter(|tag| {
        if tag == "sfw" || tag == "nsfw" {
            content_rating = Some(tag.clone());
            false  // Remove from tags
        } else {
            true   // Keep in tags
        }
    }).collect();

    // If no content rating was found, try to infer it from the tags or default to "sfw"
    let final_content_rating = if content_rating.is_none() {
        // Check if any tags suggest adult content with more comprehensive indicators
        let has_adult_content = final_tags.iter().any(|tag| {
            let lower_tag = tag.to_lowercase();
            // Explicit adult content indicators
            lower_tag.contains("nude") || lower_tag.contains("naked") ||
            lower_tag.contains("sex") || lower_tag.contains("erotic") ||
            lower_tag.contains("adult") || lower_tag.contains("porn") ||
            lower_tag.contains("sexy") || lower_tag.contains("seductive") ||
            // Body parts and suggestive terms
            lower_tag.contains("nudity") || lower_tag.contains("breast") ||
            lower_tag.contains("boob") || lower_tag.contains("butt") ||
            lower_tag.contains("ass") || lower_tag.contains("thigh") ||
            lower_tag.contains("underwear") || lower_tag.contains("lingerie") ||
            lower_tag.contains("bikini") || lower_tag.contains("swimsuit") ||
            lower_tag.contains("intimate") || lower_tag.contains("erogenous") ||
            lower_tag.contains("arousal") || lower_tag.contains("arousing") ||
            lower_tag.contains("provocative") || lower_tag.contains("suggestive") ||
            lower_tag.contains("alluring") || lower_tag.contains("tempting") ||
            lower_tag.contains("enticing") || lower_tag.contains("sultry") ||
            // Anime/manga specific indicators
            lower_tag.contains("hentai") || lower_tag.contains("ecchi") ||
            lower_tag.contains("bishoujo") || lower_tag.contains("bishounen") ||
            lower_tag.contains("bishoku") || lower_tag.contains("eromanga") ||
            // Explicit terms
            lower_tag.contains("raunchy") || lower_tag.contains("risque") ||
            lower_tag.contains("lascivious") || lower_tag.contains("lewd") ||
            lower_tag.contains("lustful") || lower_tag.contains("salacious") ||
            lower_tag.contains("indecent") || lower_tag.contains("immodest") ||
            lower_tag.contains("improper") || lower_tag.contains("unseemly") ||
            // Clothing descriptors that suggest revealing nature
            lower_tag.contains("skimpy") || lower_tag.contains("revealing") ||
            lower_tag.contains("scantily") || lower_tag.contains("scanty") ||
            lower_tag.contains("exposed") || lower_tag.contains("exposing") ||
            lower_tag.contains("exposure") || lower_tag.contains("exhibition") ||
            lower_tag.contains("undress") || lower_tag.contains("undressed") ||
            lower_tag.contains("disrobe") || lower_tag.contains("disrobed") ||
            lower_tag.contains("topless") || lower_tag.contains("bottomless") ||
            lower_tag.contains("nipple") || lower_tag.contains("areola") ||
            lower_tag.contains("genital") || lower_tag.contains("genitals") ||
            lower_tag.contains("penis") || lower_tag.contains("vagina") ||
            lower_tag.contains("pubic") || lower_tag.contains("crotch") ||
            lower_tag.contains("groin") || lower_tag.contains("thong") ||
            lower_tag.contains("micro") || lower_tag.contains("transparent") ||
            lower_tag.contains("see-through") || lower_tag.contains("sheer") ||
            lower_tag.contains("diaphanous") || lower_tag.contains("gauzy") ||
            lower_tag.contains("gossamer") || lower_tag.contains("lacy") ||
            lower_tag.contains("frilly") || lower_tag.contains("smoldering") ||
            lower_tag.contains("smouldering") || lower_tag.contains("seductive")
        });

        if has_adult_content {
            Some("nsfw".to_string())
        } else {
            // If no adult indicators, default to sfw
            Some("sfw".to_string())
        }
    } else {
        content_rating
    };

    // Debug output for final tags
    if config.debug {
        eprintln!("\n✅ Final parsed tags ({}):", final_tags.len());
        for (i, tag) in final_tags.iter().enumerate() {
            eprintln!("  {}. \"{}\"", i + 1, tag);
        }
        if let Some(rating) = &final_content_rating {
            eprintln!("  Content Rating: \"{}\"", rating);
        }
        eprintln!("\n╔════════════════════════════════════════════════════════════════════════════╗\n");
    }

    if final_tags.is_empty() {
        anyhow::bail!("No tags generated from AI response");
    }

    let ai_tags = AITags {
        tags: final_tags,
        content_rating: final_content_rating,
        confidence: 1.0,  // AI doesn't always provide confidence
        model: config.model.clone(),
        timestamp: chrono::Utc::now().timestamp(),
        cache_hit: false,
    };

    // Save to cache
    if let Some(cache_dir) = &config.cache_dir {
        let _ = save_cached_tags(cache_dir, image_path, &ai_tags);
    }

    Ok(ai_tags)
}

/// Tag multiple images in parallel
pub fn tag_images_parallel(image_paths: &[String], config: &AITaggingConfig, force: bool) -> Result<HashMap<String, AITags>> {
    use rayon::prelude::*;

    // Create progress bar
    let progress = Arc::new(Mutex::new(
        indicatif::ProgressBar::new(image_paths.len() as u64)
    ));
    let pb = progress.lock().unwrap();
    pb.set_style(indicatif::ProgressStyle::default_bar()
        .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} {msg}")
        .unwrap()
        .progress_chars("##-"));
    pb.set_message(if force { "Force regenerating tags..." } else { "Initializing..." });
    drop(pb);

    let results: Vec<(String, Result<AITags>)> = image_paths
        .par_iter()
        .map(|path| {
            let result = tag_image_ai(path, config, force);

            // Update progress
            if let Ok(ref tags) = result {
                let mut pb = progress.lock().unwrap();
                let filename = Path::new(path)
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or(path);
                pb.set_message(format!("Processing: {}", filename));
                pb.inc(1);
            }

            (path.clone(), result)
        })
        .collect();

    // Finish progress bar
    let mut pb = progress.lock().unwrap();
    pb.finish_with_message("AI tagging complete!");
    drop(pb);

    // Print summary
    let mut tags_map = HashMap::new();
    let mut success_count = 0;
    let mut cache_count = 0;
    let mut fail_count = 0;

    for (path, result) in results {
        match result {
            Ok(tags) => {
                success_count += 1;
                if tags.cache_hit {
                    cache_count += 1;
                }
                tags_map.insert(path, tags);
            }
            Err(e) => {
                fail_count += 1;
                eprintln!("✗ {}: {}", path, e);
            }
        }
    }

    // Print statistics
    if cache_count > 0 {
        eprintln!("\n📊 Statistics:");
        eprintln!("  ✓ Success: {} images", success_count);
        eprintln!("  🚀 From cache: {} images (saved API calls!)", cache_count);
        if fail_count > 0 {
            eprintln!("  ✗ Failed: {} images", fail_count);
        }
    }

    Ok(tags_map)
}

/// Encode image file to base64
fn encode_image_to_base64(image_path: &str) -> Result<String> {
    // Check file size (limit to 20MB for API)
    let metadata = fs::metadata(image_path)?;
    if metadata.len() > 20 * 1024 * 1024 {
        anyhow::bail!("Image too large for AI analysis (max 20MB)");
    }

    // Read file
    let mut file = fs::File::open(image_path)?;
    let mut buffer = Vec::new();
    file.read_to_end(&mut buffer)?;

    // Encode to base64
    Ok(base64::encode(&buffer))
}

/// Extract tags from different AI response formats
fn extract_tags_from_response(response: &serde_json::Value) -> Result<String> {
    // Try OpenAI format first
    if let Some(choices) = response.get("choices") {
        if let Some(first) = choices.as_array().and_then(|arr| arr.first()) {
            if let Some(message) = first.get("message") {
                if let Some(content) = message.get("content") {
                    if let Some(text) = content.as_str() {
                        return Ok(text.to_string());
                    }
                }
            }
        }
    }

    // Try generic format
    if let Some(content) = response.get("content") {
        if let Some(text) = content.as_str() {
            return Ok(text.to_string());
        }
    }

    // Fallback: dump entire response
    Ok(response.to_string())
}

/// Cache file path for an image
fn cache_file_path(cache_dir: &std::path::Path, image_path: &str) -> std::path::PathBuf {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut hasher = DefaultHasher::new();
    image_path.hash(&mut hasher);
    let hash = format!("{:x}", hasher.finish());

    cache_dir.join(format!("{}.json", hash))
}

/// Generate alternative cache paths for lookup (try different path formats)
fn get_cache_paths_to_try(cache_dir: &std::path::Path, image_path: &str) -> Vec<std::path::PathBuf> {
    let mut paths_to_try = Vec::new();

    // Try exact path first
    paths_to_try.push(cache_file_path(cache_dir, image_path));

    // Try with just filename (in case path was different when cached)
    if let Some(filename) = std::path::Path::new(image_path).file_name() {
        if let Some(filename_str) = filename.to_str() {
            paths_to_try.push(cache_file_path(cache_dir, filename_str));

            // Try with ./ prefix
            paths_to_try.push(cache_file_path(cache_dir, &format!("./{}", filename_str)));
        }
    }

    paths_to_try
}

/// Load cached tags from disk
pub fn load_cached_tags(cache_dir: &std::path::Path, image_path: &str) -> Result<AITags> {
    // Try multiple possible cache paths
    let paths_to_try = get_cache_paths_to_try(cache_dir, image_path);

    for cache_path in &paths_to_try {
        if cache_path.exists() {
            let cached_json = fs::read_to_string(&cache_path)?;
            let tags: AITags = serde_json::from_str(&cached_json)?;
            return Ok(tags);
        }
    }

    anyhow::bail!("Cache not found (tried {} path formats)", paths_to_try.len())
}

/// Save tags to cache
fn save_cached_tags(cache_dir: &std::path::Path, image_path: &str, tags: &AITags) -> Result<()> {
    // Ensure cache directory exists
    if !cache_dir.exists() {
        fs::create_dir_all(cache_dir)?;
    }

    let cache_path = cache_file_path(cache_dir, image_path);
    let cached_json = serde_json::to_string_pretty(tags)?;
    fs::write(&cache_path, cached_json)?;

    Ok(())
}

/// Clear AI tag cache
pub fn clear_ai_cache(config: &AITaggingConfig) -> Result<()> {
    if let Some(cache_dir) = &config.cache_dir {
        if cache_dir.exists() {
            fs::remove_dir_all(cache_dir)?;
            eprintln!("AI tag cache cleared: {}", cache_dir.display());
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cache_file_path() {
        let config = AITaggingConfig::default();
        let cache_dir = config.cache_dir.unwrap();
        let path = cache_file_path(&cache_dir, "/home/user/photo.jpg");
        assert!(path.ends_with(".json"));
    }
}
