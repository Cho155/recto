# 架构

## 组件构成

| 角色 | 选择 | 说明 |
| --- | --- | --- |
| 合成器 | niri | 滚动平铺契合“手卷”隐喻；支持圆角、阴影、窗口透明度；26.04 实现了背景模糊协议 `ext-background-effect-v1`。不自研合成器。 |
| 外壳 | 自研，Rust + GTK4 + gtk4-layer-shell，单进程 | 状态栏、通知服务、OSD（音量／亮度提示）、搜索（启动器）、今日页、锁屏。 |
| 终端 | foot（默认），server 模式 | 内存占用低，支持透明；多个窗口共享一个进程；纸和墨写在同一个配置里，靠信号切换。kitty、alacritty 也提供主题。 |
| 壁纸 | 程序生成的等高线 | 按每块屏幕的像素尺寸生成 SVG，转 PNG 后由 swaybg 显示；以后可由外壳自绘。 |
| 主题导出 | `tools/liubai-theme` | 从 `design/tokens.toml` 和 `templates/` 生成 niri、foot、fuzzel、mako、GTK 3/4、GtkSourceView、Qt 6（qt6ct）、kitty、alacritty 和外壳的配置，并负责安装、卸载、切换、壁纸、锁屏。安装的入口是仓库根目录的 `install.sh`。详见 [THEMES.md](THEMES.md)。 |
| 空闲与锁屏 | swayidle + 主题化的 swaylock | `liubai-theme lock` 用当前主题的颜色锁屏；锁屏不自己实现（D41）。 |
| polkit 认证代理 | polkit-gnome | 已安装时由 niri 配置启动（D42）；常驻约 17 MiB。 |
| 后备组件 | mako、fuzzel | 通知和搜索都已由外壳提供。mako 只在外壳没运行时由 D-Bus 自动启动；fuzzel 的主题仍会生成，但快捷键已改为外壳的搜索。 |

## 外壳技术栈

决定：Rust + GTK4 + gtk4-layer-shell（理由见决策记录 D3）。

- 外观在 HTML 样张里迭代，实现阶段不需要为了“改得快”牺牲内存。
- Pango 的文字排版和字体回退成熟，有利于以后支持其他语言。
- GTK4 的渲染器（GL／Vulkan／Cairo）对内存影响明显，在虚拟机中分别测量后选默认值。
- 验证方式：阶段 2 在虚拟机中测量最小外壳；空闲内存超出预算 50% 以上时，重新评估 Quickshell。

## 外壳现状（阶段 4 与审美优化）

`shell/` 是一个 Rust 程序 `liubai-shell`，依赖只有 gtk4 0.11、gtk4-layer-shell 0.8、serde_json 和 libc（libc 本来就是 glib 的依赖）。调试时设 `LIUBAI_DEBUG=1` 可看到系统状态更新。

| 文件 | 职责 |
| --- | --- |
| `main.rs` | 单实例与命令（`toggle-today`、`volume`、`brightness`、`mic-mute`、`focus`、`reload-style`、`quit`）；把各部分串起来 |
| `state.rs` | 由 niri 事件重建的桌面状态：工作区、窗口、列。纯数据，带单元测试 |
| `niri.rs` | niri IPC：在 GTK 主循环里异步读事件流，断开后自动重连；发送动作 |
| `bar.rs` | 状态栏：工作区圆点与名称（点击打开总览）、卷轴缩略、日期与时间（点击打开今日页）；在栏上滚动切换工作区 |
| `today.rs` | 今日页：盖住整个桌面的一页纸，日期、主题切换、专注、一句话状态、最近通知；按需创建，点页边或 Esc 关闭 |
| `inbox.rs` | 通知的屏幕显示、历史、未读、专注暂存。纯数据，带单元测试 |
| `notify.rs` | `org.freedesktop.Notifications` D-Bus 服务 |
| `margin.rs` | 右侧通知卡片；有通知时才存在；每张卡片放在伸缩容器里，进出时其他卡片平滑让位 |
| `osd.rs` | 音量和亮度提示；显示 1.2 秒后销毁 |
| `controls.rs` | 通过 wpctl / brightnessctl 调节并读回，带单元测试 |
| `status.rs` | 电量、网络、静音的按需提示与今日页的句子。纯数据，带单元测试 |
| `system.rs` | 通过系统总线读取 UPower 与 NetworkManager |
| `tray.rs` | 托盘：StatusNotifierWatcher 与宿主、托盘列表与菜单弹层 |
| `sni.rs` | 托盘项的地址、名称与说明。纯数据，带单元测试 |
| `dbusmenu.rs` | 解析与展平应用菜单。纯数据，带单元测试 |
| `ime.rs` | fcitx5 输入法指示与切换 |
| `mic.rs` | 通过 `pactl subscribe` 判断是否有程序在录音；子进程随外壳一起结束 |
| `search.rs` | 搜索窗口：应用与外壳动作 |
| `matching.rs` | 搜索的匹配与排序。纯数据，带单元测试 |
| `daylight.rs` | Auto 模式：按时刻选纸或墨。带单元测试 |
| `clock.rs` | 只在整分钟唤醒的时钟 |
| `strings.rs` | 全部界面文案（英文） |
| `style.rs` | 读取 `current/shell/style.css`（由令牌生成），切换主题时重新加载 |
| `tokens.rs` | 读取 `shared/shell/tokens.json`：代码里要用的令牌（动效时长、图标线宽） |
| `motion.rs` | 面板离开：先交出键盘，播完离开动画再销毁窗口；跟随 GTK 的 `enable-animations` |
| `glyph.rs` | 自绘的细线图标：关闭、下拉、更多 |
| `mpris.rs` | 今日页的“正在播放”：从会话总线读 MPRIS 播放器、发送控制；选哪个播放器的逻辑带单元测试 |
| `memory.rs` | 面板关闭后把空闲的堆内存还给系统；让子进程随外壳结束 |

## 外壳内部结构（规划）

- **服务层**：niri IPC、通知（`org.freedesktop.Notifications`）、音频（PipeWire）、网络（NetworkManager）、电源（UPower）、媒体（MPRIS）、托盘（StatusNotifierItem）。每个服务独立，无法连接时降级显示，而不是崩溃。
- **状态层**：把服务数据整理成界面需要的简单数据，不包含界面代码，可以单独测试。
- **界面层**：状态栏常驻；其余面板（搜索、今日页、OSD、锁屏）按需创建，关闭即销毁。
- **主题**：界面只读取生成的令牌，不写死任何颜色或尺寸。
- **文案**：所有界面字符串集中管理，默认英文，为翻译预留接口。

## 内存预算

预算以 **PSS**（按比例分摊共享库后的内存）计，同时记录 RSS 作参考。测量时单屏、空闲 1 分钟后取值。

| 项目 | 目标 |
| --- | --- |
| 外壳，空闲，仅状态栏 | ≤ 100 MiB（2026-11-03 起，原为 60） |
| 外壳，打开搜索或今日页 | ≤ 100 MiB |
| 关闭面板 30 秒后 | 首次打开留下的缓存 ≤ 5 MiB；反复开关 10 轮增长 ≤ 1 MiB（2026-11-03 起，原为“空闲的 110% 以内”，见 D61） |
| 整个会话空闲（合成器 + 外壳 + 一个终端 + 必要后台服务，不含用户应用） | ≤ 400 MiB（2026-11-03 起，原为 300；见 D48） |

2026-11-02 在测试虚拟机中实测，四项全部达标：阶段 4 时外壳空闲 18.5 MiB、打开搜索 20.8 MiB、关闭后为空闲的 106%；整个会话约 149 MiB（含音频服务和授权代理）。外壳默认用 cairo 渲染器；用 gl 时空闲约 42 MiB。详见 [`reports/memory/2026-11-02-vm/`](../reports/memory/2026-11-02-vm/README.md)。虚拟机的图形栈与真机不同，换到真机需要重新测量。

2026-11-03 加入动效、新阴影和光学尺寸字体后重测（[`reports/memory/2026-11-03-vm-polish/`](../reports/memory/2026-11-03-vm-polish/README.md)）：外壳空闲 24.6 MiB PSS（RSS 51.1，与前一天相同；PSS 变高是因为这次没有别的 GTK 4 程序和它分摊库，见 D62），打开今日页 32.0 MiB，关闭后 28.7 MiB（首次缓存约 4 MiB，再开关 10 轮只多 0.2 MiB），整个会话约 174 MiB。

参考基线（2026-11-02，作者当前的笔记本，RSS，含共享库，不是严格对比）：niri 131 MB、Noctalia 186 MB、kitty 147 MB、mako 85 MB。

### 测量方法

- 只在虚拟机中测量（本机不测试）。
- 读取 `/proc/<pid>/smaps_rollup` 中的 `Pss` 与 `Rss`。
- 记录：日期、虚拟机配置、显示分辨率与缩放、图形驱动、软件版本。虚拟机的图形栈与真机不同，结论要注明这一点。
- 每次测量结果存入 `reports/memory/`，文档中引用的数字必须能对应到一份记录。
- 测量脚本：`tools/measure-memory`（单次测量）；真机测试用 `tools/collect-report`（机器信息、三次内存、两种渲染器的帧率）。

## 目录规划

```text
liubai/
├── design/
│   ├── tokens.toml   设计令牌
│   └── specimen/     样张（设计画布源文件）
├── docs/             文档
├── templates/        各应用配置的模板
├── install.sh        安装与卸载的入口
├── tools/
│   ├── liubai-theme  生成、安装、卸载、切换主题；壁纸；锁屏
│   ├── measure-memory  测内存（PSS、RSS、USS）
│   └── collect-report  真机测试报告
├── build/            生成物（不进 git）
├── shell/            外壳源码（liubai-shell）
└── reports/          测量记录（内存、帧率、真机报告）
```
