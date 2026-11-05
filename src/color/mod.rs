//! Colour parsing and conversion (ports of Pillow `ImageColor.getrgb` and
//! CPython `colorsys`).

pub mod colormaps;
mod funcs;
pub mod names;

pub use funcs::{ColorFunc, ColorRequest};

use crate::error::{Result, WordCloudError};
use regex::Regex;
use std::sync::LazyLock;

/// An 8-bit RGBA colour.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rgba(pub [u8; 4]);

static RGB_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^rgba?\(\s*(\d+)\s*,\s*(\d+)\s*,\s*(\d+)\s*(?:,\s*(\d+)\s*)?\)$").expect("re")
});
static RGB_PCT_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^rgb\(\s*(\d+(?:\.\d+)?)%\s*,\s*(\d+(?:\.\d+)?)%\s*,\s*(\d+(?:\.\d+)?)%\s*\)$")
        .expect("re")
});
static HSL_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^hs([lvb])\(\s*(\d+\.?\d*)\s*,\s*(\d+\.?\d*)%\s*,\s*(\d+\.?\d*)%\s*\)$")
        .expect("re")
});

/// Parses a Pillow colour string: names, `#rgb[a]`, `#rrggbb[aa]`, `rgb()`,
/// `rgba()`, `rgb(%)`, `hsl()`, `hsv()`/`hsb()`.
///
/// # Errors
/// [`WordCloudError::InvalidColor`] for unknown specifiers.
///
/// ```
/// use wordcloud::color::{parse_color, Rgba};
/// assert_eq!(parse_color("Red")?, Rgba([255, 0, 0, 255]));
/// assert_eq!(parse_color("#0f08")?, Rgba([0, 255, 0, 136]));
/// assert_eq!(parse_color("hsl(0, 100%, 50%)")?, Rgba([255, 0, 0, 255]));
/// assert_eq!(parse_color("rgb(1, 2, 3)")?, Rgba([1, 2, 3, 255]));
/// # Ok::<(), wordcloud::WordCloudError>(())
/// ```
pub fn parse_color(spec: &str) -> Result<Rgba> {
    let err = || WordCloudError::InvalidColor(spec.to_string());
    let lower = spec.to_lowercase();
    if let Ok(i) = names::NAMED_COLORS.binary_search_by(|(n, _)| n.cmp(&lower.as_str())) {
        return parse_color(names::NAMED_COLORS[i].1);
    }
    if let Some(hex) = lower.strip_prefix('#') {
        let digits: Option<Vec<u8>> = hex
            .chars()
            .map(|c| c.to_digit(16).map(|d| d as u8))
            .collect();
        let d = digits.ok_or_else(err)?;
        return match d.len() {
            3 | 4 => {
                let a = d.get(3).map_or(255, |v| v * 17);
                Ok(Rgba([d[0] * 17, d[1] * 17, d[2] * 17, a]))
            }
            6 | 8 => {
                let byte = |i: usize| d[i] * 16 + d[i + 1];
                let a = if d.len() == 8 { byte(6) } else { 255 };
                Ok(Rgba([byte(0), byte(2), byte(4), a]))
            }
            _ => Err(err()),
        };
    }
    if let Some(c) = RGB_RE.captures(&lower) {
        let v = |i: usize| {
            c.get(i)
                .map(|m| m.as_str().parse::<u32>().map_or(255, |x| x.min(255) as u8))
        };
        return Ok(Rgba([
            v(1).unwrap_or(0),
            v(2).unwrap_or(0),
            v(3).unwrap_or(0),
            v(4).unwrap_or(255),
        ]));
    }
    if let Some(c) = RGB_PCT_RE.captures(&lower) {
        let v = |i: usize| {
            let p: f64 = c[i].parse().unwrap_or(0.0);
            ((p * 255.0) / 100.0 + 0.5).min(255.0) as u8
        };
        return Ok(Rgba([v(1), v(2), v(3), 255]));
    }
    if let Some(c) = HSL_RE.captures(&lower) {
        let f = |i: usize| c[i].parse::<f64>().unwrap_or(0.0);
        let (h, s, x) = (f(2) / 360.0, f(3) / 100.0, f(4) / 100.0);
        let (r, g, b) = if &c[1] == "l" {
            hls_to_rgb(h, x, s)
        } else {
            hsv_to_rgb(h, s, x)
        };
        let q = |v: f64| (v * 255.0 + 0.5) as u8;
        return Ok(Rgba([q(r), q(g), q(b), 255]));
    }
    Err(err())
}

/// Python float modulo (`a % b` with the sign of `b`).
fn py_mod(a: f64, b: f64) -> f64 {
    let m = a % b;
    if m != 0.0 && (m < 0.0) != (b < 0.0) {
        m + b
    } else {
        m
    }
}

/// CPython `colorsys.hls_to_rgb`.
pub fn hls_to_rgb(h: f64, l: f64, s: f64) -> (f64, f64, f64) {
    if s == 0.0 {
        return (l, l, l);
    }
    let m2 = if l <= 0.5 {
        l * (1.0 + s)
    } else {
        l + s - (l * s)
    };
    let m1 = 2.0 * l - m2;
    let v = |hue: f64| {
        let hue = py_mod(hue, 1.0);
        if hue < 1.0 / 6.0 {
            m1 + (m2 - m1) * hue * 6.0
        } else if hue < 0.5 {
            m2
        } else if hue < 2.0 / 3.0 {
            m1 + (m2 - m1) * (2.0 / 3.0 - hue) * 6.0
        } else {
            m1
        }
    };
    (v(h + 1.0 / 3.0), v(h), v(h - 1.0 / 3.0))
}

/// CPython `colorsys.rgb_to_hsv`.
///
/// ```
/// use wordcloud::color::rgb_to_hsv;
/// assert_eq!(rgb_to_hsv(1.0, 0.0, 0.0), (0.0, 1.0, 1.0));
/// ```
pub fn rgb_to_hsv(r: f64, g: f64, b: f64) -> (f64, f64, f64) {
    let maxc = r.max(g).max(b);
    let minc = r.min(g).min(b);
    let rangec = maxc - minc;
    if minc == maxc {
        return (0.0, 0.0, maxc);
    }
    let s = rangec / maxc;
    let (rc, gc, bc) = (
        (maxc - r) / rangec,
        (maxc - g) / rangec,
        (maxc - b) / rangec,
    );
    let h = if r == maxc {
        bc - gc
    } else if g == maxc {
        2.0 + rc - bc
    } else {
        4.0 + gc - rc
    };
    (py_mod(h / 6.0, 1.0), s, maxc)
}

/// CPython `colorsys.hsv_to_rgb`.
pub fn hsv_to_rgb(h: f64, s: f64, v: f64) -> (f64, f64, f64) {
    if s == 0.0 {
        return (v, v, v);
    }
    let i = (h * 6.0) as i64;
    let f = h * 6.0 - i as f64;
    let (p, q, t) = (v * (1.0 - s), v * (1.0 - s * f), v * (1.0 - s * (1.0 - f)));
    match i.rem_euclid(6) {
        0 => (v, t, p),
        1 => (q, v, p),
        2 => (p, v, t),
        3 => (p, q, v),
        4 => (t, p, v),
        _ => (v, p, q),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_garbage() {
        for bad in ["", "#12", "#gg0000", "notacolor", "rgb(1,2)"] {
            assert!(parse_color(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn hsv_roundtrip_all_sectors() {
        for k in 0..12 {
            let h = k as f64 / 12.0;
            let (r, g, b) = hsv_to_rgb(h, 0.7, 0.9);
            let (h2, s2, v2) = rgb_to_hsv(r, g, b);
            assert!((h - h2).abs() < 1e-9 && (s2 - 0.7).abs() < 1e-9 && (v2 - 0.9).abs() < 1e-9);
        }
        assert_eq!(hsv_to_rgb(0.3, 0.0, 0.5), (0.5, 0.5, 0.5));
        assert_eq!(hls_to_rgb(0.3, 0.4, 0.0), (0.4, 0.4, 0.4));
        assert_eq!(rgb_to_hsv(0.2, 0.2, 0.2), (0.0, 0.0, 0.2));
    }

    #[test]
    fn other_formats() {
        assert_eq!(
            parse_color("rgb(100%, 0%, 50%)").unwrap(),
            Rgba([255, 0, 128, 255])
        );
        assert_eq!(
            parse_color("hsv(120, 100%, 100%)").unwrap(),
            Rgba([0, 255, 0, 255])
        );
        assert_eq!(
            parse_color("hsl(240, 100%, 75%)").unwrap(),
            Rgba([128, 128, 255, 255])
        );
        assert_eq!(parse_color("rgba(1, 2, 3, 4)").unwrap(), Rgba([1, 2, 3, 4]));
        assert_eq!(
            parse_color("#11223344").unwrap(),
            Rgba([0x11, 0x22, 0x33, 0x44])
        );
        assert_eq!(py_mod(-0.25, 1.0), 0.75);
    }
}
