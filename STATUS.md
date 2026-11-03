# 项目状态

更新：2026-11-03（第二天，第七轮之后）。阶段 0–3 完成，阶段 4 大部分完成；第一版的定义已和作者确认（D86）：真机验证通过、定好许可证、有安装脚本、有英文 README。安装脚本已完成；许可证定为 MIT（D90，作者交给助手决定）。

今天的五轮：

1. 打磨动效、材质、排版（D48–D62）；
2. 按作者的方向改为**古典的书页风格**（D63–D72）；
3. 古典化的细节、样张同步、应用主题（GTK 标题、编辑器配色、Qt 6）、托盘键盘（D73–D83）；
4. 今日页的“正在播放”，并修好了今日页按钮点不动的问题（D84–D85）；
5. 安装与卸载脚本、真机验证清单、报告收集工具；今日页动画减轻（D86–D89）。
6. 真机测试的准备：许可证定为 MIT（D90）；`/home/shared/start` 和 `finish` 两个脚本，把安装输出、报告、截图、日志自动放进共享文件夹；用 git bundle 给 test 的副本送新版本；手机清单页（可选；结果存在页面数据库里，助手能读）。作者随后要求以后的真机测试都在 test 里做，并给 test 开了 sudo（加入 wheel，免密），`start` 改为直接装缺的包。整套流程在测试机里用一个全新的 test 账户从头走过一遍：从零安装（编译 3.5 分钟）、登录进Recto、`finish` 收集，另一个账户能读到全部结果。
7. 第七轮：作者提醒审美参照是西方的书籍和出版，不是中式审美，写进理念和设计语言（D91），换掉“朱批”“手卷”等叫法；`start` 和 `install.sh` 发现 niri 已在运行时会说明并处理（真机第一轮的问题）；图版分成线稿、素描、油画三种，支持 JPEG 和铺满（D92）；从克利夫兰美术馆和 SMK 的开放获取库挑了 10 件 CC0／公有领域作品，在测试机里做成 Paper／Ink 两套桌面截图，等作者挑（D93）。

全部在测试虚拟机中验证过，本机没有做过测试（见“没做”里的一处例外）。测试机已关机。

## 现在能用的

| 部分 | 内容 | 文档 |
| --- | --- | --- |
| 设计 | 理念（含“古典，而不是 Windows”）、设计语言、令牌、样张（9 张画板，已同步为古典风格，含锁屏扉页和图版位置） | `docs/VISION.md`、`docs/DESIGN-LANGUAGE.md`、`design/` |
| 主题系统 | 一份令牌生成 niri、foot、GTK 3/4、GtkSourceView、Qt 6（qt6ct）、mako、fuzzel、kitty、alacritty、外壳的配置；Paper／Ink／Auto 实时切换；壁纸是等高线或一张图版（线稿按主题上色；素描、油画保留原色，Paper 下叠底印在纸上；可铺满；配图版说明），带纸纹；锁屏是扉页 | `docs/THEMES.md`、`design/plates/README.md` |
| 外壳 `recto-shell` | 状态栏（页眉：衬线、罗马数字工作区、斜体日期、旧体数字时钟）、今日页（整页；正在播放）、通知（页边纸条，陶土色红字标注）、音量／麦克风／亮度提示、专注模式、搜索（索引）、托盘（可用键盘） | `docs/INTERACTION.md`、`docs/ARCHITECTURE.md` |
| 动效 | 面板从来处进出、只移动 6 px；通知原地放下、其他纸条平滑让位；开窗像一张纸落到桌上（niri 着色器） | `docs/DESIGN-LANGUAGE.md` 动效一节 |
| 材质与排版 | Source Serif 4 为主：小型大写、旧体数字、光学尺寸；小圆角（6／3／2）；1 px 陶土焦点线（窗口和按钮一致）；当前窗口阴影略深；自绘细线图标；纸条样式的提示气泡 | D55–D58、D63–D67、D74、D80 |
| 应用 | GTK 3/4 配色与衬线窗口标题；文本编辑器正文区的 `recto` 配色；Qt 6 配色；GNOME 文档字体为衬线 | `docs/THEMES.md`、D75、D77、D83 |
| 会话 | swayidle 空闲锁屏、polkit-gnome 授权弹窗、foot 服务进程 | `templates/niri/config.kdl` |
| 安装与测试 | `./install.sh`（依赖检查、编译、预览、安装、`--uninstall`）；真机验证清单；`tools/collect-report` 一条命令收集机器信息、内存和两种渲染器的帧率 | `docs/TESTING.md`、`docs/THEMES.md`、D88、D89 |

- **测试**：46 个单元测试通过，编译无警告。
- **提交**：作者 2026-11-03 同意随时提交。历史从 `c6361a8` 开始，在 master 上；提交身份只设在本仓库（名字 Tibo，作者账户邮箱）。
- **外壳内存**：空闲约 24 MiB PSS（RSS 41–51 MiB，视会话里用过的东西而定）；整页今日页打开时多约 17 MiB，关闭后释放；反复开关不增长（D61）。
- **整个桌面空闲**：166–174 MiB，预算 400 MiB（`reports/memory/2026-11-03-vm-polish/`）。
- **动效帧率**：通知、音量提示满帧（75 Hz）；搜索的第一帧晚一两帧；整页今日页刚出现时有几帧热身（中位数 15–23 ms，见 D87），真机上用 `collect-report` 比较 cairo 和 gl（`reports/motion/2026-11-03-vm/`）。
- **截图**：`docs/screenshots/`，36 张。古典风格见 `2026-11-03-vm-classical-*.png`，另有 `…-today-now-playing.png`、`…-text-editor-*.png`、`…-qt6ct.png`、`…-plate-test-drawing.png`（叶子是测试用的线稿，不随项目分发）、`…-lock-title-page.png`。
- **样张画布**：https://claude.ai/artifact/LsCAAp3mUTsEYSnUoPumiz（私有；第 13 版；源文件在 `design/specimen/project/`）。

## 真机第一轮（2026-11-03 傍晚，ThinkPad X230，test 账户）

- 安装一次成功：`start` 用 sudo 装好 12 个包，编译 3 分 33 秒（`reports/real-hardware/install-2026-11-03-1754.log`，在 `/home/shared/liubai/` 里）。
- **整轮没有状态栏和壁纸，原因是启动顺序，不是外壳坏了**：作者重启后先从 greetd 登录了 test，niri 用默认配置启动（17:28）；之后在 tty3 跑 `start` 安装（17:57），niri 热加载了Recto的配置，但 `spawn-at-startup`（外壳、`session-start` 画壁纸、swayidle、foot 服务、授权代理）只在 niri 启动时执行，所以都没有运行；`start` 最后的 `niri-session` 因为已有会话而退出。空闲内存测量里也没有 recto-shell 进程。
- `finish` 只完成了机器信息和空闲内存（整个桌面 181.7 MiB PSS，不含外壳）；collect-report 打开今日页后，作者结束了会话，帧率、截图、日志都没收集。作者的截图还在 test 的 `~/Pictures/Screenshots`。
- 作者的观察：纸模式在桌面上刺眼（这轮没有壁纸，傍晚 18 点用的还是 Paper，安装默认是 Paper 不是 Auto）；没有常驻状态栏（上面的启动问题）；终端字体（IBM Plex Mono 10.5）想改得更有味道。清单只做了一部分（手机没登录 claude.ai，打不开清单页）。
- 这轮的日志是助手在作者要求“看日志”后，从系统日志里只读 test（UID 1001）本次开机的记录得到的。

## 没做 / 已知问题

- **第一版还差**：真机验证（作者今晚在 test 账户里做，流程见 AGENTS.md）、英文 README（还没写）。
- 安装脚本只在测试机里测过“已经装好依赖”的情况；在一台全新的系统上从零安装，要等真机测试（或者开一台新的测试机，需要下载几百 MB 的包，要先问作者）。
- 整页今日页刚出现时，全屏表面有几帧热身，在测试机里会晚几帧（D87）。
- 还没有选定的图版：10 件候选只在测试机里渲染过（1440×900），没在真机上看过。素描类的老纸底色比桌面略深，Paper 下看得出一块；线稿在 Ink 下像底片。
- 今日页没有日程；Evolution Data Server 的内存代价还没测（样张里画了，实现里没有）。
- “正在播放”只用测试机里的假播放器（`~/VMs/liubai-arch/fake-mpris.py`）测过，没有用真的播放器测；没有进度条，曲目变化最迟一分钟后才在打开的今日页上更新。
- 锁屏扉页跨过午夜不会更新日期；扉页上的日期用的是普通数字（rsvg 不支持旧体数字）。
- 2026-11-03 第一轮曾在本机运行过一次 `recto-theme build`（输出到临时目录，已删除），违反了“本机不运行”的约定；之后都在虚拟机里运行。
- 动效只在虚拟机里测过。高分屏（缩放 2）上 cairo 每帧的像素是 4 倍，可能掉帧，要在真机上测；掉帧就换 GL（D50）。
- 搜索打开的第一帧较慢：一个会话里第一次约 50 ms，之后 17–25 ms，只晚一两帧，暂不处理。
- niri 的内存比前一天高（78 MiB，前一天 63 MiB）：与开窗着色器无关，刚登录的会话里也一样，原因未查明；在预算内，留到真机上对比。
- 会话里的 `at-spi2-registryd` 每次重启 niri 会话多一个（与Recto无关，待查）。
- Qt 5 应用没有主题；已打开的 Qt 应用要重开才跟随主题切换（D83）。
- GTK 应用的控件（按钮、标签页）仍是 Adwaita 的样子，只改了配色和窗口标题（D77）。
- 低电量提示没有实测（虚拟机没有电池）；空闲锁屏的 10 分钟计时没有完整等过；锁屏后用密码解锁没有测过。
- 搜索不读 .desktop 的 Keywords（D45）。
- 输入法指示不支持 IBus；托盘项不响应滚轮。
- 在别的程序里改音量不会弹出提示（D30）。
- 列缩略分不出屏幕内外的列（niri 不报告平铺窗口的视口位置）。
- polkit-gnome 常驻约 11–17 MiB，是可以优化的大头。
- 所有测量都在虚拟机（virgl 显卡）里完成；在真机上要重新测量。

## 测试机现状（`~/VMs/liubai-arch/`，2026-11-03 第七轮结束时已关机）

- 启动：`./start.sh`；执行命令：`./vm '<命令>'`；同步项目：`./sync.sh`（不覆盖测试机里的 `build/` 和 `shell/target/`）。
- 已安装：niri 26.04 一套、Rust 工具链、PipeWire、UPower、NetworkManager（取代了 systemd-networkd）、fcitx5（配置了 us 和 de 两种键盘布局）、nm-applet、ydotool、swayidle、polkit-gnome。2026-11-03 新增：imagemagick（拼慢放截图）、python-fonttools（查字体的 OpenType 特性）、qt6ct（测 Qt 主题）、python-gobject（跑假播放器）。
- 测试机里装的是最新的构建和配置（用 `./install.sh` 装的，`gsettings-before.json` 里记的是 Adwaita 的默认值）；当前主题模式是 Auto。
- 2026-11-03 第六轮：测试机里也有一个 `test` 账户（无密码、无 sudo，`~/bin/insession` 已拷入）和 `/home/shared`（`start`、`finish`、`recto.bundle`、test 的副本），用来演练真机流程；装了 git。tty1 的自动登录已改回 arch。
- 2026-11-03 第七轮：`~/art/` 里有图版候选的原图（`orig/`）、处理后的图（`plates/`）、截图（`shots/`）和渲染脚本 `render-plates.py`（逐张换壁纸、截全屏；先启动 ydotoold、停掉 swayidle）。test 账户装着 2e593fc 之后的Recto，演练过“niri 已在运行时运行 `start`”（`start --no-packages`，`RECTO_LAUNCH` 可替换最后的 `exec niri-session`）。
- 测试专用的改动：
  - tty1 自动登录进 niri；
  - `~/bin/insession` 让 SSH 命令带上会话环境；
  - `~/.config/niri/local.kdl` 关掉了指针加速和热角；
  - arch 用户加入了 wheel 组，用于测授权弹窗。
- 测试用的文件：
  - `~/fake-mpris.py`（假播放器，源文件在 `~/VMs/liubai-arch/fake-mpris.py`；用 `~/bin/insession python3 ~/fake-mpris.py &` 启动）；
  - `~/Documents/recto-sample.md`（编辑器配色样例）；
  - `~/Documents/monday-notes.md` 里有一行误打进去的 “arch”。
- 每次开机后，要模拟点击需先启动 `sudo ydotoold -p /tmp/.ydotool_socket -P 0666`。
- 测试期间要手动停掉 swayidle（`pkill -x swayidle`），否则 10 分钟后锁屏，截图全是锁屏画面。锁屏后可以用 `niri msg action quit --skip-confirmation` 重启会话来解除（自动登录会重新进入）。
- 慢放：外壳用 `GTK_SLOWDOWN=8` 启动；niri 在 `local.kdl` 里临时加 `animations { slowdown 10.0 }`，用完删掉。
- 用 `pkill -f` 时注意：`./vm` 的整条命令也会被匹配到，会把自己结束掉；用 `pkill -x <进程名>`。

## 下一步

第一版（D86）：

1. **作者**：在 test 里运行 `start`（会用 sudo 装缺的 12 个包）→ 逐项试 → `finish`。步骤在 `/home/shared/今晚的测试步骤.md` 和清单页。
2. **助手**：读真机结果：清单页的数据库、`/home/shared/liubai/reports/real-hardware/`（安装输出、`<时间>/report/summary.md`、截图、`journal.txt`）。
3. **助手**：写英文 README（截图用最新的古典风格）。
4. **助手**：根据真机报告修问题；用两组帧率决定 D50（cairo 还是 gl）；修好后重新做 bundle，请作者在 test 里再跑 `start`。
5. **作者**：在[图版候选页](https://claude.ai/artifact/APXEjbY2mGLDFANaxJAj4m)里挑壁纸（或者交来自己找的图）；助手把选中的放进 `design/plates/`、登记出处、决定默认壁纸，再做 bundle 让 test 更新。候选图在 `design/plates/candidates/`（不进 git，出处见其中的 README）。
6. 还要讨论：默认主题用 Auto 还是 Paper；纸色在真机上的亮度；终端字体（候选 Libertinus Mono、Courier Prime）；项目名“Recto”要不要换（作者说过审美与中式无关）。之后重跑完整一轮真机测试（现在 test 里已经装好，直接从登录界面登录 test 即可）。

之后（1.1）：

- 今日页的日程：先测 Evolution Data Server 的内存代价，再决定读 EDS 还是只读本地 `.ics`；
- 用真的播放器测“正在播放”（mpv 加 mpris 插件，包较大，装之前先问）；
- AUR 包；Qt 5 主题；
- 作者选好线稿后，放进 `design/plates/` 调线色和位置。
