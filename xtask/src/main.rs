//! Build helpers for AnyFont, run from the repository root:
//!
//! ```text
//! cargo run -p xtask -- brand    # app icon (.ico, PNG), logo, About banner, splash and installer pictures
//! cargo run -p xtask -- icons    # crates/anyfont/assets/icons/contact-sheet.png (our icons next to the reference)
//! ```
//!
//! The reference screenshots default to `C:\Projetos\engenharia-reversa\FontManager-ida\06-ui\screenshots`
//! and can be changed with the `ANYFONT_REFERENCE` environment variable. They are only used to put
//! crops of the reference next to our own icons on the contact sheet; nothing from them is shipped.

mod brand;
mod icons;
mod svg;

use std::path::PathBuf;

pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

/// The repository root (the parent of this crate).
pub fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).parent().expect("xtask lives in the repository").to_path_buf()
}

/// The reference screenshots (the screen map of the program AnyFont reproduces).
pub fn reference() -> PathBuf {
    std::env::var_os("ANYFONT_REFERENCE")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(r"C:\Projetos\engenharia-reversa\FontManager-ida\06-ui\screenshots"))
}

fn main() {
    let cmd = std::env::args().nth(1).unwrap_or_default();
    let result = match cmd.as_str() {
        "brand" => brand::run(),
        "icons" => icons::run(),
        _ => {
            eprintln!("usage: cargo run -p xtask -- <brand|icons>");
            std::process::exit(2);
        }
    };
    if let Err(e) = result {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}
