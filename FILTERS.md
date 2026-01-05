# lsix-rs Image Filtering

## Overview

lsix-rs now supports powerful image filtering based on:
1. **Size filters** - width, height, file size
2. **Color filters** - brightness
3. **Orientation filters** - landscape, portrait, square

## Usage Examples

### Size Filters

#### Filter by Image Dimensions

```bash
# Only show images at least 1920px wide
lsix --min-width 1920 /path/to/images

# Only show images at most 1080px high
lsix --max-height 1080 /path/to/images

# Combine min and max
lsix --min-width 800 --max-width 1920 /path/to/images
```

#### Filter by File Size

```bash
# Only show images larger than 1MB
lsix --min-file-size 1M /path/to/images

# Only show images smaller than 10MB
lsix --max-file-size 10M /path/to/images

# Only show images between 500K and 5MB
lsix --min-file-size 500K --max-file-size 5M /path/to/images
```

**File size units**: K (KB), M (MB), G (GB), T (TB)

### Color Filters

#### Filter by Brightness

Brightness is measured from 0.0 (completely dark) to 1.0 (completely bright).

```bash
# Only show bright images (> 0.7)
lsix --min-brightness 0.7 /path/to/images

# Only show dark images (< 0.3)
lsix --max-brightness 0.3 /path/to/images

# Only show medium brightness images
lsix --min-brightness 0.3 --max-brightness 0.7 /path/to/images
```

### Orientation Filters

```bash
# Only show landscape images (width > height)
lsix --orientation landscape /path/to/images

# Only show portrait images (height > width)
lsix --orientation portrait /path/to/images

# Only show square images (width ≈ height)
lsix --orientation square /path/to/images

# Short aliases
lsix --orientation h /path/to/images  # landscape
lsix --orientation v /path/to/images  # portrait
lsix --orientation s /path/to/images  # square
```

### Combine Multiple Filters

You can combine any number of filters:

```bash
# Landscape images, at least 1920px wide, > 1MB
lsix \
  --orientation landscape \
  --min-width 1920 \
  --min-file-size 1M \
  /path/to/images

# Bright, medium-sized portrait images
lsix \
  --orientation portrait \
  --min-brightness 0.6 \
  --min-width 800 \
  --max-width 1920 \
  /path/to/images

# Dark, large landscape wallpapers
lsix \
  --orientation landscape \
  --max-brightness 0.3 \
  --min-width 1920 \
  --min-height 1080 \
  --min-file-size 2M \
  /path/to/wallpapers
```

## Practical Examples

### Find High-Resolution Wallpapers

```bash
lsix \
  --orientation landscape \
  --min-width 1920 \
  --min-height 1080 \
  --min-file-size 500K \
  ~/Pictures/Wallpapers
```

### Find Small Thumbnails

```bash
lsix \
  --max-width 200 \
  --max-height 200 \
  --max-file-size 100K \
  ~/Pictures/Thumbnails
```

### Find Bright Outdoor Photos

```bash
lsix \
  --min-brightness 0.7 \
  --orientation landscape \
  ~/Pictures/Vacation
```

### Find Dark Indoor Photos

```bash
lsix \
  --max-brightness 0.4 \
  ~/Pictures/Indoor
```

### Find Phone Photos (Portrait)

```bash
lsix \
  --orientation portrait \
  --min-width 1080 \
  --min-height 1920 \
  ~/Camera/Phone
```

## Performance Notes

### Filter Speed

- **Without filters**: Instant (file system only)
- **With filters**: Slightly slower (uses ImageMagick to analyze)
  - First run: ~0.1-0.5s per image for analysis
  - Parallel processing: Multiple images analyzed simultaneously

### Optimization Tips

1. **Use filters when you need them** - Only analyze when filtering
2. **Combine with caching** - Filter results are cached
3. **Be specific** - More specific filters = faster matching
4. **Use size filters first** - Faster than color analysis

## How It Works

### Image Analysis

When filters are active, lsix-rs uses ImageMagick's `identify` command to extract:
- Width and height (in pixels)
- File size (from filesystem)
- Brightness (mean pixel value)
- Aspect ratio (for orientation)

### Filter Matching

Images are checked against ALL specified filters:
- If image matches all criteria → Display it
- If image fails any criterion → Skip it

### Performance Optimization

- Filters are checked in parallel (multiple images at once)
- Only analyzes when filters are specified
- Graceful fallback if analysis fails (includes image anyway)

## Advanced Usage

### Filter Scripts

Create filter scripts for common use cases:

```bash
#!/bin/bash
# ~/bin/lsix-wallpapers
lsix \
  --orientation landscape \
  --min-width 1920 \
  --min-height 1080 \
  --min-file-size 500K \
  "$@"
```

Usage:
```bash
lsix-wallpapers ~/Pictures
```

### Filter Chains

Combine with other tools:

```bash
# Find and display wallpapers, then copy them
lsix --orientation landscape --min-width 1920 ~/Pictures | \
  grep "wallpaper" | \
  xargs cp -t ~/Wallpapers/
```

## Troubleshooting

### "Failed to analyze" Warnings

If you see warnings about failed analysis:
- Image file might be corrupted
- Image format not supported by ImageMagick
- Image will still be included (failsafe behavior)

### Slow Performance

If filtering is slow:
- Reduce number of filters
- Use size filters instead of color filters
- Use caching (second run is instant)

### No Images Match

If no images are displayed:
- Check filter criteria are not too restrictive
- Run without filters first to verify images exist
- Use `--min-brightness 0 --max-brightness 1` to accept all

## Command Reference

### Size Filters

- `--min-width N` - Minimum width in pixels
- `--max-width N` - Maximum width in pixels
- `--min-height N` - Minimum height in pixels
- `--max-height N` - Maximum height in pixels
- `--min-file-size N` - Minimum file size (e.g., 100K, 1M, 1G)
- `--max-file-size N` - Maximum file size (e.g., 100K, 1M, 1G)

### Color Filters

- `--min-brightness N` - Minimum brightness (0.0 to 1.0)
- `--max-brightness N` - Maximum brightness (0.0 to 1.0)

### Orientation Filters

- `--orientation TYPE` - Filter by orientation
  - `landscape` or `h` - Width > height
  - `portrait` or `v` - Height > width
  - `square` or `s` - Width ≈ height

### See Also

- `lsix --help` - All command line options
- `PERFORMANCE.md` - Performance optimization guide
- `CACHING.md` - How caching works
