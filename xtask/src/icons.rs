//! `crates/anyfont/assets/icons/contact-sheet.png`: every UI icon at 1×, 2× and 4× on the dark
//! background, next to a crop of the same spot in the reference screenshots (enlarged without
//! smoothing), plus the whole toolbar rebuilt from our icons under the reference toolbar.
//!
//! The reference crops only end up on this review sheet; nothing from the screenshots is shipped.

use crate::svg::{nested, options, png_data_uri, render, save_png, split_svg};
use crate::{reference, root, Result};
use image::RgbaImage;
use std::collections::HashMap;
use std::fmt::Write as _;

/// Background of the panels, toolbar and menus (Dark theme).
const BG: &str = "#262626";
/// The installed-font bar the lock icon sits on.
const GREEN_BAR: &str = "#66BF83";
/// The selected tab of the font properties panel.
const TAB_SELECTED: &str = "#1D4470";

/// The toolbar strip in a state where Remove, Uninstall and Delete are enabled.
const TOOLBAR_REF: &str = r"04-toolbar\states\user-collection-font-selected.png";
/// Where each toolbar icon's 16×16 grid starts in that strip, and whether it is disabled there.
const TOOLBAR: [(&str, u32, u32, bool); 10] = [
    ("add-folder", 9, 8, false),
    ("add-collection", 41, 9, false),
    ("get-more", 73, 9, false),
    ("remove", 114, 9, false),
    ("install", 155, 9, true),
    ("uninstall", 187, 9, false),
    ("download", 219, 8, true),
    ("delete", 260, 9, false),
    ("show-duplicates", 301, 8, false),
    ("font-properties", 341, 8, false),
];
const TOOLBAR_SEPARATORS: [u32; 5] = [101, 142, 247, 288, 329];

/// Icon order on the sheet, with the reference spot: (file, x, y) of the icon grid's top-left corner
/// (the crop has the size of the icon's viewBox), the background to draw on, and a note.
fn entries() -> Vec<(&'static str, Option<(&'static str, i32, i32)>, &'static str, &'static str)> {
    let main = r"02-main-window\Main window - restored size.png";
    let maxi = r"02-main-window\Main window - maximized (default state).png";
    let file = r"03-menus\deselect-all\File.png";
    let help = r"03-menus\deselect-all\Help.png";
    let view = r"03-menus\deselect-all\View.png";
    let title = r"08-title-bar\Title bar buttons.png";
    let filters = r"06-panels\filters\Filters - Font Technology.png";
    let glyphs = r"06-panels\font-properties\Tab Glyphs.png";
    let info = r"06-panels\font-properties\Tab Font info - variable font (Bahnschrift).png";
    let var = r"06-panels\font-properties\full-window\Variable font selected (Bahnschrift).png";
    let font_menu = r"03-menus\font-selected\Font.png";
    let mut v = vec![];
    for (name, x, y, _) in TOOLBAR {
        v.push((name, Some((TOOLBAR_REF, x as i32, y as i32)), BG, "toolbar"));
    }
    v.extend([
        ("import", Some((file, 8, 136)), BG, "File menu"),
        ("export", Some((file, 8, 158)), BG, "File menu (disabled in ref.)"),
        ("exit", Some((file, 8, 187)), BG, "File menu"),
        ("refresh", Some((view, 139, 34)), BG, "View menu (disabled in ref.)"),
        ("help", Some((help, 187, 35)), BG, "Help menu"),
        ("about", Some((help, 188, 64)), BG, "Help menu"),
        ("chevron-right", Some((font_menu, 282, 137)), BG, "submenu arrow (ref. #969696)"),
        ("title-help", Some((title, 16, 3)), BG, "title bar (ref. is dim)"),
        ("title-options", Some((title, 59, 3)), BG, "title bar (ref. is dim)"),
        ("title-updates", Some((title, 103, 2)), BG, "title bar (ref.: trial button)"),
        ("window-minimize", Some((title, 148, 5)), BG, "title bar"),
        ("window-maximize", Some((main, 718, 7)), BG, "title bar"),
        ("window-restore", Some((title, 187, 4)), BG, "title bar"),
        ("window-close", Some((title, 226, 4)), BG, "title bar"),
        ("section-expand", Some((filters, 219, 44)), BG, "Libraries / Filters"),
        ("section-collapse", Some((filters, 219, 90)), BG, "Libraries / Filters"),
        ("filter-clear", Some((filters, 218, 11)), BG, "Filters / Glyph Filters"),
        ("chevron-down", Some((main, 486, 100)), BG, "sample text dropdown"),
        ("chevron-up", None, BG, "spin box up / scroll up"),
        ("tech-truetype", Some((main, 730, 230)), BG, "font list row"),
        ("tech-opentype", Some((main, 730, 458)), BG, "font list row"),
        ("tech-variable", Some((var, 1582, 222)), BG, "font list row"),
        ("tech-type1", None, BG, "font list row (not in ref.)"),
        ("tech-other", None, BG, "font list row (not in ref.)"),
        ("lock", Some((main, 758, 234)), GREEN_BAR, "protected font"),
        ("preview-smaller", Some((main, 634, 571)), BG, "status bar"),
        ("preview-larger", Some((maxi, 1899, 1060)), BG, "status bar"),
        ("tab-glyphs", Some((info, 10, 4)), BG, "font properties tab"),
        ("tab-font-info", Some((glyphs, 42, 4)), BG, "font properties tab"),
        ("tab-variable", Some((glyphs, 74, 4)), BG, "font properties tab"),
        ("tab-glyphs", Some((glyphs, 10, 4)), TAB_SELECTED, "selected tab"),
    ]);
    v
}

struct Refs {
    dir: std::path::PathBuf,
    cache: HashMap<&'static str, Option<RgbaImage>>,
}

impl Refs {
    /// A crop of a reference screenshot, or `None` when the screenshots are not on this PC.
    fn crop(&mut self, file: &'static str, x: i32, y: i32, w: u32, h: u32) -> Option<RgbaImage> {
        let dir = self.dir.clone();
        let img = self
            .cache
            .entry(file)
            .or_insert_with(|| image::open(dir.join(file)).ok().map(|i| i.to_rgba8()))
            .as_ref()?;
        let mut out = RgbaImage::new(w, h);
        for (cx, cy, px) in out.enumerate_pixels_mut() {
            let (sx, sy) = (x + cx as i32, y + cy as i32);
            if sx >= 0 && sy >= 0 && (sx as u32) < img.width() && (sy as u32) < img.height() {
                *px = *img.get_pixel(sx as u32, sy as u32);
            }
        }
        Some(out)
    }
}

fn image_tag(img: &RgbaImage, x: f32, y: f32, scale: f32) -> Result<String> {
    Ok(format!(
        r#"<image x="{x}" y="{y}" width="{}" height="{}" image-rendering="optimizeSpeed" href="{}"/>"#,
        img.width() as f32 * scale,
        img.height() as f32 * scale,
        png_data_uri(img)?
    ))
}

fn text(out: &mut String, x: f32, y: f32, size: f32, fill: &str, s: &str) {
    let _ = write!(out, r#"<text x="{x}" y="{y}" font-family="Segoe UI" font-size="{size}" fill="{fill}">{s}</text>"#);
}

pub fn run() -> Result<()> {
    let root = root();
    let dir = root.join("crates/anyfont/assets/icons");
    let mut svgs: HashMap<String, String> = HashMap::new();
    let mut names = vec![];
    for e in std::fs::read_dir(&dir)? {
        let p = e?.path();
        if p.extension().is_some_and(|x| x == "svg") {
            let name = p.file_stem().unwrap().to_string_lossy().to_string();
            svgs.insert(name.clone(), std::fs::read_to_string(&p)?);
            names.push(name);
        }
    }
    names.sort();
    let mut refs = Refs { dir: reference(), cache: HashMap::new() };
    let have_refs = refs.dir.join(TOOLBAR_REF).exists();

    let width = 1320.0;
    let mut body = String::new();
    let mut y = 34.0;
    text(&mut body, 16.0, 24.0, 16.0, "#E8EAED", "AnyFont UI icons – contact sheet (ours vs. reference crop, enlarged without smoothing)");

    // The toolbar: reference strip over the strip rebuilt from our icons, at 1× and 3×.
    let mut ours = format!(r#"<rect width="420" height="34" fill="{BG}"/>"#);
    for x in TOOLBAR_SEPARATORS {
        let _ = write!(ours, r##"<rect x="{x}" y="6" width="1" height="22" fill="#525252"/>"##);
    }
    for (i, (name, x, ty, disabled)) in TOOLBAR.iter().enumerate() {
        let svg = svgs.get(*name).ok_or_else(|| format!("missing icon {name}.svg"))?;
        let n = nested(svg, *x as f32, *ty as f32, 16.0, 16.0, &format!("tb{i}_"))?;
        if *disabled {
            let _ = write!(ours, r#"<g filter="url(#disabled)" opacity="0.33">{n}</g>"#);
        } else {
            ours.push_str(&n);
        }
    }
    let ours_doc = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 420 34" width="420" height="34"><defs><filter id="disabled"><feColorMatrix type="saturate" values="0"/></filter></defs>{ours}</svg>"#
    );
    let ours_img = render(&ours_doc, 420, 34, &options())?;
    text(&mut body, 16.0, y + 14.0, 12.0, "#9AA0A6", "Toolbar – top: reference (Remove/Uninstall/Delete enabled, Install/Download disabled); bottom: AnyFont icons (Install/Download drawn with the disabled recipe)");
    y += 22.0;
    if have_refs {
        let r = refs.crop(TOOLBAR_REF, 0, 0, 420, 34).unwrap();
        body.push_str(&image_tag(&r, 16.0, y, 1.0)?);
        body.push_str(&image_tag(&r, 450.0, y, 2.0)?);
    }
    body.push_str(&image_tag(&ours_img, 16.0, y + 38.0, 1.0)?);
    body.push_str(&image_tag(&ours_img, 450.0, y + 72.0, 2.0)?);
    y += 150.0;

    // The grid: two columns of rows "name | ours 1× | ref 1× | ours 2× | ours 4× | ref 4×".
    let list = entries();
    // Icons not in the list above still get a row (at the end), so none is forgotten.
    let listed: Vec<&str> = list.iter().map(|e| e.0).collect();
    let extra: Vec<String> = names.iter().filter(|n| !listed.contains(&n.as_str())).cloned().collect();
    let mut rows: Vec<(String, Option<(&str, i32, i32)>, &str, String)> =
        list.iter().map(|(n, r, bg, note)| (n.to_string(), *r, *bg, note.to_string())).collect();
    for n in extra {
        rows.push((n, None, BG, String::new()));
    }
    let col_w = width / 2.0 - 8.0;
    let header_y = y;
    for c in 0..2 {
        let x0 = 16.0 + c as f32 * (col_w + 8.0);
        for (i, h) in ["icon", "1×", "ref", "2×", "4× (ours)", "4× (ref)"].iter().enumerate() {
            let hx = [0.0, 180.0, 212.0, 248.0, 304.0, 410.0][i];
            text(&mut body, x0 + hx, header_y, 11.0, "#9AA0A6", h);
        }
    }
    y += 8.0;
    let half = rows.len().div_ceil(2);
    let mut col_y = [y, y];
    for (i, (name, r, bg, note)) in rows.iter().enumerate() {
        let c = if i < half { 0 } else { 1 };
        let x0 = 16.0 + c as f32 * (col_w + 8.0);
        let ry = col_y[c];
        let Some(svg) = svgs.get(name) else {
            text(&mut body, x0, ry + 14.0, 12.0, "#FF6250", &format!("{name}: missing"));
            col_y[c] += 24.0;
            continue;
        };
        let (vb, _) = split_svg(svg)?;
        let (w, h) = (vb[2], vb[3]);
        let row_h = h * 4.0 + 8.0;
        let _ = write!(body, r##"<rect x="{x0}" y="{ry}" width="{col_w}" height="{}" fill="#1B1B1B"/>"##, row_h);
        text(&mut body, x0 + 6.0, ry + 18.0, 12.0, "#E8EAED", name);
        text(&mut body, x0 + 6.0, ry + 34.0, 10.0, "#7F868E", note);
        let cy = ry + 4.0;
        let cell = |body: &mut String, x: f32, s: f32| {
            let _ = write!(body, r#"<rect x="{}" y="{}" width="{}" height="{}" fill="{bg}"/>"#, x - 2.0, cy - 2.0, w * s + 4.0, h * s + 4.0);
        };
        cell(&mut body, x0 + 180.0, 1.0);
        body.push_str(&nested(svg, x0 + 180.0, cy, w, h, &format!("a{i}_"))?);
        cell(&mut body, x0 + 248.0, 2.0);
        body.push_str(&nested(svg, x0 + 248.0, cy, w * 2.0, h * 2.0, &format!("b{i}_"))?);
        cell(&mut body, x0 + 304.0, 4.0);
        body.push_str(&nested(svg, x0 + 304.0, cy, w * 4.0, h * 4.0, &format!("c{i}_"))?);
        if let Some((file, rx, ry2)) = r {
            if let Some(crop) = refs.crop(file, *rx, *ry2, w as u32, h as u32) {
                cell(&mut body, x0 + 212.0, 1.0);
                body.push_str(&image_tag(&crop, x0 + 212.0, cy, 1.0)?);
                cell(&mut body, x0 + 410.0, 4.0);
                body.push_str(&image_tag(&crop, x0 + 410.0, cy, 4.0)?);
            }
        }
        col_y[c] += row_h + 6.0;
    }
    let height = col_y[0].max(col_y[1]) + 16.0;
    let doc = format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {width} {height}" width="{width}" height="{height}"><rect width="{width}" height="{height}" fill="#121212"/>{body}</svg>"##
    );
    let img = render(&doc, width as u32, height.ceil() as u32, &options())?;
    println!("contact sheet{}:", if have_refs { "" } else { " (reference screenshots not found)" });
    save_png(&img, &dir.join("contact-sheet.png"))?;
    Ok(())
}
