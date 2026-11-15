//! Integration tests on real font files (fixtures with their licences in `tests/fixtures`, plus
//! `C:\Windows\Fonts` read-only when present).

use anyfont_core::filters::{FilterOption, StyleCategory, TechnologyFilter};
use anyfont_core::*;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests").join("fixtures").join(name)
}

fn windows_font(name: &str) -> Option<PathBuf> {
    let p = Path::new("C:\\Windows\\Fonts").join(name);
    p.exists().then_some(p)
}

#[test]
fn parses_fixture_fonts() {
    let hack = parse::parse_font_file(&fixture("Hack-Regular.ttf"));
    assert_eq!(hack.faces.len(), 1);
    let f = &hack.faces[0];
    assert!(f.corrupt.is_none(), "{:?}", f.corrupt);
    assert_eq!(f.family, "Hack");
    assert_eq!(f.subfamily, "Regular");
    assert!(f.monospaced, "post.isFixedPitch");
    assert!(StyleCategory::Monospaced.matches(f));
    assert_eq!(f.weight_class, 400);
    assert!(f.glyph_count > 1000);
    assert_eq!(f.paths, vec![fixture("Hack-Regular.ttf")]);
    assert!(f.file_size > 0 && f.modified > 0);
    assert!(!f.features.is_empty());
    assert_eq!(f.label(0x409, false), "Hack (Regular, Normal)");

    let tuffy = parse::parse_font_file(&fixture("Tuffy.ttf"));
    assert!(tuffy.faces[0].corrupt.is_none());
    assert_eq!(tuffy.faces[0].family, "Tuffy");
}

#[test]
fn collection_faces() {
    let ttc = parse::parse_font_file(&fixture("TTC.ttc"));
    assert!(ttc.faces.len() >= 2, "{:?}", ttc.faces.iter().map(|f| &f.corrupt).collect::<Vec<_>>());
    assert_eq!(ttc.format, FileFormat::TrueTypeCollection);
    for (i, f) in ttc.faces.iter().enumerate() {
        assert_eq!(f.face_index as usize, i);
        assert_eq!(f.face_count as usize, ttc.faces.len());
    }
    assert_ne!(ttc.faces[0].key, ttc.faces[1].key);
}

#[test]
fn variable_font_instances_and_axes() {
    let v = parse::parse_font_file(&fixture("colr_1_variable.ttf"));
    let f = &v.faces[0];
    assert!(f.corrupt.is_none(), "{:?}", f.corrupt);
    assert!(f.is_variable);
    assert!(!f.axes.is_empty());
    assert!(f.color.is_color());
    assert!(catalog::variable_axes(&[f]).is_some());
    assert!(catalog::variable_axes(&[f, f]).is_none(), "several fonts selected");
}

#[test]
fn bahnschrift_named_instances_when_available() {
    let Some(path) = windows_font("bahnschrift.ttf") else { return };
    let p = parse::parse_font_file(&path);
    assert!(p.faces.len() > 1, "one record per named instance");
    let lc = p.faces.iter().find(|f| f.subfamily == "Light Condensed").expect("Light Condensed instance");
    assert_eq!(lc.weight_class, 300);
    assert!(lc.width_class < 5);
    let wdth = lc.axes.iter().find(|a| a.tag == "wdth").unwrap();
    let wght = lc.axes.iter().find(|a| a.tag == "wght").unwrap();
    assert_eq!((wdth.value, wght.value), (75.0, 300.0));
    assert_eq!(lc.label(0x409, false), "Bahnschrift Light Condensed");
}

#[test]
fn system_fonts_parse_when_available() {
    let Some(arial) = windows_font("arial.ttf") else { return };
    let f = &parse::parse_font_file(&arial).faces[0];
    assert_eq!(f.family, "Arial");
    assert!(f.corrupt.is_none());
    assert!(f.code_page_range[0] & 1 != 0);
    if let Some(ttc) = windows_font("cambria.ttc") {
        let p = parse::parse_font_file(&ttc);
        assert!(p.faces.len() >= 2);
        assert!(p.faces.iter().all(|f| f.corrupt.is_none()));
    }
}

#[test]
fn glyph_list_of_real_font() {
    let l = glyphs::enumerate_file(&fixture("Hack-Regular.ttf"), 0).unwrap();
    let f = &parse::parse_font_file(&fixture("Hack-Regular.ttf")).faces[0];
    assert_eq!(l.glyphs.len() as u32, f.glyph_count, "every glyph listed once (encoded, alternates, unencoded)");
    let pos = |c: char| {
        l.glyphs.iter().position(|g| g.codepoint == Some(c) && g.origin == glyphs::GlyphOrigin::Encoded).unwrap()
    };
    assert!(pos(' ') < pos('A') && pos('A') < pos('a'), "code-point order");
    assert_eq!(l.glyphs.last().unwrap().origin, glyphs::GlyphOrigin::Unencoded);
    let filters = l.available_filters();
    assert_eq!(filters[0].0, "Common");
    assert!(filters.iter().any(|(s, c)| s == "Latin" && c.contains(&glyphs::GlyphCategory::Uppercase)));
}

#[test]
fn woff_fixture_round_trip() {
    let data = std::fs::read(fixture("Tuffy.ttf")).unwrap();
    let woff = woff::encode_woff(&data).unwrap();
    let p = parse::parse_font_bytes(&woff);
    assert_eq!(p.format, FileFormat::Woff);
    assert_eq!(p.faces[0].family, "Tuffy");
    assert!(p.faces[0].corrupt.is_none());
    let mut w2 = woff.clone();
    w2[3] = b'2';
    assert_eq!(parse::parse_font_bytes(&w2).faces[0].corrupt.as_deref(), Some("WOFF2 files are not supported"));
}

fn copy(from: &Path, dir: &Path, name: &str) -> PathBuf {
    let to = dir.join(name);
    std::fs::copy(from, &to).unwrap();
    to
}

#[test]
fn incremental_rescan_folders_duplicates_and_cancel() {
    let tmp = tempfile::tempdir().unwrap();
    let fonts_dir = tmp.path().join("library");
    let sub = fonts_dir.join("sub");
    std::fs::create_dir_all(&sub).unwrap();
    let hack = copy(&fixture("Hack-Regular.ttf"), &fonts_dir, "Hack-Regular.ttf");
    copy(&fixture("Tuffy.ttf"), &sub, "Tuffy.ttf");
    copy(&fixture("TTC.ttc"), &fonts_dir, "TTC.ttc");
    std::fs::write(fonts_dir.join("broken.ttf"), b"definitely not a font").unwrap();
    // "installed" copy elsewhere (identical bytes): the folder copy counts as installed
    let installed_dir = tmp.path().join("installed");
    std::fs::create_dir_all(&installed_dir).unwrap();
    let installed_hack = copy(&fixture("Hack-Regular.ttf"), &installed_dir, "hack.ttf");

    let db_path = tmp.path().join("db").join("fonts.db");
    let mut db = FontDatabase::open(&db_path).unwrap();
    let folder = db.add_folder(&fonts_dir, false).unwrap();
    let req = ScanRequest {
        db_path: db_path.clone(),
        installed: vec![InstalledFont::new(&installed_hack, false)],
        ..Default::default()
    };
    let mut events = Vec::new();
    let r1 = scan::scan(&mut db, &req, &mut |e| events.push(e), &AtomicBool::new(false)).unwrap();
    assert_eq!(r1.files_found, 4, "non-recursive folder: Tuffy in sub/ is not scanned");
    assert_eq!(r1.files_parsed, 4);
    assert_eq!(r1.corrupt, 1);
    assert!(matches!(events.first(), Some(ScanProgress::Started { total: 4, to_parse: 4 })));
    assert!(matches!(events.last(), Some(ScanProgress::Finished(_))));

    let cat = Catalog::load(&db).unwrap();
    let hack_rec = cat.fonts().iter().find(|f| f.family == "Hack").unwrap();
    assert_eq!(hack_rec.paths.len(), 2, "one record for identical copies");
    assert!(hack_rec.installed, "a copy of an installed font counts as installed");
    let folder_q = cat.query(&FontQuery { library: LibraryRef::Folder(folder.id), ..Default::default() });
    assert_eq!(folder_q.status_text, format!("library: {} fonts", folder_q.count));
    assert!(folder_q.count >= 3);
    let corrupt = cat.query(&FontQuery {
        library: LibraryRef::Folder(folder.id),
        filters: FilterSet::new().with(FilterOption::Status(filters::StatusFilter::Corrupt)),
        ..Default::default()
    });
    assert_eq!(corrupt.count, 1);

    // second scan: nothing changed
    let r2 = scan::scan(&mut db, &req, &mut |_| {}, &AtomicBool::new(false)).unwrap();
    assert_eq!((r2.files_parsed, r2.files_cached), (0, 4));

    // recursion on, a changed file, a removed file
    db.set_folder_recursive(folder.id, true).unwrap();
    std::fs::remove_file(fonts_dir.join("TTC.ttc")).unwrap();
    std::fs::write(fonts_dir.join("broken.ttf"), b"still not a font, but longer").unwrap();
    let r3 = scan::scan(&mut db, &req, &mut |_| {}, &AtomicBool::new(false)).unwrap();
    assert_eq!(r3.files_found, 4);
    assert_eq!(r3.files_parsed, 2, "Tuffy (new) + broken.ttf (changed)");
    assert_eq!(r3.files_removed, 1);
    let cat = Catalog::load(&db).unwrap();
    assert!(cat.fonts().iter().all(|f| f.format != FileFormat::TrueTypeCollection), "TTC faces dropped");
    assert!(cat.fonts().iter().any(|f| f.family == "Tuffy"));

    // a modified copy of Hack (different bytes, same family/style) is a duplicate
    let mut bytes = std::fs::read(&hack).unwrap();
    bytes.extend_from_slice(&[0, 0, 0, 0]);
    std::fs::write(sub.join("Hack-copy.ttf"), bytes).unwrap();
    scan::scan(&mut db, &req, &mut |_| {}, &AtomicBool::new(false)).unwrap();
    let cat = Catalog::load(&db).unwrap();
    let hacks: Vec<&FontRecord> = cat.fonts().iter().filter(|f| f.family == "Hack").collect();
    assert_eq!(hacks.len(), 2);
    assert_eq!(hacks.iter().filter(|f| f.duplicate == DuplicateState::Master).count(), 1);
    let master = hacks.iter().find(|f| f.duplicate == DuplicateState::Master).unwrap();
    assert!(master.installed, "the installed copy is the master");
    let shown = cat.query(&FontQuery::default());
    let with_dups = cat.query(&FontQuery { show_duplicates: true, ..Default::default() });
    assert_eq!(with_dups.count, shown.count + 1);
    let dup_filter = cat.query(&FontQuery {
        show_duplicates: true,
        filters: FilterSet::new().with(FilterOption::Status(filters::StatusFilter::Duplicates)),
        ..Default::default()
    });
    assert_eq!(dup_filter.count, 2);

    // cancellation before start: nothing parsed, nothing removed
    std::fs::write(sub.join("late.ttf"), std::fs::read(fixture("TTC.ttc")).unwrap()).unwrap();
    let cancel = AtomicBool::new(true);
    let r4 = scan::scan(&mut db, &req, &mut |_| {}, &cancel).unwrap();
    assert!(r4.cancelled);
    assert_eq!(r4.files_parsed, 0);
    assert_eq!(r4.files_removed, 0);
}

#[test]
fn spawned_scan_reports_progress() {
    let tmp = tempfile::tempdir().unwrap();
    let db_path = tmp.path().join("fonts.db");
    {
        let db = FontDatabase::open(&db_path).unwrap();
        db.add_folder(&fixture(""), false).unwrap();
    }
    let handle = spawn_scan(ScanRequest { db_path: db_path.clone(), threads: 2, ..Default::default() });
    let report = {
        let mut seen = Vec::new();
        while !handle.is_finished() {
            seen.extend(handle.poll());
            std::thread::yield_now();
        }
        seen.extend(handle.poll());
        let r = handle.join().unwrap();
        assert!(seen.iter().any(|e| matches!(e, ScanProgress::File { .. })));
        r
    };
    assert_eq!(report.files_found, 4, "the four fixture fonts (json/txt ignored)");
    let db = FontDatabase::open(&db_path).unwrap();
    let cat = Catalog::load(&db).unwrap();
    let q = cat.query(&FontQuery {
        filters: FilterSet::new().with(FilterOption::Technology(TechnologyFilter::Variable)),
        ..Default::default()
    });
    assert!(q.count >= 1);
    let avail = cat.available_filters(&LibraryRef::MyFonts);
    assert!(avail
        .iter()
        .any(|g| g.options.contains(&FilterOption::Technology(TechnologyFilter::Variable))));
}

#[test]
fn collections_mru_export_import_on_scanned_db() {
    let tmp = tempfile::tempdir().unwrap();
    let db_path = tmp.path().join("fonts.db");
    let mut db = FontDatabase::open(&db_path).unwrap();
    db.add_folder(&fixture(""), false).unwrap();
    scan::scan(&mut db, &ScanRequest { db_path: db_path.clone(), ..Default::default() }, &mut |_| {}, &AtomicBool::new(false))
        .unwrap();
    let cat = Catalog::load(&db).unwrap();
    let hack = cat.fonts().iter().find(|f| f.family == "Hack").unwrap().clone();
    let tuffy = cat.fonts().iter().find(|f| f.family == "Tuffy").unwrap().clone();
    let c1 = db.create_collection(None).unwrap();
    db.add_to_collection(c1.id, &[hack.key.clone(), tuffy.key.clone()]).unwrap();
    db.touch_mru(&tuffy.key, 10).unwrap();
    let cat = Catalog::load(&db).unwrap();
    let q = cat.query(&FontQuery { library: LibraryRef::Collection(c1.id), show_recent: true, ..Default::default() });
    assert_eq!(q.status_text, "Collection 1: 2 fonts");
    assert_eq!(q.recent, vec![tuffy.id]);
    let e = cat.eligibility(LibraryRef::Collection(c1.id), &[hack.id], &[], false, Some(hack.id));
    assert!(e.remove_folder_or_collection && e.remove_from_open_collection && e.install && e.delete);

    let export = tmp.path().join("export.xml");
    exchange::export_file(&db, &cat, &exchange::ExportOptions::all(), &export).unwrap();

    // a fresh database with the same fonts gets the collection back
    let db2_path = tmp.path().join("fonts2.db");
    let mut db2 = FontDatabase::open(&db2_path).unwrap();
    let r = {
        db2.add_folder(&fixture(""), false).unwrap();
        scan::scan(&mut db2, &ScanRequest { db_path: db2_path.clone(), ..Default::default() }, &mut |_| {}, &AtomicBool::new(false))
            .unwrap();
        let cat2 = Catalog::load(&db2).unwrap();
        exchange::import_file(&db2, &cat2, &export).unwrap()
    };
    assert_eq!(r.folders_existing, 1);
    assert_eq!(r.fonts_linked, 2);
    let c = db2.collection_by_name("Collection 1").unwrap().unwrap();
    assert_eq!(db2.collection_fonts(c.id).unwrap().len(), 2);
}

#[test]
fn panose_matching_on_real_fonts() {
    let mut fonts: Vec<FontRecord> = ["Hack-Regular.ttf", "Tuffy.ttf"]
        .iter()
        .flat_map(|n| parse::parse_font_file(&fixture(n)).faces)
        .collect();
    for (i, f) in fonts.iter_mut().enumerate() {
        f.id = i as i64 + 1;
    }
    let missing = panose::MissingFont {
        family: "Courier Prime".into(),
        weight: 400,
        panose: Some(fonts[0].panose),
        ..Default::default()
    };
    let r = panose::find_substitute(&missing, &fonts, &[], &[], Some("Tuffy"), panose::FontFilterType::ShowAll);
    if !fonts[0].panose.is_empty() {
        assert_eq!(r.substitute, Some(1), "Hack has the same PANOSE");
        assert_eq!(r.candidates[0].font_id, 1);
    } else {
        assert_eq!(r.substitute, Some(2), "falls back to the default font");
    }
}

/// Parses every file of `C:\Windows\Fonts` (read-only). Slow: run with `cargo test -- --ignored`.
#[test]
#[ignore]
fn parse_every_windows_font() {
    let dir = Path::new(r"C:\Windows\Fonts");
    if !dir.exists() {
        return;
    }
    let mut files = 0;
    let mut faces = 0;
    let mut corrupt = Vec::new();
    for e in std::fs::read_dir(dir).unwrap().flatten() {
        let p = e.path();
        if !parse::is_font_path(&p) {
            continue;
        }
        files += 1;
        let parsed = parse::parse_font_file(&p);
        faces += parsed.faces.len();
        for f in &parsed.faces {
            if let Some(r) = &f.corrupt {
                corrupt.push(format!("{}: {r}", p.display()));
            }
        }
        if let Some(f) = parsed.faces.first().filter(|f| f.corrupt.is_none() && f.technology != Technology::Type1) {
            let g = glyphs::enumerate_file(&p, f.face_index).expect("glyphs of a good font");
            assert_eq!(g.glyphs.len() as u32, f.glyph_count, "{}", p.display());
        }
    }
    println!("{files} files, {faces} faces, {} corrupt", corrupt.len());
    for c in &corrupt {
        println!("  {c}");
    }
    assert!(files == 0 || faces >= files);
}
