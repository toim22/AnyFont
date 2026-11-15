//! # anyfont-core
//!
//! The engine of AnyFont, a Windows font manager. Pure Rust, no UI and no Win32 calls: the egui
//! application (`anyfont`) and the Windows layer (`anyfont-sys`) build on it.
//!
//! What it does (each module documents the dossier evidence it follows):
//!
//! | Module | Responsibility |
//! |---|---|
//! | [`sfnt`] | sfnt container reading and the validation rules of the original engine |
//! | [`woff`], [`type1`] | WOFF 1.0 decoding; Type 1 `.pfb/.pfa/.pfm/.afm` parsing |
//! | [`parse`] | font files -> [`FontRecord`]s (one per face / named instance) |
//! | [`glyphs`] | glyph enumeration (incl. unencoded glyphs) and glyph-filter classification |
//! | [`db`] | the SQLite font cache, libraries (folders, collections, Favorites), MRU, PANOSE tables |
//! | [`scan`] | incremental rescans off the UI thread with progress and cancellation |
//! | [`catalog`] | in-memory snapshot: query (library + filters + search + grouping), status text |
//! | [`filters`] | the Filters panel groups and their predicates |
//! | [`duplicates`] | duplicate detection (same family/style in different files) |
//! | [`eligibility`] | enable/disable rules of the toolbar, menus and context menus (FontDS) |
//! | [`panose`] | PANOSE distance, missing-font matching, exceptions and substitutions |
//! | [`vendors`] | `OS/2.achVendID` -> vendor name and URL |
//! | [`exchange`] | Import/Export Folders and Collections (XML) |
//! | [`settings`] | the persisted user settings (JSON) |
//! | [`format`] | display strings of the font list and the Font info tab |
//!
//! Threading model: [`db::FontDatabase`] wraps one SQLite connection (`Send`, not `Sync`). The UI keeps
//! one for edits and loads a [`catalog::Catalog`] snapshot that answers every query in memory. Scans run on
//! a worker thread with their own connection ([`scan::spawn_scan`]); when a scan finishes the UI reloads the
//! snapshot. See `docs/core-api.md`.

#![warn(missing_docs)]

pub mod catalog;
pub mod db;
pub mod duplicates;
pub mod eligibility;
pub mod error;
pub mod exchange;
pub mod filters;
pub mod format;
pub mod glyphs;
pub mod model;
pub mod panose;
pub mod parse;
pub mod scan;
pub mod settings;
pub mod sfnt;
pub mod type1;
pub mod vendors;
pub mod woff;

pub use catalog::{Catalog, FontQuery, LibraryRef, ListRow, QueryResult, SortOrder};
pub use db::{Collection, Folder, FontDatabase, FAVORITES_NAME};
pub use eligibility::{Eligibility, Selection};
pub use error::{CoreError, Result};
pub use filters::{FilterGroup, FilterOption, FilterSet};
pub use model::*;
pub use panose::{PanoseMode, SubstitutionStatus, SubstitutionType};
pub use scan::{spawn_scan, ScanHandle, ScanProgress, ScanReport, ScanRequest};
pub use settings::Settings;
