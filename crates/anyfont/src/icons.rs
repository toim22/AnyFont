//! The UI icons: `assets/icons/*.svg`, rasterised with resvg at the current pixels-per-point into egui
//! textures (cached per name, size and colours). States follow `assets/icons/README.md`: the light glyph
//! colour `#CACED4` is replaced by the state's colour (disabled `#5D5D5D`, title-bar `#5D5D5D`, window buttons
//! `#969696`, Light theme dark glyphs), and a disabled icon's badge becomes `#434343`.

use crate::theme;
use egui::{Color32, Context, Rect, TextureHandle, Ui};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

macro_rules! icon_table {
    ($($name:literal),* $(,)?) => {
        /// The SVG source of an icon.
        pub fn svg(name: &str) -> Option<&'static str> {
            match name {
                $($name => Some(include_str!(concat!("../assets/icons/", $name, ".svg"))),)*
                _ => None,
            }
        }
        /// Every icon name.
        pub const NAMES: &[&str] = &[$($name),*];
    };
}

icon_table!(
    "about", "add-collection", "add-folder", "chevron-down", "chevron-right", "chevron-up", "delete", "download",
    "exit", "export", "filter-clear", "font-properties", "get-more", "help", "import", "install", "lock",
    "preview-larger", "preview-smaller", "refresh", "remove", "section-collapse", "section-expand",
    "show-duplicates", "tab-font-info", "tab-glyphs", "tab-variable", "tech-opentype", "tech-other",
    "tech-truetype", "tech-type1", "tech-variable", "title-help", "title-options", "title-updates", "uninstall",
    "window-close", "window-maximize", "window-minimize", "window-restore",
);

/// How an icon is drawn.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Look {
    /// Enabled: the theme's icon colour, badges in colour.
    Normal,
    /// Disabled: `icon_disabled`, badges `icon_badge_disabled`.
    Disabled,
    /// Title-bar ?, gear, Updates at rest (`icon_title_bar`).
    TitleBar,
    /// Window buttons (`icon_window_buttons`).
    Window,
    /// One colour for the glyph (e.g. white on the red close hover).
    Color(Color32),
    /// The clear-filters funnel while filters are active: the cross in `badge_remove`.
    FilterActive,
}

#[derive(Clone, PartialEq, Eq, Hash)]
struct Key {
    name: &'static str,
    w: u32,
    h: u32,
    fg: [u8; 4],
    badge: Option<[u8; 4]>,
    chevron: [u8; 4],
    cross: Option<[u8; 4]>,
}

#[derive(Default)]
struct Cache {
    map: HashMap<Key, TextureHandle>,
}

fn cache(ctx: &Context) -> Arc<Mutex<Cache>> {
    ctx.data_mut(|d| d.get_temp_mut_or_default::<Arc<Mutex<Cache>>>(egui::Id::unique("anyfont-icon-cache")).clone())
}

fn static_name(name: &str) -> Option<&'static str> {
    NAMES.iter().copied().find(|n| *n == name)
}

/// The texture of an icon drawn `size` logical pixels square (its SVG viewBox is scaled to fit).
pub fn texture(ctx: &Context, name: &str, size: egui::Vec2, look: Look) -> Option<TextureHandle> {
    let name = static_name(name)?;
    let ppp = ctx.pixels_per_point();
    let w = (size.x * ppp).round().max(1.0) as u32;
    let h = (size.y * ppp).round().max(1.0) as u32;
    let tk = theme::t();
    let (fg, badge, cross) = match look {
        Look::Normal => (tk.icon, None, None),
        Look::Disabled => (tk.icon_disabled, Some(tk.icon_badge_disabled), None),
        Look::TitleBar => (tk.icon_title_bar, None, None),
        Look::Window => (tk.icon_window_buttons, None, None),
        Look::Color(c) => (c, None, None),
        Look::FilterActive => (tk.icon, None, Some(tk.badge_remove)),
    };
    let key = Key {
        name,
        w,
        h,
        fg: fg.to_array(),
        badge: badge.map(|c| c.to_array()),
        chevron: tk.chevron.to_array(),
        cross: cross.map(|c| c.to_array()),
    };
    let cache = cache(ctx);
    let mut cache = cache.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(t) = cache.map.get(&key) {
        return Some(t.clone());
    }
    let mut src = svg(name)?.to_string();
    if let Some(c) = cross {
        src = src.replace(
            r##"<path d="M10.5 0.5L15.5 5.5M15.5 0.5L10.5 5.5" fill="none" stroke="#CACED4"/>"##,
            &format!(r##"<path d="M10.5 0.5L15.5 5.5M15.5 0.5L10.5 5.5" fill="none" stroke="{}"/>"##, theme::hex(c)),
        );
    }
    src = src.replace("#CACED4", &theme::hex(fg)).replace("#1D4470", &theme::hex(tk.chevron));
    if let Some(b) = badge {
        src = src.replace("#1BB474", &theme::hex(b)).replace("#FF6250", &theme::hex(b));
    }
    let image = rasterize(&src, w, h)?;
    let tex = ctx.load_texture(format!("icon-{name}-{w}x{h}"), image, egui::TextureOptions::LINEAR);
    cache.map.insert(key, tex.clone());
    Some(tex)
}

/// Rasterises an SVG source into a `w`×`h` image (aspect kept, centred).
pub fn rasterize(src: &str, w: u32, h: u32) -> Option<egui::ColorImage> {
    let tree = usvg::Tree::from_str(src, &usvg::Options::default()).ok()?;
    let mut pixmap = resvg::tiny_skia::Pixmap::new(w, h)?;
    let size = tree.size();
    let s = (w as f32 / size.width()).min(h as f32 / size.height());
    let dx = (w as f32 - size.width() * s) / 2.0;
    let dy = (h as f32 - size.height() * s) / 2.0;
    resvg::render(&tree, resvg::tiny_skia::Transform::from_row(s, 0.0, 0.0, s, dx, dy), &mut pixmap.as_mut());
    Some(egui::ColorImage::from_rgba_premultiplied([w as usize, h as usize], pixmap.data()))
}

/// Paints an icon of `size` logical pixels centred in `rect`.
pub fn paint(ui: &Ui, name: &str, rect: Rect, size: f32, look: Look) {
    paint_sized(ui.painter(), ui.ctx(), name, Rect::from_center_size(rect.center(), egui::vec2(size, size)), look);
}

/// Paints an icon into exactly `rect` (snapped to physical pixels so 1 px strokes stay crisp).
pub fn paint_sized(painter: &egui::Painter, ctx: &Context, name: &str, rect: Rect, look: Look) {
    let ppp = ctx.pixels_per_point();
    let min = egui::pos2((rect.min.x * ppp).round() / ppp, (rect.min.y * ppp).round() / ppp);
    let rect = Rect::from_min_size(min, rect.size());
    if let Some(tex) = texture(ctx, name, rect.size(), look) {
        painter.image(tex.id(), rect, Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)), Color32::WHITE);
    }
}

/// The app icon (brand), for the title bar.
pub fn app_icon(ctx: &Context, size: f32, grey: bool) -> Option<TextureHandle> {
    let ppp = ctx.pixels_per_point();
    let px = (size * ppp).round().max(1.0) as u32;
    let id = egui::Id::unique(("anyfont-app-icon", px, grey));
    if let Some(t) = ctx.data(|d| d.get_temp::<TextureHandle>(id)) {
        return Some(t);
    }
    let src = include_str!("../assets/brand/anyfont-icon.svg");
    let mut img = rasterize(src, px, px)?;
    if grey {
        for p in img.pixels.iter_mut() {
            let [r, g, b, a] = p.to_array();
            let l = ((r as u32 * 30 + g as u32 * 59 + b as u32 * 11) / 100) as u8;
            let l = (l as u32 * 6 / 10) as u8;
            *p = Color32::from_rgba_premultiplied(l, l, l, a);
        }
    }
    let tex = ctx.load_texture(format!("app-icon-{px}-{grey}"), img, egui::TextureOptions::LINEAR);
    ctx.data_mut(|d| d.insert_temp(id, tex.clone()));
    Some(tex)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_icon_rasterises() {
        for n in NAMES {
            let img = rasterize(svg(n).unwrap(), 32, 32).unwrap_or_else(|| panic!("{n}"));
            assert!(img.pixels.iter().any(|p| p.a() > 0), "{n} is empty");
        }
        assert!(rasterize(include_str!("../assets/brand/anyfont-icon.svg"), 16, 16).is_some());
    }
}
