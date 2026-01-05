use anyhow::{Context, Result};
use std::collections::HashMap;
use std::io::Read;
use std::path::Path;
use std::fs;
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
}

impl Default for AITaggingConfig {
    fn default() -> Self {
        Self {
            api_endpoint: std::env::var("LSIX_AI_ENDPOINT")
                .unwrap_or_else(|_| "https://api.openai.com/v1/chat/completions".to_string()),
            api_key: std::env::var("LSIX_AI_API_KEY")
                .unwrap_or_default(),
            model: std::env::var("LSIX_AI_MODEL")
                .unwrap_or_else(|_| "gpt-4o-mini".to_string()),  // Cost-effective
            max_tags: 10,
            confidence_threshold: 0.5,
            cache_dir: Some(
                std::path::PathBuf::from(std::env::var("HOME").unwrap_or_default())
                    .join(".cache")
                    .join("lsix")
                    .join("ai_tags")
            ),
        }
    }
}

/// AI-generated tags for an image
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AITags {
    pub tags: Vec<String>,
    pub confidence: f32,
    pub model: String,
    pub timestamp: i64,
    pub cache_hit: bool,
}

/// Tag a single image using AI
pub fn tag_image_ai(image_path: &str, config: &AITaggingConfig) -> Result<AITags> {
    // Check cache first
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

    // Encode image to base64
    let image_base64 = encode_image_to_base64(image_path)?;

    // Prepare API request
    let prompt = format!(
        "Analyze this image and generate {} descriptive tags. \
        Consider: objects, people, activities, locations, colors, mood, style. \
        Return ONLY a comma-separated list of tags, no explanation. \
        Tags should be: concise (1-3 words), specific, lowercase English. \
        Examples: 'beach, sunset, family, vacation, ocean, summer, happy, casual'",
        config.max_tags
    );

    let request_body = if config.api_endpoint.contains("openai") {
        // OpenAI format
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
                                "url": format!("data:image/jpeg;base64,{}", image_base64)
                            }
                        }
                    ]
                }
            ],
            "max_tokens": 200
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

    // Call API
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()?;

    let response = client
        .post(&config.api_endpoint)
        .header("Authorization", format!("Bearer {}", config.api_key))
        .header("Content-Type", "application/json")
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

    // Extract tags based on response format
    let tags_text = extract_tags_from_response(&response_json)?;

    // Parse tags
    let tags: Vec<String> = tags_text
        .split(',')
        .map(|s| s.trim().to_lowercase())
        .filter(|s| !s.is_empty() && s.len() > 2)
        .take(config.max_tags)
        .collect();

    if tags.is_empty() {
        anyhow::bail!("No tags generated from AI response");
    }

    let ai_tags = AITags {
        tags,
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
pub fn tag_images_parallel(image_paths: &[String], config: &AITaggingConfig) -> Result<HashMap<String, AITags>> {
    use rayon::prelude::*;

    let results: Vec<(String, Result<AITags>)> = image_paths
        .par_iter()
        .map(|path| {
            let result = tag_image_ai(path, config);
            (path.clone(), result)
        })
        .collect();

    let mut tags_map = HashMap::new();
    for (path, result) in results {
        match result {
            Ok(tags) => {
                eprintln!("✓ {}: {} tags {}", path, tags.tags.len(),
                         if tags.cache_hit { "(cached)" } else { "" });
                tags_map.insert(path, tags);
            }
            Err(e) => {
                eprintln!("✗ {}: {}", path, e);
            }
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

/// Load cached tags from disk
fn load_cached_tags(cache_dir: &std::path::Path, image_path: &str) -> Result<AITags> {
    let cache_path = cache_file_path(cache_dir, image_path);

    if !cache_path.exists() {
        anyhow::bail!("Cache not found");
    }

    let cached_json = fs::read_to_string(&cache_path)?;
    let tags: AITags = serde_json::from_str(&cached_json)?;

    Ok(tags)
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
