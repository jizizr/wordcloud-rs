//! SVG export (port of `WordCloud.to_svg`, without font/image embedding).

use crate::error::Result;
use crate::layout::{Layout, WordCloud};
use crate::raster::Orientation;

/// Formats a float like Python's `repr` for the ranges used here
/// (`400.0`, `12.5`).
fn py_float(v: f64) -> String {
    if v.fract() == 0.0 && v.abs() < 1e16 {
        format!("{v:.1}")
    } else {
        format!("{v}")
    }
}

/// Escapes `&`, `<`, `>` like `xml.sax.saxutils.escape`.
fn escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

/// Renders the layout as an SVG document.
///
/// The font is referenced by family name and must be available to the SVG
/// viewer; glyph metrics come from the configured rasteriser.
///
/// # Errors
/// Rasteriser errors.
///
/// ```
/// use wordcloud::{svg, WordCloud};
/// let wc = WordCloud::builder().size(200, 100).seed(1).build()?;
/// let layout = wc.generate("svg svg export")?;
/// let doc = svg::to_svg(&wc, &layout)?;
/// assert!(doc.starts_with("<svg") && doc.ends_with("</svg>"));
/// assert!(doc.contains(">svg</text>"));
/// # Ok::<(), wordcloud::WordCloudError>(())
/// ```
pub fn to_svg(wc: &WordCloud, layout: &Layout) -> Result<String> {
    let cfg = &wc.config;
    let (w, h) = wc.canvas_size();
    let (family, style) = wc.rasterizer().family_and_style();
    let style = style.to_lowercase();
    let weight = if style.contains("bold") {
        "bold"
    } else {
        "normal"
    };
    let font_style = if style.contains("italic") {
        "italic"
    } else if style.contains("oblique") {
        "oblique"
    } else {
        "normal"
    };
    let family = format!("'{}'", family.replace('\'', "\\'"));
    let mut out = vec![
        format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="{}" height="{}">"#,
            py_float(w as f64 * cfg.scale),
            py_float(h as f64 * cfg.scale)
        ),
        format!(
            "<style>text{{font-family:{family};font-weight:{weight};font-style:{font_style};}}</style>"
        ),
    ];
    if let Some(bg) = &cfg.background_color {
        out.push(format!(
            r#"<rect width="100%" height="100%" style="fill:{bg}"></rect>"#
        ));
    }
    for pw in &layout.words {
        let size = (f64::from(pw.font_size) * cfg.scale) as u32;
        let m = wc.rasterizer().svg_metrics(&pw.word, size)?;
        let (min_x, max_x, max_y) = (-m.offset_x, m.size_x - m.offset_x, m.ascent - m.offset_y);
        let mut x = pw.position.1 as f64 * cfg.scale;
        let mut y = pw.position.0 as f64 * cfg.scale;
        let transform = if pw.orientation == Orientation::Vertical {
            x += max_y;
            y += max_x - min_x;
            format!("translate({},{}) rotate(-90)", py_float(x), py_float(y))
        } else {
            x += min_x;
            y += max_y;
            format!("translate({},{})", py_float(x), py_float(y))
        };
        out.push(format!(
            r#"<text transform="{transform}" font-size="{}" style="fill:{}">{}</text>"#,
            py_float(f64::from(pw.font_size) * cfg.scale),
            pw.color,
            escape(&pw.word)
        ));
    }
    out.push("</svg>".into());
    Ok(out.join("\n"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn helpers() {
        assert_eq!(py_float(400.0), "400.0");
        assert_eq!(py_float(12.5), "12.5");
        assert_eq!(escape("a<b>&c"), "a&lt;b&gt;&amp;c");
    }
}
