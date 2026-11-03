# 项目状态

更新：2026-11-03 上午收尾（第二天）。阶段 0–3 完成，阶段 4 大部分完成。今天上午分四轮：

1. 打磨动效、材质、排版（D48–D62）；
2. 按作者的方向改为**古典的书页风格**（D63–D72）；
3. 古典化的细节、样张同步、应用主题（GTK 标题、编辑器配色、Qt 6）、托盘键盘（D73–D83）；
4. 今日页的“正在播放”，并修好了今日页按钮点不动的问题（D84–D85）。

全部在测试虚拟机中验证过，本机没有做过测试（见“没做”里的一处例外）。测试机已关机。

## 现在能用的

| 部分 | 内容 | 文档 |
| --- | --- | --- |
| 设计 | 理念（含“古典，而不是 Windows”）、设计语言、令牌、样张（9 张画板，已同步为古典风格，含锁屏扉页和图版位置） | `docs/VISION.md`、`docs/DESIGN-LANGUAGE.md`、`design/` |
| 主题系统 | 一份令牌生成 niri、foot、GTK 3/4、GtkSourceView、Qt 6（qt6ct）、mako、fuzzel、kitty、alacritty、外壳的配置；纸／墨／Auto 实时切换；壁纸是等高线或作者的线稿（图版：按主题上色，配图版说明），带纸纹；锁屏是扉页 | `docs/THEMES.md`、`design/plates/README.md` |
| 外壳 `liubai-shell` | 状态栏（页眉：衬线、罗马数字工作区、斜体日期、旧体数字时钟）、今日页（整页；正在播放）、通知（页边纸条，陶土色朱批）、音量／麦克风／亮度提示、专注模式、搜索（索引）、托盘（可用键盘） | `docs/INTERACTION.md`、`docs/ARCHITECTURE.md` |
| 动效 | 面板从来处进出、只移动 6 px；通知原地放下、其他纸条平滑让位；开窗像一张纸落到桌上（niri 着色器） | `docs/DESIGN-LANGUAGE.md` 动效一节 |
| 材质与排版 | Source Serif 4 为主：小型大写、旧体数字、光学尺寸；小圆角（6／3／2）；1 px 陶土焦点线（窗口和按钮一致）；当前窗口阴影略深；自绘细线图标；纸条样式的提示气泡 | D55–D58、D63–D67、D74、D80 |
| 应用 | GTK 3/4 配色与衬线窗口标题；文本编辑器正文区的 `liubai` 配色；Qt 6 配色；GNOME 文档字体为衬线 | `docs/THEMES.md`、D75、D77、D83 |
| 会话 | swayidle 空闲锁屏、polkit-gnome 授权弹窗、foot 服务进程 | `templates/niri/config.kdl` |

- **测试**：46 个单元测试通过，编译无警告。
- **提交**：作者 2026-11-03 同意随时提交。历史从 `c6361a8` 开始，在 master 上；提交身份只设在本仓库（名字 Tibo，作者账户邮箱）。
- **外壳内存**：空闲约 24 MiB PSS（RSS 41–51 MiB，视会话里用过的东西而定）；整页今日页打开时多约 17 MiB，关闭后释放；反复开关不增长（D61）。
- **整个桌面空闲**：166–174 MiB，预算 400 MiB（`reports/memory/2026-11-03-vm-polish/`）。
- **动效帧率**：cairo 渲染器下满帧（75 Hz）；搜索的第一帧会晚一两帧（`reports/motion/2026-11-03-vm/`）。
- **截图**：`docs/screenshots/`，36 张。古典风格见 `2026-11-03-vm-classical-*.png`，另有 `…-today-now-playing.png`、`…-text-editor-*.png`、`…-qt6ct.png`、`…-plate-test-drawing.png`（叶子是测试用的线稿，不随项目分发）、`…-lock-title-page.png`。
- **样张画布**：https://claude.ai/artifact/LsCAAp3mUTsEYSnUoPumiz（私有；第 13 版；源文件在 `design/specimen/project/`）。

## 没做 / 已知问题

- 还没有真正的线稿：图版系统只用测试画验证过，等作者挑选线条画。
- 今日页没有日程；Evolution Data Server 的内存代价还没测（样张里画了，实现里没有）。
- “正在播放”只用测试机里的假播放器（`~/VMs/liubai-arch/fake-mpris.py`）测过，没有用真的播放器测；没有进度条，曲目变化最迟一分钟后才在打开的今日页上更新。
- 锁屏扉页跨过午夜不会更新日期；扉页上的日期用的是普通数字（rsvg 不支持旧体数字）。
- 2026-11-03 第一轮曾在本机运行过一次 `liubai-theme build`（输出到临时目录，已删除），违反了“本机不运行”的约定；之后都在虚拟机里运行。
- 动效只在虚拟机里测过。高分屏（缩放 2）上 cairo 每帧的像素是 4 倍，可能掉帧，要在真机上测；掉帧就换 GL（D50）。
- 搜索打开的第一帧较慢：一个会话里第一次约 50 ms，之后 17–25 ms，只晚一两帧，暂不处理。
- niri 的内存比前一天高（78 MiB，前一天 63 MiB）：与开窗着色器无关，刚登录的会话里也一样，原因未查明；在预算内，留到真机上对比。
- 会话里的 `at-spi2-registryd` 每次重启 niri 会话多一个（与留白无关，待查）。
- Qt 5 应用没有主题；已打开的 Qt 应用要重开才跟随主题切换（D83）。
- GTK 应用的控件（按钮、标签页）仍是 Adwaita 的样子，只改了配色和窗口标题（D77）。
- 低电量提示没有实测（虚拟机没有电池）；空闲锁屏的 10 分钟计时没有完整等过；锁屏后用密码解锁没有测过。
- 搜索不读 .desktop 的 Keywords（D45）。
- 输入法指示不支持 IBus；托盘项不响应滚轮。
- 在别的程序里改音量不会弹出提示（D30）。
- 卷轴缩略分不出屏幕内外的列（niri 不报告平铺窗口的视口位置）。
- polkit-gnome 常驻约 11–17 MiB，是可以优化的大头。
- 所有测量都在虚拟机（virgl 显卡）里完成；在真机上要重新测量。
- 许可证未定。

## 测试机现状（`~/VMs/liubai-arch/`，2026-11-03 上午收尾时已关机）

- 启动：`./start.sh`；执行命令：`./vm '<命令>'`；同步项目：`./sync.sh`（不覆盖测试机里的 `build/` 和 `shell/target/`）。
- 已安装：niri 26.04 一套、Rust 工具链、PipeWire、UPower、NetworkManager（取代了 systemd-networkd）、fcitx5（配置了 us 和 de 两种键盘布局）、nm-applet、ydotool、swayidle、polkit-gnome。2026-11-03 新增：imagemagick（拼慢放截图）、python-fonttools（查字体的 OpenType 特性）、qt6ct（测 Qt 主题）、python-gobject（跑假播放器）。
- 测试机里装的是最新的构建和配置；当前主题模式是 Auto。
- 测试专用的改动：
  - tty1 自动登录进 niri；
  - `~/bin/insession` 让 SSH 命令带上会话环境；
  - `~/.config/niri/local.kdl` 关掉了指针加速和热角；
  - arch 用户加入了 wheel 组，用于测授权弹窗。
- 测试用的文件：
  - `~/fake-mpris.py`（假播放器，源文件在 `~/VMs/liubai-arch/fake-mpris.py`；用 `~/bin/insession python3 ~/fake-mpris.py &` 启动）；
  - `~/Documents/liubai-sample.md`（编辑器配色样例）；
  - `~/Documents/monday-notes.md` 里有一行误打进去的 “arch”。
- 每次开机后，要模拟点击需先启动 `sudo ydotoold -p /tmp/.ydotool_socket -P 0666`。
- 测试期间要手动停掉 swayidle（`pkill -x swayidle`），否则 10 分钟后锁屏，截图全是锁屏画面。锁屏后可以用 `niri msg action quit --skip-confirmation` 重启会话来解除（自动登录会重新进入）。
- 慢放：外壳用 `GTK_SLOWDOWN=8` 启动；niri 在 `local.kdl` 里临时加 `animations { slowdown 10.0 }`，用完删掉。
- 用 `pkill -f` 时注意：`./vm` 的整条命令也会被匹配到，会把自己结束掉；用 `pkill -x <进程名>`。

## 下一步

1. **作者提供线稿后**：放进 `design/plates/`，在令牌里打开图版，调线色和位置；记下来源和授权。
2. **今日页的日程**：先在测试机里测 Evolution Data Server 的内存代价，再决定是读 EDS，还是只读本地的 `.ics` 文件。
3. **用真的播放器测“正在播放”**：例如 mpv 加 mpris 插件（包比较大，装之前先说）。
4. **古典化还可以继续的地方**（都不急）：
   - 空状态用一个装饰分隔（“* * *”），还在考虑：装饰用多了会显得刻意；
   - GTK 应用的控件要不要更进一步纳入纸墨（会影响可用性，要谨慎）。
5. **需要作者的事**：选线稿；定许可证；在真机上试（高分屏动效帧率是重点）。
