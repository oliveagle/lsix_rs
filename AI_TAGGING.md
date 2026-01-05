# AI-Powered Image Tagging

## Overview

lsix-rs now supports automatic image tagging using Large Language Models (LLMs)! Simply provide your API key, and AI will analyze your images and generate descriptive tags.

## Supported AI Services

### OpenAI
- **Models**: GPT-4o, GPT-4o-mini, GPT-4-turbo
- **API Endpoint**: `https://api.openai.com/v1/chat/completions`
- **Cost**: $0.01-0.10 per 100 images

### Anthropic Claude
- **Models**: Claude 3 Haiku, Claude 3 Sonnet, Claude 3 Opus
- **API Endpoint**: `https://api.anthropic.com/v1/messages`
- **Cost**: $0.02-0.25 per 100 images

### OpenAI-Compatible APIs
- Any API that follows OpenAI's format
- Examples: local LLMs, cloud alternatives, etc.

## Quick Start

### 1. Set Your API Key

```bash
# OpenAI (default)
export LSIX_AI_API_KEY='sk-your-openai-key-here'

# Anthropic Claude
export LSIX_AI_API_KEY='sk-ant-your-claude-key-here'
export LSIX_AI_ENDPOINT='https://api.anthropic.com/v1/messages'

# Custom endpoint (local LLM, etc.)
export LSIX_AI_API_KEY='your-key'
export LSIX_AI_ENDPOINT='http://localhost:11434/v1/chat/completions'
```

### 2. (Optional) Configure Model

```bash
# Use cost-effective model (recommended)
export LSIX_AI_MODEL='gpt-4o-mini'

# Or high-quality model
export LSIX_AI_MODEL='gpt-4o'

# Or Claude Haiku (fast & cheap)
export LSIX_AI_MODEL='claude-3-haiku-20240307'
```

### 3. Generate Tags

```bash
# Generate tags for all images
lsix --ai-tag ~/Photos

# Generate tags for specific directory
lsix --ai-tag ~/Photos/Vacation

# Generate tags for specific files
lsix --ai-tag photo1.jpg photo2.jpg
```

## Usage Examples

### Basic Tagging

```bash
# Tag all photos in current directory
lsix --ai-tag

# Tag with specific model
LSIX_AI_MODEL='gpt-4o' lsix --ai-tag ~/Photos
```

### View Generated Tags

After tagging completes, all generated tags are displayed:

```
╔══════════════════════════════════════════════════════════════╗
║              AI Auto-Tagging Images                            ║
╚══════════════════════════════════════════════════════════════╝

Model: gpt-4o-mini
Images to process: 150

✓ beach_sunset_001.jpg: 8 tags (cached)
✓ family_portrait_002.jpg: 10 tags
✓ vacation_mountain_003.jpg: 9 tags

✓ AI tagging complete!

beach_sunset_001.jpg:
  Tags: beach, sunset, ocean, waves, orange, sky, peaceful, nature

family_portrait_002.jpg:
  Tags: family, portrait, people, smiling, happy, outdoor, summer, casual

vacation_mountain_003.jpg:
  Tags: mountain, landscape, snow, peaks, hiking, adventure, nature
```

### Filter by AI-Generated Tags

```bash
# After generating tags, use them to filter
lsix --tag beach ~/Photos
lsix --tag family --tag portrait ~/Photos
lsix --tag mountain --orientation landscape ~/Photos
```

### Clear Cache and Regenerate

```bash
# Clear old cached tags
lsix --clear-ai-cache

# Generate fresh tags
lsix --ai-tag ~/Photos
```

## How It Works

### Tag Generation Process

1. **Image Analysis**
   - Image encoded to base64
   - Sent to AI API with analysis prompt
   - AI generates 10 descriptive tags

2. **Tag Types Generated**
   - **Objects**: people, car, tree, building
   - **Activities**: running, swimming, reading
   - **Locations**: beach, mountain, city, home
   - **Colors**: blue, green, orange, bright
   - **Mood**: happy, peaceful, dramatic, joyful
   - **Style**: portrait, landscape, abstract
   - **Time**: day, night, sunset, sunrise

3. **Caching**
   - Tags stored in `~/.cache/lsix/ai_tags/`
   - Valid for 30 days
   - Instant retrieval on subsequent runs
   - Reduces API costs significantly

### AI Prompt

The AI is instructed to:
```
Generate 10 descriptive tags for this image.
Consider: objects, people, activities, locations, colors, mood, style.
Return ONLY a comma-separated list, no explanation.
Tags should be: concise (1-3 words), specific, lowercase English.
```

## Configuration

### Environment Variables

| Variable | Required | Default | Description |
|----------|----------|---------|-------------|
| `LSIX_AI_API_KEY` | Yes | - | Your API key |
| `LSIX_AI_ENDPOINT` | No | OpenAI endpoint | API URL |
| `LSIX_AI_MODEL` | No | `gpt-4o-mini` | Model name |

### Model Recommendations

**For Cost-Effectiveness**:
```bash
export LSIX_AI_MODEL='gpt-4o-mini'     # $0.01/100 images
export LSIX_AI_MODEL='claude-3-haiku'  # $0.02/100 images
```

**For Quality**:
```bash
export LSIX_AI_MODEL='gpt-4o'           # $0.10/100 images
export LSIX_AI_MODEL='claude-3-sonnet'   # $0.15/100 images
```

**For Speed**:
```bash
export LSIX_AI_MODEL='gpt-4o-mini'     # Fastest
export LSIX_AI_MODEL='claude-3-haiku'  # Very fast
```

## Workflow Examples

### Example 1: Organize Vacation Photos

```bash
# Step 1: Generate tags
lsix --ai-tag ~/Photos/Vacation

# Step 2: View what tags were generated
# (Output is displayed automatically)

# Step 3: Filter by specific tags
lsix --tag beach ~/Photos/Vacation
lsix --tag mountain ~/Photos/Vacation
lsix --tag family ~/Photos/Vacation
```

### Example 2: Find All Portrait Photos

```bash
# Generate tags first
lsix --ai-tag ~/Photos

# View all portraits
lsix --tag portrait --orientation portrait ~/Photos

# View family portraits
lsix --tag portrait --tag family ~/Photos
```

### Example 3: Create Mood Albums

```bash
# Generate tags
lsix --ai-tag ~/Photos

# Happy moments
lsix --tag happy --tag smiling ~/Photos

# Dramatic scenes
lsix --tag dramatic --tag sunset ~/Photos

# Peaceful nature
lsix --tag peaceful --tag nature ~/Photos
```

## Performance & Cost

### Speed

- **First run**: ~2-5 seconds per image (API dependent)
- **Cached run**: <0.1 seconds per image
- **Parallel processing**: Multiple images tagged simultaneously

### Cost Estimates (per 100 images)

| Model | Cost | Speed | Quality |
|-------|------|-------|--------|
| gpt-4o-mini | $0.01 | Fast | Good |
| gpt-4o | $0.10 | Fast | Excellent |
| claude-3-haiku | $0.02 | Very Fast | Good |
| claude-3-sonnet | $0.15 | Fast | Excellent |

**Caching Benefit**: After first run, 30 days of free tag access!

## Best Practices

### 1. Start Small

```bash
# Test with a few images first
lsix --ai-tag test_image.jpg

# Verify tags look good, then process full collection
lsix --ai-tag ~/Photos
```

### 2. Use Cost-Effective Models

```bash
# gpt-4o-mini is usually good enough
export LSIX_AI_MODEL='gpt-4o-mini'
lsix --ai-tag ~/Photos
```

### 3. Organize Before Tagging

```bash
# Tag specific folders separately
lsix --ai-tag ~/Photos/2024/Vacation
lsix --ai-tag ~/Photos/2024/Birthday
lsix --ai-tag ~/Photos/2024/Christmas
```

### 4. Combine with Other Features

```bash
# Tag + filter + group
lsix --ai-tag ~/Photos
lsix --tag beach --min-width 1920 --group-by time ~/Photos
```

## Troubleshooting

### "API key not set"

```bash
# Make sure API key is exported
export LSIX_AI_API_KEY='your-key-here'

# Verify it's set
echo $LSIX_AI_API_KEY

# Try again
lsix --ai-tag ~/Photos
```

### "Image too large"

AI tagging has a 20MB limit per image:

```bash
# Resize large images first
mogrify -resize "2048x2048>" large_photo.jpg

# Then tag
lsix --ai-tag large_photo.jpg
```

### "API timeout"

Increase timeout or reduce image size:

```bash
# Use smaller model (faster)
export LSIX_AI_MODEL='gpt-4o-mini'

# Or tag fewer images at once
lsix --ai-tag ~/Photos/Subfolder
```

### Poor Quality Tags

```bash
# Try a better model
export LSIX_AI_MODEL='gpt-4o'
lsix --clear-ai-cache
lsix --ai-tag ~/Photos
```

## Advanced Usage

### Custom API Endpoint (Local LLM)

```bash
# Using Ollama or similar
export LSIX_AI_API_KEY='ollama'
export LSIX_AI_ENDPOINT='http://localhost:11434/v1/chat/completions'
export LSIX_AI_MODEL='llava:latest'

lsix --ai-tag ~/Photos
```

### Batch Processing

```bash
# Process multiple folders
for folder in ~/Photos/2024/*; do
    echo "Tagging $folder..."
    lsix --ai-tag "$folder"
done
```

### Script Integration

```bash
#!/bin/bash
# auto-tag-and-organize.sh

API_KEY='your-key'
export LSIX_AI_API_KEY=$API_KEY

PHOTOS_DIR=~/Photos

# Generate tags
lsix --ai-tag "$PHOTOS_DIR"

# Organize by tags
mkdir -p "$PHOTOS_DIR/Organized"
for tag in beach mountain family portrait; do
    mkdir -p "$PHOTOS_DIR/Organized/$tag"
    lsix --tag "$tag" "$PHOTOS_DIR" | while read file; do
        ln -s "$file" "$PHOTOS_DIR/Organized/$tag/"
    done
done
```

## Tips & Tricks

### 1. Progressive Tagging

```bash
# Tag in stages if you have many photos
lsix --ai-tag ~/Photos/2024/01
lsix --ai-tag ~/Photos/2024/02
# ... etc
```

### 2. Tag Validation

```bash
# Generate tags
lsix --ai-tag test.jpg

# View if tags make sense
# If not, try different model
export LSIX_AI_MODEL='gpt-4o'
lsix --clear-ai-cache
lsix --ai-tag test.jpg
```

### 3. Cost Monitoring

```bash
# Estimate cost before tagging
IMAGE_COUNT=$(find ~/Photos -type f \( -name "*.jpg" -o -name "*.png" \) | wc -l)
echo "Approx images: $IMAGE_COUNT"
echo "Cost with gpt-4o-mini: ~$((IMAGE_COUNT * 0.01 / 100)) USD"

# Then tag
lsix --ai-tag ~/Photos
```

### 4. Tag Combination Strategies

```bash
# Find specific combinations
lsix --tag beach --tag sunset ~/Photos
lsix --tag family --tag happy --tag outdoor ~/Photos
lsix --tag mountain --tag landscape --tag dramatic ~/Photos
```

## Comparison: AI vs Manual vs Filename

| Method | Time | Quality | Consistency | Cost |
|--------|------|--------|-------------|------|
| AI Auto-Tagging | Initial: Slow, Cached: Instant | Excellent | High | $0.01-0.10/100 images |
| Manual Tagging | Very Slow | Best | Variable | Free (time expensive) |
| Filename Extraction | Instant | Poor | Low | Free |

## API Key Security

### Best Practices

```bash
# Don't hardcode API key in scripts
# Use environment variables instead

# Good
export LSIX_AI_API_KEY='sk-key'
lsix --ai-tag ~/Photos

# Bad (don't do this)
LSIX_AI_API_KEY='sk-key' lsix --ai-tag ~/Photos

# Add to ~/.bashrc or ~/.zshrc for persistence
echo 'export LSIX_AI_API_KEY="sk-your-key"' >> ~/.bashrc
source ~/.bashrc
```

### Temporary Key

```bash
# Set key for current session only
export LSIX_AI_API_KEY='sk-temp-key'
lsix --ai-tag ~/Photos
# Key is gone when terminal closes
```

## Future Enhancements

Potential improvements:
- Custom tag categories
- Confidence scores for tags
- Tag relationships and hierarchy
- Face detection and naming
- EXIF data integration
- Batch tag editing
- Tag export/import

## See Also

- `FILTERS.md` - How to filter by tags
- `GROUPING.md` - How to group by tags
- `PERFORMANCE.md` - Performance optimization
