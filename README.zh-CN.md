# wordcloud (Rust)

[![crates.io](https://img.shields.io/crates/v/wordcloud.svg)](https://crates.io/crates/wordcloud)
[![docs.rs](https://docs.rs/wordcloud/badge.svg)](https://docs.rs/wordcloud)

[English](README.md) | **简体中文**

一个快速的 Rust 词云生成器，既可以当库用，也可以当命令行工具用。它移植自 Python 的
[amueller/word_cloud](https://github.com/amueller/word_cloud)（v1.9.6），参数和默认值都与其一致。

<table>
  <tr>
    <td width="33%" align="center" valign="top">
      <img src="docs/gallery/zh/mask.png" width="100%" alt="云朵形状的西游记词云，带轮廓线"><br>
      <b>任意形状</b><br>
      <sub>用一张遮罩图片决定形状，可加轮廓线<br><code>--mask shape.png --contour_width 3</code><br><a href="examples/shape_mask.rs">examples/shape_mask.rs</a></sub>
    </td>
    <td width="33%" align="center" valign="top">
      <img src="docs/gallery/zh/image_colors.png" width="100%" alt="心形的红楼梦词云，按彩虹色带着色"><br>
      <b>从图片取色</b><br>
      <sub>每个词使用它背后图片区域的颜色<br><code>--colormask picture.png</code><br><a href="examples/image_colors.rs">examples/image_colors.rs</a></sub>
    </td>
    <td width="33%" align="center" valign="top">
      <img src="docs/gallery/zh/colormap.png" width="100%" alt="黑色背景、plasma 配色的三国演义词云"><br>
      <b>配色方案</b><br>
      <sub>内置 18 种，从 viridis 到 jet<br><code>--colormap plasma</code><br><a href="examples/colormap.rs">examples/colormap.rs</a></sub>
    </td>
  </tr>
  <tr>
    <td width="33%" align="center" valign="top">
      <img src="docs/gallery/zh/font.png" width="100%" alt="书法字体的诗经词云"><br>
      <b>任意字体</b><br>
      <sub>支持 TTF、OTF、TTC，书法字体也可以<br><code>--fontfile MaShanZheng-Regular.ttf</code><br><a href="examples/custom_font.rs">examples/custom_font.rs</a></sub>
    </td>
    <td width="33%" align="center" valign="top">
      <img src="docs/gallery/zh/single_color.png" width="100%" alt="单一铁锈色、全部横排的水浒传词云"><br>
      <b>单色、只横排</b><br>
      <sub>同一颜色的不同深浅，可控制文字方向<br><code>--color "#b7410e" --prefer_horizontal 1</code><br><a href="examples/single_color.rs">examples/single_color.rs</a></sub>
    </td>
    <td width="33%" align="center" valign="top">
      <img src="docs/gallery/zh/transparent.png" width="100%" alt="透明背景的聊斋志异词云"><br>
      <b>透明背景</b><br>
      <sub>输出 RGBA PNG，可直接放到任何页面上<br><code>--mode RGBA --background None</code><br><a href="examples/transparent.rs">examples/transparent.rs</a></sub>
    </td>
  </tr>
</table>

<sub>以上图片全部由 <a href="examples/"><code>examples/</code></a> 里的代码生成（见下文<a href="#示例">示例</a>）。
文字取自《西游记》《红楼梦》《三国演义》《诗经》《水浒传》《聊斋志异》的原文节选，经分词和 TF-IDF 统计得到关键词；
字体为霞鹜文楷、站酷快乐体、思源宋体和马善政楷书（均为 SIL OFL 开源字体）。</sub>

> **关于包名：** 本仓库名为 `wordcloud-rs`，但在 crates.io 上发布的名字是
> **[`wordcloud`](https://crates.io/crates/wordcloud)**，与 `pip install wordcloud` 一致。
> crates.io 上名为 `wordcloud-rs` 的 crate 是另一个无关的项目。
> 安装用 `cargo add wordcloud`，导入用 `use wordcloud::...`。

## 功能

* **输入**：原始文本（分词、停用词、复数合并以及 "White Rabbit" 这类双词短语都会自动处理），
  或者你自己给出的 `(词, 权重)` 列表。
* **形状与外观**：带轮廓线的遮罩、配色方案、单一颜色、从图片取色或自定义着色函数、任意字体、
  边距、文字方向和字号缩放。
* **输出**：PNG（RGB 或透明 RGBA）、SVG，或者原始布局数据（词、字号、位置、颜色），方便你自己绘制。
* **与种子严格一致**：复现了 CPython 的梅森旋转随机数、Python 字典顺序、Pillow 的 `BLEND`
  以及所有历史布局怪癖，因此在字形位图相同的情况下，布局和 PNG 像素与 Python 版逐字节一致。
* **快且省内存**：端到端比 Python/Cython 原版快 7–19 倍，内存只占一小部分（见[性能测试](#性能测试)）。
* **安全**：`#![deny(unsafe_code)]`。唯一的例外是一处经过审查的代码块（占用窗口计数中省略边界检查，
  并由 `assert!` 保护），保留它只是因为基准测试显示比安全写法快 12–15%。

## 性能测试

两边使用相同的输入和参数；每个数字都是多个随机种子的中位数，每次运行都是独立进程。

| 场景 | Python | Rust | 提速 | 峰值内存 Python → Rust |
|---|---:|---:|---:|---:|
| 默认，400×200 | 628 ms | **33 ms** | **19 倍** | 66 MB → **8 MB** |
| 大图，1000×700，400 个词 | 2.61 s | **0.22 s** | **12 倍** | 83 MB → **11 MB** |
| 遮罩，900×900，400 个词 | 2.89 s | **0.27 s** | **11 倍** | 86 MB → **18 MB** |

**极限测试**

| 场景 | Python | Rust | 提速 | 峰值内存 Python → Rust |
|---|---:|---:|---:|---:|
| 50 MB 文本 | 9.0 s | **1.0 s** | **9 倍** | 756 MB → **86 MB** |
| 4K 画布，3840×2160，1000 个词 | 87.9 s | **12.4 s** | **7 倍** | 305 MB → **59 MB** |
| 极高密度：1000×700，3000 个词，最小字号 2，边距 1，`--repeat` | 35.2 s | **2.7 s** | **13 倍** | 86 MB → **12 MB** |

以上是命令行端到端耗时（`python -m wordcloud` 对比 `wordcloud`），包含 Python 0.4–0.6 秒的解释器启动和导入时间。
如果只在已启动的解释器里计时 `WordCloud(...).generate(text).to_image()`，前三个场景 Python 分别需要
214 ms、2.06 s 和 2.25 s，即作为库使用时 Rust 仍然**快 6–9 倍**。

<details>
<summary>测试环境与方法</summary>

- CPU：Intel Xeon @ 2.80 GHz（Cascade Lake：family 6，model 85，stepping 7），4 核，KVM 虚拟机
- 内存：15.7 GiB
- 系统：Ubuntu 24.04.5 LTS，Linux 6.18
- Rust：rustc 1.97.0，`cargo build --release`（fat LTO，未启用 PGO）
- Python：3.13.18，wordcloud 1.9.6，NumPy 2.5.4，Pillow 13.0.0
- 两者都是单线程运行，核心数不影响结果。
- 输入：`tests/golden/alice.txt`（上游项目的示例文本）。50 MB 的输入由 `alice.txt`、`a_new_hope.txt`
  和 `constitution.txt` 重复拼接而成。
- 常规场景每个跑 10 个种子，极限测试每个跑 3 个。耗时和峰值内存（RSS）通过 `wait4` 按进程测量
  （见 `tools/peak_rss.rs`）。
- 两者用不同的库光栅化字形（FreeType 与 `ab_glyph`），同一种子下的布局略有不同，
  所以两边的工作量接近但不完全相同。

复现方法：

```bash
cargo build --release
python3 -m venv /tmp/wc-venv && /tmp/wc-venv/bin/pip install wordcloud==1.9.6
python3 tools/benchmark.py --python /tmp/wc-venv/bin/python --runs 10 --stress-runs 3
```

</details>

## 安装

```bash
cargo install wordcloud      # 命令行工具，安装 `wordcloud` 可执行文件
cargo add wordcloud          # 作为库使用
```

## 命令行

参数和默认值与 Python 的 `wordcloud_cli` 相同。运行 `wordcloud --help` 查看全部参数。

```bash
# 读文本文件，输出 PNG，固定种子以得到可复现的布局
wordcloud --text alice.txt --imagefile alice.png --random_state 1

# 从标准输入读，写到标准输出
cat alice.txt | wordcloud > alice.png

# 更大的画布、白色背景、配色方案，并同时输出 SVG
wordcloud --text alice.txt --imagefile alice.png --svgfile alice.svg \
  --width 800 --height 400 --background white --colormap plasma

# 填充形状：遮罩中的白色像素保持空白
wordcloud --text alice.txt --mask alice_mask.png --contour_width 3 \
  --contour_color steelblue --imagefile alice.png

# 中文：先分好词并用空格隔开，再指定一个中文字体
wordcloud --text 已分词.txt --fontfile LXGWWenKai-Medium.ttf --no_collocations --imagefile zh.png
```

相比 Python 版新增：`--colormap` 真正生效（Python 版会静默忽略它）、`--svgfile out.svg`
以及 `--fix-legacy-bugs`。

## 库

最简示例：输入文本，输出 PNG。

```rust
use std::fs::{self, File};
use wordcloud::{render, WordCloud};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let text = fs::read_to_string("alice.txt")?;

    let wc = WordCloud::builder()
        .size(800, 400)
        .max_words(300)
        .seed(1) // 种子相同，布局就相同
        .build()?;

    let layout = wc.generate(&text)?;
    render::write_png(&render::to_image(&wc, &layout)?, File::create("alice.png")?)?;
    Ok(())
}
```

自己提供词频、设置颜色并输出 SVG：

```rust
use wordcloud::{color::ColorFunc, svg, Config, WordCloud};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let freqs = vec![
        ("Rust".to_string(), 10.0),
        ("safe".to_string(), 6.0),
        ("fast".to_string(), 5.0),
        ("cargo".to_string(), 3.0),
    ];

    let config = Config {
        width: 600,
        height: 300,
        background_color: Some("white".into()),
        ..Config::default()
    };
    let wc = WordCloud::builder()
        .config(config)
        .color_func(ColorFunc::colormap("plasma")?) // 或 ColorFunc::single("darkblue")?
        .seed(7)
        .build()?;

    let layout = wc.generate_from_frequencies(&freqs)?;
    for w in &layout.words {
        println!("{} size={} at {:?}", w.word, w.font_size, w.position);
    }
    std::fs::write("cloud.svg", svg::to_svg(&wc, &layout)?)?;
    Ok(())
}
```

形状由 `Mask` 决定：白色像素处不放文字，画布大小取遮罩的大小。下面的例子用
[`image`](https://crates.io/crates/image) crate 加载遮罩（`cargo add image`）：

```rust
use std::fs::{self, File};
use wordcloud::{render, Mask, WordCloud};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let img = image::open("alice_mask.png")?.to_rgb8();
    let mask = Mask::from_pixels(img.width() as usize, img.height() as usize, 3, img.as_raw())?;

    let wc = WordCloud::builder().mask(mask).seed(1).build()?;
    let layout = wc.generate(&fs::read_to_string("alice.txt")?)?;
    render::write_png(&render::to_image(&wc, &layout)?, File::create("alice.png")?)?;
    Ok(())
}
```

**中文、日文等不用空格分词的语言**：本库按非字母字符切分单词，所以一整句中文会被当成一个词。
请先分词（例如用 [`jieba-rs`](https://crates.io/crates/jieba-rs)）并用空格把词隔开，
或者直接把 `(词, 权重)` 传给 `generate_from_frequencies`。另外还需要一个包含这些字的字体，
命令行用 `--fontfile`，库里这样写：

```rust
use std::fs::{self, File};
use wordcloud::{raster::AbGlyphRasterizer, render, WordCloud};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let font = AbGlyphRasterizer::from_bytes(fs::read("LXGWWenKai-Medium.ttf")?, true)?;
    let freqs: Vec<(String, f64)> = [("词云", 60.0), ("数据", 45.0), ("可视化", 40.0), ("开源", 32.0)]
        .into_iter()
        .map(|(w, n)| (w.to_string(), n))
        .collect();

    let wc = WordCloud::builder().rasterizer(Box::new(font)).seed(1).build()?;
    let layout = wc.generate_from_frequencies(&freqs)?;
    render::write_png(&render::to_image(&wc, &layout)?, File::create("zh.png")?)?;
    Ok(())
}
```

停用词、双词短语、分词正则和最小词长都可以通过 `Config::text` 设置。全部选项见
[API 文档](https://docs.rs/wordcloud)。

## 示例

画廊里的每张图都对应一个可以直接运行的示例，同时生成英文版和中文版。中文字体（SIL OFL 许可）
由脚本下载，不随仓库分发：

```bash
sh examples/fetch_fonts.sh
cargo run --release --example shape_mask
```

| 示例 | 演示内容 |
|---|---|
| [`shape_mask`](examples/shape_mask.rs) | 构造 `Mask` 遮罩并绘制轮廓线 |
| [`image_colors`](examples/image_colors.rs) | 用 `ImageColorGenerator` 从图片取色 |
| [`colormap`](examples/colormap.rs) | 内置配色方案 |
| [`custom_font`](examples/custom_font.rs) | 加载 TTF/OTF 字体、中文词云 |
| [`single_color`](examples/single_color.rs) | 单色、只横排、字号缩放 |
| [`transparent`](examples/transparent.rs) | 无背景的 RGBA 输出 |

中文示例使用古登堡计划（Project Gutenberg）中已进入公有领域的古典名著（节选放在
[`examples/data/zh/`](examples/data/zh/)），先用 [`fast2s`](https://crates.io/crates/fast2s)
繁转简，再用 [`jieba-rs`](https://crates.io/crates/jieba-rs) 分词并按 TF-IDF 提取关键词
（见 [`examples/common/mod.rs`](examples/common/mod.rs)）。

## 字体渲染与一致性

默认的光栅化器是纯 Rust 的 `ab_glyph`。它与 FreeType + HarfBuzz 的字形微调略有差别，
所以同一个种子下的**精确**布局会与 Python 版不同。为了证明引擎其余部分完全一致，
`tools/record_pillow.py` 会记录 Pillow 生成的每个字形，再通过隐藏参数 `--replay-glyphs`
交给 Rust 引擎使用，最后由 `tools/parity.py` 比较布局和像素。

## 已知差异

* 调色板模式（`P`）的遮罩图片会先解码成颜色（Python 版是拿调色板*索引*和 255 比较）。
* 接受灰度的取色图片（Python 版会抛出 `NotImplementedError`）。
* `--contour_width` 与 `--mode RGBA` 可以一起用（Python 版会因广播错误崩溃）。
* Python 的 `\w` 用 `[\p{L}\p{N}_]` 近似。
* 未实现 SVG 字体嵌入（`embed_font`）。
* `scale != 1` 时的轮廓绘制使用 Catmull-Rom 缩放，近似 Pillow 的双三次滤波。

## 仓库结构

| 路径 | 内容 |
|---|---|
| `src/` | 库和 `wordcloud` 可执行文件 |
| `tests/` | 黄金数据一致性测试、属性测试、命令行测试和引擎测试（`tests/golden/` 存放由 Python 包生成的数据） |
| `tools/` | `benchmark.py` + `peak_rss.rs`（README 中的性能测试）、`record_pillow.py` + `parity.py`（A/B 对比工具）、`make_golden.py`、`gen_tables.py`、`bench.py` |
| `docs/` | 迁移方案、遥测数据、交互式报告（`migration_infographic.html`）以及 README 示例图（`gallery/`） |
| `examples/` | 生成示例图的代码，输入文本在 `examples/data/` |

## 许可证

MIT，与上游项目相同。内置字体的说明见 `LICENSE` 和 `assets/README.md`。
