//! Persisted user settings (JSON at a path chosen by the caller).
//!
//! Mirrors the original's settings (dossier `05-data-models/settings.md`, Options dialog screenshots):
//! View menu toggles (Group by family, Show recent, Show Latin names, Show duplicate fonts, Show font
//! properties), preview size and sample text with history (`FONTSAMPLES` presets), the Options dialog
//! (UI scale %, theme, window border colour), PANOSE preferences, and the panel layout.
//!
//! [`Settings::load`] never fails: a missing or unreadable file yields defaults (and unknown fields are
//! ignored, missing fields take their default), so a damaged file cannot stop the application.
//! [`Settings::save`] writes atomically (temporary file + rename).

use crate::error::{CoreError, Result};
use crate::panose::PanoseMode;
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Colour theme (Options > Theme). The original strings confirm "Light" and "Dark"; the two medium
/// variants are AnyFont additions.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum Theme {
    /// Dark (default in the screenshots).
    #[default]
    Dark,
    /// Medium dark.
    MediumDark,
    /// Medium light.
    MediumLight,
    /// Light.
    Light,
}

impl Theme {
    /// Caption.
    pub fn caption(self) -> &'static str {
        match self {
            Theme::Dark => "Dark",
            Theme::MediumDark => "Medium Dark",
            Theme::MediumLight => "Medium Light",
            Theme::Light => "Light",
        }
    }
    /// All themes.
    pub const ALL: [Theme; 4] = [Theme::Light, Theme::MediumLight, Theme::MediumDark, Theme::Dark];
}

/// How updates are checked.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum UpdateMode {
    /// Keep the product up to date: download new versions by itself, ask before installing.
    KeepUpToDate,
    /// Never check.
    Never,
    /// Check and notify.
    #[default]
    Notify,
}

/// The library selected when the application closed.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum LastLibrary {
    /// My fonts.
    #[default]
    MyFonts,
    /// A user folder, by path.
    Folder(String),
    /// A collection, by name.
    Collection(String),
}

/// Window placement.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct WindowState {
    /// Outer position (logical px), `None` = let the OS decide.
    pub position: Option<[f32; 2]>,
    /// Inner size (logical px).
    pub size: [f32; 2],
    /// Maximised.
    pub maximized: bool,
    /// Width of the left panel (libraries + filters).
    pub left_panel_width: f32,
    /// Width of the right panel (font info / glyphs / variable).
    pub right_panel_width: f32,
    /// Height of the filters section.
    pub filters_height: f32,
    /// Right panel visible ("Show font properties").
    pub right_panel_visible: bool,
    /// Selected tab of the right panel (0 Font info, 1 Glyphs, 2 Variable).
    pub right_panel_tab: u8,
}

impl Default for WindowState {
    fn default() -> Self {
        WindowState {
            position: None,
            size: [1280.0, 800.0],
            maximized: false,
            left_panel_width: 260.0,
            right_panel_width: 320.0,
            filters_height: 360.0,
            right_panel_visible: true,
            right_panel_tab: 0,
        }
    }
}

/// PANOSE preferences.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct PanoseSettings {
    /// When to show matching results.
    pub mode: PanoseMode,
    /// Default font family used when nothing matches.
    pub default_font: String,
}

impl Default for PanoseSettings {
    fn default() -> Self {
        PanoseSettings { mode: PanoseMode::Text, default_font: "Arial".into() }
    }
}

/// The sample text presets of the preview combo (`FONTSAMPLES` columns).
pub const SAMPLE_PRESETS: [&str; 5] = [
    "Font Name",
    "abcdefghijklmnopqrstuvwxyz",
    "ABCDEFGHIJKLMNOPQRSTUVWXYZ",
    "1234567890",
    "The quick brown fox jumps over the lazy dog",
];

/// The preset that means "render the font's own name".
pub const SAMPLE_FONT_NAME: &str = "Font Name";

/// All settings.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    /// Settings format version.
    pub version: u32,
    /// Window and panels.
    pub window: WindowState,
    /// Preview size in points.
    pub preview_size: f32,
    /// Current sample text ([`SAMPLE_FONT_NAME`] = the font's name).
    pub sample_text: String,
    /// Sample texts typed by the user, most recent first.
    pub sample_history: Vec<String>,
    /// View > Group by family.
    pub group_by_family: bool,
    /// View > Show recent (MRU section at the top of the list).
    pub show_recent: bool,
    /// Render each list entry in its own font.
    pub render_name_in_font: bool,
    /// View > Show Latin names.
    pub show_latin_names: bool,
    /// View > Show duplicate fonts.
    pub show_duplicates: bool,
    /// View > Show font properties.
    pub show_font_properties: bool,
    /// Theme.
    pub theme: Theme,
    /// UI scale in percent (Options slider, 100.0 default).
    pub ui_scale_percent: f32,
    /// Apply the UI scale (Options checkbox).
    pub ui_scale_enabled: bool,
    /// Custom window border colour (`None` = theme default).
    pub window_border_color: Option<[u8; 3]>,
    /// PANOSE preferences.
    pub panose: PanoseSettings,
    /// Update checks.
    pub update_mode: UpdateMode,
    /// UI language override (BCP-47 such as "pt-BR"), `None` = system.
    pub language: Option<String>,
    /// Number of recently used fonts kept.
    pub mru_size: usize,
    /// Library selected on exit.
    pub last_library: LastLibrary,
    /// Expanded filter groups (captions).
    pub expanded_filter_groups: Vec<String>,
    /// Glyph grid cell size in px.
    pub glyph_cell_size: f32,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            version: 1,
            window: WindowState::default(),
            preview_size: 24.0,
            sample_text: SAMPLE_FONT_NAME.to_string(),
            sample_history: Vec::new(),
            group_by_family: true,
            show_recent: true,
            render_name_in_font: true,
            show_latin_names: false,
            show_duplicates: false,
            show_font_properties: true,
            theme: Theme::Dark,
            ui_scale_percent: 100.0,
            ui_scale_enabled: false,
            window_border_color: None,
            panose: PanoseSettings::default(),
            update_mode: UpdateMode::Notify,
            language: None,
            mru_size: 10,
            last_library: LastLibrary::MyFonts,
            expanded_filter_groups: Vec::new(),
            glyph_cell_size: 48.0,
        }
    }
}

impl Settings {
    /// Loads settings; returns defaults when the file is missing or invalid.
    pub fn load(path: &Path) -> Settings {
        match std::fs::read(path) {
            Ok(bytes) => match serde_json::from_slice::<Settings>(&bytes) {
                Ok(s) => s.sanitized(),
                Err(e) => {
                    log::warn!("settings file {} is invalid ({e}); using defaults", path.display());
                    Settings::default()
                }
            },
            Err(_) => Settings::default(),
        }
    }

    /// Saves settings atomically (creates the parent directory).
    pub fn save(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
            std::fs::create_dir_all(parent).map_err(|e| CoreError::io(parent, e))?;
        }
        let json = serde_json::to_vec_pretty(self)?;
        let tmp = path.with_extension("json.tmp");
        std::fs::write(&tmp, json).map_err(|e| CoreError::io(&tmp, e))?;
        std::fs::rename(&tmp, path).map_err(|e| CoreError::io(path, e))
    }

    /// Clamps values into their valid ranges.
    pub fn sanitized(mut self) -> Settings {
        if !(self.preview_size.is_finite() && (4.0..=400.0).contains(&self.preview_size)) {
            self.preview_size = 24.0;
        }
        if !(self.ui_scale_percent.is_finite() && (50.0..=300.0).contains(&self.ui_scale_percent)) {
            self.ui_scale_percent = 100.0;
        }
        if !(self.glyph_cell_size.is_finite() && (16.0..=256.0).contains(&self.glyph_cell_size)) {
            self.glyph_cell_size = 48.0;
        }
        self.mru_size = self.mru_size.clamp(0, 100);
        self.sample_history.truncate(20);
        self
    }

    /// Records a sample text typed by the user (most recent first, no duplicates, 20 entries).
    pub fn remember_sample(&mut self, text: &str) {
        let t = text.trim();
        if t.is_empty() || SAMPLE_PRESETS.contains(&t) {
            return;
        }
        self.sample_history.retain(|s| s != t);
        self.sample_history.insert(0, t.to_string());
        self.sample_history.truncate(20);
    }

    /// Effective UI scale factor (1.0 when disabled).
    pub fn ui_scale(&self) -> f32 {
        if self.ui_scale_enabled {
            self.ui_scale_percent / 100.0
        } else {
            1.0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_and_bad_files() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("sub").join("settings.json");
        assert_eq!(Settings::load(&p), Settings::default());
        let mut s = Settings { theme: Theme::Light, show_latin_names: true, ..Default::default() };
        s.remember_sample("Hello");
        s.remember_sample("World");
        s.remember_sample("Hello");
        assert_eq!(s.sample_history, vec!["Hello", "World"]);
        s.save(&p).unwrap();
        assert_eq!(Settings::load(&p), s);
        std::fs::write(&p, b"{ not json").unwrap();
        assert_eq!(Settings::load(&p), Settings::default());
        std::fs::write(&p, br#"{"theme":"Light","preview_size":-3,"unknown":1}"#).unwrap();
        let l = Settings::load(&p);
        assert_eq!(l.theme, Theme::Light);
        assert_eq!(l.preview_size, 24.0);
        assert!(l.group_by_family);
    }
}
