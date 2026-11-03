# 内存测量 · 2026-11-02 · 阶段 4 · 测试虚拟机

环境同 [`../2026-11-02-vm-phase3b/`](../2026-11-02-vm-phase3b/README.md)，会话里新增 swayidle 和 polkit-gnome。外壳用 cairo 渲染器。

## 外壳

| 时刻 | 外壳 PSS | 文件 |
| --- | --- | --- |
| 重启外壳后空闲 60 秒 | 18.5 MiB | `idle.json` |
| 搜索窗口打开 | 20.8 MiB | `search-open.json` |
| 关闭 30 秒后 | 19.6 MiB（空闲的 106%） | `after.json` |

## 整个桌面（`after.json`）

| 进程 | PSS |
| --- | --- |
| niri | 63.2 MiB |
| liubai-shell | 19.6 MiB |
| polkit-gnome（授权代理） | 16.9 MiB |
| xdg-desktop-portal（两个进程） | 17.9 MiB |
| wireplumber、pipewire、pipewire-pulse | 18.4 MiB |
| foot（服务） | 3.6 MiB |
| swaybg | 3.3 MiB |
| pactl（麦克风监听） | 1.2 MiB |
| swayidle | 0.4 MiB |
| dbus | 约 2 MiB |
| **合计** | **约 149 MiB**（预算 300 MiB） |

说明：
- 和阶段 2 的 133.5 MiB 相比，增加的部分主要是音频服务（阶段 2 的测试机还没装 PipeWire）和 polkit-gnome。
- 这次的 `after.json` 里有 3 个 `pactl` 进程，原因是外壳重启后旧的 `pactl subscribe` 子进程没有退出（缺陷）。已修复，修复后连续重启 3 次，始终只有 1 个。上表只计入 1 个。
- `tools/measure-memory` 的“桌面”名单原先漏了 swayidle、polkit-gnome 和 pactl，已补上；`after.json` 里的 `desktop_pss_kib`（128.2 MiB）是补名单之前算的。
- polkit-gnome 常驻 16.9 MiB，是目前除 niri 以外最大的一项。以后如果由外壳自己实现授权代理，大约能省下这么多；但授权涉及密码处理，需要单独评估安全性。
