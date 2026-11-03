# 图版（壁纸线稿）

把线稿放在这里，然后在 `design/tokens.toml` 的 `[wallpaper]` 里写上：

```toml
plate         = "plates/文件名.svg"
plate_place   = "right"        # left | center | right
plate_width   = 0.42
plate_caption = "Fig. 1 — 标题, after 作者"
```

- 格式：SVG（要有 `viewBox`）或 PNG。黑线白底或透明底都可以。
- 颜色不用管：生成壁纸时，线条按深浅换算成墨量，用当前主题的 `plate_line` 重新上色，纸和墨两套都会自动适配。
- 线稿会随 `liubai-theme build` 和 `install` 一起复制，安装后也能找到。
- 随项目分发的线稿必须是公有领域或允许再分发的授权，并在图版说明或下面的列表里写明来源。

| 文件 | 来源 | 授权 |
| --- | --- | --- |
| （暂无） | | |
