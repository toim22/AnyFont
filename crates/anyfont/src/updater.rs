//! Product updates (owned by the services UI developer). STUB – interface agreed in docs/UI-TEAM.md.
use std::path::PathBuf;

/// What the main window must do after `Updater::ui`.
pub enum UpdaterRequest {
    /// Close AnyFont (asking nothing – font manager has no documents), then run this setup silently with
    /// `/RELAUNCH=1`; `main` calls [`run_setup_after_exit`] after the event loop ends.
    ExitAndInstall(PathBuf),
}

pub struct Updater;

impl Updater {
    pub fn new() -> Self { Updater }
    /// Called once when the main window is shown: checks in the background according to the update mode.
    pub fn on_startup(&mut self, _ctx: &egui::Context, _mode: anyfont_core::settings::UpdateMode) {}
    /// Help ▸ Check for Updates... and the title-bar Updates button.
    pub fn open(&mut self) {}
    /// True while a newer version is known (the title-bar Updates button may show a dot).
    pub fn update_available(&self) -> bool { false }
    pub fn ui(&mut self, _ctx: &egui::Context) -> Option<UpdaterRequest> { None }
}

/// Runs the downloaded setup after the window closed.
pub fn run_setup_after_exit(_setup: &std::path::Path) {}
