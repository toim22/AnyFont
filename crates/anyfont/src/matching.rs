//! PANOSE font matching dialogs and the Type 1 replacement message (owned by the services UI developer). STUB.
//! - Exceptions dialog (from Options ▸ Font Matching ▸ Exceptions...)
//! - Font Matching Results dialog (after importing folders/collections that reference missing fonts)
//! - Type 1 information message ("Replace fonts")
use anyfont_core::db::FontDatabase;
use anyfont_core::catalog::Catalog;

pub struct Matching;

impl Matching {
    pub fn new() -> Self { Matching }
    pub fn open_exceptions(&mut self) {}
    /// Missing font names (family + style) to resolve with PANOSE.
    pub fn open_results(&mut self, _missing: Vec<String>) {}
    /// Type 1 fonts found in the current selection/library.
    pub fn open_type1_message(&mut self, _font_ids: Vec<anyfont_core::model::FontId>) {}
    /// Returns true when the database changed (substitutions/exceptions saved) so the caller reloads its catalog.
    pub fn ui(&mut self, _ctx: &egui::Context, _db: &mut FontDatabase, _catalog: &Catalog) -> bool { false }
}
