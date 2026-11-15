//! AnyFont – a font manager for Windows. The application is this library (every module and the public
//! [`app::AnyFontApp`] that the headless end-to-end tests in `tests/` drive) plus the thin `main.rs` that runs
//! the start-up sequence (`/InstallHelper:`, single instance, logging, crash hook, splash, main window).
//!
//! See `docs/ARCHITECTURE.md` for the crate layout, the threads and the data flow.

// --- the shell (main window) ---
pub mod app;
pub mod i18n;
pub mod icons;
pub mod ops;
pub mod platform;
pub mod preview;
pub mod settings;
pub mod theme;
pub mod watcher;
pub mod widgets;

mod commands;
mod dialogs;
mod fontlist;
mod libraries;
mod menus;
mod options;
mod properties;
mod splash;
mod statusbar;
mod titlebar;
mod toolbar;

// --- the services dialogs (owned by the services UI developer; `help` declares `help::dlg`, `about`
// declares `about::sysinfo` and `about::legal`) ---
pub mod about;
pub mod bugreport;
pub mod getmore;
pub mod help;
pub mod matching;
pub mod updater;

pub use app::{AnyFontApp, TestConfig};

/// Translates a UI string (`docs/strings-en.tsv` key) into the current language.
pub fn tr(text: &str) -> String {
    i18n::t(text)
}

/// Fills a format string of the TSV: translates `template` first, then replaces each `{}` with the next
/// argument, in order. Font data in the arguments is never translated.
pub fn fmt(template: &str, args: &[&str]) -> String {
    let t = i18n::t(template);
    let mut out = String::with_capacity(t.len() + 16);
    let mut it = args.iter();
    let mut rest = t.as_str();
    while let Some(i) = rest.find("{}") {
        out.push_str(&rest[..i]);
        out.push_str(it.next().copied().unwrap_or(""));
        rest = &rest[i + 2..];
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    #[test]
    fn fmt_fills_in_order() {
        crate::i18n::set_lang(crate::i18n::Lang::ALL[0]);
        assert_eq!(super::fmt("{}: {} fonts", &["My fonts", "3"]), "My fonts: 3 fonts");
        assert_eq!(super::fmt("Collection {}", &["2"]), "Collection 2");
        assert_eq!(super::fmt("{} glyphs", &[]), " glyphs");
    }
}
