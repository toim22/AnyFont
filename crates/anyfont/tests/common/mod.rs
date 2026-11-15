//! Shared set-up of the end-to-end tests: every test runs the real window (`AnyFontApp`) on a sandbox font
//! system and a data folder inside a temporary directory, in en-US, with the scan on the UI thread. Nothing
//! touches the fonts of this PC or the user's AnyFont profile (`anyfont-sys` is built with
//! `forbid-real-writes` for the tests).
#![allow(dead_code)]

use anyfont::{AnyFontApp, TestConfig};
use egui_kittest::Harness;
use std::path::{Path, PathBuf};

pub struct Env {
    pub dir: tempfile::TempDir,
}

impl Env {
    pub fn new() -> Env {
        Env { dir: tempfile::Builder::new().prefix("anyfont-e2e-").tempdir().expect("temp dir") }
    }
    pub fn root(&self) -> PathBuf {
        self.dir.path().join("sandbox")
    }
    pub fn config(&self) -> TestConfig {
        TestConfig::new(&self.root())
    }
    /// A folder of font files copied from the sandbox seed (fonts that ship with Windows).
    pub fn font_folder(&self, name: &str, files: &[&str]) -> PathBuf {
        let dir = self.dir.path().join(name);
        std::fs::create_dir_all(&dir).unwrap();
        let windir = std::env::var_os("WINDIR").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(r"C:\Windows"));
        for f in files {
            let src = windir.join("Fonts").join(f);
            if src.exists() {
                std::fs::copy(&src, dir.join(f)).unwrap();
            }
        }
        dir
    }
}

pub fn harness<'a>(cfg: TestConfig, size: (f32, f32)) -> Harness<'a, AnyFontApp> {
    let mut h = Harness::builder()
        .with_size(egui::vec2(size.0, size.1))
        .with_max_steps(64)
        .wgpu()
        .build_eframe(move |cc| AnyFontApp::new_for_test(cc, cfg));
    settle(&mut h);
    h
}

/// Runs a few frames (the app keeps asking for repaints while previews load, so `run` may not settle).
pub fn settle(h: &mut Harness<'_, AnyFontApp>) {
    for _ in 0..4 {
        h.run_steps(4);
        std::thread::sleep(std::time::Duration::from_millis(30));
    }
}

/// Renders the window to `target/ui-compare/<name>.png` (comparison screenshots, not committed).
pub fn screenshot(h: &mut Harness<'_, AnyFontApp>, name: &str) -> Option<PathBuf> {
    let img = match h.render() {
        Ok(i) => i,
        Err(e) => {
            eprintln!("render failed: {e}");
            return None;
        }
    };
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/ui-compare");
    std::fs::create_dir_all(&dir).ok()?;
    let path = dir.join(format!("{name}.png"));
    img.save(&path).ok()?;
    Some(path)
}
