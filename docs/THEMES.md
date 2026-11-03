# 主题系统（阶段 1）

不写外壳，只靠配置和主题，让一套全新的 niri 会话看起来就是留白。

**状态**：2026-11-02 在测试虚拟机（Arch，niri 26.04）中验证，清单结果见文末。本机不做测试。

## 工作方式

```text
design/tokens.toml ──┐
                     ├─ liubai-theme build ──> build/
templates/**  ───────┘                          ├─ shared/  两种主题写在一起（niri 主配置、foot、GTK 4）
                                                ├─ paper/   纸
                                                └─ ink/     墨

liubai-theme install ──> ~/.local/share/liubai/{shared,paper,ink,bin,tokens.toml}
                         ~/.local/share/liubai/current -> paper 或 ink（符号链接）
                         ~/.config/<app>/... -> 链接到上面的文件
```

- **只有令牌是源头**。模板里用 `{{ t.base }}`、`{{ paper.ansi.red | bare }}` 这样的占位符取值；写错名字、颜色格式不对，`build` 会直接报错并指出模板和行号。
- **纸和墨必须定义相同的令牌**，否则 `build` 拒绝生成。这样不会出现“某个模板只在一种主题下能用”。
- **切换主题只换一个链接**。`current` 指向 `paper` 或 `ink`；fuzzel、mako、GTK 的配置都经过 `current`，所以不用重新链接。

### 各应用怎么跟着切换

| 应用 | 配置 | 切换时 |
| --- | --- | --- |
| niri | 主配置共用，末尾 include `current/niri/theme.kdl`（只有颜色） | `niri msg action load-config-file` |
| foot | 一个文件同时写纸（`[colors-light]`）和墨（`[colors-dark]`） | 向 foot 发 SIGUSR2（纸）或 SIGUSR1（墨），已开的终端原地变色 |
| fuzzel | 经 `current` 链接 | 每次启动重新读取，无需处理 |
| mako（后备） | 经 `current` 链接；外壳运行时不启动 | `makoctl reload` |
| GTK 4 | 一个文件：纸为默认，墨写在 `@media (prefers-color-scheme: dark)` 里；窗口标题用衬线（D77） | 设置 `color-scheme`，已打开的 libadwaita 应用实时切换；字体改动在应用重开后生效 |
| GTK 3 | 经 `current` 链接；同时切换 adw-gtk3 / adw-gtk3-dark | 我们的 CSS 在应用重开后生效 |
| 外壳 | 经 `current` 读取 `shell/style.css`；动效时长和图标线宽在 `shared/shell/tokens.json` | `liubai-shell reload-style`，实时生效 |
| kitty / alacritty | 不自动链接，需在各自配置里引用 | kitty 收到 SIGUSR1 重读配置；alacritty 下次重读时生效 |
| GtkSourceView（文本编辑器的正文区） | `liubai`（纸）和 `liubai-dark`（墨）两套方案都链接到 `~/.local/share/gtksourceview-{4,5}/styles/`；安装时把 GNOME 文本编辑器设为 `liubai` | 编辑器随系统明暗自动选浅色或深色方案 |
| 壁纸 | 按每块屏幕的实际分辨率生成 SVG：等高线，或作者的线稿（图版，`design/plates/`，按主题重新上色）；叠上极淡的纸纹，转成 PNG 交给 swaybg；结果缓存在 `~/.cache/liubai/` | 新壁纸显示后再结束旧的 swaybg，避免黑屏闪烁；同时生成当天的锁屏扉页 |
| 锁屏 | `liubai-theme lock` 用当前主题的颜色和扉页图（壁纸加当天日期）调用 swaylock | 每次锁屏时读取；扉页每天生成一次 |

## 命令

```sh
tools/liubai-theme build                 # 生成 build/
tools/liubai-theme install --dry-run     # 只打印将要做的改动
tools/liubai-theme install               # 复制到 ~/.local/share/liubai 并链接配置
tools/liubai-theme switch ink            # 或 paper、auto（按时刻切换）；toggle 在纸墨之间切换
tools/liubai-theme status
tools/liubai-theme wallpaper --width 2560 --height 1600 --scale 1.5 -o wall.svg
```

`install` 遇到已有的配置文件，不覆盖，而是改名为 `<文件名>.liubai-backup-<时间>`。

快捷键（写在 niri 配置里）：`Mod+Return` 终端、`Mod+Space` 搜索、`Mod+A` 今日页、`Mod+Shift+T` 切换纸／墨、`Super+Alt+L` 锁屏、`Mod+O` 总览、`Mod+Shift+/` 全部快捷键。音量、麦克风、亮度键交给外壳处理并弹出提示。

个人设置（显示器、键盘布局、额外快捷键）写在 `~/.config/niri/local.kdl`，主配置最后会 include 它，重新生成主题不会覆盖。

## 依赖（Arch 包名，2026-11-02 在软件包数据库中查到）

| 用途 | 包 |
| --- | --- |
| 必需 | `niri` `foot` `swaybg` `swaylock` `swayidle` `librsvg`（提供 rsvg-convert） `python`（≥ 3.11） `libpulse`（提供 pactl） |
| 外壳编译 | `rust` `gtk4` `gtk4-layer-shell` |
| 后备 | `mako`（外壳没运行时的通知） `fuzzel` |
| 字体 | `adobe-source-serif-fonts`（Source Serif 4） `inter-font` `ttf-ibm-plex` |
| 建议 | `adw-gtk-theme`（让 GTK 3 应用也用上配色） `polkit-gnome`（授权弹窗） `upower` `networkmanager` `fcitx5` `xdg-desktop-portal-gnome` `xdg-desktop-portal-gtk` `xwayland-satellite` `wireplumber` `brightnessctl` `playerctl` |

授权弹窗由 polkit-gnome 提供：装上即可，niri 配置会在登录时自动启动它。

## 虚拟机验证清单（2026-11-02）

测试机：Arch，niri 26.04、foot 1.28、fuzzel 1.15、mako 1.11、GTK 4.24，virgl 虚拟显卡，1440×900。截图在 `docs/screenshots/`。

| # | 项目 | 结果 |
| --- | --- | --- |
| 1 | `liubai-theme build` 生成全部文件 | 通过，一次成功 |
| 2 | `niri validate` 校验主配置和两个颜色文件（含背景色、总览背景色、图层阴影、动画写法） | 通过 |
| 3 | 登录后：等高线壁纸、2px 陶土色焦点框、柔和阴影 | 通过；壁纸按屏幕实际尺寸生成 |
| 4 | foot：纸色半透明、无标题栏、焦点框不从背后透出 | 通过；但透过 90% 不透明度几乎看不到等高线 |
| 5 | `switch ink`：foot 原地变色，niri、通知、壁纸跟着变 | 通过；fuzzel 未截图确认 |
| 6 | 专注模式：普通通知暂存，紧急通知照常弹出 | 通过 |
| 7 | libadwaita 应用配色 | 发现问题后修正（决策 D27），现已实时切换；GTK 3 应用未测 |
| 8 | 锁屏 | 2026-11-03：能上锁，背景是当前主题的等高线壁纸；解锁仍**未测**（需要在虚拟机里用键盘输入密码，测试时靠重启 niri 会话解除） |
| 9 | 内存基线 | 已记录：`reports/memory/2026-11-02-vm/` |

测试中修正的问题：
- 测试机登录脚本的循环（niri-session 会以登录 shell 身份重新启动自己），只影响测试机，不影响项目。
- GTK 4 两套配色分开时，标题栏和正文颜色不一致（D27）。
- 状态栏按钮文字被默认主题加粗。

仍待处理：gnome-text-editor 等应用的正文区用的是 GtkSourceView 自带的配色方案，与纸墨略有差异，可以另做一套 `liubai` 配色方案。
