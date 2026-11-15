//! Sandbox round trips for install / uninstall / delete. Nothing here touches the machine's font
//! registration: every operation runs on `SandboxFontSystem` in a temporary folder. System fonts
//! are only *read* (copied) from `%WINDIR%\Fonts`.

use std::path::{Path, PathBuf};

use anyfont_sys::{
    helper, DeleteOutcome, FileRemoval, FontOpError, FontSystem, InstallOutcome, RegKind, SandboxConfig, SandboxFontSystem, Scope,
    UsedFontAction,
};

fn sys_font(name: &str) -> Option<PathBuf> {
    let windir = std::env::var_os("WINDIR").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(r"C:\Windows"));
    let p = windir.join("Fonts").join(name);
    p.is_file().then_some(p)
}

/// Copy a system font into `dir` under `as_name` (writable copy).
fn font_copy(name: &str, dir: &Path, as_name: &str) -> PathBuf {
    let src = sys_font(name).unwrap_or_else(|| panic!("{name} is required in %WINDIR%\\Fonts for this test"));
    std::fs::create_dir_all(dir).unwrap();
    let dest = dir.join(as_name);
    std::fs::copy(&src, &dest).unwrap();
    let mut perm = std::fs::metadata(&dest).unwrap().permissions();
    #[allow(clippy::permissions_set_readonly_false)]
    perm.set_readonly(false);
    std::fs::set_permissions(&dest, perm).unwrap();
    dest
}

fn sandbox(seed: &[&str]) -> (tempfile::TempDir, SandboxFontSystem) {
    let dir = tempfile::tempdir().unwrap();
    let sb = SandboxFontSystem::open(dir.path().join("sb"), SandboxConfig { seed: seed.iter().map(PathBuf::from).collect() }).unwrap();
    (dir, sb)
}

fn registry_json(sb: &SandboxFontSystem) -> serde_json::Value {
    serde_json::from_slice(&std::fs::read(sb.root().join("sandbox-registry.json")).unwrap()).unwrap()
}

fn installed_value(outcome: &Result<InstallOutcome, FontOpError>) -> (String, PathBuf) {
    match outcome {
        Ok(InstallOutcome::Installed { value_name, installed_path, .. }) => (value_name.clone(), installed_path.clone()),
        other => panic!("expected Installed, got {other:?}"),
    }
}

#[test]
fn seeded_sandbox_has_protected_system_fonts() {
    let (_d, sb) = sandbox(&["arial.ttf", "cambria.ttc", "does-not-exist.ttf"]);
    let fonts = sb.installed_fonts().unwrap();
    let arial = fonts.iter().find(|f| f.value_name == "Arial (TrueType)").expect("seeded Arial");
    assert_eq!(arial.scope, Scope::AllUsers);
    assert_eq!(arial.kind, RegKind::Fonts);
    assert!(arial.exists && arial.protected);
    assert!(arial.path.starts_with(sb.font_dirs().all_users));
    assert!(sb.is_protected(&arial.path));
    assert!(fonts.iter().any(|f| f.value_name == "Cambria & Cambria Math (TrueType)" && f.protected));
    assert_eq!(fonts.len(), 2, "missing seed files are skipped");
    assert_eq!(sb.protected_fonts().len(), 2);
    // Data is the bare file name, as Windows stores HKLM fonts.
    let reg = registry_json(&sb);
    let hklm = &reg["registry"][r"HKLM\SOFTWARE\Microsoft\Windows NT\CurrentVersion\Fonts"];
    assert_eq!(hklm["Arial (TrueType)"]["Sz"], "arial.ttf");
}

#[test]
fn reopening_does_not_reseed() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("sb");
    let sb = SandboxFontSystem::open(root.clone(), SandboxConfig { seed: vec!["arial.ttf".into()] }).unwrap();
    let a = sb.installed_fonts().unwrap()[0].path.clone();
    sb.set_in_use(&a, true);
    drop(sb);
    let sb = SandboxFontSystem::open(root, SandboxConfig { seed: vec!["times.ttf".into()] }).unwrap();
    let fonts = sb.installed_fonts().unwrap();
    assert_eq!(fonts.len(), 1);
    assert_eq!(fonts[0].value_name, "Arial (TrueType)");
}

#[test]
fn per_user_install_uninstall_round_trip() {
    let (d, sb) = sandbox(&[]);
    let src = font_copy("georgia.ttf", &d.path().join("dl"), "georgia.ttf");
    let before = sb.font_change_count();

    let r = sb.install(std::slice::from_ref(&src), Scope::CurrentUser, UsedFontAction::Ask);
    let (name, installed) = installed_value(&r.results[0].result);
    assert_eq!(name, "Georgia (TrueType)");
    assert_eq!(installed, sb.font_dirs().current_user.join("georgia.ttf"));
    assert!(installed.is_file() && src.is_file(), "copied, source kept");
    assert!(sb.font_change_count() > before, "WM_FONTCHANGE would be broadcast");

    // HKCU value with the full path.
    let reg = registry_json(&sb);
    let hkcu = &reg["registry"][r"HKCU\SOFTWARE\Microsoft\Windows NT\CurrentVersion\Fonts"];
    assert_eq!(hkcu["Georgia (TrueType)"]["Sz"], installed.to_string_lossy().as_ref());

    let fonts = sb.installed_fonts().unwrap();
    let g = fonts.iter().find(|f| f.value_name == name).unwrap();
    assert_eq!(g.scope, Scope::CurrentUser);
    assert!(!g.protected);
    assert_eq!(sb.registrations(&installed).len(), 1);

    // Same file again, and an identical copy under another name: already installed.
    let r = sb.install(std::slice::from_ref(&src), Scope::CurrentUser, UsedFontAction::Ask);
    assert!(matches!(r.results[0].result, Ok(InstallOutcome::AlreadyInstalled { .. })));
    let twin = font_copy("georgia.ttf", &d.path().join("dl2"), "other-name.ttf");
    let r = sb.install(&[twin], Scope::CurrentUser, UsedFontAction::Ask);
    assert!(matches!(r.results[0].result, Ok(InstallOutcome::AlreadyInstalled { .. })));
    assert_eq!(std::fs::read_dir(&sb.font_dirs().current_user).unwrap().count(), 1);

    let r = sb.uninstall(std::slice::from_ref(&installed), UsedFontAction::Ask);
    let out = r.results[0].result.as_ref().unwrap();
    assert_eq!(out.value_names, vec![name.clone()]);
    assert_eq!(out.file, FileRemoval::Removed);
    assert!(!installed.exists());
    assert!(sb.installed_fonts().unwrap().is_empty());

    let r = sb.uninstall(&[installed], UsedFontAction::Ask);
    assert_eq!(r.results[0].result, Err(FontOpError::NotInstalled));
}

#[test]
fn file_name_collision_gets_numbered_copy() {
    let (d, sb) = sandbox(&[]);
    let georgia = font_copy("georgia.ttf", &d.path().join("a"), "font.ttf");
    let verdana = font_copy("verdana.ttf", &d.path().join("b"), "font.ttf");
    let r = sb.install(&[georgia, verdana], Scope::CurrentUser, UsedFontAction::Ask);
    let (n1, p1) = installed_value(&r.results[0].result);
    let (n2, p2) = installed_value(&r.results[1].result);
    assert_eq!((n1.as_str(), p1.file_name().unwrap().to_str().unwrap()), ("Georgia (TrueType)", "font.ttf"));
    assert_eq!((n2.as_str(), p2.file_name().unwrap().to_str().unwrap()), ("Verdana (TrueType)", "font_1.ttf"));
    assert_eq!(r.succeeded(), 2);
}

#[test]
fn different_file_with_same_name_replaces_the_old_registration() {
    let (d, sb) = sandbox(&[]);
    let v1 = font_copy("georgia.ttf", &d.path().join("v1"), "georgia.ttf");
    let v2 = font_copy("georgia.ttf", &d.path().join("v2"), "georgia-v2.ttf");
    // Make v2 a different file with the same font name (trailing bytes are ignored by parsers).
    let mut data = std::fs::read(&v2).unwrap();
    data.extend_from_slice(&[0u8; 16]);
    std::fs::write(&v2, data).unwrap();

    let r = sb.install(&[v1], Scope::CurrentUser, UsedFontAction::Ask);
    let (_, old) = installed_value(&r.results[0].result);
    let r = sb.install(&[v2], Scope::CurrentUser, UsedFontAction::Ask);
    match &r.results[0].result {
        Ok(InstallOutcome::Installed { value_name, installed_path, replaced }) => {
            assert_eq!(value_name, "Georgia (TrueType)");
            assert_eq!(replaced.as_deref(), Some(old.as_path()));
            assert!(installed_path.is_file());
        }
        other => panic!("{other:?}"),
    }
    assert!(!old.exists(), "the replaced copy is removed");
    assert_eq!(sb.installed_fonts().unwrap().len(), 1);
}

#[test]
fn replacing_a_protected_font_is_refused() {
    let (d, sb) = sandbox(&["arial.ttf"]);
    let mut data = std::fs::read(sys_font("arial.ttf").unwrap()).unwrap();
    data.extend_from_slice(b"different");
    let p = d.path().join("arial-mod.ttf");
    std::fs::write(&p, data).unwrap();
    let r = sb.install(&[p], Scope::AllUsers, UsedFontAction::Ask);
    assert_eq!(r.results[0].result, Err(FontOpError::Protected));
}

#[test]
fn all_users_install_uses_system_folder_and_relative_data() {
    let (d, sb) = sandbox(&[]);
    let src = font_copy("consola.ttf", &d.path().join("x"), "consola.ttf");
    assert!(!sb.needs_elevation(Scope::AllUsers));
    let r = sb.install(&[src], Scope::AllUsers, UsedFontAction::Ask);
    let (name, installed) = installed_value(&r.results[0].result);
    assert_eq!(name, "Consolas (TrueType)");
    assert_eq!(installed, sb.font_dirs().all_users.join("consola.ttf"));
    let reg = registry_json(&sb);
    assert_eq!(reg["registry"][r"HKLM\SOFTWARE\Microsoft\Windows NT\CurrentVersion\Fonts"][&name]["Sz"], "consola.ttf");
    assert!(!sb.is_protected(&installed), "user-installed fonts are not protected");
    let r = sb.uninstall(&[installed.clone()], UsedFontAction::Ask);
    assert_eq!(r.results[0].result.as_ref().unwrap().file, FileRemoval::Removed);
    assert!(!installed.exists());
}

#[test]
fn file_already_in_fonts_folder_is_registered_in_place() {
    let (_d, sb) = sandbox(&[]);
    let in_place = font_copy("verdana.ttf", &sb.font_dirs().current_user, "verdana.ttf");
    let r = sb.install(std::slice::from_ref(&in_place), Scope::CurrentUser, UsedFontAction::Ask);
    let (_, installed) = installed_value(&r.results[0].result);
    assert_eq!(installed, in_place);
    assert_eq!(std::fs::read_dir(&sb.font_dirs().current_user).unwrap().count(), 1);
}

#[test]
fn fonts_outside_the_fonts_folders_are_kept_on_uninstall() {
    let (d, sb) = sandbox(&[]);
    // Register in place via the session scope (no copy), then uninstall: the file stays.
    let src = font_copy("tahoma.ttf", &d.path().join("s"), "tahoma.ttf");
    let r = sb.install(std::slice::from_ref(&src), Scope::Session, UsedFontAction::Ask);
    let (name, installed) = installed_value(&r.results[0].result);
    assert_eq!(installed, src);
    let fonts = sb.installed_fonts().unwrap();
    assert!(fonts.iter().any(|f| f.scope == Scope::Session && f.value_name == name && f.kind == RegKind::None));
    let r = sb.install(std::slice::from_ref(&src), Scope::Session, UsedFontAction::Ask);
    assert!(matches!(r.results[0].result, Ok(InstallOutcome::AlreadyInstalled { .. })));
    let r = sb.uninstall(std::slice::from_ref(&src), UsedFontAction::Ask);
    assert_eq!(r.results[0].result.as_ref().unwrap().file, FileRemoval::Kept);
    assert!(src.is_file());
    assert!(sb.installed_fonts().unwrap().is_empty());
}

#[test]
fn protected_fonts_are_never_uninstalled_or_deleted() {
    let (_d, sb) = sandbox(&["arial.ttf"]);
    let arial = sb.installed_fonts().unwrap()[0].path.clone();
    let r = sb.uninstall(std::slice::from_ref(&arial), UsedFontAction::RebootLater);
    assert_eq!(r.results[0].result, Err(FontOpError::Protected));
    let r = sb.delete(std::slice::from_ref(&arial), UsedFontAction::RebootLater);
    assert_eq!(r.results[0].result, Err(FontOpError::Protected));
    assert!(arial.is_file());
    assert_eq!(sb.installed_fonts().unwrap().len(), 1);
    // The critical-font safety net applies inside the all-users folder even when not seeded.
    let marlett = sb.font_dirs().all_users.join("marlett.ttf");
    std::fs::write(&marlett, b"x").unwrap();
    assert!(sb.is_protected(&marlett));
}

#[test]
fn in_use_fonts_follow_the_used_font_action() {
    let (d, sb) = sandbox(&[]);
    let a = font_copy("georgia.ttf", &d.path().join("a"), "georgia.ttf");
    let b = font_copy("verdana.ttf", &d.path().join("b"), "verdana.ttf");
    let r = sb.install(&[a, b], Scope::CurrentUser, UsedFontAction::Ask);
    let (_, ia) = installed_value(&r.results[0].result);
    let (_, ib) = installed_value(&r.results[1].result);
    sb.set_in_use(&ia, true);

    // Ask: in-use font untouched and listed; the free one goes.
    let r = sb.uninstall(&[ia.clone(), ib.clone()], UsedFontAction::Ask);
    assert_eq!(r.results[0].result, Err(FontOpError::InUse));
    assert!(r.results[1].result.is_ok());
    assert_eq!(r.in_use, vec![ia.clone()]);
    assert!(!r.reboot_required);
    assert_eq!(sb.registrations(&ia).len(), 1);

    // Cancel: nothing changes when anything is in use.
    let c = font_copy("verdana.ttf", &d.path().join("c"), "verdana.ttf");
    let r = sb.install(&[c], Scope::CurrentUser, UsedFontAction::Ask);
    let (_, ic) = installed_value(&r.results[0].result);
    let r = sb.uninstall(&[ia.clone(), ic.clone()], UsedFontAction::Cancel);
    assert!(r.results.iter().all(|x| x.result == Err(FontOpError::Cancelled)));
    assert_eq!(sb.installed_fonts().unwrap().len(), 2);

    // Ignore: skip the busy one.
    let r = sb.uninstall(&[ia.clone()], UsedFontAction::Ignore);
    assert_eq!(r.results[0].result, Err(FontOpError::InUse));

    // Reboot later: unregistered now, file removal deferred. Per-user deletes cannot use
    // MOVEFILE_DELAY_UNTIL_REBOOT without admin rights -> remembered and done at the next start.
    let r = sb.uninstall(&[ia.clone()], UsedFontAction::RebootLater);
    assert_eq!(r.results[0].result.as_ref().unwrap().file, FileRemoval::PendingReboot);
    assert!(r.reboot_required);
    assert!(sb.registrations(&ia).is_empty());
    assert!(ia.exists());
    assert!(sb.paths().data_dir.join("pending-deletes.json").is_file());
    sb.set_in_use(&ia, false);
    assert_eq!(sb.process_pending(), 1);
    assert!(!ia.exists());
    assert_eq!(sb.process_pending(), 0);
}

#[test]
fn all_users_in_use_font_is_deleted_at_reboot() {
    let (d, sb) = sandbox(&[]);
    let src = font_copy("times.ttf", &d.path().join("t"), "times.ttf");
    let r = sb.install(&[src], Scope::AllUsers, UsedFontAction::Ask);
    let (_, inst) = installed_value(&r.results[0].result);
    sb.set_in_use(&inst, true);
    let r = sb.uninstall(&[inst.clone()], UsedFontAction::RebootNow);
    assert!(r.reboot_required);
    assert_eq!(sb.pending_reboot(), vec![inst.clone()]);
    sb.reboot_now().unwrap();
    assert_eq!(sb.reboot_count(), 1);
    assert!(inst.exists());
    sb.simulate_reboot();
    assert!(!inst.exists());
}

#[test]
fn delete_recycles_and_uninstalls_first() {
    let (d, sb) = sandbox(&[]);
    let content = sb.content_folder();
    assert!(content.ends_with(r"Documents\AnyFont\Content\Fonts") && content.is_dir());
    let loose = font_copy("verdana.ttf", &content, "verdana.ttf");
    let src = font_copy("georgia.ttf", &d.path().join("g"), "georgia.ttf");
    let r = sb.install(&[src], Scope::CurrentUser, UsedFontAction::Ask);
    let (_, installed) = installed_value(&r.results[0].result);

    let r = sb.delete(&[loose.clone(), installed.clone()], UsedFontAction::Ask);
    assert_eq!(r.results[0].result, Ok(DeleteOutcome::Recycled { uninstalled: None }));
    match &r.results[1].result {
        Ok(DeleteOutcome::Recycled { uninstalled: Some(u) }) => assert_eq!(u.value_names, vec!["Georgia (TrueType)".to_string()]),
        other => panic!("{other:?}"),
    }
    assert!(!loose.exists() && !installed.exists());
    assert!(sb.installed_fonts().unwrap().is_empty());
    let bin = sb.recycle_bin();
    assert_eq!(bin.len(), 2);
    assert!(bin.iter().all(|(_, stored)| stored.is_file()));
}

#[test]
fn delete_refuses_read_only_missing_and_busy_files() {
    let (d, sb) = sandbox(&[]);
    let ro = font_copy("cour.ttf", &d.path().join("r"), "cour.ttf");
    let mut perm = std::fs::metadata(&ro).unwrap().permissions();
    perm.set_readonly(true);
    std::fs::set_permissions(&ro, perm).unwrap();
    let busy = font_copy("verdana.ttf", &d.path().join("b"), "verdana.ttf");
    let held = std::fs::File::open(&busy).unwrap(); // really held open by "another application"
    let missing = d.path().join("nope.ttf");
    let not_a_font = d.path().join("notes.txt");
    std::fs::write(&not_a_font, b"keep me").unwrap();

    let r = sb.delete(&[ro.clone(), missing, busy.clone(), not_a_font.clone()], UsedFontAction::Ask);
    assert_eq!(r.results[0].result, Err(FontOpError::ReadOnly));
    assert_eq!(r.results[1].result, Err(FontOpError::NotFound));
    assert_eq!(r.results[2].result, Err(FontOpError::InUse));
    assert!(matches!(r.results[3].result, Err(FontOpError::UnsupportedFormat(_))));
    assert!(not_a_font.is_file());
    assert_eq!(r.in_use, vec![busy.clone()]);
    assert!(ro.exists() && busy.exists());
    drop(held);
    let r = sb.delete(&[busy.clone()], UsedFontAction::Ask);
    assert!(r.results[0].result.is_ok());
    assert!(!busy.exists());
}

#[test]
fn type1_pairs_register_in_the_type1_key_for_all_users_only() {
    let (d, sb) = sandbox(&[]);
    let dir = d.path().join("t1");
    std::fs::create_dir_all(&dir).unwrap();
    // Minimal PFM (face "Helvetica", PostScript name "Helvetica-Bold") + dummy PFB.
    let mut pfm = vec![0u8; 147];
    pfm[0..2].copy_from_slice(&0x0100u16.to_le_bytes());
    let face_off = pfm.len() as u32;
    pfm.extend_from_slice(b"Helvetica\0");
    let ps_off = pfm.len() as u32;
    pfm.extend_from_slice(b"Helvetica-Bold\0");
    pfm[105..109].copy_from_slice(&face_off.to_le_bytes());
    pfm[139..143].copy_from_slice(&ps_off.to_le_bytes());
    std::fs::write(dir.join("HVB.PFM"), &pfm).unwrap();
    std::fs::write(dir.join("HVB.PFB"), b"\x80\x01dummy").unwrap();

    let r = sb.install(&[dir.join("HVB.PFM")], Scope::CurrentUser, UsedFontAction::Ask);
    assert_eq!(r.results[0].result, Err(FontOpError::Type1NeedsAllUsers));

    let r = sb.install(&[dir.join("HVB.PFM"), dir.join("HVB.PFB")], Scope::AllUsers, UsedFontAction::Ask);
    let (name, pfm_path) = installed_value(&r.results[0].result);
    assert_eq!(name, "Helvetica Bold");
    assert_eq!(r.results[1].result, r.results[0].result, "the pair is installed once");
    let reg = registry_json(&sb);
    let t1 = &reg["registry"][r"HKLM\SOFTWARE\Microsoft\Windows NT\CurrentVersion\Type 1 Installer\Type 1 Fonts"];
    assert_eq!(t1[&name]["MultiSz"], serde_json::json!(["T", "HVB.PFM", "HVB.PFB"]));

    let fonts = sb.installed_fonts().unwrap();
    let f = fonts.iter().find(|f| f.kind == RegKind::Type1).unwrap();
    assert_eq!(f.path, pfm_path);
    assert_eq!(f.extra_paths, vec![sb.font_dirs().all_users.join("HVB.PFB")]);

    // Uninstall through the .pfb removes both files.
    let r = sb.uninstall(&[f.extra_paths[0].clone()], UsedFontAction::Ask);
    assert_eq!(r.results[0].result.as_ref().unwrap().value_names, vec![name]);
    assert!(!pfm_path.exists() && !f.extra_paths[0].exists());
}

#[test]
fn unsupported_and_invalid_files() {
    let (d, sb) = sandbox(&[]);
    let txt = d.path().join("readme.txt");
    std::fs::write(&txt, b"not a font").unwrap();
    let fake = d.path().join("broken.ttf");
    std::fs::write(&fake, b"not a font").unwrap();
    let fon = d.path().join("old.fon");
    std::fs::write(&fon, b"MZ").unwrap();
    let r = sb.install(&[txt, fake, fon, d.path().join("missing.otf")], Scope::CurrentUser, UsedFontAction::Ask);
    assert!(matches!(r.results[0].result, Err(FontOpError::UnsupportedFormat(_))));
    assert!(matches!(r.results[1].result, Err(FontOpError::InvalidFont(_))));
    assert!(matches!(r.results[2].result, Err(FontOpError::UnsupportedFormat(_))));
    assert_eq!(r.results[3].result, Err(FontOpError::NotFound));
    assert_eq!(std::fs::read_dir(&sb.font_dirs().current_user).unwrap().count(), 0, "nothing left behind");
}

#[test]
fn progress_callback_can_cancel() {
    let (d, sb) = sandbox(&[]);
    let a = font_copy("georgia.ttf", &d.path().join("a"), "georgia.ttf");
    let b = font_copy("verdana.ttf", &d.path().join("b"), "verdana.ttf");
    let mut seen = Vec::new();
    let r = sb.install_with(&[a, b], Scope::CurrentUser, UsedFontAction::Ask, &mut |done, total, _| {
        seen.push((done, total));
        done < 1
    });
    assert!(r.results[0].result.is_ok());
    assert_eq!(r.results[1].result, Err(FontOpError::Cancelled));
    assert_eq!(seen[0], (0, 2));
}

#[test]
fn helper_requests_run_against_the_backend() {
    let (d, sb) = sandbox(&[]);
    let src = font_copy("georgia.ttf", &d.path().join("h"), "georgia.ttf");
    let req = helper::HelperRequest::Install { files: vec![src], scope: Scope::AllUsers, used: UsedFontAction::Ask };
    let file = helper::write_request(&sb.paths().data_dir.join("helper"), &req).unwrap();
    let parsed = helper::read_request(&file).unwrap();
    match helper::execute(&sb, parsed) {
        helper::HelperResponse::Install(r) => {
            let (name, p) = installed_value(&r.results[0].result);
            assert_eq!(name, "Georgia (TrueType)");
            assert!(p.starts_with(sb.font_dirs().all_users));
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn open_location_and_reboot_are_recorded_only() {
    let (_d, sb) = sandbox(&["arial.ttf"]);
    let a = sb.installed_fonts().unwrap()[0].path.clone();
    sb.open_location(&a).unwrap();
    assert_eq!(sb.opened_locations(), vec![a]);
    sb.reboot_now().unwrap();
    assert_eq!(sb.reboot_count(), 1);
}
