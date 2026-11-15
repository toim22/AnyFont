//! Help ▸ Product Help... and the title-bar "?" (owned by the services UI developer): the user guide
//! `docs/HELP.md`, opened on GitHub in the default browser.
//!
//! This file also declares [`dlg`], the dialog look shared by the services dialogs (and available to the
//! shell): `crate::help::dlg`.

#[path = "dlg.rs"]
pub mod dlg;

/// The user guide on GitHub.
pub const HELP_URL: &str = "https://github.com/toim22/AnyFont/blob/main/docs/HELP.md";

/// The guide, or `ANYFONT_HELP_URL` when set (tests, a local copy).
pub fn help_url() -> String {
    std::env::var("ANYFONT_HELP_URL").ok().filter(|s| !s.trim().is_empty()).unwrap_or_else(|| HELP_URL.to_string())
}

/// A section of the guide (`#anchor`, GitHub's heading anchors), e.g. `"panose-font-matching"`.
pub fn section_url(anchor: &str) -> String {
    format!("{}#{}", help_url(), anchor.trim_start_matches('#'))
}

/// Help ▸ Product Help... and the title-bar "?".
pub fn open_product_help() {
    if !dlg::open_url(&help_url()) {
        log::warn!("could not open the browser for {}", help_url());
    }
}

/// Opens one section of the guide (the "?" buttons of the PANOSE dialogs).
pub fn open_section(anchor: &str) {
    let _ = dlg::open_url(&section_url(anchor));
}

#[cfg(test)]
mod tests {
    #[test]
    fn help_urls() {
        assert!(super::HELP_URL.starts_with("https://github.com/toim22/AnyFont/"));
        assert!(super::section_url("#updates").ends_with("HELP.md#updates") || std::env::var("ANYFONT_HELP_URL").is_ok());
        // the guide exists in the repository
        let guide = include_str!("../../../docs/HELP.md");
        assert!(guide.len() > 1000);
    }
}
