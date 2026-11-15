//! File ▸ Get More... – the online font catalog (owned by the services UI developer). STUB.
use std::path::PathBuf;

pub struct GetMore;

impl GetMore {
    pub fn new() -> Self { GetMore }
    pub fn open(&mut self) {}
    /// `content_dir`: where downloaded fonts go (anyfont_sys content folder). Returns the files downloaded
    /// this frame; the main window then rescans so they appear in My fonts as Not installed.
    pub fn ui(&mut self, _ctx: &egui::Context, _content_dir: &std::path::Path) -> Vec<PathBuf> { Vec::new() }
}
