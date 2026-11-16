//! Shell settings: `anyfont_core::Settings` (the persisted model) plus the window/panel state only the shell
//! uses, in one JSON file, `%APPDATA%\AnyFont\settings.json`.
//!
//! Folders can be redirected for tests and development: `ANYFONT_DATA_DIR=<dir>` puts the settings in
//! `<dir>\settings` and the font database in `<dir>\data`, so nothing touches the user's real profile. The
//! sandbox (`ANYFONT_SANDBOX_ROOT`) redirects them too (see `anyfont_sys::paths`).

use crate::theme::Scheme;
use anyfont_core::settings::Settings as CoreSettings;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Environment variable redirecting AnyFont's settings and database folders.
pub const DATA_DIR_ENV: &str = "ANYFONT_DATA_DIR";

/// Default widths and sizes (UI-SPEC §1.2, §5.3, §5.4).
pub const LEFT_PANEL_DEFAULT: f32 = 241.0;
pub const LEFT_PANEL_MIN: f32 = 240.0;
pub const RIGHT_PANEL_DEFAULT: f32 = 271.0;
pub const RIGHT_PANEL_MIN: f32 = 220.0;
pub const RIGHT_PANEL_MAX: f32 = 600.0;
pub const LIBRARIES_HEIGHT_DEFAULT: f32 = 208.0;
pub const PREVIEW_MIN: f32 = 8.0;
pub const PREVIEW_MAX: f32 = 86.0;
pub const PREVIEW_DEFAULT: f32 = 36.0;
pub const GLYPH_CELL_MIN: f32 = 24.0;
pub const GLYPH_CELL_MAX: f32 = 96.0;
pub const GLYPH_CELL_DEFAULT: f32 = 41.0;
pub const SCALE_MIN: f32 = 75.0;
pub const SCALE_MAX: f32 = 225.0;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AppSettings {
    /// The core settings model (View toggles, preview, Options values, PANOSE, window placement).
    #[serde(flatten)]
    pub core: CoreSettings,
    /// Options ▸ Color ▸ Theme (the four schemes of the original).
    pub scheme: Scheme,
    /// Height of the Libraries part of the left panel (the Libraries/Filters splitter).
    pub libraries_height: f32,
    /// Collapsed section headers: "Folders", "My Collections", glyph filter groups ("glyph:Common").
    pub collapsed_sections: Vec<String>,
    /// View ▸ Hide filter list / Show filter list.
    pub show_filter_list: bool,
    /// The setup's language choice already applied (its stamp, see `i18n::installer_choice`).
    pub installer_lang_stamp: String,
    /// "Do not show this message again" of the remove confirmation.
    pub skip_remove_confirmation: bool,
    /// "Do not show this message again." of the Delete fonts dialog.
    pub skip_delete_confirmation: bool,
}

impl Default for AppSettings {
    fn default() -> Self {
        let mut core = CoreSettings {
            preview_size: PREVIEW_DEFAULT,
            show_recent: false,
            show_font_properties: false,
            glyph_cell_size: GLYPH_CELL_DEFAULT,
            ui_scale_enabled: true,
            ..CoreSettings::default()
        };
        core.window.left_panel_width = LEFT_PANEL_DEFAULT;
        core.window.right_panel_width = RIGHT_PANEL_DEFAULT;
        core.window.right_panel_visible = false;
        core.window.right_panel_tab = 0;
        core.window.size = [1280.0, 800.0];
        core.window.maximized = true;
        AppSettings {
            core,
            scheme: Scheme::Dark, // lead decision 5: Dark by default, whatever the Windows mode
            libraries_height: LIBRARIES_HEIGHT_DEFAULT,
            collapsed_sections: Vec::new(),
            show_filter_list: true,
            installer_lang_stamp: String::new(),
            skip_remove_confirmation: false,
            skip_delete_confirmation: false,
        }
    }
}

impl AppSettings {
    /// Loads the settings; defaults when the file is missing or unreadable.
    pub fn load(path: &Path) -> AppSettings {
        let mut s = match std::fs::read(path) {
            Ok(bytes) => serde_json::from_slice::<AppSettings>(&bytes).unwrap_or_else(|e| {
                log::warn!("settings file {} is invalid ({e}); using defaults", path.display());
                AppSettings::default()
            }),
            Err(_) => AppSettings::default(),
        };
        s.core = s.core.sanitized();
        s.sanitize();
        s
    }

    /// Clamps the shell's values.
    pub fn sanitize(&mut self) {
        let c = &mut self.core;
        c.preview_size = c.preview_size.clamp(PREVIEW_MIN, PREVIEW_MAX).round();
        c.glyph_cell_size = c.glyph_cell_size.clamp(GLYPH_CELL_MIN, GLYPH_CELL_MAX).round();
        c.ui_scale_percent = c.ui_scale_percent.clamp(SCALE_MIN, SCALE_MAX);
        c.ui_scale_enabled = true;
        if !c.window.left_panel_width.is_finite() || c.window.left_panel_width < LEFT_PANEL_MIN {
            c.window.left_panel_width = LEFT_PANEL_DEFAULT;
        }
        if !c.window.right_panel_width.is_finite() {
            c.window.right_panel_width = RIGHT_PANEL_DEFAULT;
        }
        c.window.right_panel_width = c.window.right_panel_width.clamp(RIGHT_PANEL_MIN, RIGHT_PANEL_MAX);
        if !self.libraries_height.is_finite() || self.libraries_height < 60.0 {
            self.libraries_height = LIBRARIES_HEIGHT_DEFAULT;
        }
        c.window.right_panel_tab = c.window.right_panel_tab.min(2);
    }

    /// Saves atomically (temporary file + rename).
    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let json = serde_json::to_vec_pretty(self).map_err(std::io::Error::other)?;
        let tmp = path.with_extension("json.tmp");
        std::fs::write(&tmp, json)?;
        std::fs::rename(&tmp, path)
    }

    pub fn is_collapsed(&self, key: &str) -> bool {
        self.collapsed_sections.iter().any(|s| s == key)
    }

    pub fn set_collapsed(&mut self, key: &str, collapsed: bool) {
        self.collapsed_sections.retain(|s| s != key);
        if collapsed {
            self.collapsed_sections.push(key.to_string());
        }
    }
}

/// Where AnyFont keeps its files.
#[derive(Clone, Debug)]
pub struct Paths {
    /// `settings.json`.
    pub settings_file: PathBuf,
    /// The font database (`fonts.db`).
    pub db_file: PathBuf,
    /// Downloaded fonts (AnyFont Content folder).
    pub content_dir: PathBuf,
    /// Get More's cache (the font catalog, previews): `%LOCALAPPDATA%\AnyFont`.
    pub cache_dir: PathBuf,
}

impl Paths {
    /// From the font system's folders, unless `ANYFONT_DATA_DIR` redirects them.
    pub fn resolve(app: &anyfont_sys::AppPaths) -> Paths {
        if let Some(dir) = std::env::var_os(DATA_DIR_ENV).filter(|v| !v.is_empty()) {
            return Paths::in_dir(Path::new(&dir), app.content_fonts_dir.clone());
        }
        Paths {
            settings_file: app.settings_dir.join("settings.json"),
            db_file: app.data_dir.join("fonts.db"),
            content_dir: app.content_fonts_dir.clone(),
            cache_dir: app.data_dir.clone(),
        }
    }

    /// Everything under one folder (tests).
    pub fn in_dir(dir: &Path, content_dir: PathBuf) -> Paths {
        Paths {
            settings_file: dir.join("settings").join("settings.json"),
            db_file: dir.join("data").join("fonts.db"),
            content_dir,
            cache_dir: dir.join("cache"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_keeps_shell_and_core_fields() {
        let dir = std::env::temp_dir().join(format!("anyfont-settings-test-{}", std::process::id()));
        let p = dir.join("settings.json");
        let mut s = AppSettings { scheme: Scheme::Black, ..AppSettings::default() };
        s.core.preview_size = 50.0;
        s.set_collapsed("Folders", true);
        s.core.remember_sample("Hello");
        s.save(&p).unwrap();
        let l = AppSettings::load(&p);
        assert_eq!(l, s);
        std::fs::write(&p, b"{\"scheme\":\"Light\",\"preview_size\":500}").unwrap();
        let l = AppSettings::load(&p);
        assert_eq!(l.scheme, Scheme::Light);
        assert_eq!(l.core.preview_size, 24.0f32.clamp(PREVIEW_MIN, PREVIEW_MAX));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn defaults_follow_the_spec() {
        let s = AppSettings::default();
        assert_eq!(s.core.preview_size, 36.0);
        assert!(!s.core.show_font_properties);
        assert!(s.core.group_by_family);
        assert!(!s.core.show_recent);
        assert!(s.core.render_name_in_font);
        assert!(!s.core.show_latin_names);
        assert_eq!(s.core.window.left_panel_width, 241.0);
        assert_eq!(s.core.window.right_panel_width, 271.0);
    }
}
