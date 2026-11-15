//! The integration-test build of the library has the `forbid-real-writes` guard on (dev-dependency
//! on itself), so even a mistaken `system()` call without the sandbox cannot change the machine.
//! Only non-existent paths are used: without the guard these calls would merely fail.

use anyfont_sys::{Backend, Scope, UsedFontAction};

#[test]
fn real_backend_refuses_writes_in_test_builds() {
    assert!(std::env::var_os(anyfont_sys::SANDBOX_ENV).is_none(), "run without ANYFONT_SANDBOX_ROOT");
    let sys = anyfont_sys::system();
    assert_eq!(sys.backend(), Backend::Real);
    // Reading is fine.
    assert!(!sys.installed_fonts().unwrap().is_empty());
    let ghost = vec![std::env::temp_dir().join("anyfont-guard-does-not-exist.otf")];
    let s = sys.as_ref();
    refused(|| drop(s.install(&ghost, Scope::CurrentUser, UsedFontAction::Ask)));
    refused(|| drop(s.uninstall(&ghost, UsedFontAction::Ask)));
    refused(|| drop(s.delete(&ghost, UsedFontAction::Ask)));
}

fn refused(f: impl FnOnce()) {
    let err = std::panic::catch_unwind(std::panic::AssertUnwindSafe(f)).expect_err("the guard must panic");
    let msg = err.downcast_ref::<String>().cloned().or_else(|| err.downcast_ref::<&str>().map(|s| s.to_string())).unwrap_or_default();
    assert!(msg.contains("use the sandbox backend"), "unexpected panic: {msg}");
}
