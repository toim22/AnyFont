//! Small helpers around resvg shared by the brand and icon commands.

use crate::{root, Result};
use image::RgbaImage;
use resvg::{tiny_skia, usvg};
use std::path::Path;

/// usvg options with the system fonts loaded (the brand SVGs use Segoe UI for their text).
pub fn options() -> usvg::Options<'static> {
    let mut opt = usvg::Options::default();
    opt.fontdb_mut().load_system_fonts();
    opt
}

/// Renders an SVG to straight-alpha RGBA at exactly `w`×`h` pixels.
pub fn render(svg: &str, w: u32, h: u32, opt: &usvg::Options) -> Result<RgbaImage> {
    let tree = usvg::Tree::from_str(svg, opt)?;
    let size = tree.size();
    let mut pix = tiny_skia::Pixmap::new(w, h).ok_or("empty picture")?;
    let ts = tiny_skia::Transform::from_scale(w as f32 / size.width(), h as f32 / size.height());
    resvg::render(&tree, ts, &mut pix.as_mut());
    let mut img = RgbaImage::new(w, h);
    for (px, out) in pix.pixels().iter().zip(img.pixels_mut()) {
        let c = px.demultiply();
        *out = image::Rgba([c.red(), c.green(), c.blue(), c.alpha()]);
    }
    Ok(img)
}

/// The `viewBox` numbers of an SVG document and its markup without the outer `<svg>` element.
pub fn split_svg(svg: &str) -> Result<([f32; 4], String)> {
    let start = svg.find("<svg").ok_or("no <svg> element")?;
    let open_end = start + svg[start..].find('>').ok_or("unterminated <svg>")?;
    let open = &svg[start..=open_end];
    let vb = open.split("viewBox=\"").nth(1).and_then(|s| s.split('"').next()).ok_or("no viewBox")?;
    let v: Vec<f32> = vb.split([' ', ',']).filter_map(|n| n.trim().parse().ok()).collect();
    if v.len() != 4 {
        return Err(format!("bad viewBox {vb:?}").into());
    }
    let close = svg.rfind("</svg>").ok_or("no </svg>")?;
    Ok(([v[0], v[1], v[2], v[3]], svg[open_end + 1..close].to_string()))
}

/// Makes every `id="…"` (and the references to it) unique by prefixing it, so several documents
/// can be nested in one SVG without their gradients, masks and filters clashing.
pub fn prefix_ids(markup: &str, prefix: &str) -> String {
    let mut ids = vec![];
    let mut rest = markup;
    while let Some(i) = rest.find(" id=\"") {
        let s = &rest[i + 5..];
        let end = s.find('"').unwrap_or(0);
        ids.push(s[..end].to_string());
        rest = &s[end..];
    }
    let mut out = markup.to_string();
    for id in ids {
        out = out
            .replace(&format!("id=\"{id}\""), &format!("id=\"{prefix}{id}\""))
            .replace(&format!("url(#{id})"), &format!("url(#{prefix}{id})"))
            .replace(&format!("href=\"#{id}\""), &format!("href=\"#{prefix}{id}\""));
    }
    out
}

/// Another SVG document placed at (`x`, `y`) with size `w`×`h`, as a nested `<svg>`.
pub fn nested(svg: &str, x: f32, y: f32, w: f32, h: f32, id_prefix: &str) -> Result<String> {
    let (v, inner) = split_svg(svg)?;
    let inner = prefix_ids(&inner, id_prefix);
    Ok(format!(
        r#"<svg x="{x}" y="{y}" width="{w}" height="{h}" viewBox="{} {} {} {}" overflow="visible">{inner}</svg>"#,
        v[0], v[1], v[2], v[3]
    ))
}

/// Standard base64 (used for `data:` URIs of embedded pictures).
pub fn base64(data: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for c in data.chunks(3) {
        let n = (c[0] as u32) << 16 | (*c.get(1).unwrap_or(&0) as u32) << 8 | *c.get(2).unwrap_or(&0) as u32;
        out.push(T[(n >> 18) as usize & 63] as char);
        out.push(T[(n >> 12) as usize & 63] as char);
        out.push(if c.len() > 1 { T[(n >> 6) as usize & 63] as char } else { '=' });
        out.push(if c.len() > 2 { T[n as usize & 63] as char } else { '=' });
    }
    out
}

/// A picture as a PNG `data:` URI.
pub fn png_data_uri(img: &image::RgbaImage) -> Result<String> {
    let mut buf = std::io::Cursor::new(Vec::new());
    img.write_to(&mut buf, image::ImageFormat::Png)?;
    Ok(format!("data:image/png;base64,{}", base64(buf.get_ref())))
}

pub fn save_png(img: &RgbaImage, path: &Path) -> Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    img.save(path)?;
    println!("  {}", path.strip_prefix(root()).unwrap_or(path).display());
    Ok(())
}
