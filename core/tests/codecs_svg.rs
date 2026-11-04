//! SVG rasterizer tests. Inputs are built in the test; no binary samples.

use photosee_core::decode::svg;
use photosee_core::{ErrorCode, Orientation, Rgba8, Size};
use resvg::usvg;

const RED_BLUE: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" width="20" height="10">
  <rect width="20" height="10" fill="#ff0000"/>
  <rect x="10" width="10" height="10" fill="#0000ff"/>
</svg>"##;

fn decode(s: &str, max: Option<Size>) -> Rgba8 {
    svg::decode(&s.as_bytes().to_vec(), max).expect("svg decodes")
}

fn px(img: &Rgba8, x: u32, y: u32) -> [u8; 4] {
    let i = ((y * img.size.w + x) * 4) as usize;
    img.data[i..i + 4].try_into().expect("4 bytes")
}

#[test]
fn info_reports_the_rounded_up_tree_size() {
    let i = svg::info(&RED_BLUE.as_bytes().to_vec()).unwrap();
    assert_eq!(
        (i.size, i.orientation, i.animated),
        (Size::new(20, 10), Orientation::Normal, false)
    );
    let frac = r#"<svg xmlns="http://www.w3.org/2000/svg" width="10.2" height="3.5"/>"#;
    assert_eq!(
        svg::info(&frac.as_bytes().to_vec()).unwrap().size,
        Size::new(11, 4)
    );
    let tiny = r#"<svg xmlns="http://www.w3.org/2000/svg" width="0.5" height="0.5"/>"#;
    assert_eq!(
        svg::info(&tiny.as_bytes().to_vec()).unwrap().size,
        Size::new(1, 1)
    );
}

#[test]
fn native_decode_has_exact_pixels_and_transparent_background() {
    let img = decode(RED_BLUE, None);
    assert_eq!(img.size, Size::new(20, 10));
    assert_eq!(px(&img, 2, 2), [255, 0, 0, 255]);
    assert_eq!(px(&img, 15, 8), [0, 0, 255, 255]);
    let empty = r#"<svg xmlns="http://www.w3.org/2000/svg" width="4" height="4"/>"#;
    assert_eq!(px(&decode(empty, None), 1, 1), [0, 0, 0, 0]);
}

#[test]
fn alpha_is_straight_not_premultiplied() {
    let s = r##"<svg xmlns="http://www.w3.org/2000/svg" width="4" height="4">
      <rect width="4" height="4" fill="#ff0000" fill-opacity="0.5"/></svg>"##;
    let p = px(&decode(s, None), 1, 1);
    assert!((127..=128).contains(&p[3]), "{p:?}");
    assert!(
        p[0] >= 254 && p[1] == 0 && p[2] == 0,
        "premultiplied red would be ~128: {p:?}"
    );
}

#[test]
fn max_scales_to_fit_preserving_aspect() {
    let down = decode(RED_BLUE, Some(Size::new(10, 10)));
    assert_eq!(down.size, Size::new(10, 5));
    assert_eq!(px(&down, 1, 1), [255, 0, 0, 255]);
    assert_eq!(px(&down, 8, 3), [0, 0, 255, 255]);
    // A vector has no native resolution, so a larger bound is rendered sharp.
    let up = decode(RED_BLUE, Some(Size::new(200, 200)));
    assert_eq!(up.size, Size::new(200, 100));
    assert_eq!(px(&up, 99, 50), [255, 0, 0, 255]);
    assert_eq!(px(&up, 101, 50), [0, 0, 255, 255]);
}

#[test]
fn native_size_is_capped_at_8192_on_the_long_edge() {
    let big = r##"<svg xmlns="http://www.w3.org/2000/svg" width="20000" height="10000">
      <rect width="20000" height="10000" fill="#00ff00"/></svg>"##;
    let info = svg::info(&big.as_bytes().to_vec()).unwrap();
    assert_eq!(info.size, Size::new(20_000, 10_000));
    let img = decode(big, None);
    assert_eq!(img.size, Size::new(8192, 4096));
    assert_eq!(px(&img, 4000, 2000), [0, 255, 0, 255]);
}

fn red_png_file(dir: &std::path::Path) -> std::path::PathBuf {
    let path = dir.join("secret.png");
    image::save_buffer(
        &path,
        &[255, 0, 0, 255].repeat(16),
        4,
        4,
        image::ColorType::Rgba8,
    )
    .expect("write test png");
    path
}

fn image_svg(href: &str) -> String {
    format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" width="8" height="8"><image xlink:href="{href}" width="8" height="8"/></svg>"#
    )
}

#[test]
fn local_files_and_urls_in_image_href_are_never_loaded() {
    let dir = tempfile::tempdir().unwrap();
    let png = red_png_file(dir.path());
    let abs = png.to_string_lossy().replace('\\', "/");

    // Control: usvg's default resolver DOES read that file, so the test below
    // proves the restriction rather than a path that never resolved.
    let control = image_svg(&abs);
    let tree = usvg::Tree::from_str(&control, &usvg::Options::default()).unwrap();
    let mut pm = resvg::tiny_skia::Pixmap::new(8, 8).unwrap();
    resvg::render(
        &tree,
        resvg::tiny_skia::Transform::identity(),
        &mut pm.as_mut(),
    );
    assert_eq!(
        pm.pixels()[27].alpha(),
        255,
        "control: default usvg should load the PNG"
    );

    let hrefs = [
        abs.clone(),
        format!("file:///{abs}"),
        "file:///C:/Windows/win.ini".to_string(),
        "secret.png".to_string(),
        "http://127.0.0.1:9/x.png".to_string(),
        "https://example.invalid/x.png".to_string(),
    ];
    for href in hrefs {
        let img = decode(&image_svg(&href), None);
        assert_eq!(px(&img, 4, 4), [0, 0, 0, 0], "href {href} was loaded");
    }
}

fn base64(data: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    for c in data.chunks(3) {
        let n = c
            .iter()
            .enumerate()
            .fold(0u32, |a, (i, &b)| a | u32::from(b) << (16 - 8 * i));
        for k in 0..4 {
            out.push(if k <= c.len() {
                T[(n >> (18 - 6 * k) & 63) as usize] as char
            } else {
                '='
            });
        }
    }
    out
}

#[test]
fn data_url_images_still_render() {
    let dir = tempfile::tempdir().unwrap();
    let png = std::fs::read(red_png_file(dir.path())).unwrap();
    let s = image_svg(&format!("data:image/png;base64,{}", base64(&png)));
    assert_eq!(px(&decode(&s, None), 4, 4), [255, 0, 0, 255]);
}

#[test]
fn oversized_file_is_too_large() {
    let mut big = RED_BLUE.as_bytes().to_vec();
    big.extend(b"<!--");
    big.resize(17 * 1024 * 1024, b' ');
    big.extend(b"-->");
    assert_eq!(svg::info(&big).unwrap_err().code, ErrorCode::TooLarge);
    assert_eq!(
        svg::decode(&big, None).unwrap_err().code,
        ErrorCode::TooLarge
    );
    // Right at the limit it is accepted.
    let mut ok = RED_BLUE.as_bytes().to_vec();
    ok.extend(b"<!--");
    ok.resize(16 * 1024 * 1024 - 3, b' ');
    ok.extend(b"-->");
    assert_eq!(svg::info(&ok).unwrap().size, Size::new(20, 10));
}

#[test]
fn bad_input_is_a_typed_error_never_a_panic() {
    let bad: [&[u8]; 5] = [
        b"",
        b"<svg",
        b"not xml",
        &[0xff, 0xfe, 0x00, 0x80],
        &[0x1f, 0x8b, 8, 0],
    ];
    for b in bad {
        let e = svg::decode(&b.to_vec(), None).expect_err("must fail");
        assert!(
            matches!(e.code, ErrorCode::Corrupt | ErrorCode::Unsupported),
            "{b:?}: {e}"
        );
        assert!(svg::info(&b.to_vec()).is_err());
    }
    for len in 0..RED_BLUE.len() {
        let cut = RED_BLUE.as_bytes()[..len].to_vec();
        let _ = svg::decode(&cut, None);
    }
    assert_eq!(
        svg::decode(&[0x1f, 0x8b, 8, 0].to_vec(), None)
            .unwrap_err()
            .code,
        ErrorCode::Unsupported
    );
}

#[test]
fn text_is_skipped_without_failing() {
    let s = r##"<svg xmlns="http://www.w3.org/2000/svg" width="40" height="20">
      <rect width="40" height="20" fill="#0f0"/><text x="2" y="15">hello</text></svg>"##;
    assert_eq!(px(&decode(s, None), 1, 1), [0, 255, 0, 255]);
}
