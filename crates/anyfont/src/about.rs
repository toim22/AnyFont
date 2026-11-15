//! Help ▸ About AnyFont, with System Info... and Legal Notices... (owned by the services UI developer).
//!
//! Laid out like the captured About box (`06-ui/screenshots/05-dialogs/Help ▸ About … .png`, 579×386,
//! UI-SPEC §7.2.1): the black banner (`assets/banner.png`, drawn at 2×) in a grey 1 px border, the version and
//! release month, the copyright line, the bordered "Source ID:" box and the buttons System Info... /
//! Legal Notices... (left) and Close (right, default). System Info (599×411, §7.2.2) and Legal Notices
//! (514×739, §7.2.3) open on top of it.

#[path = "sysinfo.rs"]
pub mod sysinfo;

#[path = "legal.rs"]
pub mod legal;

use crate::help::dlg;
use egui::{pos2, vec2, Align2, Color32, Rect, Sense, Stroke, StrokeKind};

/// The build id shown in the Source ID box (set by `build.rs`: the commit).
pub fn source_id() -> &'static str {
    option_env!("ANYFONT_SOURCE_ID").filter(|s| !s.is_empty()).unwrap_or("unknown")
}

/// "November 2026" (set by `build.rs`).
pub fn release() -> &'static str {
    option_env!("ANYFONT_RELEASE").filter(|s| !s.is_empty()).unwrap_or("November 2026")
}

/// The copyright year.
pub const YEAR: &str = "2026";

/// "Version 0.1.0 - November 2026 Release"
pub fn version_line() -> String {
    format!("Version {} - {} Release", env!("CARGO_PKG_VERSION"), release())
}

/// "(C) 2026 AnyFont Team.  All rights reserved." (two spaces, as in the original)
pub fn copyright_line() -> String {
    format!("(C) {} AnyFont Team.  All rights reserved.", YEAR)
}

const BANNER_PNG: &[u8] = include_bytes!("../assets/banner.png");

struct SystemInfo {
    category: usize,
    rows: sysinfo::Rows,
}

pub struct About {
    open: bool,
    system_info: Option<SystemInfo>,
    legal: Option<String>,
    message: Option<dlg::Message>,
    banner: Option<egui::TextureHandle>,
}

impl Default for About {
    fn default() -> Self {
        Self::new()
    }
}

impl About {
    pub fn new() -> Self {
        About { open: false, system_info: None, legal: None, message: None, banner: None }
    }

    pub fn open(&mut self) {
        self.open = true;
    }

    /// Opens System Info directly (without the About box behind it).
    pub fn open_system_info(&mut self) {
        self.system_info = Some(SystemInfo { category: 0, rows: sysinfo::collect(0) });
    }

    /// Opens Legal Notices directly.
    pub fn open_legal_notices(&mut self) {
        self.legal = Some(legal::text());
    }

    /// True while the About box or one of its child dialogs is open.
    pub fn is_open(&self) -> bool {
        self.open || self.system_info.is_some() || self.legal.is_some() || self.message.is_some()
    }

    pub fn ui(&mut self, ctx: &egui::Context) {
        if self.open {
            self.about_ui(ctx);
        }
        if self.system_info.is_some() {
            self.system_info_ui(ctx);
        }
        if let Some(text) = &self.legal {
            if legal::ui(ctx, text) {
                self.legal = None;
            }
        }
        if let Some(m) = &self.message {
            if m.ui(ctx, "about_message") {
                self.message = None;
            }
        }
    }

    fn banner(&mut self, ctx: &egui::Context) -> egui::TextureHandle {
        if let Some(t) = &self.banner {
            return t.clone();
        }
        let img = image::load_from_memory(BANNER_PNG).map(|i| i.to_rgba8()).unwrap_or_else(|_| image::RgbaImage::new(1, 1));
        let ci = egui::ColorImage::from_rgba_unmultiplied([img.width() as usize, img.height() as usize], img.as_raw());
        let t = ctx.load_texture("anyfont_about_banner", ci, egui::TextureOptions::LINEAR);
        self.banner = Some(t.clone());
        t
    }

    fn about_ui(&mut self, ctx: &egui::Context) {
        let tex = self.banner(ctx);
        let mut close = false;
        let mut sysinfo = false;
        let mut legal = false;
        let child_open = self.system_info.is_some() || self.legal.is_some();
        let ((), dismissed) = dlg::window(ctx, "about", "About AnyFont", vec2(579.0, 386.0), |ui| {
            let b = ui.max_rect();
            let p = ui.painter().clone();
            let text = dlg::text_color(ui);
            let f = dlg::font(12.5);
            // banner: 548×206 inside a 1 px #808080 border
            let banner = Rect::from_min_size(pos2(b.left(), b.top() + 4.0), vec2(b.width(), 208.0));
            p.rect_filled(banner, 0.0, Color32::BLACK);
            let inner = banner.shrink(1.0);
            // the image keeps its aspect ratio, centred
            let sz = tex.size_vec2();
            let scale = (inner.width() / sz.x).min(inner.height() / sz.y);
            let ir = Rect::from_center_size(inner.center(), sz * scale);
            p.image(tex.id(), ir, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
            p.rect_stroke(banner, 0.0, Stroke::new(1.0, dlg::BANNER_BORDER), StrokeKind::Inside);
            // version and copyright, centred
            p.text(pos2(b.center().x, b.top() + 226.0), Align2::CENTER_CENTER, version_line(), f.clone(), text);
            p.text(pos2(b.center().x, b.top() + 248.0), Align2::CENTER_CENTER, copyright_line(), f.clone(), text);
            // Source ID box
            let sid = Rect::from_min_max(pos2(b.left(), b.top() + 263.0), pos2(b.right(), b.top() + 294.0));
            p.rect_stroke(sid, 0.0, Stroke::new(1.0, dlg::line_color(ui)), StrokeKind::Inside);
            let label = p.text(pos2(sid.left() + 8.0, sid.center().y), Align2::LEFT_CENTER, "Source ID:", f.clone(), text);
            p.text(pos2((sid.left() + 84.0).max(label.right() + 12.0), sid.center().y), Align2::LEFT_CENTER, source_id(), f.clone(), text);
            // buttons, at the captured places
            let y = b.top() + 311.0;
            ui.add_enabled_ui(!child_open, |ui| {
                sysinfo = dlg::button_at(ui, Rect::from_min_size(pos2(b.left(), y), vec2(118.0, 24.0)), "System Info...", false, true);
                legal = dlg::button_at(ui, Rect::from_min_size(pos2(b.left() + 130.0, y), vec2(125.0, 24.0)), "Legal Notices...", false, true);
                close = dlg::button_at(ui, Rect::from_min_size(pos2(b.right() - 79.0, y), vec2(79.0, 24.0)), "Close", true, true);
            });
            if !child_open && dlg::enter_pressed(ui) {
                close = true;
            }
        });
        if sysinfo {
            self.open_system_info();
        }
        if legal {
            self.open_legal_notices();
        }
        if (close || dismissed) && !child_open {
            self.open = false;
        }
    }

    fn system_info_ui(&mut self, ctx: &egui::Context) {
        let Some(st) = &mut self.system_info else { return };
        let mut close = false;
        let mut save = false;
        let ((), dismissed) = dlg::window(ctx, "system_info", "System Info", vec2(599.0, 411.0), |ui| {
            let b = ui.max_rect();
            let text = dlg::text_color(ui);
            let f = dlg::font(12.5);
            ui.painter().text(pos2(b.left(), b.top() + 17.0), Align2::LEFT_CENTER, "Choose a category:", f.clone(), text);
            let combo = Rect::from_min_size(pos2(b.left() + 133.0, b.top() + 5.0), vec2(270.0, 24.0));
            let mut cat = st.category;
            dlg::at(ui, combo, |ui| {
                egui::ComboBox::from_id_salt("sysinfo_category").width(262.0).selected_text(sysinfo::CATEGORIES[cat]).show_ui(ui, |ui| {
                    for (i, c) in sysinfo::CATEGORIES.iter().enumerate() {
                        ui.selectable_value(&mut cat, i, *c);
                    }
                })
                .response
                .accessible_name("Choose a category:");
            });
            if cat != st.category {
                st.category = cat;
                st.rows = sysinfo::collect(cat);
            }
            // the list: name and value columns in a bordered, scrolling box
            let list = Rect::from_min_max(pos2(b.left(), b.top() + 34.0), pos2(b.right(), b.top() + 284.0));
            dlg::frame_rect(ui, list);
            let inner = list.shrink2(vec2(6.0, 2.0));
            dlg::at(ui, inner, |ui| {
                egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                    for (k, v) in &st.rows {
                        let (r, _) = ui.allocate_exact_size(vec2(ui.available_width(), 25.0), Sense::hover());
                        let p = ui.painter();
                        // the name is interface text (translated), then its colon
                        let indent = (k.len() - k.trim_start().len()) as f32 * 3.0;
                        let g = p.layout_no_wrap(k.trim().to_string(), f.clone(), text);
                        let gx = r.left() + indent;
                        let w = g.size().x;
                        p.galley(pos2(gx, r.center().y - g.size().y / 2.0), g, text);
                        p.text(pos2(gx + w, r.center().y), Align2::LEFT_CENTER, ":", f.clone(), text);
                        // the value is data: a LayoutJob is not translated
                        let vr = Rect::from_min_max(pos2(r.left() + 171.0, r.top()), r.max);
                        let job = egui::text::LayoutJob::simple_singleline(v.clone(), f.clone(), text);
                        let vg = ui.painter().layout_job(job);
                        ui.painter().with_clip_rect(vr).galley(pos2(vr.left(), r.center().y - vg.size().y / 2.0), vg, text);
                    }
                });
            });
            let y = b.top() + 324.0;
            close = dlg::button_at(ui, Rect::from_min_size(pos2(b.left() + 128.0, y), vec2(80.0, 24.0)), "Close", true, true);
            save = dlg::button_at(ui, Rect::from_min_size(pos2(b.left() + 361.0, y), vec2(80.0, 24.0)), "Save", false, true);
            close |= dlg::enter_pressed(ui);
        });
        if save {
            if let Some(p) = rfd::FileDialog::new().set_title("Save System Info").add_filter("Text (*.txt)", &["txt"]).set_file_name("System Info.txt").save_file() {
                let shown = p.display().to_string();
                self.message = Some(match std::fs::write(&p, sysinfo::report()) {
                    Ok(()) => dlg::Message::new("System Info", format!("System info was saved to {shown}.")),
                    Err(e) => {
                        log::warn!("System Info ▸ Save {shown}: {e}");
                        dlg::Message::new("System Info", format!("Unable to save system info to {shown}."))
                    }
                });
            }
        }
        if close || dismissed {
            self.system_info = None;
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn about_lines() {
        assert_eq!(super::copyright_line(), "(C) 2026 AnyFont Team.  All rights reserved.");
        assert!(super::version_line().starts_with(&format!("Version {} - ", env!("CARGO_PKG_VERSION"))));
        assert!(super::version_line().ends_with(" Release"));
        assert!(!super::source_id().is_empty());
        // the banner is the generated 2× image of a 548×206 area
        let img = image::load_from_memory(super::BANNER_PNG).unwrap();
        assert_eq!((img.width(), img.height()), (1096, 412));
    }
}
