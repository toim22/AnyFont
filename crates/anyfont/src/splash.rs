//! The start-up splash (`assets/splash.png`, 640×360 in the undecorated start window) with the loading
//! status at the bottom left and the version at the bottom right. It stays until the first scan has finished
//! (at least 1.2 s; at most ~20 s, the scan then goes on behind the main window).

use crate::app::AnyFontApp;
use egui::{pos2, Align2, Color32, Rect, Ui};

const SPLASH_PNG: &[u8] = include_bytes!("../assets/splash.png");
const MIN_SECONDS: f64 = 1.2;
const MAX_SECONDS: f64 = 20.0;

pub struct Splash {
    tex: Option<egui::TextureHandle>,
    started: Option<f64>,
}

impl Splash {
    pub fn new() -> Splash {
        Splash { tex: None, started: None }
    }
}

impl Default for Splash {
    fn default() -> Self {
        Splash::new()
    }
}

/// Draws the splash. Returns true while it is still showing.
pub fn show(app: &mut AnyFontApp, ui: &mut Ui) -> bool {
    let ctx = ui.ctx().clone();
    let now = ctx.input(|i| i.time);
    let ready = app.db_ready;
    let (done, done_t) = (app.scan_progress.0, app.scan_progress.1);
    let Some(sp) = app.splash.as_mut() else { return false };
    let t0 = *sp.started.get_or_insert(now);
    let elapsed = now - t0;
    if (ready && elapsed >= MIN_SECONDS) || elapsed >= MAX_SECONDS {
        app.splash = None;
        ctx.request_repaint();
        return false;
    }
    let tex = sp
        .tex
        .get_or_insert_with(|| {
            let img = image::load_from_memory(SPLASH_PNG).map(|i| i.to_rgba8()).unwrap_or_else(|_| image::RgbaImage::new(1, 1));
            let ci = egui::ColorImage::from_rgba_unmultiplied([img.width() as usize, img.height() as usize], img.as_raw());
            ctx.load_texture("splash", ci, egui::TextureOptions::LINEAR)
        })
        .clone();
    let full = ui.max_rect();
    let p = ui.painter();
    p.rect_filled(full, 0.0, Color32::BLACK);
    p.image(tex.id(), full, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
    let status = if elapsed < 0.3 {
        crate::tr("Initializing...")
    } else if done_t > 0 {
        format!("{} {}%", crate::tr("Loading fonts..."), (done * 100 / done_t.max(1)).min(100))
    } else {
        crate::tr("Loading fonts...")
    };
    let y = full.bottom() - full.height() * 11.0 / 360.0;
    let font = crate::theme::body(11.0);
    let grey = Color32::from_gray(0x8c);
    crate::widgets::data_text(ui, p, pos2(full.left() + 24.0, y), Align2::LEFT_CENTER, &status, font.clone(), grey);
    let version = crate::fmt("Version {}", &[env!("CARGO_PKG_VERSION")]);
    crate::widgets::data_text(ui, p, pos2(full.right() - 24.0, y), Align2::RIGHT_CENTER, &version, font, grey);
    ctx.request_repaint_after(std::time::Duration::from_millis(50));
    true
}
