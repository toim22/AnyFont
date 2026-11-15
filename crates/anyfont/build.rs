//! Windows resources (icon, version information, manifest) and the build facts the About box shows.

fn main() {
    println!("cargo:rerun-if-changed=assets/anyfont.ico");
    println!("cargo:rerun-if-changed=app.manifest");
    println!("cargo:rerun-if-changed=../../.git/HEAD");
    println!("cargo:rerun-if-changed=../../.git/refs/heads");

    // Source ID (About box): the commit the program was built from.
    let source_id = std::process::Command::new("git")
        .args(["rev-parse", "--short=10", "HEAD"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "unknown".into());
    println!("cargo:rustc-env=ANYFONT_SOURCE_ID={source_id}");
    // Release (About box): "Version 0.1.0 - November 2026 Release".
    println!("cargo:rustc-env=ANYFONT_RELEASE=November 2026");

    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let version = std::env::var("CARGO_PKG_VERSION").unwrap_or_else(|_| "0.1.0".into());
        let mut res = winresource::WindowsResource::new();
        res.set_icon("assets/anyfont.ico");
        res.set("ProductName", "AnyFont");
        res.set("FileDescription", "AnyFont - font manager");
        res.set("CompanyName", "AnyFont Team");
        res.set("LegalCopyright", "Copyright (c) 2026 AnyFont Team");
        res.set("OriginalFilename", "anyfont.exe");
        res.set("InternalName", "anyfont");
        res.set("ProductVersion", &version);
        res.set("FileVersion", &version);
        if let Ok(manifest) = std::fs::read_to_string("app.manifest") {
            res.set_manifest(&manifest);
        }
        if let Err(e) = res.compile() {
            println!("cargo:warning=could not embed Windows resources: {e}");
        }
    }
}
