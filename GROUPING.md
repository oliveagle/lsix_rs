# lsix-rs Image Grouping System

## Overview

lsix-rs now supports intelligent image grouping to organize your photos automatically. Group by visual similarity, color, size, time, or tags!

## Grouping Modes

### 1. Similarity Grouping (按相似度分组)

Groups visually similar images together using perceptual hashing.

**Best for**:
- Finding duplicate/near-duplicate photos
- Grouping photos of the same subject
- Organizing burst mode photos
- Finding different angles of the same scene

```bash
# Group similar images (default threshold: 0.85)
lsix --group-by similarity ~/Photos

# Use stricter threshold (only very similar images)
lsix --group-by similarity --similarity-threshold 0.95 ~/Photos

# Use looser threshold (more permissive matching)
lsix --group-by similarity --similarity-threshold 0.70 ~/Photos
```

**How it works**:
- Calculates a perceptual hash for each image
- Groups images with hash similarity above threshold
- Threshold range: 0.0 (no similarity) to 1.0 (identical)

**Example output**:
```
╔═══════════════════════════════════════════════════════════════
║ Group 1: Similar Group 1 (3 images)
╚═══════════════════════════════════════════════════════════════

[Images display here...]

╔═══════════════════════════════════════════════════════════════
║ Group 2: Similar Group 2 (5 images)
╚═══════════════════════════════════════════════════════════════
```

### 2. Color Grouping (按颜色分组)

Groups images by dominant color.

**Best for**:
- Finding photos with similar color schemes
- Organizing by mood/atmosphere
- Color-based photo sorting

```bash
# Group by color
lsix --group-by color ~/Photos

# Combine with color filters
lsix --group-by color --min-brightness 0.7 ~/Photos
```

**How it works**:
- Analyzes color histogram of each image
- Groups images with similar color distributions
- Uses similarity threshold (default: 0.85)

**Example output**:
```
╔═══════════════════════════════════════════════════════════════
║ Group 1: Color Images (4 images)
║ dominant_color: Color
╚═══════════════════════════════════════════════════════════════
```

### 3. Size Grouping (按尺寸分组)

Groups images by resolution (width × height).

**Best for**:
- Organizing wallpapers by resolution
- Finding photos from specific cameras
- Separating thumbnails from full-size images
- Batch processing by size

```bash
# Group by resolution
lsix --group-by size ~/Photos

# Combine with size filters
lsix --group-by size --min-width 1920 ~/Photos
```

**How it works**:
- Rounds dimensions to nearest 100px
- Groups images with same rounded resolution
- Example: 1920×1080, 1954×1093, 1899×1076 → same group

**Example output**:
```
╔═══════════════════════════════════════════════════════════════
║ Group 1: 1920x1080 Images (12 images)
║ resolution: 1900x1100
╚═══════════════════════════════════════════════════════════════

╔═══════════════════════════════════════════════════════════════
║ Group 2: 800x600 Images (8 images)
║ resolution: 800x600
╚═══════════════════════════════════════════════════════════════
```

### 4. Time Grouping (按时间分组)

Groups images by date taken.

**Best for**:
- Timeline-based photo organization
- Finding photos from specific events
- Chronological browsing
- Daily/weekly photo dumps

```bash
# Group by date
lsix --group-by time ~/Photos

# Show photos from last vacation
lsix --group-by time ~/Photos/Vacation
```

**How it works**:
- Uses file modification time
- Groups photos taken on same day
- Shows date in group header

**Example output**:
```
╔═══════════════════════════════════════════════════════════════
║ Group 1: 2024-01-15 Images (45 images)
║ date: 2024-01-15
╚═══════════════════════════════════════════════════════════════

╔═══════════════════════════════════════════════════════════════
║ Group 2: 2024-01-16 Images (23 images)
║ date: 2024-01-16
╚═══════════════════════════════════════════════════════════════
```

### 5. Tags Grouping (按标签分组)

Groups images by auto-detected tags from filename/path.

**Best for**:
- Organizing by subject (people, places, events)
- Finding photos from specific cameras
- Organizing by file type
- Quick categorization

```bash
# Group by tags
lsix --group-by tags ~/Photos

# Shows tags extracted from:
# - Directory names (e.g., "Beach", "Birthday")
# - Filename parts (e.g., "vacation", "party")
# - File extensions (e.g., "JPG", "PNG")
```

**How it works**:
- Extracts tags from directory names
- Extracts tags from filename parts
- Groups by common tags
- Automatic, no manual tagging needed

**Example**:
For file: `/home/user/Photos/Birthday/alice_party_001.jpg`

Tags extracted:
- `Birthday` (directory)
- `alice` (filename)
- `party` (filename)
- `JPG` (extension)

**Example output**:
```
╔═══════════════════════════════════════════════════════════════
║ Group 1: Vacation Images (15 images)
║ tag: Vacation
╚═══════════════════════════════════════════════════════════════

╔═══════════════════════════════════════════════════════════════
║ Group 2: alice Images (8 images)
║ tag: alice
╚═══════════════════════════════════════════════════════════════
```

## Advanced Usage

### Combining Grouping with Filtering

You can combine grouping with any filter:

```bash
# Group high-res landscape photos by date
lsix \
  --group-by time \
  --orientation landscape \
  --min-width 1920 \
  ~/Photos

# Group bright photos by color
lsix \
  --group-by color \
  --min-brightness 0.7 \
  ~/Photos

# Group recent photos by similarity
lsix \
  --group-by similarity \
  --min-file-size 1M \
  ~/Camera
```

### Choosing the Right Threshold

The `--similarity-threshold` controls how strict grouping is:

**Threshold Guidelines**:
- `0.90-0.99` - Nearly identical (duplicates, burst mode)
- `0.80-0.89` - Very similar (same subject, slightly different)
- `0.70-0.79` - Similar (same scene, different angles)
- `0.60-0.69` - Somewhat similar (same location, different time)
- `0.50-0.59` - Loose similarity (same colors/composition)

**Examples**:
```bash
# Find exact/near-duplicates
lsix --group-by similarity --similarity-threshold 0.95 ~/Downloads

# Group photos of same person
lsix --group-by similarity --similarity-threshold 0.75 ~/Photos

# Group by general scene
lsix --group-by similarity --similarity-threshold 0.65 ~/Vacation
```

## Performance

### Speed by Grouping Mode

1. **Size** - Fastest (< 1s for 1000 images)
2. **Time** - Very fast (< 2s for 1000 images)
3. **Tags** - Fast (< 3s for 1000 images)
4. **Color** - Medium (5-10s for 1000 images)
5. **Similarity** - Slower (10-30s for 1000 images)

### Optimization Tips

1. **Use size/time for quick organization** - Instant results
2. **Combine with filters** - Reduce analysis load
3. **Use caching** - Second run is much faster
4. **Start with looser threshold** - Then adjust tighter

## Practical Examples

### Photo Organization Workflow

```bash
# Step 1: Find duplicates
lsix --group-by similarity --similarity-threshold 0.95 ~/Downloads

# Step 2: Organize by date
lsix --group-by time ~/Photos

# Step 3: Find wallpapers
lsix --group-by size --orientation landscape --min-width 1920 ~/Pictures

# Step 4: Organize by event (tags)
lsix --group-by tags ~/Photos/2024
```

### Finding Similar Photos

```bash
# Find photos of the same person
lsix --group-by similarity --similarity-threshold 0.80 ~/Photos/Family

# Find photos from same trip
lsix --group-by similarity --similarity-threshold 0.70 ~/Photos/Vacation

# Find alternative shots
lsix --group-by similarity --similarity-threshold 0.85 ~/Camera
```

### Wallpaper Management

```bash
# 4K wallpapers by similarity
lsix \
  --group-by similarity \
  --min-width 3840 \
  --min-height 2160 \
  ~/Wallpapers

# Phone wallpapers by size
lsix \
  --group-by size \
  --orientation portrait \
  --min-width 1080 \
  --min-height 1920 \
  ~/Mobile/Wallpapers
```

### Social Media Management

```bash
# Group Instagram photos by date
lsix --group-by time ~/Pictures/Instagram

# Find unused photos (not posted)
lsix --group-by time ~/Photos/New

# Group portraits
lsix --group-by similarity --orientation portrait ~/Photos/Portraits
```

## Tips and Tricks

### Naming Convention for Better Tagging

Organize files with descriptive names for better tag grouping:

```
Good:
~/Photos/2024/Birthday/alice_cake_001.jpg
~/Photos/2024/Birthday/bob_gifts_002.jpg

Bad:
~/Photos/IMG_001.jpg
~/Photos/IMG_002.jpg
```

### Combining Grouping Methods

Run multiple grouping passes to get different perspectives:

```bash
# First pass: time groups
lsix --group-by time ~/Photos

# Second pass: similarity within each day
lsix --group-by similarity ~/Photos/2024-01-15
```

### Performance vs Accuracy

- **Fastest**: `--group-by size` or `--group-by time`
- **Best accuracy**: `--group-by similarity --similarity-threshold 0.90`
- **Balanced**: `--group-by color` or `--group-by tags`

## Troubleshooting

### Too Many/Few Groups

**Problem**: All images in one group
**Solution**: Lower threshold
```bash
lsix --group-by similarity --similarity-threshold 0.70
```

**Problem**: Too many small groups
**Solution**: Raise threshold
```bash
lsix --group-by similarity --similarity-threshold 0.90
```

### Slow Performance

**Problem**: Grouping takes too long
**Solutions**:
1. Use faster grouping mode (size/time)
2. Filter first to reduce image count
3. Use caching (second run is fast)
4. Lower threshold (fewer comparisons)

### Wrong Groups

**Problem**: Images don't belong together
**Solutions**:
1. Adjust similarity threshold
2. Try different grouping mode
3. Add filters to narrow scope
4. Check image quality (blurry/dark images)

## Command Reference

### Grouping Options

- `--group-by MODE` - Group by: none, similarity, color, size, time, tags
- `--similarity-threshold N` - Similarity threshold (0.0 to 1.0, default: 0.85)

### See Also

- `lsix --help` - All command line options
- `FILTERS.md` - Image filtering guide
- `PERFORMANCE.md` - Performance optimization
- `CACHING.md` - How caching works

## Examples Gallery

### Duplicate Finder
```bash
lsix --group-by similarity --similarity-threshold 0.98 ~/Downloads
```

### Timeline View
```bash
lsix --group-by time ~/Photos
```

### Wallpapers by Resolution
```bash
lsix --group-by size --orientation landscape ~/Wallpapers
```

### Event Organization
```bash
lsix --group-by tags ~/Photos/Vacation
```

### Color Mood Board
```bash
lsix --group-by color --min-brightness 0.7 ~/Inspiration
```
