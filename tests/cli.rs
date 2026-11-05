//! End-to-end tests of the `wordcloud` binary.

use std::process::Command;

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_wordcloud"))
}

fn golden(f: &str) -> String {
    format!("{}/tests/golden/{f}", env!("CARGO_MANIFEST_DIR"))
}

fn tmp(name: &str) -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!("wordcloud-rs-test-{}", std::process::id()));
    std::fs::create_dir_all(&d).unwrap();
    d.join(name)
}

#[test]
fn writes_png_svg_and_layout() {
    let (png, svg, json) = (tmp("a.png"), tmp("a.svg"), tmp("a.json"));
    let st = bin()
        .args([
            "--text",
            &golden("alice.txt"),
            "--random_state",
            "3",
            "--colormap",
            "plasma",
        ])
        .arg("--imagefile")
        .arg(&png)
        .arg("--svgfile")
        .arg(&svg)
        .arg("--dump-layout")
        .arg(&json)
        .status()
        .unwrap();
    assert!(st.success());
    let img = image::open(&png).unwrap();
    assert_eq!((img.width(), img.height()), (400, 200));
    assert!(std::fs::read_to_string(&svg).unwrap().contains("<text"));
    assert!(
        std::fs::read_to_string(&json)
            .unwrap()
            .contains("\"word\":\"said\"")
    );
}

#[test]
fn mask_contour_rgba_scale_and_options() {
    let png = tmp("m.png");
    let st = bin()
        .args([
            "--text",
            &golden("constitution.txt"),
            "--mask",
            &golden("alice_mask.png"),
            "--contour_width",
            "2",
            "--contour_color",
            "red",
            "--mode",
            "RGBA",
            "--background",
            "None",
            "--scale",
            "0.5",
            "--repeat",
            "--relative_scaling",
            "0.5",
            "--color",
            "teal",
            "--no_collocations",
            "--no_normalize_plurals",
            "--include_numbers",
            "--min_word_length",
            "2",
            "--max_font_size",
            "60",
            "--fix-legacy-bugs",
            "--random_state",
            "1",
        ])
        .arg("--imagefile")
        .arg(&png)
        .status()
        .unwrap();
    assert!(st.success());
    let img = image::open(&png).unwrap();
    assert!(img.color().has_alpha());
}

#[test]
fn stdin_stdout_and_stopwords() {
    use std::io::Write;
    let stop = tmp("stop.txt");
    std::fs::write(&stop, "beta\n").unwrap();
    let mut child = bin()
        .arg("--stopwords")
        .arg(&stop)
        .args(["--random_state", "0"])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(b"alpha alpha beta gamma")
        .unwrap();
    let out = child.wait_with_output().unwrap();
    assert!(out.status.success());
    assert_eq!(&out.stdout[1..4], b"PNG");
}

#[test]
fn reports_errors() {
    let st = bin()
        .args(["--text", "/nonexistent/file"])
        .output()
        .unwrap();
    assert!(!st.status.success());
    let st = bin()
        .args([
            "--text",
            &golden("alice.txt"),
            "--color",
            "red",
            "--colormask",
            &golden("alice_mask.png"),
        ])
        .arg("--imagefile")
        .arg(tmp("x.png"))
        .output()
        .unwrap();
    assert!(String::from_utf8_lossy(&st.stderr).contains("either a color mask"));
    let st = bin()
        .args([
            "--text",
            &golden("unicode_text.txt"),
            "--relative_scaling",
            "3",
        ])
        .output()
        .unwrap();
    assert!(!st.status.success());
}

#[test]
fn colormask_and_fontfile() {
    let png = tmp("c.png");
    let font = format!("{}/assets/DroidSansMono.ttf", env!("CARGO_MANIFEST_DIR"));
    let st = bin()
        .args([
            "--text",
            &golden("alice.txt"),
            "--mask",
            &golden("alice_mask.png"),
            "--colormask",
            &golden("alice_mask.png"),
            "--fontfile",
            &font,
            "--max_words",
            "40",
            "--random_state",
            "2",
        ])
        .arg("--imagefile")
        .arg(&png)
        .status()
        .unwrap();
    assert!(st.success());
}
