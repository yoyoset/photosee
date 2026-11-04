//! SVG rasterization via resvg, for thumbnails and transcodes only (display
//! is passthrough). Package D.
//!
//! Known gaps, all deliberate:
//! - No `text` feature: `<text>` renders as nothing. It would pull in a font
//!   stack and system-font scanning for little value in a photo viewer.
//! - No `svgz` feature: gzip-compressed SVG is `Unsupported`.
//!
//! Security: an SVG is attacker-controlled. `resources_dir` stays unset and
//! the `<image>` string resolver is replaced by one that refuses everything,
//! so `href="file:///..."`, relative paths and `http://` are never opened (usvg
//! never does network IO). Only `data:` URLs reach the default data resolver,
//! which just sniffs the bytes. The file is capped at 16 MiB, and usvg itself
//! refuses documents with more than a million elements.
//!
//! Scaling: a vector image has no native resolution, so with `max` the result
//! is scaled (up or down) to the largest size that fits inside `max`. With
//! `None` it is the tree's native size, reduced if the long edge exceeds
//! `NATIVE_LONG_EDGE_CAP`. resvg renders premultiplied alpha; `Rgba8` is
//! straight alpha, so every pixel is demultiplied.

use std::panic::{AssertUnwindSafe, catch_unwind};

use resvg::tiny_skia::{Pixmap, Transform};
use resvg::usvg::{self, ImageHrefResolver};

use crate::error::{Error, ErrorCode, Result};
use crate::io::{ReadAt, read_all};
use crate::types::{Format, ImageInfo, Orientation, Rgba8, Size};

pub const MAX_FILE_BYTES: u64 = 16 * 1024 * 1024;
const NATIVE_LONG_EDGE_CAP: u32 = 8192;

fn read_tree(src: &dyn ReadAt) -> Result<usvg::Tree> {
    if src.len()? > MAX_FILE_BYTES {
        return Err(Error::new(
            ErrorCode::TooLarge,
            "svg: file larger than 16 MiB",
        ));
    }
    let data = read_all(src)?;
    let opt = usvg::Options {
        resources_dir: None,
        image_href_resolver: ImageHrefResolver {
            resolve_data: ImageHrefResolver::default_data_resolver(),
            resolve_string: Box::new(|_href, _opt| None),
        },
        ..usvg::Options::default()
    };
    // A parser bug on hostile input must not take the server down.
    catch_unwind(AssertUnwindSafe(|| usvg::Tree::from_data(&data, &opt)))
        .map_err(|_| Error::corrupt("svg: parser panicked"))?
        .map_err(|e| match e {
            usvg::Error::ElementsLimitReached => {
                Error::new(ErrorCode::TooLarge, format!("svg: {e}"))
            }
            usvg::Error::SvgzFeatureNotEnabled => Error::unsupported("svg: compressed SVG (svgz)"),
            _ => Error::corrupt(format!("svg: {e}")),
        })
}

/// Tree size rounded up, at least 1x1 (a 0.5 x 0.5 SVG is still an image).
fn native_size(tree: &usvg::Tree) -> Size {
    let up = |v: f32| (v.ceil().max(1.0)).min(u32::MAX as f32) as u32;
    let s = tree.size();
    Size::new(up(s.width()), up(s.height()))
}

pub fn info(src: &dyn ReadAt) -> Result<ImageInfo> {
    let tree = read_tree(src)?;
    Ok(ImageInfo {
        format: Format::Svg,
        size: native_size(&tree),
        orientation: Orientation::Normal,
        animated: false,
    })
}

/// Output size for a native size and an optional bound; aspect preserved.
fn target_size(native: Size, max: Option<Size>) -> Size {
    let (w, h) = (f64::from(native.w), f64::from(native.h));
    let bound = match max {
        Some(m) => (f64::from(m.w.max(1)), f64::from(m.h.max(1))),
        None => (w, h),
    };
    let cap = f64::from(NATIVE_LONG_EDGE_CAP);
    let scale = (bound.0 / w).min(bound.1 / h).min(cap / w).min(cap / h);
    let fit = |v: f64, limit: f64| ((v * scale).round().max(1.0)).min(limit.min(cap)) as u32;
    Size::new(fit(w, bound.0), fit(h, bound.1))
}

pub fn decode(src: &dyn ReadAt, max: Option<Size>) -> Result<Rgba8> {
    let tree = read_tree(src)?;
    let native = native_size(&tree);
    let size = target_size(native, max);
    let mut pixmap = Pixmap::new(size.w, size.h)
        .ok_or_else(|| Error::new(ErrorCode::TooLarge, "svg: cannot allocate the pixmap"))?;
    // Scale from the exact tree size, not the rounded one, so the artwork
    // fills the pixmap edge to edge.
    let ts = tree.size();
    let transform = Transform::from_scale(size.w as f32 / ts.width(), size.h as f32 / ts.height());
    catch_unwind(AssertUnwindSafe(|| {
        resvg::render(&tree, transform, &mut pixmap.as_mut());
    }))
    .map_err(|_| Error::corrupt("svg: renderer panicked"))?;
    let mut data = Vec::with_capacity(pixmap.data().len());
    for px in pixmap.pixels() {
        let c = px.demultiply();
        data.extend_from_slice(&[c.red(), c.green(), c.blue(), c.alpha()]);
    }
    Ok(Rgba8 { size, data })
}
