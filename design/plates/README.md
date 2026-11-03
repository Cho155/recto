# 图版（壁纸）

把线稿放在这里，然后在 `design/tokens.toml` 的 `[wallpaper]` 里写上：

```toml
plate         = "plates/文件名.svg"
plate_place   = "right"        # left | center | right
plate_width   = 0.42
plate_caption = "Fig. 1 — 标题, after 作者"
```

- 格式：SVG（要有 `viewBox`）、PNG 或 JPEG。
- 三种图版（`plate_kind`）：
  - `line`（默认）：线稿、版画、铅笔素描，黑线白底或透明底。下面说的重新上色只对它生效；四边淡入桌面，纸边和压印痕不会框出方块。
  - `drawing`：纸上的彩色素描（红粉笔、水彩、植物画），保留原色。Paper 里像印在纸上（正片叠底，画纸的白变成桌面的纸色），四边淡入桌面；Ink 里是一张压暗的纸。
  - `painting`：油画，保留原色和清楚的边。Paper 里同样叠底，Ink 里按 `plate_ink_dim` 压暗并略微偏暖。
- `plate_place = "full"` 让图版铺满整个桌面（不排图版说明），适合横幅的油画。

## 准备图片

在测试机里用 ImageMagick 处理（本机不运行）：

```sh
# 线稿：转灰度，把发黄的纸调白，裁掉纸边，最长边 2400
magick in.jpg -shave 3%x3% +repage -colorspace Gray -contrast-stretch 0.3%x45% -resize '2400x2400>' -quality 88 -strip out.jpg
# 素描：三个通道一起调亮（不改色调），裁掉衬纸
magick in.jpg -shave 3%x2% +repage -level 0%,90% -resize '2400x2400>' -quality 88 -strip out.jpg
# 油画：只缩小
magick in.jpg -resize '2560x2560>' -quality 86 -strip out.jpg
```
- 线稿的颜色不用管：生成壁纸时，线条按深浅换算成油墨量，用当前主题的 `plate_line` 重新上色，Paper 和 Ink 两套都会自动适配。
- 线稿会随 `recto-theme build` 和 `install` 一起复制，安装后也能找到。
- 随项目分发的线稿必须是公有领域或允许再分发的授权，并在图版说明或下面的列表里写明来源。

换成另外两张：把 `design/tokens.toml` 里 `plate` 改成对应文件（Corot 再加 `plate_place = "full"`），说明改成表里的作品名，然后重新运行 `./install.sh`。其余候选见 `candidates/`。

| 文件 | 来源 | 授权 |
| --- | --- | --- |
| `hammershoi-strandgade.jpg`（默认，painting，右） | Vilhelm Hammershøi, *Interior in Strandgade, Sunlight on the Floor*, 1901；[SMK – National Gallery of Denmark](https://open.smk.dk/artwork/image/KMS3696) | 公有领域（Public Domain Mark 1.0） |
| `chardin-kitchen.jpg`（painting，右） | Jean-Siméon Chardin, *Kitchen Utensils with Leeks, Fish, and Eggs*, c. 1734；[Cleveland Museum of Art](https://clevelandart.org/art/1980.37) | CC0 |
| `corot-ville-davray.jpg`（painting，`full`） | Camille Corot, *Pond at Ville-d'Avray*, late 1860s；[Cleveland Museum of Art](https://clevelandart.org/art/1944.80) | CC0 |
