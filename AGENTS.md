# 项目工作约定

## 入口

- 每次先读 `STATUS.md`，再按任务读 `docs/` 中的对应文档。不依赖聊天记忆。
- 本项目是开发工作，不是学习任务；不读取学习说明，不记学习日志。
- 本项目与作者的其他项目无关，不引用、不复用其他项目的代码或文档。

## 工作边界（作者 2026-11-02 确认）

- **本机不做任何测试**：不安装依赖，不构建、不运行，不修改本机桌面配置（`~/.config` 等），不写自启动或快捷键。
- **只能在虚拟机里测试**。作者 2026-11-02 授权：助手可以自行决定何时使用虚拟机。登录信息不写进仓库、文档或记忆。
- 测试机：`~/VMs/liubai-arch/`（Arch 官方 basic 镜像 + 覆盖盘 `liubai.qcow2`；`start.sh` 启动，`vm` 执行命令，SSH 端口 2223，专用密钥在同目录）。删除覆盖盘即可重置。
- 测试机里可用 ydotool 模拟点击（`ydotoold` 以 root 运行，套接字 `/tmp/.ydotool_socket`）；测试机的 `~/.config/niri/local.kdl` 关掉了指针加速和热角，只为测试。
- 不使用、不修改 `~/VMs/debian/`（作者的 Debian 虚拟机），也不连接 `~/.ssh/config` 里的远程主机。
- **真机测试**（作者 2026-11-03 确认）：以后所有真机测试都在本机的 `test` 账户里进行，由作者操作；作者自己的账户仍然不做任何测试、不运行Recto。
  - 项目副本：`/home/shared/liubai`（归 test 所有，从作者账户可读）。共享文件夹 `/home/shared` 两个账户都可写。
  - test 在 wheel 组里（作者 2026-11-03 要求），和作者账户一样是免密 sudo；`start` 会在 test 里用 sudo 装缺的软件包（作者自己运行，pacman 会让作者确认）。助手自己要装包，仍然先问作者。
  - 助手可以从作者账户读取 `/home/shared/liubai/reports/real-hardware/` 里的结果。
  - 流程（2026-11-03 第六轮）：test 在 tty3 登录后运行 `bash /home/shared/start`（快进副本、查包、安装并把输出存进 `reports/real-hardware/install-*.log`、启动 niri）；测完在Recto里运行 `bash /home/shared/finish`（报告、截图、本次开机的用户日志，放进 `reports/real-hardware/<时间>/`）。两个脚本在 `/home/shared`，归作者账户，不在仓库里。
  - 本机 tty1 是 greetd（tuigreet），作者登录着时不会再出现登录界面，所以 test 只能从文字控制台启动 `niri-session`；救援用 tty4。
  - 给副本送新版本：在作者账户里提交后运行 `git bundle create /home/shared/recto.bundle master`，test 下次运行 `start` 时快进过去。不要用 sudo 改副本。
  - 作者在清单页上记的结果：https://claude.ai/artifact/4MMCpufq6P1RhbrJT2stNL（数据库集合 `results`（文档 id 是清单编号 A1…H5，字段 status 为 ok／bad／skip，note）、`steps`（s0–s4）、`notes/general`），用 ArtifactData 读取。页面源文件可用 Artifact 的 read 取回。
  - wheel 组可以读整个系统日志（含 test 的用户日志），但约定只通过共享文件夹和清单页拿结果；要直接读日志先问作者。`finish` 会把本次开机的内核警告存成 `kernel-warnings.txt`。
  - test 的登录信息不写进仓库、文档或记忆。
- 可以在项目目录内编写文档、令牌、样张和源码。

## 设计约定

- 设计和工具上的选择由助手决定，记入 `docs/DECISIONS.md` 并写明理由。作者本机装了什么（如 niri、Noctalia）不代表项目要求；本机配置只可用来确认语法。
- Recto交付面向全新系统的完整配置，不针对作者现有的配置来设计。
- 界面文案默认英文；其他语言的渲染由作者单独处理。
- 颜色、字体、尺寸、动效只从 `design/tokens.toml` 读取，不在代码或主题中写死。
- 新增颜色组合要计算对比度，正文 ≥ 4.5 : 1。
- GTK 里会自动换行的文字要设最大宽度（`set_max_width_chars`），否则会把窗口撑宽。
- 不使用 Anthropic 的标志、名称和商业字体；只借鉴风格。
- 内存数字必须附测量记录（`reports/memory/`），写明是 PSS 还是 RSS；没有测量的数字只能称为“目标”。

## 记录约定

- 每次工作结束更新 `STATUS.md`：做了什么、没做什么、下一步。
- 项目文档用中文；界面文案、代码注释与令牌文件用英文。
