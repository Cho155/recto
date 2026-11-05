# Recto

**一套像纸一样安静的 Linux 桌面。**

Recto 是一套开源的 Wayland 桌面方案：以 niri 为合成器，配一个自研的轻量外壳（状态栏、通知、搜索、今日页、锁屏），以及一份覆盖常用应用的统一主题。

## 它想成为什么

- **温暖、克制**：暖白的纸色与暖黑色，只有一个强调色（陶土橙）。审美上借鉴 Anthropic 的简约风格，但不使用其品牌标识与商业字体。
- **不是极客风**：不堆图标、不放实时曲线、不用霓虹配色。文字优先，平时安静，有事才提醒。
- **保留质感**：透明终端、柔和阴影、细线条，这些让桌面好看的东西都留下，但当作材质来用，而不是特效。
- **省内存**：整个外壳一个进程，面板按需创建、关闭即释放；所有内存数字都附测量方法，不只说“很轻”。

界面语言默认英文；其他语言的渲染单独支持。

## 现在的状态

阶段 0–3 完成，阶段 4 大部分完成，第二天做了动效、材质与排版的打磨；全部在测试虚拟机中验证过：
- 主题系统可以生成、安装、实时切换 Paper 和 Ink，也可以按时刻自动切换。
- 外壳 `recto-shell` 提供：
  - 状态栏、今日页（含正在播放）、搜索；
  - 通知、音量和亮度提示、专注模式；
  - 托盘、输入法指示、麦克风使用提示。

  空闲时约 25 MiB PSS（51 MiB RSS）；整个桌面空闲约 174 MiB（PSS，含音频服务和授权代理）。
- 风格是古典的书页，不是浮动卡片：衬线正文、小型大写的页眉、旧体数字、陶土色的红字标注；今日页是一整页纸，锁屏是扉页；壁纸可以换成你自己的线条画（图版）。

截图见 [docs/screenshots/](docs/screenshots/)，详见 [STATUS.md](STATUS.md)。

## 换成 Recto，或换回原来的桌面

切换由你自己做，脚本不会自动替你换：

```sh
cd ~/Projects/recto
./install.sh --dry-run    # 先看：缺哪些包、会改哪些文件（什么都不改）
./install.sh              # 换成 Recto：编译外壳（几分钟），已有配置先改名备份，问一次就装
./install.sh --uninstall  # 换回原来：删掉 Recto 的文件，放回备份的配置和 GNOME 设置
```

两个方向都要**注销后重新登录**才完整生效。在原来的桌面里运行安装时，niri 会立刻换上 Recto 的配色和快捷键，但原来的状态栏、壁纸还在跑；重新登录后才是完整的 Recto（状态栏、壁纸、空闲锁屏）。已经在用 Recto 时再运行 `./install.sh`，就是更新到仓库里的最新版本。

- 备份的文件名是 `*.recto-backup-<时间>`，就在原来的位置旁边。
- 自己的 niri 设置（显示器、键盘布局）写在 `~/.config/niri/local.kdl`，不会被安装覆盖。
- 壁纸默认是 Hammershøi 的一幅油画；换图见 [design/plates/README.md](design/plates/README.md)。

详见 [主题系统](docs/THEMES.md)；在真机上试用，请按 [真机验证清单](docs/TESTING.md)。

## 快捷键

记不住时按 **Super+Shift+/**，屏幕上会列出常用的快捷键（niri 自带的列表，说明由 Recto 写）。Super 就是 Windows 键。

| 按键 | 作用 |
| --- | --- |
| Super+Enter 或 Super+T | 打开终端 |
| Super+Space 或 Super+D | 搜索应用和操作 |
| Super+A | 今日页：通知、主题（Paper · Ink · Auto）、专注模式 |
| Super+Shift+T | 在 Paper 和 Ink 之间切换 |
| Super+Alt+L | 锁屏 |
| Super+Q | 关闭窗口 |
| Super+O | 总览所有窗口 |
| Super+← → 或 Super+H L | 切换到左边或右边的列 |
| Super+↑ ↓ 或 Super+K J | 同一列里的上一个、下一个窗口 |
| Super+Ctrl+方向键 | 移动列或窗口 |
| Super+1 … 9 | 去第 1 到 9 个工作区 |
| Super+Ctrl+1 … 9 | 把当前列移到第 1 到 9 个工作区 |
| Super+Page Down / Page Up，或 Super+U / I | 下一个、上一个工作区 |
| Super+R | 在几种预设宽度之间切换 |
| Super+F / Super+Shift+F | 列占满宽度 / 窗口全屏 |
| Super+- / Super+= | 列窄一点 / 宽一点 |
| Super+C | 把当前列放到屏幕中间 |
| Super+[ / ] | 把窗口并进左边或右边的列（已在同一列时则分出去） |
| Super+W | 把一列里的窗口叠成标签 |
| Super+V | 窗口浮动或回到平铺 |
| PrtSc | 截取选区；Ctrl+PrtSc 截整屏，Alt+PrtSc 截当前窗口（存到 `~/Pictures/Screenshots`） |
| Super+Shift+P | 关闭显示器 |
| Super+Shift+E | 注销 |

## 文档

| 文档 | 内容 |
| --- | --- |
| [理念与想法](docs/VISION.md) | 为什么做、核心想法、明确不做什么 |
| [设计语言](docs/DESIGN-LANGUAGE.md) | 配色、字体、形状、动效、图标、文案 |
| [架构](docs/ARCHITECTURE.md) | 组件构成、技术选型、内存预算与测量方法 |
| [路线图](docs/ROADMAP.md) | 分阶段计划与每阶段的完成标准 |
| [决策记录](docs/DECISIONS.md) | 已做的决定与理由 |
| [主题系统](docs/THEMES.md) | 令牌如何变成各应用配置、安装与卸载、如何切换 |
| [真机验证清单](docs/TESTING.md) | 第一版在真机上要逐项试的内容，以及 `tools/collect-report` |
| [交互规格](docs/INTERACTION.md) | 通知、提示、托盘、专注模式的行为（阶段 3） |
| [设计令牌](design/tokens.toml) | 所有颜色、字号、圆角的唯一来源（草案） |
| [样张](design/specimen/) | 桌面、状态栏、今日页、配色与字体的静态设计稿 |

## 许可证

[MIT](LICENSE)。随项目分发的图版线稿各有自己的授权，列在 [design/plates/README.md](design/plates/README.md)。
