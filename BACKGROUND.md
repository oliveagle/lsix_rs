# lsix-rs 背景色设置说明

## SIXEL 格式限制

**重要：SIXEL 格式不支持透明背景**

SIXEL 是一种较老的位图格式，不支持 alpha 通道（透明度）。即使您指定 `-background none`，ImageMagick 也会用默认颜色替代。

## 解决方案

### 方案 1：使用环境变量设置背景色（推荐）

```bash
# 暗色终端（最常见）
export LSIX_BACKGROUND="#1e1e1e"
export LSIX_FOREGROUND="white"

# 纯黑终端
export LSIX_BACKGROUND="black"
export LSIX_FOREGROUND="white"

# 浅色终端
export LSIX_BACKGROUND="white"
export LSIX_FOREGROUND="black"

# 自定义颜色
export LSIX_BACKGROUND="#282a36"
export LSIX_FOREGROUND="#f8f8f2"
```

### 方案 2：永久设置（添加到 ~/.bashrc 或 ~/.zshrc）

```bash
# 添加到 ~/.bashrc
echo 'export LSIX_BACKGROUND="#1e1e1e"' >> ~/.bashrc
echo 'export LSIX_FOREGROUND="white"' >> ~/.bashrc
source ~/.bashrc
```

### 方案 3：使用默认值

程序默认使用 `#282828`（中性暗灰色）作为背景色，这对大多数暗色终端效果不错。

## 如何找到您的终端背景色

### 方法 1：查看终端设置

大多数终端模拟器的设置中会显示背景色的十六进制值。

### 方法 2：使用脚本检测

```bash
# 检测终端背景色（需要终端支持）
echo -ne '\e]11;?\e\\'
read -s -t 1 -d "\\" response
echo "Terminal background: $response"
```

### 方法 3：常见终端的默认背景色

| 终端 | 默认背景色 |
|------|-----------|
| GNOME Terminal | `#2e3436` |
| iTerm2 | `#1e1e1e` |
| Alacritty | `#1e1e1e` |
| WezTerm | `#1e1e1e` |
| XFCE Terminal | `#000000` |
| Konsole | `#232323` |
| VS Code | `#1e1e1e` |

## 测试不同背景色

```bash
# 测试暗灰色
export LSIX_BACKGROUND="#1e1e1e"
./target/release/lsix /path/to/images

# 测试纯黑
export LSIX_BACKGROUND="black"
./target/release/lsix /path/to/images

# 测试深蓝灰色
export LSIX_BACKGROUND="#282a36"
./target/release/lsix /path/to/images
```

## 推荐配置

### 暗色主题用户（90% 的用户）

```bash
export LSIX_BACKGROUND="#1e1e1e"
export LSIX_FOREGROUND="#ffffff"
```

### 浅色主题用户

```bash
export LSIX_BACKGROUND="#ffffff"
export LSIX_FOREGROUND="#000000"
```

### Dracula 主题

```bash
export LSIX_BACKGROUND="#282a36"
export LSIX_FOREGROUND="#f8f8f2"
```

### Nord 主题

```bash
export LSIX_BACKGROUND="#2e3440"
export LSIX_FOREGROUND="#eceff4"
```

## 故障排除

### Q: 背景色还是不对？

A: 尝试以下步骤：

1. 确认环境变量已设置：
   ```bash
   echo $LSIX_BACKGROUND
   ```

2. 如果需要，重新编译：
   ```bash
   cargo build --release
   ```

3. 尝试使用不同的颜色格式：
   ```bash
   # 十六进制
   export LSIX_BACKGROUND="#1e1e1e"

   # 颜色名称
   export LSIX_BACKGROUND="black"
   ```

### Q: 可以让程序自动检测吗？

A: 可以，但需要终端支持特定的转义序列查询。当前的实现为了速度（避免慢速查询），使用了一个合理的默认值。如果您愿意等待稍长的启动时间，可以修改 `src/terminal.rs` 中的 `detect_colorscheme()` 函数来启用实际的终端查询。

### Q: SIXEL 为什么不支持透明？

A: SIXEL 是 1980 年代开发的格式，当时没有透明度的概念。它是为 DEC VT340 等终端设计的简单的位图格式。
