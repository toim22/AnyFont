#![allow(dead_code)]
//! A small window that opens every services dialog (About, System Info, Legal Notices, Updates, Report a
//! Bug, Submit Ideas and Feedback, Get More, PANOSE Exceptions / Results, Type 1 message) on their own,
//! with an in-memory font database, so they can be tried without the main window:
//!
//! ```text
//! cargo run -p anyfont --example dialogs
//! set ANYFONT_UPDATE_FEED=C:\path\latest.json   (offline "update available")
//! set ANYFONT_CATALOG_URL=C:\path\catalog.json  (offline Get More)
//! set ANYFONT_ISSUE_RELAY=http://127.0.0.1:8787 (a local relay; never the real one while testing)
//! ```
//!
//! Downloads from Get More go to `%TEMP%\AnyFont dialogs example\Fonts`, never to the real Content folder.

#[path = "../src/about.rs"]
mod about;
#[path = "../src/bugreport.rs"]
mod bugreport;
#[path = "../src/getmore.rs"]
mod getmore;
#[path = "../src/help.rs"]
mod help;
#[path = "../src/matching.rs"]
mod matching;
#[path = "../src/updater.rs"]
mod updater;

use anyfont_core::{Catalog, FontDatabase};

struct Demo {
    about: about::About,
    bug: bugreport::BugReport,
    get_more: getmore::GetMore,
    matching: matching::Matching,
    updater: updater::Updater,
    db: FontDatabase,
    catalog: Catalog,
    mode: anyfont_core::settings::UpdateMode,
    prefs: anyfont_core::settings::PanoseSettings,
    content: std::path::PathBuf,
    log: Vec<String>,
    started: bool,
}

impl eframe::App for Demo {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        if !self.started {
            self.started = true;
            self.updater.on_startup(&ctx, self.mode);
        }
        egui::Frame::new().inner_margin(16.0).show(ui, |ui| {
            ui.heading("AnyFont services dialogs");
            ui.add_space(8.0);
            ui.horizontal_wrapped(|ui| {
                if ui.button("About AnyFont...").clicked() {
                    self.about.open();
                }
                if ui.button("System Info...").clicked() {
                    self.about.open_system_info();
                }
                if ui.button("Legal Notices...").clicked() {
                    self.about.open_legal_notices();
                }
                if ui.button("Check for Updates...").clicked() {
                    self.updater.open();
                }
                if ui.button("Report a Bug...").clicked() {
                    self.bug.open(bugreport::Kind::Bug);
                }
                if ui.button("Submit Ideas and Feedback...").clicked() {
                    self.bug.open(bugreport::Kind::Idea);
                }
                if ui.button("Crash report").clicked() {
                    self.bug.open_with_crash("thread 'main' panicked at src/app.rs:1:1:\nexample crash".into());
                }
                if ui.button("Get More...").clicked() {
                    self.get_more.open();
                }
                if ui.button("Exceptions...").clicked() {
                    self.matching.open_exceptions();
                }
                if ui.button("Font Matching Results").clicked() {
                    self.matching.open_results(vec!["Helvetica Bold".into(), "Futura (Light)".into(), "Arial-BoldMT".into()]);
                }
                if ui.button("Type 1 message").clicked() {
                    self.matching.open_type1_message(vec![1]);
                }
                if ui.button("Product Help").clicked() {
                    help::open_product_help();
                }
            });
            ui.separator();
            if updater::mode_ui(ui, &mut self.mode) {
                self.updater.set_mode(self.mode);
            }
            ui.label(format!("Update available: {}", self.updater.update_available()));
            ui.separator();
            ui.label("Font Matching (Options):");
            self.matching.preferences_ui(ui, &mut self.prefs, &self.catalog);
            ui.separator();
            for l in self.log.iter().rev().take(8) {
                ui.label(l.as_str());
            }
        });
        self.about.ui(&ctx);
        self.bug.ui(&ctx);
        for p in self.get_more.ui(&ctx, &self.content) {
            self.log.push(format!("downloaded {}", p.display()));
        }
        if self.matching.ui(&ctx, &mut self.db, &self.catalog) {
            self.catalog = Catalog::load(&self.db).expect("catalog");
            self.log.push("font database changed".into());
        }
        if let Some(r) = self.matching.take_resolved() {
            self.log.push(format!("resolved: {r:?}"));
        }
        if let Some(m) = self.updater.take_mode_change() {
            self.mode = m;
        }
        if let Some(updater::UpdaterRequest::ExitAndInstall(path)) = self.updater.ui(&ctx) {
            // the example never runs a setup
            self.log.push(format!("would close and run {}", path.display()));
        }
    }
}

fn main() -> eframe::Result {
    let db = FontDatabase::open_in_memory().expect("database");
    let catalog = Catalog::load(&db).expect("catalog");
    let content = std::env::temp_dir().join("AnyFont dialogs example").join("Fonts");
    let _ = std::fs::create_dir_all(&content);
    let options = eframe::NativeOptions { viewport: egui::ViewportBuilder::default().with_inner_size([1200.0, 800.0]), ..Default::default() };
    eframe::run_native(
        "AnyFont dialogs",
        options,
        Box::new(move |cc| {
            // the Dark tokens the shell's theme uses (design-tokens.md)
            let mut v = egui::Visuals::dark();
            v.window_fill = egui::Color32::from_rgb(0x26, 0x26, 0x26);
            v.panel_fill = v.window_fill;
            v.override_text_color = Some(egui::Color32::from_rgb(0xCA, 0xCE, 0xD4));
            cc.egui_ctx.set_visuals(v);
            Ok(Box::new(Demo {
                about: about::About::new(),
                bug: bugreport::BugReport::new(),
                get_more: getmore::GetMore::new(),
                matching: matching::Matching::new(),
                updater: updater::Updater::new(),
                db,
                catalog,
                mode: anyfont_core::settings::UpdateMode::default(),
                prefs: Default::default(),
                content,
                log: vec![],
                started: false,
            }))
        }),
    )
}
