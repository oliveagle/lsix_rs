# lsix-rs 故障排除指南

## SIXEL 检测问题

### 问题：终端不支持 SIXEL 检测

如果您看到错误 "Your terminal does not report having sixel graphics support"，但实际上您的终端支持 SIXEL，可以尝试以下解决方案：

### 解决方案 1：设置环境变量（推荐）

```bash
# 临时设置（仅当前会话）
export LSIX_FORCE_SIXEL_SUPPORT=1
./target/release/lsix

# 永久设置（添加到 ~/.bashrc 或 ~/.zshrc）
echo 'export LSIX_FORCE_SIXEL_SUPPORT=1' >> ~/.bashrc
source ~/.bashrc
```

### 解决方案 2：检查 TERM 环境变量

```bash
# 查看当前的 TERM 值
echo $TERM

# 如果是常见的 SIXEL 终端，新的检测逻辑应该能识别
# 支持的终端类型：
# - xterm (使用 xterm -ti vt340 启动)
# - mlterm
# - wezterm
# - foot
# - contour
# - kitty (部分支持)
# - alacritty (部分支持)
# - mintty
# - cygwin
```

### 解决方案 3：手动验证 SIXEL 支持

使用 ImageMagick 直接测试：

```bash
# 如果您的终端支持 SIXEL，这应该会显示一个彩色图案
convert -colors 16 -size 100x100 xc:red -fill blue -draw "circle 50,50 50,10" sixel:-

# 或者测试一个真实图像
convert /path/to/image.jpg sixel:-
```

### 解决方案 4：使用正确的终端启动参数

**XTerm:**
```bash
# 启动支持 SIXEL 的 xterm
xterm -ti vt340

# 或者在 ~/.Xresources 中设置：
echo 'xterm*decTerminalID: vt340' >> ~/.Xresources
xrdb -merge ~/.Xresources
```

**其他终端:**
```bash
# mlterm（默认支持）
mlterm

# WezTerm（默认支持 SIXEL）
wezterm

# Foot（默认支持 SIXEL）
foot
```

## 常见问题

### Q: 为什么检测会失败？

A: 一些终端不完全响应 VT220 设备属性查询，或者响应格式不同。改进后的检测逻辑会：
1. 检查已知的 SIXEL 终端类型
2. 如果终端类型匹配但检测失败，会给出警告但仍继续尝试
3. 支持通过环境变量强制启用

### Q: 我的终端不在支持列表中，但支持 SIXEL

A: 设置 `LSIX_FORCE_SIXEL_SUPPORT=1` 环境变量，或者提交 issue 添加您的终端到支持列表。

### Q: 检测成功但图像不显示

A: 可能的原因：
1. ImageMagick 未安装或版本不兼容
2. 需要使用 `magick` 命令（ImageMagick 7）
3. 图像格式不支持
4. 终端窗口太小

验证 ImageMagick:
```bash
# 检查是否安装
which magick

# 检查版本
magick -version

# 测试 SIXEL 输出
magick -size 100x100 xc:red sixel:-
```

## 调试模式

如果问题仍然存在，可以手动测试终端响应：

```bash
# 测试 1: 发送设备属性查询
echo -ne '\e[c'
# 终端应该响应类似: \e[?64;4;15c
# 其中 "4" 表示支持 SIXEL

# 测试 2: 检测颜色数量
echo -ne '\e[?1;1;0S'
# 终端应该响应颜色数量

# 测试 3: 检测几何尺寸
echo -ne '\e[?2;1;0S'
# 终端应该响应 SIXEL 图形尺寸
```

## 终端兼容性矩阵

| 终端 | SIXEL 支持 | 检测方式 | 备注 |
|------|-----------|---------|------|
| xterm -ti vt340 | ✅ | 自动查询 | 最佳支持 |
| mlterm | ✅ | TERM 检测 | 优秀支持 |
| wezterm | ✅ | TERM 检测 | 优秀支持 |
| foot | ✅ | TERM 检测 | 良好支持 |
| contour | ✅ | TERM 检测 | 良好支持 |
| kitty | ⚠️ | 需要环境变量 | 需要配置 |
| alacritty | ⚠️ | 需要环境变量 | 需要配置 |
| gnome-terminal | ❌ | - | 不支持 |
| macOS Terminal | ❌ | - | 不支持 |

## 报告问题

如果您的终端支持 SIXEL 但 lsix-rs 无法识别，请提交 issue 并包含：
1. `echo $TERM` 的输出
2. `magick -version` 的输出
3. 终端名称和版本
4. 尝试上述测试的结果
