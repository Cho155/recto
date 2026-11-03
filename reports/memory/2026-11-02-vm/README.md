# 内存测量 · 2026-11-02 · 测试虚拟机

## 环境

- QEMU 11.1，KVM，3 核，3 GiB 内存；虚拟显卡 `virtio-vga-gl`（virgl，宿主机 GPU 渲染），屏幕 1440×900，缩放 1。
- Arch Linux，内核 7.2.14；niri 26.04；GTK 4.24.1；Mesa 26.2.6。
- 会话：niri（tty1 自动登录）、liubai-shell、foot 服务、mako、swaybg；另开着 2 个 footclient 和 1 个 gnome-text-editor。
- 工具：`tools/measure-memory`，读 `/proc/<pid>/smaps_rollup`。下表除特别说明外都是 **PSS**。

**注意**：虚拟机的图形栈（virgl）和真机不同；gl 渲染器的那部分开销在真机上会因 GPU 驱动而异。cairo 渲染器不加载 GPU 驱动，受此影响小。

## 外壳：不同渲染器

每种渲染器都重启外壳，空闲 60 秒后测量，打开今日页 5 秒后测量，关闭今日页 30 秒后再测量。

| 渲染器 | 空闲 | 今日页打开 | 关闭后 | 文件 |
| --- | --- | --- | --- | --- |
| 默认（GTK 选 gl） | 41.9 MiB | 44.1 MiB | 43.2 MiB | `default-*.json` |
| gl | 41.7 MiB | 44.2 MiB | 43.4 MiB | `gl-*.json` |
| vulkan | 42.1 MiB | 44.2 MiB | 43.4 MiB | `vulkan-*.json`（虚拟机不支持 Vulkan，退回 gl） |
| **cairo** | **18.3 MiB** | **20.0 MiB** | **18.8 MiB** | `cairo-*.json` |

结论：cairo 比 gl 少约 24 MiB，差额基本是 GPU 驱动库的开销。外壳因此默认使用 cairo（决策 D26），设置了 `GSK_RENDERER` 时以设置为准。改为默认后复测，外壳空闲 18.9 MiB（`cairo-default-idle.json`）。

## 整个桌面（外壳用 cairo，空闲）

| 进程 | PSS | RSS | USS |
| --- | --- | --- | --- |
| niri | 67.3 | 101.7 | 48.1 |
| liubai-shell | 18.9 | 45.5 | 8.9 |
| xdg-desktop-portal（两个进程） | 22.6 | 44.8 | 18.4 |
| foot（服务） | 8.6 | 19.1 | 4.1 |
| mako | 6.4 | 27.1 | 3.6 |
| swaybg | 5.7 | 14.3 | 4.6 |
| pipewire、dbus 等 | 约 4 | — | — |
| **桌面合计** | **133.5 MiB** | | |

不含用户应用（gnome-text-editor 66.6 MiB、测量脚本本身 13.4 MiB）。门户服务是 GTK 应用启动时由 D-Bus 拉起的，不是留白主动启动的。

## 对照预算（docs/ARCHITECTURE.md）

| 项目 | 预算 | 实测 | |
| --- | --- | --- | --- |
| 外壳空闲，仅状态栏 | ≤ 60 MiB | 18.9 MiB | 达标 |
| 外壳，打开今日页 | ≤ 100 MiB | 20.0 MiB | 达标 |
| 关闭面板 30 秒后 | ≤ 空闲的 110% | 103% | 达标 |
| 整个会话空闲 | ≤ 300 MiB | 133.5 MiB | 达标 |
