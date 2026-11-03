# 项目状态

更新：2026-11-03（第二天）。阶段 0–3 完成，阶段 4 大部分完成。上午打磨了动效、材质、排版；下午按作者的方向改为**古典的书页风格**（D63–D72）。全部在测试虚拟机中验证过，本机没有做过测试（见下方“没做”里的一处例外）。

## 现在能用的

| 部分 | 内容 | 文档 |
| --- | --- | --- |
| 设计 | 理念（含“古典，而不是 Windows”）、设计语言、令牌、样张（9 张画板，2026-11-03 同步为古典风格，含锁屏扉页和图版位置） | `docs/VISION.md`、`docs/DESIGN-LANGUAGE.md`、`design/` |
| 主题系统 | 一份令牌生成 niri、foot、GTK 3/4、mako、fuzzel、kitty、alacritty、外壳的配置；纸／墨／Auto 实时切换；壁纸是等高线或作者的线稿（图版，自动按主题上色、配图版说明），带纸纹；锁屏是扉页 | `docs/THEMES.md`、`design/plates/README.md` |
| 外壳 `liubai-shell` | 状态栏（页眉式，整条衬线：罗马数字工作区、斜体日期、旧体数字时钟、斜体的专注提示）、今日页（整页）、通知（页边纸条，朱批）、音量／麦克风／亮度提示、专注模式、搜索（索引）、托盘 | `docs/INTERACTION.md`、`docs/ARCHITECTURE.md` |
| 动效 | 面板从来处进出、只移动 6 px；通知原地放下、其他纸条平滑让位；开窗像一张纸落到桌上（niri 着色器） | `docs/DESIGN-LANGUAGE.md` 动效一节 |
| 材质与排版 | Source Serif 4 为主：小型大写、旧体数字、光学尺寸；小圆角（6／3／2）；1 px 陶土焦点线；当前窗口阴影略深；自绘细线图标 | D55–D58、D63–D67 |
| 应用 | GTK 3/4 配色；文本编辑器正文区的 `liubai` 配色；GNOME 文档字体为衬线 | `docs/THEMES.md`、D75 |
| 会话 | swayidle 空闲锁屏、polkit-gnome 授权弹窗、foot 服务进程 | `templates/niri/config.kdl` |

- **测试**：43 个单元测试通过，编译无警告。
- **提交**：作者 2026-11-03 同意随时提交；第一次提交 `c6361a8`。
- **外壳内存**：空闲约 24 MiB PSS（RSS 41–51 MiB，视会话里用过的东西而定）；整页今日页打开时多约 17 MiB，关闭后释放；反复开关不增长（D61）。
- **整个桌面空闲**：约 174 MiB，预算 400 MiB（`reports/memory/2026-11-03-vm-polish/`）。
- **动效帧率**：cairo 渲染器下满帧（75 Hz），只有搜索的第一帧要 53 ms（`reports/motion/2026-11-03-vm/`）。
- **截图**：`docs/screenshots/`，31 张。下午的古典风格见 `2026-11-03-vm-classical-*.png`、`…-plate-test-drawing.png`（图里的叶子是测试用的线稿，不随项目分发）、`…-lock-title-page.png`。
- **样张画布**：https://claude.ai/artifact/LsCAAp3mUTsEYSnUoPumiz（私有；第 13 版，已同步古典风格；源文件在 `design/specimen/project/`）。

## 没做 / 已知问题

- 还没有真正的线稿：图版系统只用测试画验证过，等作者挑选线条画。
- 锁屏扉页跨过午夜不会更新日期；扉页上的日期用的是普通数字（rsvg 不支持旧体数字）。
- 2026-11-03 上午曾在本机运行过一次 `liubai-theme build`（输出到临时目录，已删除），违反了“本机不运行”的约定；之后都在虚拟机里运行。
- 终端背景模糊没有评估（原计划在材质一项里做，今天没做到）。
- 动效只在虚拟机里测过。高分屏（缩放 2）上 cairo 每帧的像素是 4 倍，可能掉帧，要在真机上测；掉帧就换 GL（D50）。
- 搜索打开的第一帧要 53 ms（要先排版整张应用列表）。
- niri 的内存比前一天高（78 MiB，前一天 63 MiB）；对照测试表明与开窗着色器无关，要在刚登录的会话里重测。
- 会话里的 `at-spi2-registryd` 每次重启 niri 会话多一个（与留白无关，待查）。
- 今日页没有日程；Evolution Data Server 的内存代价还没测。
- 没有 Qt 主题。
- 低电量提示没有实测（虚拟机没有电池）；空闲锁屏的 10 分钟计时没有完整等过；锁屏后用密码解锁没有测过。
- 搜索不读 .desktop 的 Keywords（D45）。
- 输入法指示不支持 IBus；托盘项不响应滚轮；托盘面板不能用键盘操作（Esc 也关不掉）。
- 在别的程序里改音量不会弹出提示（D30）。
- 卷轴缩略分不出屏幕内外的列（niri 不报告平铺窗口的视口位置）。
- polkit-gnome 常驻约 11–17 MiB，是可以优化的大头。
- 所有测量都在虚拟机（virgl 显卡）里完成；在真机上要重新测量。
- 许可证未定。
- 项目已 `git init`，**还没有任何提交**。

## 测试机现状（`~/VMs/liubai-arch/`，2026-11-03 下午仍在运行）

- 启动：`./start.sh`；执行命令：`./vm '<命令>'`；同步项目：`./sync.sh`（不覆盖测试机里的 `build/` 和 `shell/target/`）。
- 已安装：niri 26.04 一套、Rust 工具链、PipeWire、UPower、NetworkManager（取代了 systemd-networkd）、fcitx5（配置了 us 和 de 两种键盘布局）、nm-applet、ydotool、swayidle、polkit-gnome。
- 测试专用的改动：
  - tty1 自动登录进 niri；
  - `~/bin/insession` 让 SSH 命令带上会话环境；
  - `~/.config/niri/local.kdl` 关掉了指针加速和热角；
  - arch 用户加入了 wheel 组，用于测授权弹窗。
- 每次开机后，要模拟点击需先启动 `sudo ydotoold -p /tmp/.ydotool_socket -P 0666`。
- 测试过程中往 `~/Documents/monday-notes.md` 误打进了一行 “arch”（只在测试机里）。
- 2026-11-03 新增：装了 imagemagick（用于把慢放截图拼成一张）。
- 测试期间会手动停掉 swayidle（`pkill -x swayidle`），否则 10 分钟后锁屏、截图全是锁屏画面；重启 niri 会话后它会自动回来。锁屏后可以用 `niri msg action quit --skip-confirmation` 重启会话来解除（自动登录会重新进入）。
- 慢放：外壳用 `GTK_SLOWDOWN=8` 启动；niri 在 `local.kdl` 里临时加 `animations { slowdown 10.0 }`，用完删掉。

## 下一步

1. **作者提供线稿后**：放进 `design/plates/`，在令牌里打开图版，调线色和位置；记下来源和授权。
2. **继续古典化的细节**：
   - 空状态用一个装饰分隔（“* * *”）代替纯文字（还在考虑：装饰用多了会显得刻意）；
   - GTK 应用自身的界面（libadwaita 的标题栏、按钮）还是 Adwaita 的样子，可以评估多大程度上纳入纸墨。
3. **其余待办**：
   - 终端背景模糊的评估；
   - 搜索第一帧的卡顿；
   - 托盘的键盘操作；
   - 在刚登录的会话里重测 niri 内存；
   - 今日页的日程（先测 Evolution Data Server）；
   - Qt 主题；
   - 真机验证（高分屏动效帧率是重点）；
   - 许可证与第一次提交（提交需要作者同意）。
