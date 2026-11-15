//! `system()` picks the sandbox when ANYFONT_SANDBOX_ROOT is set (own test binary: it changes
//! the process environment).

use anyfont_sys::{Backend, Scope, UsedFontAction};

#[test]
fn system_uses_the_sandbox_when_the_variable_is_set() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("env-sb");
    // SAFETY: this test binary has a single test; nothing else reads the environment concurrently.
    unsafe {
        std::env::set_var(anyfont_sys::SANDBOX_ENV, &root);
        std::env::set_var(anyfont_sys::SANDBOX_SEED_ENV, "arial.ttf;times.ttf");
    }
    let sys = anyfont_sys::system();
    match sys.backend() {
        Backend::Sandbox { root: r } => assert!(r.ends_with("env-sb")),
        Backend::Real => panic!("expected the sandbox backend"),
    }
    let canon = dir.path().canonicalize().unwrap();
    let canon = canon.to_string_lossy();
    let canon = std::path::PathBuf::from(canon.strip_prefix(r"\\?\").unwrap_or(&canon));
    assert!(sys.font_dirs().all_users.starts_with(&canon));
    assert!(sys.paths().settings_dir.starts_with(&canon), "app data stays inside the sandbox");
    assert_eq!(anyfont_sys::paths::app_paths().logs_dir, sys.paths().logs_dir);
    let fonts = sys.installed_fonts().unwrap();
    assert_eq!(fonts.iter().filter(|f| f.protected).count(), 2);

    // Uninstalling a protected font through system() is refused.
    let arial = fonts.iter().find(|f| f.value_name == "Arial (TrueType)").unwrap().path.clone();
    let r = sys.uninstall(&[arial], UsedFontAction::Ask);
    assert!(r.results[0].result.is_err());
    assert!(!sys.needs_elevation(Scope::AllUsers));
}
