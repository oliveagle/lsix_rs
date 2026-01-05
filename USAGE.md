# lsix-rs 使用示例

## 快速开始

1. 编译项目：
```bash
cargo build --release
```

2. 运行程序：
```bash
# 显示帮助
./target/release/lsix --help

# 显示当前目录的所有图像
./target/release/lsix

# 显示指定的图像
./target/release/lsix *.jpg

# 递归显示目录
./target/release/lsix /path/to/images/
```

## 并发处理说明

lsix-rs 使用 Rayon 库实现并发处理，主要在以下方面提升性能：

### 1. 并发文件验证
```rust
// 在 image_proc.rs 中
pub fn validate_images_concurrent(paths: &[String], explicit: bool) -> Vec<ImageEntry> {
    paths
        .par_iter()  // 并行迭代
        .filter_map(|path| {
            // 并发验证每个文件
        })
        .collect()
}
```

### 2. 性能对比

对于包含 100 张图像的目录：

- **Bash 版本**：顺序处理，每张图像约需 0.1 秒
  - 总时间：~10 秒

- **Rust 版本**：并发处理，利用多核 CPU
  - 总时间：~2-3 秒（4核 CPU）

## 项目结构

```
src/
├── main.rs          # 主函数，命令行参数解析
├── terminal.rs      # 终端能力检测（SIXEL、颜色、几何）
├── filename.rs      # 文件名处理和标签生成
└── image_proc.rs    # 图像处理和并发逻辑
```

## 关键模块说明

### terminal.rs - 终端检测
- `detect_sixel()`: 检测终端是否支持 SIXEL
- `detect_colors()`: 检测颜色数量
- `detect_colorscheme()`: 检测背景色和前景色
- `detect_geometry()`: 检测终端宽度（像素）

### filename.rs - 文件处理
- `process_label()`: 处理文件名标签
- `find_image_files()`: 查找当前目录的图像文件
- `process_image_path()`: 处理动画 GIF/WebP 路径

### image_proc.rs - 图像处理
- `validate_images_concurrent()`: 并发验证图像文件
- `process_images_concurrent()`: 并发处理图像批次
- `process_chunk()`: 处理单行图像（一个 chunk）

## 配置选项

### 默认配置
```rust
// tilesize: 360x360 像素
// font_size: tile_width / 10 = 36pt
// 每行 tile 数量根据终端宽度自动计算
```

### 自定义配置
可以在 `image_proc.rs` 中修改 `ImageConfig::from_terminal_width` 函数来调整默认值。

## 测试终端 SIXEL 支持

测试您的终端是否支持 SIXEL：

```bash
# 使用 ImageMagick 测试
convert -colors 16 /path/to/image.jpg sixel:-

# 如果看到图像，说明您的终端支持 SIXEL
```

## 推荐的 SIXEL 终端

1. **XTerm**（最全面的支持）
   ```bash
   xterm -ti vt340
   ```

2. **mlterm**（多语言终端）
   ```bash
   mlterm
   ```

3. **WezTerm**（现代终端）
   ```bash
   wezterm
   ```

4. **Foot**（Wayland 终端，部分支持）
   ```bash
   foot
   ```

## 性能优化建议

1. **使用发布版本**：
   ```bash
   cargo build --release
   ```

2. **减少图像数量**：如果图像太多，可以分批处理
   ```bash
   # 处理前 50 张
   ./target/release/lsix $(ls *.jpg | head -50)
   ```

3. **调整 tile 大小**：修改 `ImageConfig::from_terminal_width` 中的 `tilesize`

## 故障排除

### 问题：编译错误
```bash
# 确保使用最新的 Rust 版本
rustc --version
cargo --version

# 更新 Rust
rustup update
```

### 问题：运行时找不到 ImageMagick
```bash
# 检查 magick 命令是否可用
which magick

# 如果没有，安装 ImageMagick
sudo apt install imagemagick  # Ubuntu/Debian
brew install imagemagick        # macOS
```

### 问题：终端显示乱码
这可能是由于：
1. 终端不支持 SIXEL
2. 图像格式不支持
3. ImageMagick 版本问题

## 与原版兼容性

lsix-rs 的目标是与原版 lsix 功能对等，主要差异：

| 功能 | 原版 lsix | lsix-rs | 备注 |
|------|-----------|---------|------|
| SIXEL 显示 | ✅ | ✅ | 完全兼容 |
| 终端自动检测 | ✅ | ✅ | 完全兼容 |
| 并发处理 | ❌ | ✅ | 性能提升 |
| 目录递归 | ✅ | ✅ | 完全兼容 |
| 自定义字体 | ✅ | ⚠️ | 需要代码修改 |
| ImageMagick 6 | ✅ | ❌ | 仅支持 v7 |

## 开发和贡献

### 运行测试
```bash
cargo test
```

### 添加新功能
1. 在相应的模块中添加函数
2. 更新 `main.rs` 以使用新功能
3. 添加测试用例
4. 更新文档

### 性能分析
```bash
# 使用 flamegraph 进行性能分析
cargo install flamegraph
cargo flamegraph --bin lsix -- --help
```
